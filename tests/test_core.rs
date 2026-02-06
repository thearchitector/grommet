#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_clone)]

use pyo3::prelude::*;

fn with_py<F, R>(f: F) -> R
where
    F: for<'py> FnOnce(Python<'py>) -> R,
{
    use std::sync::Once;
    static INIT: Once = Once::new();

    Python::initialize();
    // Eagerly import asyncio and grommet submodules exactly once to avoid
    // circular-import errors and per-module import-lock deadlocks that
    // surface on Python 3.14+ when parallel test threads race to first-import.
    INIT.call_once(|| {
        Python::attach(|py| {
            py.import("asyncio").unwrap();
            py.import("grommet.plan").unwrap();
        });
    });
    Python::attach(f)
}

mod errors {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/errors.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use pyo3::exceptions::{PyTypeError, PyValueError};
        use pyo3::types::{PyAnyMethods, PyStringMethods};

        fn err_message(err: &PyErr) -> String {
            crate::with_py(|py| err.value(py).str().unwrap().to_str().unwrap().to_string())
        }

        /// Verifies error helper constructors return expected errors and messages.
        #[test]
        fn error_helpers_round_trip() {
            let err = py_type_error("boom");
            assert!(crate::with_py(|py| err.is_instance_of::<PyTypeError>(py)));
            assert_eq!(err_message(&err), "boom");

            let err = py_value_error("nope");
            assert!(crate::with_py(|py| err.is_instance_of::<PyValueError>(py)));
            assert_eq!(err_message(&err), "nope");

            let err = subscription_requires_async_iterator();
            assert!(crate::with_py(|py| err.is_instance_of::<PyTypeError>(py)));

            let err = expected_list_value();
            assert_eq!(err_message(&err), "Expected list for GraphQL list type");

            let err = abstract_type_requires_object();
            assert_eq!(
                err_message(&err),
                "Abstract types must return @grommet.type objects"
            );

            let err = unsupported_value_type();
            assert_eq!(err_message(&err), "Unsupported value type");

            let err = runtime_threads_conflict();
            assert_eq!(
                err_message(&err),
                "worker_threads cannot be set for a current-thread runtime"
            );

            let gql_err = py_err_to_error(py_value_error("oops"));
            assert_eq!(gql_err.message, "ValueError: oops");

            let gql_err = no_parent_value();
            assert_eq!(gql_err.message, "No parent value for field");
        }
    }
}

mod runtime {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/runtime.rs"));
}

mod types {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/types.rs"));
}

mod values {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/values.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::types::{PyObj, ScalarBinding};
        use async_graphql::dynamic::TypeRef;
        use async_graphql::{
            ErrorExtensionValues, Name, PathSegment, Pos, Response, ServerError, Value,
        };
        use indexmap::IndexMap;
        use pyo3::IntoPyObject;
        use pyo3::types::{PyAnyMethods, PyBool, PyBytes, PyDict, PyInt, PyList, PyStringMethods};
        use std::collections::HashSet;

        fn make_scalar_binding(py: Python<'_>) -> ScalarBinding {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
class ScalarType:
    def __init__(self, value):
        self.value = value

def serialize(value):
    return value.value
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();
            let scalar_type = locals.get_item("ScalarType").unwrap().unwrap().unbind();
            let serialize = locals.get_item("serialize").unwrap().unwrap().unbind();
            ScalarBinding {
                _name: "ScalarType".to_string(),
                py_type: PyObj::new(scalar_type),
                serialize: PyObj::new(serialize),
            }
        }

        fn make_meta_objects<'py>(py: Python<'py>) -> Bound<'py, PyDict> {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
import enum
import dataclasses
class MetaType(enum.Enum):
    TYPE = "type"
    ENUM = "enum"
    INPUT = "input"

class Meta:
    def __init__(self, type, name=None):
        self.type = type
        self.name = name

class NoType:
    pass

class Obj:
    pass
Obj.__grommet_meta__ = Meta(MetaType.TYPE, "Obj")

class Plain:
    pass

class Weird:
    pass
Weird.__grommet_meta__ = NoType()

class Color(enum.Enum):
    RED = 1
Color.__grommet_meta__ = Meta("enum", "Color")

@dataclasses.dataclass
class Input:
    value: int
Input.__grommet_meta__ = Meta("input", "Input")
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();
            locals
        }

        /// Verifies grommet meta helpers handle type, enum, and input variants.
        #[test]
        fn meta_helpers_cover_branches() {
            crate::with_py(|py| {
                let locals = make_meta_objects(py);
                let obj_cls = locals.get_item("Obj").unwrap().unwrap();
                let plain_cls = locals.get_item("Plain").unwrap().unwrap();
                let weird_cls = locals.get_item("Weird").unwrap().unwrap();
                let color_cls = locals.get_item("Color").unwrap().unwrap();

                let obj = obj_cls.call0().unwrap();
                let plain = plain_cls.call0().unwrap();
                let weird = weird_cls.call0().unwrap();

                assert_eq!(
                    grommet_type_name(py, &obj).unwrap(),
                    Some("Obj".to_string())
                );
                assert_eq!(grommet_type_name(py, &plain).unwrap(), None);
                assert_eq!(grommet_type_name(py, &weird).unwrap(), None);

                let enum_instance = color_cls.getattr("RED").unwrap();
                assert_eq!(
                    enum_name_for_value(py, &enum_instance).unwrap(),
                    Some("RED".to_string())
                );
                assert_eq!(grommet_type_name(py, &enum_instance).unwrap(), None);
                assert!(input_object_as_dict(py, &obj).unwrap().is_none());

                let input_cls = locals.get_item("Input").unwrap().unwrap();
                let input_instance = input_cls.call1((5,)).unwrap();
                let dict = input_object_as_dict(py, &input_instance).unwrap().unwrap();
                let dict = dict.cast::<PyDict>().unwrap();
                assert_eq!(
                    dict.get_item("value")
                        .unwrap()
                        .unwrap()
                        .extract::<i64>()
                        .unwrap(),
                    5
                );
                assert!(input_object_as_dict(py, &plain).unwrap().is_none());
            });
        }

        /// Verifies Python values convert to const and field GraphQL values.
        #[test]
        fn py_to_const_value_and_field_value_cover_paths() {
            crate::with_py(|py| {
                let binding = make_scalar_binding(py);
                let bindings = [binding];

                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
import enum
class Meta:
    def __init__(self, type, name=None):
        self.type = type
        self.name = name

class Color(enum.Enum):
    RED = 1
Color.__grommet_meta__ = Meta("enum", "Color")

class Custom:
    pass
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let scalar_instance = bindings[0].py_type.bind(py).call1(("hi",)).unwrap();
                let field_value = py_to_field_value(py, &scalar_instance, &bindings).unwrap();
                let _ = field_value;

                let enum_value = locals
                    .get_item("Color")
                    .unwrap()
                    .unwrap()
                    .getattr("RED")
                    .unwrap();
                let field_value = py_to_field_value(py, &enum_value, &bindings).unwrap();
                let _ = field_value;

                let none_value = py.None();
                let _ = py_to_field_value(py, &none_value.bind(py), &bindings).unwrap();

                let bool_value = PyBool::new(py, true).to_owned().into_any();
                let _ = py_to_field_value(py, &bool_value, &bindings).unwrap();

                let float_value = 1.5f64.into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value(py, &float_value, &bindings).unwrap();

                let list = PyList::new(py, [1, 2]).unwrap();
                let list_any = list.into_any();
                let _ = py_to_field_value(py, &list_any, &bindings).unwrap();

                let tuple_any = ("a", "b").into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value(py, &tuple_any, &bindings).unwrap();

                let custom = locals.get_item("Custom").unwrap().unwrap().call0().unwrap();
                let _ = py_to_field_value(py, &custom, &bindings).unwrap();

                let const_value = py_to_const_value(py, &float_value, &bindings).unwrap();
                assert_eq!(const_value, Value::from(1.5));
            });
        }

        /// Verifies Python values convert to GraphQL values across variants.
        #[test]
        fn py_to_value_covers_scalar_enum_input_and_collections() {
            crate::with_py(|py| {
                let binding = make_scalar_binding(py);
                let scalar_type = binding.py_type.bind(py);
                let bindings = [binding];

                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
import enum
import dataclasses
class ScalarType:
    def __init__(self, value):
        self.value = value

class Meta:
    def __init__(self, type, name=None):
        self.type = type
        self.name = name

class Color(enum.Enum):
    RED = 1
Color.__grommet_meta__ = Meta("enum", "Color")

@dataclasses.dataclass
class Input:
    value: int
Input.__grommet_meta__ = Meta("input", "Input")
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let scalar_instance = scalar_type.call1(("hi",)).unwrap();
                let value = py_to_value(py, &scalar_instance, &bindings, true).unwrap();
                assert_eq!(value, Value::String("hi".to_string()));

                let enum_value = locals
                    .get_item("Color")
                    .unwrap()
                    .unwrap()
                    .getattr("RED")
                    .unwrap();
                let value = py_to_value(py, &enum_value, &bindings, true).unwrap();
                assert_eq!(value, Value::Enum(Name::new("RED")));

                let input_instance = locals
                    .get_item("Input")
                    .unwrap()
                    .unwrap()
                    .call1((3,))
                    .unwrap();
                let value = py_to_value(py, &input_instance, &bindings, true).unwrap();
                match value {
                    Value::Object(map) => {
                        assert_eq!(map.get("value").unwrap(), &Value::from(3));
                    }
                    _ => panic!("expected object"),
                }

                let none_obj = py.None();
                let none_value = none_obj.bind(py);
                let value = py_to_value(py, &none_value, &bindings, true).unwrap();
                assert_eq!(value, Value::Null);
                let bool_value = PyBool::new(py, true).to_owned().into_any();
                let value = py_to_value(py, &bool_value, &bindings, true).unwrap();
                assert_eq!(value, Value::Boolean(true));
                let int_value = PyInt::new(py, 42).into_any();
                let value = py_to_value(py, &int_value, &bindings, true).unwrap();
                assert_eq!(value, Value::from(42));
                let float_value = 1.25f64.into_pyobject(py).unwrap().into_any();
                let value = py_to_value(py, &float_value, &bindings, true).unwrap();
                assert_eq!(value, Value::from(1.25));
                let str_value = "hi".into_pyobject(py).unwrap().into_any();
                let value = py_to_value(py, &str_value, &bindings, true).unwrap();
                assert_eq!(value, Value::String("hi".to_string()));

                let bytes = PyBytes::new(py, b"bin");
                let value = py_to_value(py, &bytes.into_any(), &bindings, true).unwrap();
                assert_eq!(value, Value::Binary(b"bin".to_vec().into()));

                let list = PyList::new(py, [1, 2]).unwrap();
                let list_any = list.into_any();
                let value = py_to_value(py, &list_any, &bindings, true).unwrap();
                assert_eq!(value, Value::List(vec![Value::from(1), Value::from(2)]));

                let tuple = ("a", "b").into_pyobject(py).unwrap().into_any();
                let value = py_to_value(py, &tuple, &bindings, true).unwrap();
                assert_eq!(
                    value,
                    Value::List(vec![
                        Value::String("a".to_string()),
                        Value::String("b".to_string())
                    ])
                );

                let dict = PyDict::new(py);
                dict.set_item("x", 1).unwrap();
                let value = py_to_value(py, &dict.into_any(), &bindings, true).unwrap();
                match value {
                    Value::Object(map) => assert_eq!(map.get("x").unwrap(), &Value::from(1)),
                    _ => panic!("expected object"),
                }

                let err = py_to_value(
                    py,
                    &locals.get_item("ScalarType").unwrap().unwrap(),
                    &bindings,
                    false,
                )
                .expect_err("unsupported type should error");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Unsupported value type");
            });
        }

        /// Ensures field value conversion enforces list and abstract type rules.
        #[test]
        fn py_to_field_value_for_type_covers_lists_and_abstracts() {
            crate::with_py(|py| {
                let locals = make_meta_objects(py);
                let obj = locals.get_item("Obj").unwrap().unwrap().call0().unwrap();

                let mut abstract_types = HashSet::new();
                abstract_types.insert("Obj".to_string());
                let value = py_to_field_value_for_type(
                    py,
                    &obj,
                    &TypeRef::named("Obj"),
                    &[],
                    &abstract_types,
                )
                .unwrap();
                let _ = value;

                let err = py_to_field_value_for_type(
                    py,
                    &locals.get_item("Plain").unwrap().unwrap(),
                    &TypeRef::named("Obj"),
                    &[],
                    &abstract_types,
                )
                .expect_err("abstract type should error for non-grommet value");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Abstract types must return @grommet.type objects");

                let list_ref = TypeRef::List(Box::new(TypeRef::named("String")));
                let list = PyList::new(py, ["a", "b"]).unwrap();
                let list_any = list.into_any();
                let _ = py_to_field_value_for_type(py, &list_any, &list_ref, &[], &HashSet::new())
                    .unwrap();
                let tuple_any = ("a", "b").into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value_for_type(py, &tuple_any, &list_ref, &[], &HashSet::new())
                    .unwrap();

                let int_any = PyInt::new(py, 42).into_any();
                let err = py_to_field_value_for_type(py, &int_any, &list_ref, &[], &HashSet::new())
                    .expect_err("expected list error");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Expected list for GraphQL list type");

                let non_null = TypeRef::NonNull(Box::new(TypeRef::named("String")));
                let ok_any = "ok".into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value_for_type(py, &ok_any, &non_null, &[], &HashSet::new())
                    .unwrap();

                let none_obj = py.None();
                let none_any = none_obj.bind(py);
                let null_value = py_to_field_value_for_type(
                    py,
                    &none_any,
                    &TypeRef::named("String"),
                    &[],
                    &HashSet::new(),
                )
                .unwrap();
                let _ = null_value;
            });
        }

        /// Verifies GraphQL values and responses convert to Python structures.
        #[test]
        fn value_to_py_and_response_to_py_cover_variants() {
            crate::with_py(|py| {
                let value = value_to_py(py, &Value::Null).unwrap();
                assert!(value.bind(py).is_none());

                let value = value_to_py(py, &Value::Boolean(true)).unwrap();
                assert_eq!(value.bind(py).extract::<bool>().unwrap(), true);

                let value = value_to_py(py, &Value::from(1)).unwrap();
                assert_eq!(value.bind(py).extract::<i64>().unwrap(), 1);

                let value = value_to_py(py, &Value::from(1.5)).unwrap();
                assert_eq!(value.bind(py).extract::<f64>().unwrap(), 1.5);

                let value = value_to_py(py, &Value::String("hi".to_string())).unwrap();
                assert_eq!(value.bind(py).extract::<String>().unwrap(), "hi");

                let value = value_to_py(py, &Value::Enum(Name::new("RED"))).unwrap();
                assert_eq!(value.bind(py).extract::<String>().unwrap(), "RED");

                let value = value_to_py(py, &Value::Binary(b"bin".to_vec().into())).unwrap();
                assert_eq!(value.bind(py).cast::<PyBytes>().unwrap().as_bytes(), b"bin");

                let value =
                    value_to_py(py, &Value::List(vec![Value::from(1), Value::from(2)])).unwrap();
                assert_eq!(value.bind(py).cast::<PyList>().unwrap().len(), 2);

                let mut map = IndexMap::new();
                map.insert(Name::new("x"), Value::from(1));
                let value = value_to_py(py, &Value::Object(map)).unwrap();
                assert_eq!(
                    value
                        .bind(py)
                        .cast::<PyDict>()
                        .unwrap()
                        .get_item("x")
                        .unwrap()
                        .unwrap()
                        .extract::<i64>()
                        .unwrap(),
                    1
                );

                let mut error = ServerError::new("boom", Some(Pos { line: 1, column: 2 }));
                error.path = vec![
                    PathSegment::Field("field".to_string()),
                    PathSegment::Index(1),
                ];
                let mut extensions = ErrorExtensionValues::default();
                extensions.set("code", Value::from("ERR"));
                error.extensions = Some(extensions);

                let empty_ext = ErrorExtensionValues::default();
                let mut error_empty = ServerError::new("empty", Some(Pos { line: 2, column: 3 }));
                error_empty.extensions = Some(empty_ext);

                let response = Response::new(Value::from(1)).extension("meta", Value::from("ok"));
                let mut response = response;
                response.errors.push(error);
                response.errors.push(error_empty);

                let result = response_to_py(py, response).unwrap();
                let dict = result.bind(py).cast::<PyDict>().unwrap();
                assert!(dict.get_item("data").unwrap().is_some());
                assert!(dict.get_item("extensions").unwrap().is_some());
                let errors = dict.get_item("errors").unwrap().unwrap();
                assert_eq!(errors.cast::<PyList>().unwrap().len(), 2);
            });
        }

        /// Verifies is_builtin_type correctly identifies Python built-in types.
        #[test]
        fn is_builtin_type_identifies_common_types() {
            crate::with_py(|py| {
                let none_obj = py.None();
                assert!(is_builtin_type(&none_obj.bind(py)));

                let bool_obj = PyBool::new(py, true).to_owned().into_any();
                assert!(is_builtin_type(&bool_obj));

                let int_obj = PyInt::new(py, 42).into_any();
                assert!(is_builtin_type(&int_obj));

                let float_obj = 3.14f64.into_pyobject(py).unwrap().into_any();
                assert!(is_builtin_type(&float_obj));

                let str_obj = "hello".into_pyobject(py).unwrap().into_any();
                assert!(is_builtin_type(&str_obj));

                let list_obj = PyList::empty(py).into_any();
                assert!(is_builtin_type(&list_obj));

                let tuple_obj = (1, 2).into_pyobject(py).unwrap().into_any();
                assert!(is_builtin_type(&tuple_obj));

                let dict_obj = PyDict::new(py).into_any();
                assert!(is_builtin_type(&dict_obj));

                // Custom class should NOT be a builtin type
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!("class Custom: pass\nobj = Custom()"),
                    None,
                    Some(&locals),
                )
                .unwrap();
                let custom_obj = locals.get_item("obj").unwrap().unwrap();
                assert!(!is_builtin_type(&custom_obj));
            });
        }

        /// Verifies sequence conversion helpers handle lists and tuples correctly.
        #[test]
        fn convert_sequence_helpers_cover_paths() {
            crate::with_py(|py| {
                let bindings: Vec<ScalarBinding> = Vec::new();
                let abstract_types = HashSet::new();
                let inner_type = TypeRef::named("String");

                // Test list conversion with type
                let list = PyList::new(py, ["a", "b"]).unwrap();
                let result = convert_sequence_to_field_values(
                    py,
                    &list.into_any(),
                    &inner_type,
                    &bindings,
                    &abstract_types,
                )
                .unwrap();
                let _ = result;

                // Test tuple conversion with type
                let tuple = ("x", "y").into_pyobject(py).unwrap().into_any();
                let result = convert_sequence_to_field_values(
                    py,
                    &tuple,
                    &inner_type,
                    &bindings,
                    &abstract_types,
                )
                .unwrap();
                let _ = result;

                // Test untyped list conversion
                let list = PyList::new(py, [1, 2, 3]).unwrap();
                let result =
                    convert_sequence_to_field_values_untyped(py, &list.into_any(), &bindings)
                        .unwrap();
                let _ = result;

                // Test untyped tuple conversion
                let tuple = (4, 5, 6).into_pyobject(py).unwrap().into_any();
                let result =
                    convert_sequence_to_field_values_untyped(py, &tuple, &bindings).unwrap();
                let _ = result;

                // Test error case: non-sequence passed to typed converter
                let int_obj = PyInt::new(py, 42).into_any();
                let err = convert_sequence_to_field_values(
                    py,
                    &int_obj,
                    &inner_type,
                    &bindings,
                    &abstract_types,
                )
                .expect_err("should error for non-list");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Expected list for GraphQL list type");

                // Test error case: non-sequence passed to untyped converter
                let err = convert_sequence_to_field_values_untyped(py, &int_obj, &bindings)
                    .expect_err("should error for non-list");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Expected list for GraphQL list type");
            });
        }
    }
}

mod resolver {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/resolver.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use pyo3::exceptions::PyRuntimeError;

        /// Verifies resolve_from_parent reads mapping, attribute, and missing values.
        #[test]
        fn resolve_from_parent_covers_sources() {
            crate::with_py(|py| {
                let dict = PyDict::new(py);
                dict.set_item("value", 3).unwrap();
                let parent = PyObj::new(dict.into_any().unbind());
                let value = resolve_from_parent(py, &parent, "value").unwrap();
                assert_eq!(value.bind(py).extract::<i64>().unwrap(), 3);

                let dict = PyDict::new(py);
                let parent = PyObj::new(dict.into_any().unbind());
                let value = resolve_from_parent(py, &parent, "missing").unwrap();
                assert!(value.bind(py).is_none());

                let class = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
class Obj:
    def __init__(self):
        self.attr = 4
obj = Obj()
"#
                    ),
                    None,
                    Some(&class),
                )
                .unwrap();
                let obj = class.get_item("obj").unwrap().unwrap().unbind();
                let parent = PyObj::new(obj);
                let value = resolve_from_parent(py, &parent, "attr").unwrap();
                assert_eq!(value.bind(py).extract::<i64>().unwrap(), 4);

                let class = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
class Obj:
    def __getitem__(self, key):
        if key == "item":
            return 5
        raise KeyError(key)
obj = Obj()
"#
                    ),
                    None,
                    Some(&class),
                )
                .unwrap();
                let obj = class.get_item("obj").unwrap().unwrap().unbind();
                let parent = PyObj::new(obj);
                let value = resolve_from_parent(py, &parent, "item").unwrap();
                assert_eq!(value.bind(py).extract::<i64>().unwrap(), 5);

                let class = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
class Obj:
    pass
obj = Obj()
"#
                    ),
                    None,
                    Some(&class),
                )
                .unwrap();
                let obj = class.get_item("obj").unwrap().unwrap().unbind();
                let parent = PyObj::new(obj);
                let value = resolve_from_parent(py, &parent, "missing").unwrap();
                assert!(value.bind(py).is_none());
            });
        }

        /// Ensures await_value resolves Python awaitables into concrete values.
        #[test]
        fn await_value_waits_for_future() {
            let awaitable = crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
import asyncio
async def coro():
    return 7
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();
                let coro = locals.get_item("coro").unwrap().unwrap();
                coro.call0().unwrap().unbind()
            });
            let awaited = crate::with_py(|py| {
                pyo3_async_runtimes::tokio::run(py, async move {
                    await_value(awaitable)
                        .await
                        .map_err(|err| PyRuntimeError::new_err(err.message))
                })
            })
            .unwrap();
            let value = crate::with_py(|py| awaited.bind(py).extract::<i64>().unwrap());
            assert_eq!(value, 7);
        }
    }
}

mod parse {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/parse.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use pyo3::types::PyDict;

        /// Verifies parse_schema_plan extracts plan dataclass attributes correctly.
        #[test]
        fn parse_schema_plan_round_trip() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
from grommet.plan import SchemaPlan, TypePlan, FieldPlan, ArgPlan
from grommet.metadata import TypeKind, TypeSpec

async def resolver(parent, info):
    return 1

plan = SchemaPlan(
    query="Query", mutation=None, subscription=None,
    types=(
        TypePlan(kind=TypeKind.OBJECT, name="Query", cls=object, fields=(
            FieldPlan(name="value", source="value",
                      type_spec=TypeSpec(kind="named", name="Int", nullable=True),
                      args=(ArgPlan(name="limit",
                                    type_spec=TypeSpec(kind="named", name="Int", nullable=True),
                                    default=10),),
                      resolver_key="Query.value"),
        )),
    ),
    scalars=(), enums=(), unions=(),
    resolvers={"Query.value": resolver},
)
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let plan = locals.get_item("plan").unwrap().unwrap();
                let (schema_def, type_defs, _, _, _, resolver_map, _) =
                    parse_schema_plan(py, &plan).unwrap();
                assert_eq!(schema_def.query, "Query");
                assert_eq!(type_defs.len(), 1);
                assert_eq!(type_defs[0].name, "Query");
                assert_eq!(type_defs[0].fields[0].name, "value");
                assert_eq!(type_defs[0].fields[0].args[0].name, "limit");
                assert!(type_defs[0].fields[0].args[0].default_value.is_some());
                assert_eq!(resolver_map.len(), 1);
                assert!(resolver_map.contains_key("Query.value"));
            });
        }

        /// Ensures all optional plan fields are extracted correctly.
        #[test]
        fn parse_schema_plan_with_optional_fields() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
import enum
from grommet.plan import SchemaPlan, TypePlan, FieldPlan, ArgPlan, ScalarPlan, EnumPlan, UnionPlan
from grommet.metadata import TypeKind, TypeSpec, ScalarMeta, EnumMeta, UnionMeta

async def resolver(parent, info):
    return 1

class Color(enum.Enum):
    RED = "RED"
    BLUE = "BLUE"

class QueryType:
    pass
QueryType.__grommet_meta__ = type('M', (), {'name': 'Query'})()

plan = SchemaPlan(
    query="Query", mutation="Mutation", subscription="Subscription",
    types=(
        TypePlan(kind=TypeKind.OBJECT, name="Query", cls=object, fields=(
            FieldPlan(name="value", source="value",
                      type_spec=TypeSpec(kind="named", name="String", nullable=True),
                      resolver_key="Query.value",
                      description="field desc", deprecation="old"),
        ), implements=("Node",), description="type desc"),
    ),
    scalars=(
        ScalarPlan(cls=object, meta=ScalarMeta(
            name="Date", serialize=str, parse_value=str,
            description="date scalar", specified_by_url="https://example.com/date")),
    ),
    enums=(
        EnumPlan(cls=Color, meta=EnumMeta(name="Color", description="colors")),
    ),
    unions=(
        UnionPlan(cls=object, meta=UnionMeta(
            name="Search", types=(QueryType,), description="search")),
    ),
    resolvers={"Query.value": resolver},
)
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let plan = locals.get_item("plan").unwrap().unwrap();
                let (schema_def, type_defs, scalar_defs, enum_defs, union_defs, _, bindings) =
                    parse_schema_plan(py, &plan).unwrap();
                assert_eq!(schema_def.mutation.as_deref(), Some("Mutation"));
                assert_eq!(schema_def.subscription.as_deref(), Some("Subscription"));
                assert_eq!(type_defs[0].description.as_deref(), Some("type desc"));
                assert_eq!(type_defs[0].implements, vec!["Node".to_string()]);
                assert_eq!(
                    type_defs[0].fields[0].description.as_deref(),
                    Some("field desc")
                );
                assert_eq!(scalar_defs[0].description.as_deref(), Some("date scalar"));
                assert_eq!(
                    enum_defs[0].values,
                    vec!["RED".to_string(), "BLUE".to_string()]
                );
                assert_eq!(union_defs[0].types, vec!["Query".to_string()]);
                assert_eq!(bindings.len(), 1);
            });
        }
    }
}

mod build {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/build.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::types::{
            ArgDef, EnumDef, FieldDef, PyObj, ScalarDef, SchemaDef, TypeDef, UnionDef,
        };
        use async_graphql::dynamic::TypeRef;
        use pyo3::types::{PyDict, PyInt};
        use std::collections::HashMap;

        /// Verifies input field defaults are applied during schema construction.
        #[test]
        fn build_input_field_applies_default() {
            crate::with_py(|py| {
                let field_def = FieldDef {
                    name: "value".to_string(),
                    source: "value".to_string(),
                    type_ref: TypeRef::named("Int"),
                    args: Vec::new(),
                    resolver: None,
                    description: None,
                    deprecation: None,
                    default_value: Some(crate::types::PyObj::new(
                        PyInt::new(py, 3).into_any().unbind(),
                    )),
                };
                let input = build_input_field(field_def, Arc::new(Vec::new())).unwrap();
                let _ = input;
            });
        }

        /// Verifies schema building registers all supported type kinds.
        #[test]
        fn build_schema_registers_all_type_kinds() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
def resolver(parent, info, limit: int = 1):
    return limit
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();
                let resolver = locals.get_item("resolver").unwrap().unwrap().unbind();

                let mut resolver_map = HashMap::new();
                resolver_map.insert(
                    "Query.value".to_string(),
                    PyObj::new(resolver.clone_ref(py)),
                );
                resolver_map.insert(
                    "Subscription.ticks".to_string(),
                    PyObj::new(resolver.clone_ref(py)),
                );

                let default_value = PyObj::new(PyInt::new(py, 2).into_any().unbind());
                let make_arg = || ArgDef {
                    name: "limit".to_string(),
                    type_ref: TypeRef::named("Int"),
                    default_value: Some(default_value.clone()),
                };

                let query_field = FieldDef {
                    name: "value".to_string(),
                    source: "value".to_string(),
                    type_ref: TypeRef::named("String"),
                    args: vec![make_arg()],
                    resolver: Some("Query.value".to_string()),
                    description: Some("field desc".to_string()),
                    deprecation: Some("old".to_string()),
                    default_value: None,
                };

                let id_field = FieldDef {
                    name: "id".to_string(),
                    source: "id".to_string(),
                    type_ref: TypeRef::NonNull(Box::new(TypeRef::named("ID"))),
                    args: Vec::new(),
                    resolver: None,
                    description: None,
                    deprecation: None,
                    default_value: None,
                };

                let interface_field = FieldDef {
                    name: "id".to_string(),
                    source: "id".to_string(),
                    type_ref: TypeRef::NonNull(Box::new(TypeRef::named("ID"))),
                    args: vec![make_arg()],
                    resolver: None,
                    description: Some("iface field".to_string()),
                    deprecation: Some("iface old".to_string()),
                    default_value: None,
                };

                let subscription_field = FieldDef {
                    name: "ticks".to_string(),
                    source: "ticks".to_string(),
                    type_ref: TypeRef::NonNull(Box::new(TypeRef::named("Int"))),
                    args: vec![make_arg()],
                    resolver: Some("Subscription.ticks".to_string()),
                    description: Some("sub field".to_string()),
                    deprecation: Some("sub old".to_string()),
                    default_value: None,
                };

                let input_field = FieldDef {
                    name: "count".to_string(),
                    source: "count".to_string(),
                    type_ref: TypeRef::named("Int"),
                    args: Vec::new(),
                    resolver: None,
                    description: None,
                    deprecation: None,
                    default_value: Some(default_value),
                };

                let schema_def = SchemaDef {
                    query: "Query".to_string(),
                    mutation: None,
                    subscription: Some("Subscription".to_string()),
                };

                let type_defs = vec![
                    TypeDef {
                        kind: TypeKind::Interface,
                        name: "Node".to_string(),
                        fields: vec![interface_field],
                        description: Some("iface".to_string()),
                        implements: Vec::new(),
                    },
                    TypeDef {
                        kind: TypeKind::Object,
                        name: "Query".to_string(),
                        fields: vec![id_field, query_field],
                        description: Some("query desc".to_string()),
                        implements: vec!["Node".to_string()],
                    },
                    TypeDef {
                        kind: TypeKind::Subscription,
                        name: "Subscription".to_string(),
                        fields: vec![subscription_field],
                        description: Some("sub desc".to_string()),
                        implements: Vec::new(),
                    },
                    TypeDef {
                        kind: TypeKind::Input,
                        name: "InputData".to_string(),
                        fields: vec![input_field],
                        description: Some("input desc".to_string()),
                        implements: Vec::new(),
                    },
                ];

                let scalar_defs = vec![ScalarDef {
                    name: "Date".to_string(),
                    description: Some("date scalar".to_string()),
                    specified_by_url: Some("https://example.com/date".to_string()),
                }];

                let enum_defs = vec![EnumDef {
                    name: "Color".to_string(),
                    description: Some("colors".to_string()),
                    values: vec!["RED".to_string(), "BLUE".to_string()],
                }];

                let union_defs = vec![UnionDef {
                    name: "Search".to_string(),
                    description: Some("search".to_string()),
                    types: vec!["Query".to_string()],
                }];

                let schema = build_schema(
                    schema_def,
                    type_defs,
                    scalar_defs,
                    enum_defs,
                    union_defs,
                    resolver_map,
                    Arc::new(Vec::new()),
                )
                .unwrap();

                let sdl = schema.sdl();
                assert!(sdl.contains("type Query"));
                assert!(sdl.contains("interface Node"));
                assert!(sdl.contains("enum Color"));
                assert!(sdl.contains("union Search"));
                assert!(sdl.contains("input InputData"));
            });
        }
    }
}

mod api {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/api.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use pyo3::types::{PyAnyMethods, PyDict, PyList, PyStringMethods};

        fn build_plan_with_args(py: Python<'_>) -> Py<PyAny> {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
from grommet.plan import SchemaPlan, TypePlan, FieldPlan, ArgPlan
from grommet.metadata import TypeKind, TypeSpec, MISSING

async def greet(parent, info, name: str):
    return f"{info['root']['prefix']}{name}{info['context']['suffix']}"

async def ticks(parent, info, limit: int):
    for i in range(limit):
        yield i

plan = SchemaPlan(
    query="Query", mutation=None, subscription="Subscription",
    types=(
        TypePlan(kind=TypeKind.OBJECT, name="Query", cls=object, fields=(
            FieldPlan(name="greet", source="greet",
                      type_spec=TypeSpec(kind="named", name="String"),
                      args=(ArgPlan(name="name",
                                    type_spec=TypeSpec(kind="named", name="String"),
                                    default=MISSING),),
                      resolver_key="Query.greet"),
        )),
        TypePlan(kind=TypeKind.SUBSCRIPTION, name="Subscription", cls=object, fields=(
            FieldPlan(name="ticks", source="ticks",
                      type_spec=TypeSpec(kind="named", name="Int"),
                      args=(ArgPlan(name="limit",
                                    type_spec=TypeSpec(kind="named", name="Int"),
                                    default=MISSING),),
                      resolver_key="Subscription.ticks"),
        )),
    ),
    scalars=(), enums=(), unions=(),
    resolvers={"Query.greet": greet, "Subscription.ticks": ticks},
)
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();
            locals.get_item("plan").unwrap().unwrap().unbind()
        }

        fn build_subscription_plan(
            py: Python<'_>,
            query_resolver: &Bound<'_, PyAny>,
            subscription_resolver: &Bound<'_, PyAny>,
            field_type: &str,
        ) -> Py<PyAny> {
            let locals = PyDict::new(py);
            locals.set_item("query_resolver", query_resolver).unwrap();
            locals
                .set_item("subscription_resolver", subscription_resolver)
                .unwrap();
            // Build the TypeSpec for the subscription field type in Rust,
            // then pass the pre-built object to Python.
            let type_spec = {
                let meta = py.import("grommet.metadata").unwrap();
                let ts_cls = meta.getattr("TypeSpec").unwrap();
                let mut name = field_type;
                let non_null = name.ends_with('!');
                if non_null {
                    name = &name[..name.len() - 1];
                }
                if name.starts_with('[') && name.ends_with(']') {
                    let inner_name = &name[1..name.len() - 1];
                    let inner = ts_cls
                        .call1(("named", inner_name, py.None(), true))
                        .unwrap();
                    ts_cls.call1(("list", py.None(), inner, !non_null)).unwrap()
                } else {
                    ts_cls.call1(("named", name, py.None(), !non_null)).unwrap()
                }
            };
            locals.set_item("tick_type_spec", &type_spec).unwrap();
            py.run(
                pyo3::ffi::c_str!(
                    r#"
from grommet.plan import SchemaPlan, TypePlan, FieldPlan
from grommet.metadata import TypeKind, TypeSpec

plan = SchemaPlan(
    query="Query", mutation=None, subscription="Subscription",
    types=(
        TypePlan(kind=TypeKind.OBJECT, name="Query", cls=object, fields=(
            FieldPlan(name="noop", source="noop",
                      type_spec=TypeSpec(kind="named", name="Int"),
                      resolver_key="Query.noop"),
        )),
        TypePlan(kind=TypeKind.SUBSCRIPTION, name="Subscription", cls=object, fields=(
            FieldPlan(name="tick", source="tick",
                      type_spec=tick_type_spec,
                      resolver_key="Subscription.tick"),
        )),
    ),
    scalars=(), enums=(), unions=(),
    resolvers={"Query.noop": query_resolver, "Subscription.tick": subscription_resolver},
)
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();
            locals.get_item("plan").unwrap().unwrap().unbind()
        }

        fn assert_response_has_errors(response: &Bound<'_, PyAny>) {
            if response.is_none() {
                return;
            }
            let dict = response.cast::<PyDict>().unwrap();
            let errors = dict.get_item("errors").unwrap().unwrap();
            assert!(!errors.cast::<PyList>().unwrap().is_empty());
        }

        /// Verifies SchemaWrapper executes queries and subscriptions with variables.
        #[test]
        fn schema_wrapper_executes_and_subscribes_with_variables() {
            let plan = crate::with_py(|py| build_plan_with_args(py));
            crate::with_py(|py| {
                let query_vars = PyDict::new(py);
                query_vars.set_item("name", "Ada").unwrap();
                let query_vars = query_vars.into_any().unbind();

                let sub_vars = PyDict::new(py);
                sub_vars.set_item("limit", 1).unwrap();
                let sub_vars = sub_vars.into_any().unbind();

                let root = PyDict::new(py);
                root.set_item("prefix", "hi ").unwrap();
                let root = root.into_any().unbind();

                let context = PyDict::new(py);
                context.set_item("suffix", "!").unwrap();
                let context = context.into_any().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let _ = wrapper.sdl()?;

                    let awaitable = Python::attach(|py| {
                        wrapper
                            .execute(
                                py,
                                "query($name: String!) { greet(name: $name) }".to_string(),
                                Some(query_vars),
                                Some(root.clone_ref(py)),
                                Some(context.clone_ref(py)),
                            )
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let query_result = crate::runtime::into_future(awaitable)?.await?;
                    Python::attach(|py| {
                        let dict = query_result.bind(py).cast::<PyDict>().unwrap();
                        assert!(dict.get_item("data").unwrap().is_some());
                    });

                    let stream = Python::attach(|py| {
                        wrapper.subscribe(
                            py,
                            "subscription($limit: Int!) { ticks(limit: $limit) }".to_string(),
                            Some(sub_vars),
                            Some(root.clone_ref(py)),
                            Some(context.clone_ref(py)),
                        )
                    })?;

                    let next = Python::attach(|py| -> PyResult<Py<PyAny>> {
                        Ok(stream.__anext__(py)?.expect("expected awaitable").unbind())
                    })?;
                    let sub_result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| {
                        let dict = sub_result.bind(py).cast::<PyDict>().unwrap();
                        assert!(dict.get_item("data").unwrap().is_some());
                    });

                    let close =
                        Python::attach(|py| stream.aclose(py).map(|awaitable| awaitable.unbind()))?;
                    let _ = crate::runtime::into_future(close)?.await?;
                    let closed =
                        Python::attach(|py| Ok::<bool, PyErr>(stream.__anext__(py)?.is_none()))?;
                    assert!(closed);

                    Ok(())
                })
            })
            .unwrap();
        }

        /// Ensures SchemaWrapper can execute multiple queries concurrently.
        #[test]
        fn schema_wrapper_executes_concurrently() {
            let plan = crate::with_py(|py| build_plan_with_args(py));
            crate::with_py(|py| {
                let vars_one = PyDict::new(py);
                vars_one.set_item("name", "Ada").unwrap();
                let vars_one = vars_one.into_any().unbind();

                let vars_two = PyDict::new(py);
                vars_two.set_item("name", "Turing").unwrap();
                let vars_two = vars_two.into_any().unbind();

                let root = PyDict::new(py);
                root.set_item("prefix", "hi ").unwrap();
                let root = root.into_any().unbind();

                let context = PyDict::new(py);
                context.set_item("suffix", "!").unwrap();
                let context = context.into_any().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;

                    let await_one = Python::attach(|py| {
                        wrapper
                            .execute(
                                py,
                                "query($name: String!) { greet(name: $name) }".to_string(),
                                Some(vars_one.clone_ref(py)),
                                Some(root.clone_ref(py)),
                                Some(context.clone_ref(py)),
                            )
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let await_two = Python::attach(|py| {
                        wrapper
                            .execute(
                                py,
                                "query($name: String!) { greet(name: $name) }".to_string(),
                                Some(vars_two.clone_ref(py)),
                                Some(root.clone_ref(py)),
                                Some(context.clone_ref(py)),
                            )
                            .map(|awaitable| awaitable.unbind())
                    })?;

                    let fut_one = crate::runtime::into_future(await_one)?;
                    let fut_two = crate::runtime::into_future(await_two)?;
                    let (res_one, res_two) = tokio::join!(fut_one, fut_two);

                    let res_one = res_one?;
                    let res_two = res_two?;
                    Python::attach(|py| {
                        let dict = res_one.bind(py).cast::<PyDict>().unwrap();
                        let data = dict.get_item("data").unwrap().unwrap();
                        let data = data.cast::<PyDict>().unwrap();
                        let greet = data.get_item("greet").unwrap().unwrap();
                        assert_eq!(greet.extract::<String>().unwrap(), "hi Ada!");
                    });
                    Python::attach(|py| {
                        let dict = res_two.bind(py).cast::<PyDict>().unwrap();
                        let data = dict.get_item("data").unwrap().unwrap();
                        let data = data.cast::<PyDict>().unwrap();
                        let greet = data.get_item("greet").unwrap().unwrap();
                        assert_eq!(greet.extract::<String>().unwrap(), "hi Turing!");
                    });

                    Ok(())
                })
            })
            .unwrap();
        }

        /// Verifies SubscriptionStream reports errors for missing or empty streams.
        #[test]
        fn subscription_stream_handles_empty_and_missing_stream() {
            use async_graphql::futures_util::StreamExt;
            use async_graphql::futures_util::stream;
            use std::sync::Arc;
            use std::sync::atomic::AtomicBool;
            use tokio::sync::Mutex;

            crate::with_py(|py| {
                pyo3_async_runtimes::tokio::run(py, async move {
                    let missing = SubscriptionStream {
                        stream: Arc::new(Mutex::new(None)),
                        closed: Arc::new(AtomicBool::new(false)),
                    };
                    let next =
                        Python::attach(|py| missing.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await;
                    assert!(result.is_err());

                    let empty_stream = stream::empty::<async_graphql::Response>().boxed();
                    let empty = SubscriptionStream {
                        stream: Arc::new(Mutex::new(Some(empty_stream))),
                        closed: Arc::new(AtomicBool::new(false)),
                    };
                    let next = Python::attach(|py| empty.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await;
                    assert!(result.is_err());
                    Ok(())
                })
            })
            .unwrap();
        }

        /// Ensures SchemaWrapper requires root values for parent resolution.
        #[test]
        fn schema_wrapper_requires_root_for_parent_resolution() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
from grommet.plan import SchemaPlan, TypePlan, FieldPlan
from grommet.metadata import TypeKind, TypeSpec

plan = SchemaPlan(
    query="Query", mutation=None, subscription=None,
    types=(
        TypePlan(kind=TypeKind.OBJECT, name="Query", cls=object, fields=(
            FieldPlan(name="value", source="value",
                      type_spec=TypeSpec(kind="named", name="Int")),
        )),
    ),
    scalars=(), enums=(), unions=(),
)
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();
                let plan = locals.get_item("plan").unwrap().unwrap().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;

                    let awaitable = Python::attach(|py| {
                        wrapper
                            .execute(py, "{ value }".to_string(), None, None, None)
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let without_root = crate::runtime::into_future(awaitable)?.await?;
                    Python::attach(|py| {
                        assert_response_has_errors(without_root.bind(py));
                    });

                    Ok(())
                })
            })
            .unwrap();
        }

        /// Verifies subscription resolvers support async iterators with __anext__.
        #[test]
        fn subscription_resolver_only_anext() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
async def noop(parent, info):
    return 1

class OnlyAnext:
    async def __anext__(self):
        return 1

async def sub_only_anext(parent, info):
    return OnlyAnext()
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let noop = locals.get_item("noop").unwrap().unwrap().unbind();
                let sub_only_anext = locals.get_item("sub_only_anext").unwrap().unwrap().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let plan = Python::attach(|py| {
                        build_subscription_plan(
                            py,
                            &noop.bind(py),
                            &sub_only_anext.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| {
                        let dict = result.bind(py).cast::<PyDict>().unwrap();
                        let data = dict.get_item("data").unwrap().unwrap();
                        if data.is_none() {
                            assert_response_has_errors(result.bind(py));
                        } else {
                            let data = data.cast::<PyDict>().unwrap();
                            assert_eq!(
                                data.get_item("tick")
                                    .unwrap()
                                    .unwrap()
                                    .extract::<i64>()
                                    .unwrap(),
                                1
                            );
                        }
                    });
                    Ok(())
                })
            })
            .unwrap();
        }

        /// Ensures subscription resolvers reject non-async-iterator results.
        #[test]
        fn subscription_resolver_requires_async_iterator() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
async def noop(parent, info):
    return 1

class NotAsync:
    pass

async def sub_not_async(parent, info):
    return NotAsync()
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let noop = locals.get_item("noop").unwrap().unwrap().unbind();
                let sub_not_async = locals.get_item("sub_not_async").unwrap().unwrap().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let plan = Python::attach(|py| {
                        build_subscription_plan(py, &noop.bind(py), &sub_not_async.bind(py), "Int!")
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| {
                        assert_response_has_errors(result.bind(py));
                    });
                    Ok(())
                })
            })
            .unwrap();
        }

        /// Verifies subscription resolver error cases surface as GraphQL errors.
        #[test]
        fn subscription_resolver_error_branches() {
            use pyo3::exceptions::PyStopAsyncIteration;

            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
async def noop(parent, info):
    return 1

class RaiseInAnext:
    def __anext__(self):
        raise RuntimeError("boom")

class NonAwaitableAnext:
    def __anext__(self):
        return 1

class ErrorAsync:
    async def __anext__(self):
        raise ValueError("bad")

class OnlyAnext:
    async def __anext__(self):
        return 1

async def sub_raise(parent, info):
    return RaiseInAnext()

async def sub_non_awaitable(parent, info):
    return NonAwaitableAnext()

async def sub_stop(parent, info):
    if False:
        yield 1

async def sub_error(parent, info):
    return ErrorAsync()

async def sub_wrong_type(parent, info):
    return OnlyAnext()
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let noop = locals.get_item("noop").unwrap().unwrap().unbind();
                let sub_raise = locals.get_item("sub_raise").unwrap().unwrap().unbind();
                let sub_non_awaitable = locals
                    .get_item("sub_non_awaitable")
                    .unwrap()
                    .unwrap()
                    .unbind();
                let sub_stop = locals.get_item("sub_stop").unwrap().unwrap().unbind();
                let sub_error = locals.get_item("sub_error").unwrap().unwrap().unbind();
                let sub_wrong_type = locals.get_item("sub_wrong_type").unwrap().unwrap().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let plan = Python::attach(|py| {
                        build_subscription_plan(py, &noop.bind(py), &sub_raise.bind(py), "Int!")
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let _ = crate::runtime::into_future(next)?.await;

                    let plan = Python::attach(|py| {
                        build_subscription_plan(
                            py,
                            &noop.bind(py),
                            &sub_non_awaitable.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));

                    let plan = Python::attach(|py| {
                        build_subscription_plan(py, &noop.bind(py), &sub_stop.bind(py), "Int!")
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await;
                    if let Err(err) = result {
                        let is_stop =
                            Python::attach(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                        assert!(is_stop);
                    } else {
                        panic!("expected stop async iteration");
                    }

                    let plan = Python::attach(|py| {
                        build_subscription_plan(py, &noop.bind(py), &sub_error.bind(py), "Int!")
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));

                    let plan = Python::attach(|py| {
                        build_subscription_plan(
                            py,
                            &noop.bind(py),
                            &sub_wrong_type.bind(py),
                            "[Int]",
                        )
                    });
                    let wrapper = Python::attach(|py| SchemaWrapper::new(py, &plan.bind(py)))?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = crate::runtime::into_future(next)?.await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));

                    Ok(())
                })
            })
            .unwrap();
        }

        /// Ensures runtime configuration rejects invalid thread settings.
        #[test]
        fn configure_runtime_rejects_invalid_threads() {
            let err = configure_runtime(true, Some(2)).err().unwrap();
            let msg =
                crate::with_py(|py| err.value(py).str().unwrap().to_str().unwrap().to_string());
            assert_eq!(
                msg,
                "worker_threads cannot be set for a current-thread runtime"
            );

            assert!(configure_runtime(true, None).unwrap());
            assert!(configure_runtime(false, Some(1)).unwrap());
        }
    }
}
