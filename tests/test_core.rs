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

            let err = unsupported_value_type();
            assert_eq!(err_message(&err), "Unsupported value type");

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
        use async_graphql::dynamic::TypeRef;
        use async_graphql::{
            ErrorExtensionValues, Name, PathSegment, Pos, Response, ServerError, Value,
        };
        use indexmap::IndexMap;
        use pyo3::IntoPyObject;
        use pyo3::types::{PyAnyMethods, PyBool, PyBytes, PyDict, PyInt, PyList, PyStringMethods};

        fn make_meta_objects<'py>(py: Python<'py>) -> Bound<'py, PyDict> {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
import dataclasses
import enum

class TypeKind(enum.Enum):
    OBJECT = "object"
    INPUT = "input"

class Meta:
    def __init__(self, kind, name=None):
        self.kind = kind
        self.name = name

class NoType:
    pass

class Obj:
    pass
Obj.__grommet_meta__ = Meta(TypeKind.OBJECT, "Obj")

class Plain:
    pass

class Weird:
    pass
Weird.__grommet_meta__ = NoType()

@dataclasses.dataclass
class Input:
    value: int
Input.__grommet_meta__ = Meta(TypeKind.INPUT, "Input")
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();
            locals
        }

        /// Verifies input_object_as_dict handles type and input variants.
        #[test]
        fn meta_helpers_cover_branches() {
            crate::with_py(|py| {
                let locals = make_meta_objects(py);
                let obj_cls = locals.get_item("Obj").unwrap().unwrap();

                let obj = obj_cls.call0().unwrap();

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
                let plain_cls = locals.get_item("Plain").unwrap().unwrap();
                let plain = plain_cls.call0().unwrap();
                assert!(input_object_as_dict(py, &plain).unwrap().is_none());
            });
        }

        /// Verifies Python values convert to field GraphQL values.
        #[test]
        fn py_to_field_value_covers_paths() {
            crate::with_py(|py| {
                let none_value = py.None();
                let _ = py_to_field_value(py, &none_value.bind(py)).unwrap();

                let bool_value = PyBool::new(py, true).to_owned().into_any();
                let _ = py_to_field_value(py, &bool_value).unwrap();

                let float_value = 1.5f64.into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value(py, &float_value).unwrap();

                let list = PyList::new(py, [1, 2]).unwrap();
                let list_any = list.into_any();
                let _ = py_to_field_value(py, &list_any).unwrap();

                let tuple_any = ("a", "b").into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value(py, &tuple_any).unwrap();

                let locals = make_meta_objects(py);
                let custom = locals.get_item("Obj").unwrap().unwrap().call0().unwrap();
                let _ = py_to_field_value(py, &custom).unwrap();
            });
        }

        /// Verifies Python values convert to GraphQL values across variants.
        #[test]
        fn py_to_value_covers_primitives_and_collections() {
            crate::with_py(|py| {
                let locals = make_meta_objects(py);

                let input_instance = locals
                    .get_item("Input")
                    .unwrap()
                    .unwrap()
                    .call1((3,))
                    .unwrap();
                let value = py_to_value(py, &input_instance).unwrap();
                match value {
                    Value::Object(map) => {
                        assert_eq!(map.get("value").unwrap(), &Value::from(3));
                    }
                    _ => panic!("expected object"),
                }

                let none_obj = py.None();
                let none_value = none_obj.bind(py);
                let value = py_to_value(py, &none_value).unwrap();
                assert_eq!(value, Value::Null);
                let bool_value = PyBool::new(py, true).to_owned().into_any();
                let value = py_to_value(py, &bool_value).unwrap();
                assert_eq!(value, Value::Boolean(true));
                let int_value = PyInt::new(py, 42).into_any();
                let value = py_to_value(py, &int_value).unwrap();
                assert_eq!(value, Value::from(42));
                let float_value = 1.25f64.into_pyobject(py).unwrap().into_any();
                let value = py_to_value(py, &float_value).unwrap();
                assert_eq!(value, Value::from(1.25));
                let str_value = "hi".into_pyobject(py).unwrap().into_any();
                let value = py_to_value(py, &str_value).unwrap();
                assert_eq!(value, Value::String("hi".to_string()));

                let bytes = PyBytes::new(py, b"bin");
                let value = py_to_value(py, &bytes.into_any()).unwrap();
                assert_eq!(value, Value::Binary(b"bin".to_vec().into()));

                let list = PyList::new(py, [1, 2]).unwrap();
                let list_any = list.into_any();
                let value = py_to_value(py, &list_any).unwrap();
                assert_eq!(value, Value::List(vec![Value::from(1), Value::from(2)]));

                let tuple = ("a", "b").into_pyobject(py).unwrap().into_any();
                let value = py_to_value(py, &tuple).unwrap();
                assert_eq!(
                    value,
                    Value::List(vec![
                        Value::String("a".to_string()),
                        Value::String("b".to_string())
                    ])
                );

                let dict = PyDict::new(py);
                dict.set_item("x", 1).unwrap();
                let value = py_to_value(py, &dict.into_any()).unwrap();
                match value {
                    Value::Object(map) => assert_eq!(map.get("x").unwrap(), &Value::from(1)),
                    _ => panic!("expected object"),
                }

                // Custom class should error
                let plain = locals.get_item("Plain").unwrap().unwrap().call0().unwrap();
                let err = py_to_value(py, &plain).expect_err("unsupported type should error");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Unsupported value type");
            });
        }

        /// Ensures field value conversion enforces list rules.
        #[test]
        fn py_to_field_value_for_type_covers_lists() {
            crate::with_py(|py| {
                let list_ref = TypeRef::List(Box::new(TypeRef::named("String")));
                let list = PyList::new(py, ["a", "b"]).unwrap();
                let list_any = list.into_any();
                let _ = py_to_field_value_for_type(py, &list_any, &list_ref, ScalarHint::Unknown)
                    .unwrap();
                let tuple_any = ("a", "b").into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value_for_type(py, &tuple_any, &list_ref, ScalarHint::Unknown)
                    .unwrap();

                let int_any = PyInt::new(py, 42).into_any();
                let err = py_to_field_value_for_type(py, &int_any, &list_ref, ScalarHint::Unknown)
                    .expect_err("expected list error");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Expected list for GraphQL list type");

                let non_null = TypeRef::NonNull(Box::new(TypeRef::named("String")));
                let ok_any = "ok".into_pyobject(py).unwrap().into_any();
                let _ = py_to_field_value_for_type(py, &ok_any, &non_null, ScalarHint::Unknown)
                    .unwrap();

                let none_obj = py.None();
                let none_any = none_obj.bind(py);
                let _ = py_to_field_value_for_type(
                    py,
                    &none_any,
                    &TypeRef::named("String"),
                    ScalarHint::Unknown,
                )
                .unwrap();
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
                let bound = result.bind(py);
                assert!(!bound.getattr("data").unwrap().is_none());
                assert!(!bound.getattr("extensions").unwrap().is_none());
                let errors = bound.getattr("errors").unwrap();
                assert_eq!(errors.cast::<PyList>().unwrap().len(), 2);
            });
        }

        /// Verifies sequence conversion helpers handle lists and tuples correctly.
        #[test]
        fn convert_sequence_helpers_cover_paths() {
            crate::with_py(|py| {
                let inner_type = TypeRef::named("String");

                // Test list conversion with type
                let list = PyList::new(py, ["a", "b"]).unwrap();
                let result = convert_sequence_to_field_values(
                    py,
                    &list.into_any(),
                    &inner_type,
                    ScalarHint::Unknown,
                )
                .unwrap();
                let _ = result;

                // Test tuple conversion with type
                let tuple = ("x", "y").into_pyobject(py).unwrap().into_any();
                let result =
                    convert_sequence_to_field_values(py, &tuple, &inner_type, ScalarHint::Unknown)
                        .unwrap();
                let _ = result;

                // Test untyped list conversion
                let list = PyList::new(py, [1, 2, 3]).unwrap();
                let result =
                    convert_sequence_to_field_values_untyped(py, &list.into_any()).unwrap();
                let _ = result;

                // Test untyped tuple conversion
                let tuple = (4, 5, 6).into_pyobject(py).unwrap().into_any();
                let result = convert_sequence_to_field_values_untyped(py, &tuple).unwrap();
                let _ = result;

                // Test error case: non-sequence passed to typed converter
                let int_obj = PyInt::new(py, 42).into_any();
                let err = convert_sequence_to_field_values(
                    py,
                    &int_obj,
                    &inner_type,
                    ScalarHint::Unknown,
                )
                .expect_err("should error for non-list");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Expected list for GraphQL list type");

                // Test error case: non-sequence passed to untyped converter
                let err = convert_sequence_to_field_values_untyped(py, &int_obj)
                    .expect_err("should error for non-list");
                let msg = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert_eq!(msg, "Expected list for GraphQL list type");
            });
        }
    }
}

mod lookahead {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lookahead.rs"));
}

mod resolver {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/resolver.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Verifies resolve_from_parent reads attributes and returns None for missing.
        #[test]
        fn resolve_from_parent_covers_sources() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
class Obj:
    def __init__(self):
        self.attr = 4
obj = Obj()
empty = Obj.__new__(type('Empty', (), {}))
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let obj = locals.get_item("obj").unwrap().unwrap().unbind();
                let parent = PyObj::new(obj);
                let value = resolve_from_parent(py, &parent, "attr").unwrap();
                assert_eq!(value.bind(py).extract::<i64>().unwrap(), 4);

                let empty = locals.get_item("empty").unwrap().unwrap().unbind();
                let parent = PyObj::new(empty);
                let value = resolve_from_parent(py, &parent, "missing").unwrap();
                assert!(value.bind(py).is_none());
            });
        }

        /// Ensures into_future resolves Python awaitables into concrete values.
        #[test]
        fn into_future_waits_for_coroutine() {
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
                    let future = crate::runtime::into_future(awaitable)?;
                    future.await
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

from grommet.resolver import ResolverInfo

async def resolver(self):
    return 1

info = ResolverInfo(func=resolver, shape="self_only", arg_coercers=[], is_async_gen=False)

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
    resolvers={"Query.value": info},
)
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let plan = locals.get_item("plan").unwrap().unwrap();
                let (schema_def, type_defs, resolver_map) = parse_schema_plan(py, &plan).unwrap();
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
    }
}
