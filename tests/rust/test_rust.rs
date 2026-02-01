#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_clone)]

use std::sync::Arc;

use async_graphql::dynamic::Schema as DynamicSchema;
use async_graphql::{Name, Value};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

fn with_py<F, R>(f: F) -> R
where
    F: for<'py> FnOnce(Python<'py>) -> R,
{
    Python::initialize();
    Python::attach(f)
}

#[test]
fn core_module_registers_items() {
    crate::with_py(|py| {
        let module = PyModule::new(py, "grommet._core").unwrap();
        grommet::_core(py, &module).unwrap();
        assert!(module.getattr("Schema").is_ok());
        assert!(module.getattr("SubscriptionStream").is_ok());
        assert!(module.getattr("configure_runtime").is_ok());
    });
}

#[cfg(not(coverage))]
#[test]
fn python_schema_executes_library_paths() {
    let result = crate::with_py(|py| {
        let module = PyModule::new(py, "grommet._core").unwrap();
        grommet::_core(py, &module).unwrap();
        let schema_type = module.getattr("Schema").unwrap().unbind();

        let locals = PyDict::new(py);
        py.run(
            pyo3::ffi::c_str!(
                r#"
async def greet(parent, info, name: str):
    return f"hi {name}"

async def list_values(parent, info):
    return [1, 2]

async def tuple_values(parent, info):
    return (3, 4)

async def fail(parent, info):
    raise ValueError("boom")

async def ticks(parent, info):
    for i in range(1):
        yield i
"#
            ),
            None,
            Some(&locals),
        )
        .unwrap();

        let greet = locals.get_item("greet").unwrap().unwrap();
        let list_values = locals.get_item("list_values").unwrap().unwrap();
        let tuple_values = locals.get_item("tuple_values").unwrap().unwrap();
        let fail = locals.get_item("fail").unwrap().unwrap();
        let ticks = locals.get_item("ticks").unwrap().unwrap();

        let arg_lang_named = PyDict::new(py);
        arg_lang_named.set_item("name", "lang").unwrap();
        arg_lang_named.set_item("type", "String").unwrap();
        arg_lang_named.set_item("default", "en").unwrap();
        let name_args = PyList::new(py, [arg_lang_named]).unwrap();

        let named_field = PyDict::new(py);
        named_field.set_item("name", "name").unwrap();
        named_field.set_item("type", "String!").unwrap();
        named_field.set_item("args", name_args).unwrap();

        let named_def = PyDict::new(py);
        named_def.set_item("kind", "interface").unwrap();
        named_def.set_item("name", "Named").unwrap();
        named_def
            .set_item("fields", PyList::new(py, [named_field]).unwrap())
            .unwrap();

        let node_field_name = PyDict::new(py);
        node_field_name.set_item("name", "name").unwrap();
        node_field_name.set_item("type", "String!").unwrap();
        let arg_lang_node = PyDict::new(py);
        arg_lang_node.set_item("name", "lang").unwrap();
        arg_lang_node.set_item("type", "String").unwrap();
        arg_lang_node.set_item("default", "en").unwrap();
        node_field_name
            .set_item("args", PyList::new(py, [arg_lang_node]).unwrap())
            .unwrap();

        let node_field_id = PyDict::new(py);
        node_field_id.set_item("name", "id").unwrap();
        node_field_id.set_item("type", "ID!").unwrap();

        let node_def = PyDict::new(py);
        node_def.set_item("kind", "interface").unwrap();
        node_def.set_item("name", "Node").unwrap();
        node_def
            .set_item("implements", PyList::new(py, ["Named"]).unwrap())
            .unwrap();
        node_def
            .set_item(
                "fields",
                PyList::new(py, [node_field_name, node_field_id]).unwrap(),
            )
            .unwrap();

        let greet_arg = PyDict::new(py);
        greet_arg.set_item("name", "name").unwrap();
        greet_arg.set_item("type", "String!").unwrap();
        let greet_args = PyList::new(py, [greet_arg]).unwrap();

        let greet_field = PyDict::new(py);
        greet_field.set_item("name", "greet").unwrap();
        greet_field.set_item("type", "String!").unwrap();
        greet_field.set_item("resolver", "Query.greet").unwrap();
        greet_field.set_item("args", greet_args).unwrap();

        let list_field = PyDict::new(py);
        list_field.set_item("name", "list_values").unwrap();
        list_field.set_item("type", "[Int!]").unwrap();
        list_field
            .set_item("resolver", "Query.list_values")
            .unwrap();

        let tuple_field = PyDict::new(py);
        tuple_field.set_item("name", "tuple_values").unwrap();
        tuple_field.set_item("type", "[Int!]").unwrap();
        tuple_field
            .set_item("resolver", "Query.tuple_values")
            .unwrap();

        let fail_field = PyDict::new(py);
        fail_field.set_item("name", "fail").unwrap();
        fail_field.set_item("type", "String").unwrap();
        fail_field.set_item("resolver", "Query.fail").unwrap();

        let query_id = PyDict::new(py);
        query_id.set_item("name", "id").unwrap();
        query_id.set_item("type", "ID!").unwrap();

        let query_name = PyDict::new(py);
        query_name.set_item("name", "name").unwrap();
        query_name.set_item("type", "String!").unwrap();
        let arg_lang_query = PyDict::new(py);
        arg_lang_query.set_item("name", "lang").unwrap();
        arg_lang_query.set_item("type", "String").unwrap();
        arg_lang_query.set_item("default", "en").unwrap();
        query_name
            .set_item("args", PyList::new(py, [arg_lang_query]).unwrap())
            .unwrap();

        let query_def = PyDict::new(py);
        query_def.set_item("kind", "object").unwrap();
        query_def.set_item("name", "Query").unwrap();
        query_def
            .set_item("implements", PyList::new(py, ["Node"]).unwrap())
            .unwrap();
        query_def
            .set_item(
                "fields",
                PyList::new(
                    py,
                    [
                        query_id,
                        query_name,
                        greet_field,
                        list_field,
                        tuple_field,
                        fail_field,
                    ],
                )
                .unwrap(),
            )
            .unwrap();

        let input_field = PyDict::new(py);
        input_field.set_item("name", "count").unwrap();
        input_field.set_item("type", "Int").unwrap();
        input_field.set_item("default", 7).unwrap();

        let input_def = PyDict::new(py);
        input_def.set_item("kind", "input").unwrap();
        input_def.set_item("name", "InputData").unwrap();
        input_def
            .set_item("fields", PyList::new(py, [input_field]).unwrap())
            .unwrap();

        let sub_field = PyDict::new(py);
        sub_field.set_item("name", "ticks").unwrap();
        sub_field.set_item("type", "Int!").unwrap();
        sub_field
            .set_item("resolver", "Subscription.ticks")
            .unwrap();

        let sub_def = PyDict::new(py);
        sub_def.set_item("kind", "subscription").unwrap();
        sub_def.set_item("name", "Subscription").unwrap();
        sub_def
            .set_item("fields", PyList::new(py, [sub_field]).unwrap())
            .unwrap();

        let schema = PyDict::new(py);
        schema.set_item("query", "Query").unwrap();
        schema.set_item("subscription", "Subscription").unwrap();

        let definition = PyDict::new(py);
        definition.set_item("schema", schema).unwrap();
        definition
            .set_item(
                "types",
                PyList::new(py, [named_def, node_def, query_def, input_def, sub_def]).unwrap(),
            )
            .unwrap();
        definition.set_item("scalars", PyList::empty(py)).unwrap();
        definition.set_item("enums", PyList::empty(py)).unwrap();
        definition.set_item("unions", PyList::empty(py)).unwrap();

        let resolvers = PyDict::new(py);
        resolvers.set_item("Query.greet", greet).unwrap();
        resolvers
            .set_item("Query.list_values", list_values)
            .unwrap();
        resolvers
            .set_item("Query.tuple_values", tuple_values)
            .unwrap();
        resolvers.set_item("Query.fail", fail).unwrap();
        resolvers.set_item("Subscription.ticks", ticks).unwrap();

        let definition = definition.into_any().unbind();
        let resolvers = resolvers.unbind();

        pyo3_async_runtimes::tokio::run(py, async move {
            let schema_obj = Python::attach(|py| {
                schema_type
                    .bind(py)
                    .call1((definition.bind(py), resolvers.bind(py)))
                    .map(|obj| obj.unbind())
            })?;

            let vars = Python::attach(|py| {
                let vars = PyDict::new(py);
                vars.set_item("name", "Ada")?;
                Ok::<_, PyErr>(vars.into_any().unbind())
            })?;

            let awaitable = Python::attach(|py| {
                schema_obj
                    .bind(py)
                    .call_method1(
                        "execute",
                        (
                            "query($name: String!) { greet(name: $name) list_values tuple_values fail }",
                            vars.clone_ref(py),
                            py.None(),
                            py.None(),
                        ),
                    )
                    .map(|awaitable| awaitable.unbind())
            })?;
            let result = Python::attach(|py| {
                pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
            })?
            .await?;

            Python::attach(|py| {
                let dict = result.bind(py).cast::<PyDict>().unwrap();
                let data = dict.get_item("data").unwrap().unwrap();
                let data = data.cast::<PyDict>().unwrap();
                assert_eq!(
                    data.get_item("greet")
                        .unwrap()
                        .unwrap()
                        .extract::<String>()
                        .unwrap(),
                    "hi Ada"
                );
                let list_values_any = data.get_item("list_values").unwrap().unwrap();
                let list_values = list_values_any.cast::<PyList>().unwrap();
                assert_eq!(list_values.len(), 2);
                let tuple_values_any = data.get_item("tuple_values").unwrap().unwrap();
                let tuple_values = tuple_values_any.cast::<PyList>().unwrap();
                assert_eq!(tuple_values.len(), 2);

                let errors = dict.get_item("errors").unwrap().unwrap();
                let errors = errors.cast::<PyList>().unwrap();
                assert!(!errors.is_empty());
            });

            let awaitable = Python::attach(|py| {
                schema_obj
                    .bind(py)
                    .call_method1("execute", ("{ fail }", py.None(), py.None(), py.None()))
                    .map(|awaitable| awaitable.unbind())
            })?;
            let fail_result = Python::attach(|py| {
                pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
            })?
            .await?;

            Python::attach(|py| {
                let dict = fail_result.bind(py).cast::<PyDict>().unwrap();
                let errors = dict.get_item("errors").unwrap().unwrap();
                let errors = errors.cast::<PyList>().unwrap();
                assert!(!errors.is_empty());
                let err = errors.get_item(0).unwrap();
                let err = err.cast::<PyDict>().unwrap();
                if let Ok(Some(path_any)) = err.get_item("path") {
                    let path = path_any.cast::<PyList>().unwrap();
                    assert_eq!(
                        path.get_item(0).unwrap().extract::<String>().unwrap(),
                        "fail"
                    );
                }
            });

            Ok(())
        })
    });
    result.unwrap();
}

#[cfg(not(coverage))]
#[test]
fn python_schema_subscription_branches() {
    let result = crate::with_py(|py| {
        let module = PyModule::new(py, "grommet._core").unwrap();
        grommet::_core(py, &module).unwrap();
        let schema_type = module.getattr("Schema").unwrap().unbind();

        let locals = PyDict::new(py);
        py.run(
            pyo3::ffi::c_str!(
                r#"
async def noop(parent, info):
    return 1

class OnlyAnext:
    async def __anext__(self):
        return 1

class NotAsync:
    pass

class RaiseInAnext:
    def __anext__(self):
        raise RuntimeError("boom")

class NonAwaitableAnext:
    def __anext__(self):
        return 1

async def only_anext(parent, info):
    return OnlyAnext()

async def not_async(parent, info):
    return NotAsync()

async def raise_in_anext(parent, info):
    return RaiseInAnext()

async def non_awaitable(parent, info):
    return NonAwaitableAnext()

async def stop(parent, info):
    if False:
        yield 1

async def wrong_type(parent, info):
    return OnlyAnext()
"#
            ),
            None,
            Some(&locals),
        )
        .unwrap();

        let noop = locals.get_item("noop").unwrap().unwrap();
        let only_anext = locals.get_item("only_anext").unwrap().unwrap();
        let not_async = locals.get_item("not_async").unwrap().unwrap();
        let raise_in_anext = locals.get_item("raise_in_anext").unwrap().unwrap();
        let non_awaitable = locals.get_item("non_awaitable").unwrap().unwrap();
        let stop = locals.get_item("stop").unwrap().unwrap();
        let wrong_type = locals.get_item("wrong_type").unwrap().unwrap();

        let sub_fields = PyList::empty(py);
        let field = PyDict::new(py);
        field.set_item("name", "only_anext").unwrap();
        field.set_item("type", "Int!").unwrap();
        field
            .set_item("resolver", "Subscription.only_anext")
            .unwrap();
        sub_fields.append(field).unwrap();

        let field = PyDict::new(py);
        field.set_item("name", "not_async").unwrap();
        field.set_item("type", "Int!").unwrap();
        field
            .set_item("resolver", "Subscription.not_async")
            .unwrap();
        sub_fields.append(field).unwrap();

        let field = PyDict::new(py);
        field.set_item("name", "raise_in_anext").unwrap();
        field.set_item("type", "Int!").unwrap();
        field
            .set_item("resolver", "Subscription.raise_in_anext")
            .unwrap();
        sub_fields.append(field).unwrap();

        let field = PyDict::new(py);
        field.set_item("name", "non_awaitable").unwrap();
        field.set_item("type", "Int!").unwrap();
        field
            .set_item("resolver", "Subscription.non_awaitable")
            .unwrap();
        sub_fields.append(field).unwrap();

        let field = PyDict::new(py);
        field.set_item("name", "stop").unwrap();
        field.set_item("type", "Int!").unwrap();
        field.set_item("resolver", "Subscription.stop").unwrap();
        sub_fields.append(field).unwrap();

        let field = PyDict::new(py);
        field.set_item("name", "wrong_type").unwrap();
        field.set_item("type", "[Int]").unwrap();
        field
            .set_item("resolver", "Subscription.wrong_type")
            .unwrap();
        sub_fields.append(field).unwrap();

        let query_field = PyDict::new(py);
        query_field.set_item("name", "noop").unwrap();
        query_field.set_item("type", "Int!").unwrap();
        query_field.set_item("resolver", "Query.noop").unwrap();

        let query_def = PyDict::new(py);
        query_def.set_item("kind", "object").unwrap();
        query_def.set_item("name", "Query").unwrap();
        query_def
            .set_item("fields", PyList::new(py, [query_field]).unwrap())
            .unwrap();

        let sub_def = PyDict::new(py);
        sub_def.set_item("kind", "subscription").unwrap();
        sub_def.set_item("name", "Subscription").unwrap();
        sub_def.set_item("fields", sub_fields).unwrap();

        let schema = PyDict::new(py);
        schema.set_item("query", "Query").unwrap();
        schema.set_item("subscription", "Subscription").unwrap();

        let definition = PyDict::new(py);
        definition.set_item("schema", schema).unwrap();
        definition
            .set_item("types", PyList::new(py, [query_def, sub_def]).unwrap())
            .unwrap();
        definition.set_item("scalars", PyList::empty(py)).unwrap();
        definition.set_item("enums", PyList::empty(py)).unwrap();
        definition.set_item("unions", PyList::empty(py)).unwrap();

        let resolvers = PyDict::new(py);
        resolvers.set_item("Query.noop", noop).unwrap();
        resolvers
            .set_item("Subscription.only_anext", only_anext)
            .unwrap();
        resolvers
            .set_item("Subscription.not_async", not_async)
            .unwrap();
        resolvers
            .set_item("Subscription.raise_in_anext", raise_in_anext)
            .unwrap();
        resolvers
            .set_item("Subscription.non_awaitable", non_awaitable)
            .unwrap();
        resolvers.set_item("Subscription.stop", stop).unwrap();
        resolvers
            .set_item("Subscription.wrong_type", wrong_type)
            .unwrap();

        let definition = definition.into_any().unbind();
        let resolvers = resolvers.unbind();

        pyo3_async_runtimes::tokio::run(py, async move {
            let schema_obj = Python::attach(|py| {
                schema_type
                    .bind(py)
                    .call1((definition.bind(py), resolvers.bind(py)))
                    .map(|obj| obj.unbind())
            })?;

            let run_case =
                |py: Python<'_>, schema: &Bound<'_, PyAny>, query: &str| -> PyResult<Py<PyAny>> {
                    schema
                        .call_method1("subscribe", (query, py.None(), py.None(), py.None()))
                        .map(|stream| stream.unbind())
                };

            let only_stream = Python::attach(|py| {
                run_case(py, schema_obj.bind(py), "subscription { only_anext }")
            })?;
            let next = Python::attach(|py| {
                only_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;

            let not_async_stream = Python::attach(|py| {
                run_case(py, schema_obj.bind(py), "subscription { not_async }")
            })?;
            let next = Python::attach(|py| {
                not_async_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;

            let raise_stream = Python::attach(|py| {
                run_case(py, schema_obj.bind(py), "subscription { raise_in_anext }")
            })?;
            let next = Python::attach(|py| {
                raise_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;
            let next = Python::attach(|py| {
                raise_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;

            let non_awaitable_stream = Python::attach(|py| {
                run_case(py, schema_obj.bind(py), "subscription { non_awaitable }")
            })?;
            let next = Python::attach(|py| {
                non_awaitable_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;

            let stop_stream =
                Python::attach(|py| run_case(py, schema_obj.bind(py), "subscription { stop }"))?;
            let next = Python::attach(|py| {
                stop_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;

            let wrong_stream = Python::attach(|py| {
                run_case(py, schema_obj.bind(py), "subscription { wrong_type }")
            })?;
            let next = Python::attach(|py| {
                wrong_stream
                    .bind(py)
                    .call_method0("__anext__")
                    .map(|awaitable| awaitable.unbind())
            })?;
            let _ =
                Python::attach(|py| pyo3_async_runtimes::tokio::into_future(next.into_bound(py)))?
                    .await;

            Ok(())
        })
    });
    result.unwrap();
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

        #[test]
        fn error_helpers_round_trip() {
            let err = py_type_error("boom");
            assert!(crate::with_py(|py| err.is_instance_of::<PyTypeError>(py)));
            assert_eq!(err_message(&err), "boom");

            let err = py_value_error("nope");
            assert!(crate::with_py(|py| err.is_instance_of::<PyValueError>(py)));
            assert_eq!(err_message(&err), "nope");

            let err = missing_field("query");
            assert_eq!(err_message(&err), "Missing query");

            let err = unknown_type_kind("mystery");
            assert_eq!(err_message(&err), "Unknown type kind: mystery");

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

mod types {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/types.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use pyo3::IntoPyObject;

        #[test]
        fn pyobj_bind_clone_round_trip() {
            crate::with_py(|py| {
                let obj = "hello".into_pyobject(py).unwrap().into_any().unbind();
                let pyobj = PyObj::new(obj);
                let bound = pyobj.bind(py);
                assert_eq!(bound.extract::<String>().unwrap(), "hello");
                let cloned = pyobj.clone_ref(py);
                assert_eq!(cloned.bind(py).extract::<String>().unwrap(), "hello");

                let root = RootValue(pyobj.clone());
                let ctx = ContextValue(pyobj);
                let _ = root;
                let _ = ctx;
            });
        }
    }
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
        use pyo3::types::{PyAnyMethods, PyBool, PyBytes, PyDict, PyInt, PyList, PyStringMethods};
        use pyo3::IntoPyObject;
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
    }
}

mod resolver {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/resolver.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use pyo3::exceptions::PyRuntimeError;

        #[test]
        fn resolve_from_parent_covers_sources() {
            crate::with_py(|py| {
                let dict = PyDict::new(py);
                dict.set_item("value", 3).unwrap();
                let parent = PyObj::new(dict.into_any().unbind());
                let (_awaitable, value) = resolve_from_parent(py, &parent, "value").unwrap();
                assert_eq!(value.bind(py).extract::<i64>().unwrap(), 3);

                let dict = PyDict::new(py);
                let parent = PyObj::new(dict.into_any().unbind());
                let (_awaitable, value) = resolve_from_parent(py, &parent, "missing").unwrap();
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
                let (_awaitable, value) = resolve_from_parent(py, &parent, "attr").unwrap();
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
                let (_awaitable, value) = resolve_from_parent(py, &parent, "item").unwrap();
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
                let (_awaitable, value) = resolve_from_parent(py, &parent, "missing").unwrap();
                assert!(value.bind(py).is_none());
            });
        }

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
        use pyo3::IntoPyObject;

        fn err_message(err: PyErr) -> String {
            crate::with_py(|py| err.value(py).str().unwrap().to_str().unwrap().to_string())
        }

        #[test]
        fn parse_definitions_and_resolvers() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
class Root:
    pass

def resolver(parent, info, value: int = 1):
    return value
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let resolver = locals.get_item("resolver").unwrap().unwrap();
                let resolvers = PyDict::new(py);
                resolvers.set_item("Query.value", &resolver).unwrap();
                let map = parse_resolvers(py, Some(&resolvers)).unwrap();
                assert_eq!(map.len(), 1);

                let scalar_list = PyList::empty(py);
                let scalar_def = PyDict::new(py);
                scalar_def.set_item("name", "Scalar").unwrap();
                scalar_def
                    .set_item("python_type", locals.get_item("Root").unwrap().unwrap())
                    .unwrap();
                scalar_def.set_item("serialize", &resolver).unwrap();
                scalar_list.append(scalar_def).unwrap();
                let bindings = parse_scalar_bindings(py, Some(&scalar_list)).unwrap();
                assert_eq!(bindings.len(), 1);

                let field = PyDict::new(py);
                field.set_item("name", "value").unwrap();
                field.set_item("type", "Int").unwrap();
                let args = PyList::empty(py);
                let arg = PyDict::new(py);
                arg.set_item("name", "value").unwrap();
                arg.set_item("type", "Int").unwrap();
                arg.set_item("default", 1).unwrap();
                args.append(arg).unwrap();
                field.set_item("args", args).unwrap();

                let type_def = PyDict::new(py);
                type_def.set_item("kind", "object").unwrap();
                type_def.set_item("name", "Query").unwrap();
                let fields = PyList::new(py, [field]).unwrap();
                type_def.set_item("fields", fields).unwrap();

                let schema = PyDict::new(py);
                schema.set_item("query", "Query").unwrap();
                let definition = PyDict::new(py);
                definition.set_item("schema", schema).unwrap();
                let types = PyList::new(py, [type_def]).unwrap();
                definition.set_item("types", types).unwrap();
                definition.set_item("scalars", PyList::empty(py)).unwrap();
                definition.set_item("enums", PyList::empty(py)).unwrap();
                definition.set_item("unions", PyList::empty(py)).unwrap();

                let (schema_def, type_defs, _, _, _) =
                    parse_schema_definition(py, &definition.into_any()).unwrap();
                assert_eq!(schema_def.query, "Query");
                assert_eq!(type_defs.len(), 1);
            });
        }

        #[test]
        fn parse_definition_with_optional_fields() {
            crate::with_py(|py| {
                let empty = parse_resolvers(py, None).unwrap();
                assert!(empty.is_empty());

                let none = extract_optional_string(Some(py.None().into_bound(py)));
                assert!(none.is_none());

                let arg = PyDict::new(py);
                arg.set_item("name", "limit").unwrap();
                arg.set_item("type", "Int").unwrap();
                arg.set_item("default", 3).unwrap();
                let args = PyList::new(py, [arg]).unwrap();

                let field = PyDict::new(py);
                field.set_item("name", "value").unwrap();
                field.set_item("type", "String").unwrap();
                field.set_item("resolver", "Query.value").unwrap();
                field.set_item("description", "field desc").unwrap();
                field.set_item("deprecation", "old").unwrap();
                field.set_item("default", "hello").unwrap();
                field.set_item("args", args).unwrap();

                let type_def = PyDict::new(py);
                type_def.set_item("kind", "object").unwrap();
                type_def.set_item("name", "Query").unwrap();
                type_def.set_item("description", "type desc").unwrap();
                let implements = PyList::new(py, ["Node"]).unwrap();
                type_def.set_item("implements", implements).unwrap();
                let fields = PyList::new(py, [field]).unwrap();
                type_def.set_item("fields", fields).unwrap();

                let scalar_def = PyDict::new(py);
                scalar_def.set_item("name", "Date").unwrap();
                scalar_def.set_item("description", "date scalar").unwrap();
                scalar_def
                    .set_item("specified_by_url", "https://example.com/date")
                    .unwrap();

                let enum_def = PyDict::new(py);
                enum_def.set_item("name", "Color").unwrap();
                enum_def.set_item("description", "colors").unwrap();
                let enum_values = PyList::new(py, ["RED", "BLUE"]).unwrap();
                enum_def.set_item("values", enum_values).unwrap();

                let union_def = PyDict::new(py);
                union_def.set_item("name", "Search").unwrap();
                union_def.set_item("description", "search").unwrap();
                let union_types = PyList::new(py, ["Query"]).unwrap();
                union_def.set_item("types", union_types).unwrap();

                let schema = PyDict::new(py);
                schema.set_item("query", "Query").unwrap();
                schema.set_item("mutation", "Mutation").unwrap();
                schema.set_item("subscription", "Subscription").unwrap();

                let definition = PyDict::new(py);
                definition.set_item("schema", schema).unwrap();
                definition
                    .set_item("types", PyList::new(py, [type_def]).unwrap())
                    .unwrap();
                definition
                    .set_item("scalars", PyList::new(py, [scalar_def]).unwrap())
                    .unwrap();
                definition
                    .set_item("enums", PyList::new(py, [enum_def]).unwrap())
                    .unwrap();
                definition
                    .set_item("unions", PyList::new(py, [union_def]).unwrap())
                    .unwrap();

                let (schema_def, type_defs, scalar_defs, enum_defs, union_defs) =
                    parse_schema_definition(py, &definition.into_any()).unwrap();
                assert_eq!(schema_def.mutation.as_deref(), Some("Mutation"));
                assert_eq!(schema_def.subscription.as_deref(), Some("Subscription"));
                assert_eq!(type_defs[0].description.as_deref(), Some("type desc"));
                assert_eq!(type_defs[0].implements, vec!["Node".to_string()]);
                assert!(type_defs[0].fields[0].default_value.is_some());
                assert!(type_defs[0].fields[0].args[0].default_value.is_some());
                assert_eq!(scalar_defs[0].description.as_deref(), Some("date scalar"));
                assert_eq!(
                    enum_defs[0].values,
                    vec!["RED".to_string(), "BLUE".to_string()]
                );
                assert_eq!(union_defs[0].types, vec!["Query".to_string()]);
            });
        }

        #[test]
        fn parse_missing_fields_report_errors() {
            crate::with_py(|py| {
                let empty = PyDict::new(py);
                let err = parse_schema_definition(py, &empty.into_any())
                    .err()
                    .unwrap();
                assert_eq!(err_message(err), "Missing schema");

                let schema = PyDict::new(py);
                schema.set_item("schema", PyDict::new(py)).unwrap();
                let err = parse_schema_definition(py, &schema.into_any())
                    .err()
                    .unwrap();
                assert_eq!(err_message(err), "Missing query");

                let schema = PyDict::new(py);
                let schema_block = PyDict::new(py);
                schema_block.set_item("query", "Query").unwrap();
                schema.set_item("schema", schema_block).unwrap();
                let err = parse_schema_definition(py, &schema.into_any())
                    .err()
                    .unwrap();
                assert_eq!(err_message(err), "Missing types");

                let type_dict = PyDict::new(py);
                let err = parse_type_def(py, &type_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing type kind");

                let type_dict = PyDict::new(py);
                type_dict.set_item("kind", "object").unwrap();
                let err = parse_type_def(py, &type_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing type name");

                let type_dict = PyDict::new(py);
                type_dict.set_item("kind", "object").unwrap();
                type_dict.set_item("name", "Query").unwrap();
                let err = parse_type_def(py, &type_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing fields");

                let enum_dict = PyDict::new(py);
                let err = parse_enum_def(&enum_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing enum name");

                let enum_dict = PyDict::new(py);
                enum_dict.set_item("name", "Color").unwrap();
                let err = parse_enum_def(&enum_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing enum values");

                let union_dict = PyDict::new(py);
                let err = parse_union_def(&union_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing union name");

                let union_dict = PyDict::new(py);
                union_dict.set_item("name", "Union").unwrap();
                let err = parse_union_def(&union_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing union types");

                let scalar_dict = PyDict::new(py);
                let err = parse_scalar_def(&scalar_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing scalar name");

                let field_dict = PyDict::new(py);
                let err = parse_field_def(py, &field_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing field name");

                let field_dict = PyDict::new(py);
                field_dict.set_item("name", "value").unwrap();
                let err = parse_field_def(py, &field_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing field type");

                let arg_dict = PyDict::new(py);
                let err = parse_arg_def(py, &arg_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing arg name");

                let arg_dict = PyDict::new(py);
                arg_dict.set_item("name", "value").unwrap();
                let err = parse_arg_def(py, &arg_dict.into_any()).err().unwrap();
                assert_eq!(err_message(err), "Missing arg type");

                let scalar_list = PyList::empty(py);
                let dict = PyDict::new(py);
                dict.set_item("python_type", py.None()).unwrap();
                dict.set_item("serialize", py.None()).unwrap();
                scalar_list.append(dict).unwrap();
                let err = parse_scalar_bindings(py, Some(&scalar_list)).err().unwrap();
                assert_eq!(err_message(err), "Missing scalar name");

                let scalar_list = PyList::empty(py);
                let dict = PyDict::new(py);
                dict.set_item("name", "Scalar").unwrap();
                dict.set_item("serialize", py.None()).unwrap();
                scalar_list.append(dict).unwrap();
                let err = parse_scalar_bindings(py, Some(&scalar_list)).err().unwrap();
                assert_eq!(err_message(err), "Missing python_type");

                let scalar_list = PyList::empty(py);
                let dict = PyDict::new(py);
                dict.set_item("name", "Scalar").unwrap();
                dict.set_item("python_type", py.None()).unwrap();
                scalar_list.append(dict).unwrap();
                let err = parse_scalar_bindings(py, Some(&scalar_list)).err().unwrap();
                assert_eq!(err_message(err), "Missing serialize");
            });
        }

        #[test]
        fn extract_optional_string_handles_none() {
            crate::with_py(|py| {
                let none = extract_optional_string(None);
                assert!(none.is_none());
                let value =
                    extract_optional_string(Some("hi".into_pyobject(py).unwrap().into_any()));
                assert_eq!(value, Some("hi".to_string()));
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
        use pyo3::types::{PyAnyMethods, PyDict, PyInt, PyStringMethods};
        use std::collections::HashMap;

        #[test]
        fn parse_type_ref_covers_list_and_non_null() {
            let ty = parse_type_ref("String!");
            match ty {
                TypeRef::NonNull(inner) => match *inner {
                    TypeRef::Named(name) => assert_eq!(name.as_ref(), "String"),
                    _ => panic!("unexpected inner"),
                },
                _ => panic!("expected non-null"),
            }

            let ty = parse_type_ref("[Int]");
            match ty {
                TypeRef::List(inner) => match *inner {
                    TypeRef::Named(name) => assert_eq!(name.as_ref(), "Int"),
                    _ => panic!("unexpected inner"),
                },
                _ => panic!("expected list"),
            }
        }

        #[test]
        fn build_schema_unknown_kind_errors() {
            let schema_def = SchemaDef {
                query: "Query".to_string(),
                mutation: None,
                subscription: None,
            };
            let type_defs = vec![TypeDef {
                kind: "mystery".to_string(),
                name: "Query".to_string(),
                fields: Vec::new(),
                description: None,
                implements: Vec::new(),
            }];
            let err = build_schema(
                schema_def,
                type_defs,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                Arc::new(Vec::new()),
            )
            .err()
            .unwrap();
            let msg =
                crate::with_py(|py| err.value(py).str().unwrap().to_str().unwrap().to_string());
            assert_eq!(msg, "Unknown type kind: mystery");
        }

        #[test]
        fn build_input_field_applies_default() {
            crate::with_py(|py| {
                let field_def = FieldDef {
                    name: "value".to_string(),
                    source: "value".to_string(),
                    type_name: "Int".to_string(),
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
                    type_name: "Int".to_string(),
                    default_value: Some(default_value.clone()),
                };

                let query_field = FieldDef {
                    name: "value".to_string(),
                    source: "value".to_string(),
                    type_name: "String".to_string(),
                    args: vec![make_arg()],
                    resolver: Some("Query.value".to_string()),
                    description: Some("field desc".to_string()),
                    deprecation: Some("old".to_string()),
                    default_value: None,
                };

                let id_field = FieldDef {
                    name: "id".to_string(),
                    source: "id".to_string(),
                    type_name: "ID!".to_string(),
                    args: Vec::new(),
                    resolver: None,
                    description: None,
                    deprecation: None,
                    default_value: None,
                };

                let interface_field = FieldDef {
                    name: "id".to_string(),
                    source: "id".to_string(),
                    type_name: "ID!".to_string(),
                    args: vec![make_arg()],
                    resolver: None,
                    description: Some("iface field".to_string()),
                    deprecation: Some("iface old".to_string()),
                    default_value: None,
                };

                let subscription_field = FieldDef {
                    name: "ticks".to_string(),
                    source: "ticks".to_string(),
                    type_name: "Int!".to_string(),
                    args: vec![make_arg()],
                    resolver: Some("Subscription.ticks".to_string()),
                    description: Some("sub field".to_string()),
                    deprecation: Some("sub old".to_string()),
                    default_value: None,
                };

                let input_field = FieldDef {
                    name: "count".to_string(),
                    source: "count".to_string(),
                    type_name: "Int".to_string(),
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
                        kind: "interface".to_string(),
                        name: "Node".to_string(),
                        fields: vec![interface_field],
                        description: Some("iface".to_string()),
                        implements: Vec::new(),
                    },
                    TypeDef {
                        kind: "object".to_string(),
                        name: "Query".to_string(),
                        fields: vec![id_field, query_field],
                        description: Some("query desc".to_string()),
                        implements: vec!["Node".to_string()],
                    },
                    TypeDef {
                        kind: "subscription".to_string(),
                        name: "Subscription".to_string(),
                        fields: vec![subscription_field],
                        description: Some("sub desc".to_string()),
                        implements: Vec::new(),
                    },
                    TypeDef {
                        kind: "input".to_string(),
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
        use pyo3::types::{PyAnyMethods, PyList, PyStringMethods};

        fn build_definition(py: Python<'_>) -> (Py<PyAny>, Py<PyDict>) {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
async def hello(parent, info):
    return "hi"

async def ticks(parent, info):
    for i in range(2):
        yield i
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();

            let resolver = locals.get_item("hello").unwrap().unwrap();
            let tick_resolver = locals.get_item("ticks").unwrap().unwrap();

            let query_field = PyDict::new(py);
            query_field.set_item("name", "hello").unwrap();
            query_field.set_item("source", "hello").unwrap();
            query_field.set_item("type", "String!").unwrap();
            query_field.set_item("resolver", "Query.hello").unwrap();

            let sub_field = PyDict::new(py);
            sub_field.set_item("name", "ticks").unwrap();
            sub_field.set_item("source", "ticks").unwrap();
            sub_field.set_item("type", "Int!").unwrap();
            sub_field
                .set_item("resolver", "Subscription.ticks")
                .unwrap();

            let query_def = PyDict::new(py);
            query_def.set_item("kind", "object").unwrap();
            query_def.set_item("name", "Query").unwrap();
            let query_fields = PyList::new(py, [query_field]).unwrap();
            query_def.set_item("fields", query_fields).unwrap();

            let subscription_def = PyDict::new(py);
            subscription_def.set_item("kind", "subscription").unwrap();
            subscription_def.set_item("name", "Subscription").unwrap();
            let subscription_fields = PyList::new(py, [sub_field]).unwrap();
            subscription_def
                .set_item("fields", subscription_fields)
                .unwrap();

            let schema = PyDict::new(py);
            schema.set_item("query", "Query").unwrap();
            schema.set_item("subscription", "Subscription").unwrap();

            let definition = PyDict::new(py);
            definition.set_item("schema", schema).unwrap();
            let types = PyList::new(py, [query_def, subscription_def]).unwrap();
            definition.set_item("types", types).unwrap();

            let resolvers = PyDict::new(py);
            resolvers.set_item("Query.hello", resolver).unwrap();
            resolvers
                .set_item("Subscription.ticks", tick_resolver)
                .unwrap();

            (definition.into_any().unbind(), resolvers.unbind())
        }

        fn build_definition_with_args(py: Python<'_>) -> (Py<PyAny>, Py<PyDict>) {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
async def greet(parent, info, name: str):
    return f"{info['root']['prefix']}{name}{info['context']['suffix']}"

async def ticks(parent, info, limit: int):
    for i in range(limit):
        yield i
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();

            let greet_resolver = locals.get_item("greet").unwrap().unwrap();
            let tick_resolver = locals.get_item("ticks").unwrap().unwrap();

            let arg_name = PyDict::new(py);
            arg_name.set_item("name", "name").unwrap();
            arg_name.set_item("type", "String!").unwrap();
            let query_args = PyList::new(py, [arg_name]).unwrap();

            let query_field = PyDict::new(py);
            query_field.set_item("name", "greet").unwrap();
            query_field.set_item("source", "greet").unwrap();
            query_field.set_item("type", "String!").unwrap();
            query_field.set_item("resolver", "Query.greet").unwrap();
            query_field.set_item("args", query_args).unwrap();

            let arg_limit = PyDict::new(py);
            arg_limit.set_item("name", "limit").unwrap();
            arg_limit.set_item("type", "Int!").unwrap();
            let sub_args = PyList::new(py, [arg_limit]).unwrap();

            let sub_field = PyDict::new(py);
            sub_field.set_item("name", "ticks").unwrap();
            sub_field.set_item("source", "ticks").unwrap();
            sub_field.set_item("type", "Int!").unwrap();
            sub_field
                .set_item("resolver", "Subscription.ticks")
                .unwrap();
            sub_field.set_item("args", sub_args).unwrap();

            let query_def = PyDict::new(py);
            query_def.set_item("kind", "object").unwrap();
            query_def.set_item("name", "Query").unwrap();
            let query_fields = PyList::new(py, [query_field]).unwrap();
            query_def.set_item("fields", query_fields).unwrap();

            let subscription_def = PyDict::new(py);
            subscription_def.set_item("kind", "subscription").unwrap();
            subscription_def.set_item("name", "Subscription").unwrap();
            let subscription_fields = PyList::new(py, [sub_field]).unwrap();
            subscription_def
                .set_item("fields", subscription_fields)
                .unwrap();

            let schema = PyDict::new(py);
            schema.set_item("query", "Query").unwrap();
            schema.set_item("subscription", "Subscription").unwrap();

            let definition = PyDict::new(py);
            definition.set_item("schema", schema).unwrap();
            let types = PyList::new(py, [query_def, subscription_def]).unwrap();
            definition.set_item("types", types).unwrap();

            let resolvers = PyDict::new(py);
            resolvers.set_item("Query.greet", greet_resolver).unwrap();
            resolvers
                .set_item("Subscription.ticks", tick_resolver)
                .unwrap();

            (definition.into_any().unbind(), resolvers.unbind())
        }

        fn build_subscription_definition(
            py: Python<'_>,
            query_resolver: &Bound<'_, PyAny>,
            subscription_resolver: &Bound<'_, PyAny>,
            field_type: &str,
        ) -> (Py<PyAny>, Py<PyDict>) {
            let query_field = PyDict::new(py);
            query_field.set_item("name", "noop").unwrap();
            query_field.set_item("source", "noop").unwrap();
            query_field.set_item("type", "Int!").unwrap();
            query_field.set_item("resolver", "Query.noop").unwrap();

            let sub_field = PyDict::new(py);
            sub_field.set_item("name", "tick").unwrap();
            sub_field.set_item("source", "tick").unwrap();
            sub_field.set_item("type", field_type).unwrap();
            sub_field.set_item("resolver", "Subscription.tick").unwrap();

            let query_def = PyDict::new(py);
            query_def.set_item("kind", "object").unwrap();
            query_def.set_item("name", "Query").unwrap();
            let query_fields = PyList::new(py, [query_field]).unwrap();
            query_def.set_item("fields", query_fields).unwrap();

            let subscription_def = PyDict::new(py);
            subscription_def.set_item("kind", "subscription").unwrap();
            subscription_def.set_item("name", "Subscription").unwrap();
            let subscription_fields = PyList::new(py, [sub_field]).unwrap();
            subscription_def
                .set_item("fields", subscription_fields)
                .unwrap();

            let schema = PyDict::new(py);
            schema.set_item("query", "Query").unwrap();
            schema.set_item("subscription", "Subscription").unwrap();

            let definition = PyDict::new(py);
            definition.set_item("schema", schema).unwrap();
            let types = PyList::new(py, [query_def, subscription_def]).unwrap();
            definition.set_item("types", types).unwrap();

            let resolvers = PyDict::new(py);
            resolvers.set_item("Query.noop", query_resolver).unwrap();
            resolvers
                .set_item("Subscription.tick", subscription_resolver)
                .unwrap();

            (definition.into_any().unbind(), resolvers.unbind())
        }

        fn assert_response_has_errors(response: &Bound<'_, PyAny>) {
            if response.is_none() {
                return;
            }
            let dict = response.cast::<PyDict>().unwrap();
            let errors = dict.get_item("errors").unwrap().unwrap();
            assert!(!errors.cast::<PyList>().unwrap().is_empty());
        }

        #[test]
        fn schema_wrapper_executes_and_streams() {
            let (schema, resolvers) = crate::with_py(|py| build_definition(py));
            let (query_result, sub_result) = crate::with_py(|py| {
                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(py, &schema.bind(py), Some(&resolvers.bind(py)), None)
                    })?;

                    let awaitable = Python::attach(|py| {
                        wrapper
                            .execute(py, "{ hello }".to_string(), None, None, None)
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let query_result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
                    })?
                    .await?;

                    let stream = Python::attach(|py| {
                        wrapper.subscribe(
                            py,
                            "subscription { ticks }".to_string(),
                            None,
                            None,
                            None,
                        )
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let sub_result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;

                    let close =
                        Python::attach(|py| stream.aclose(py).map(|awaitable| awaitable.unbind()))?;
                    let _ = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(close.into_bound(py))
                    })?
                    .await?;

                    Ok((query_result, sub_result))
                })
            })
            .unwrap();

            crate::with_py(|py| {
                let dict = query_result.bind(py).cast::<PyDict>().unwrap();
                let data_any = dict.get_item("data").unwrap().unwrap();
                let data = data_any.cast::<PyDict>().unwrap();
                assert_eq!(
                    data.get_item("hello")
                        .unwrap()
                        .unwrap()
                        .extract::<String>()
                        .unwrap(),
                    "hi"
                );
            });

            crate::with_py(|py| {
                let dict = sub_result.bind(py).cast::<PyDict>().unwrap();
                let data_any = dict.get_item("data").unwrap().unwrap();
                let data = data_any.cast::<PyDict>().unwrap();
                assert_eq!(
                    data.get_item("ticks")
                        .unwrap()
                        .unwrap()
                        .extract::<i64>()
                        .unwrap(),
                    0
                );
            });
        }

        #[test]
        fn schema_wrapper_sdl_and_executes_with_variables() {
            let (schema, resolvers) = crate::with_py(|py| build_definition_with_args(py));
            let (query_result, sub_result) = crate::with_py(|py| {
                let query_vars = PyDict::new(py);
                query_vars.set_item("name", "Ada").unwrap();
                let query_vars = query_vars.into_any().unbind();

                let sub_vars = PyDict::new(py);
                sub_vars.set_item("limit", 2).unwrap();
                let sub_vars = sub_vars.into_any().unbind();

                let root_query = PyDict::new(py);
                root_query.set_item("prefix", "hi ").unwrap();
                let root_query = root_query.into_any().unbind();

                let root_sub = PyDict::new(py);
                root_sub.set_item("prefix", "hi ").unwrap();
                let root_sub = root_sub.into_any().unbind();

                let context_query = PyDict::new(py);
                context_query.set_item("suffix", "!").unwrap();
                let context_query = context_query.into_any().unbind();

                let context_sub = PyDict::new(py);
                context_sub.set_item("suffix", "!").unwrap();
                let context_sub = context_sub.into_any().unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(py, &schema.bind(py), Some(&resolvers.bind(py)), None)
                    })?;
                    let sdl = wrapper.sdl()?;
                    assert!(sdl.contains("schema"));

                    let awaitable = Python::attach(|py| {
                        wrapper
                            .execute(
                                py,
                                "query($name: String!) { greet(name: $name) }".to_string(),
                                Some(query_vars),
                                Some(root_query),
                                Some(context_query),
                            )
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let query_result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
                    })?
                    .await?;

                    let stream = Python::attach(|py| {
                        wrapper.subscribe(
                            py,
                            "subscription($limit: Int!) { ticks(limit: $limit) }".to_string(),
                            Some(sub_vars),
                            Some(root_sub),
                            Some(context_sub),
                        )
                    })?;

                    let next = Python::attach(|py| -> PyResult<Py<PyAny>> {
                        Ok(stream.__anext__(py)?.expect("expected awaitable").unbind())
                    })?;
                    let sub_result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;

                    Ok((query_result, sub_result))
                })
            })
            .unwrap();

            crate::with_py(|py| {
                let dict = query_result.bind(py).cast::<PyDict>().unwrap();
                let data_any = dict.get_item("data").unwrap().unwrap();
                let data = data_any.cast::<PyDict>().unwrap();
                assert_eq!(
                    data.get_item("greet")
                        .unwrap()
                        .unwrap()
                        .extract::<String>()
                        .unwrap(),
                    "hi Ada!"
                );
            });

            crate::with_py(|py| {
                let dict = sub_result.bind(py).cast::<PyDict>().unwrap();
                let data_any = dict.get_item("data").unwrap().unwrap();
                let data = data_any.cast::<PyDict>().unwrap();
                assert_eq!(
                    data.get_item("ticks")
                        .unwrap()
                        .unwrap()
                        .extract::<i64>()
                        .unwrap(),
                    0
                );
            });
        }

        #[test]
        fn subscription_stream_closed_returns_none() {
            let (schema, resolvers) = crate::with_py(|py| build_definition(py));
            crate::with_py(|py| {
                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(py, &schema.bind(py), Some(&resolvers.bind(py)), None)
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(
                            py,
                            "subscription { ticks }".to_string(),
                            None,
                            None,
                            None,
                        )
                    })?;
                    let close =
                        Python::attach(|py| stream.aclose(py).map(|awaitable| awaitable.unbind()))?;
                    Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(close.into_bound(py))
                    })?
                    .await?;

                    let next = Python::attach(|py| -> PyResult<Option<Py<PyAny>>> {
                        Ok(stream.__anext__(py)?.map(|awaitable| awaitable.unbind()))
                    })?;
                    assert!(next.is_none());
                    Ok(())
                })
            })
            .unwrap();
        }

        #[test]
        fn subscription_stream_aiter_returns_self() {
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;
            use tokio::sync::Mutex;

            crate::with_py(|py| {
                let stream = SubscriptionStream {
                    stream: Arc::new(Mutex::new(None)),
                    closed: Arc::new(AtomicBool::new(false)),
                };
                let py_stream = Py::new(py, stream).unwrap();
                {
                    let aiter = SubscriptionStream::__aiter__(py_stream.borrow(py));
                    aiter.closed.store(true, Ordering::SeqCst);
                }
                let py_ref = py_stream.borrow(py);
                assert!(py_ref.closed.load(Ordering::SeqCst));
            });
        }

        #[test]
        fn subscription_stream_close_after_next_yields_stop() {
            use pyo3::exceptions::PyStopAsyncIteration;

            let (schema, resolvers) = crate::with_py(|py| build_definition(py));
            crate::with_py(|py| {
                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(py, &schema.bind(py), Some(&resolvers.bind(py)), None)
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(
                            py,
                            "subscription { ticks }".to_string(),
                            None,
                            None,
                            None,
                        )
                    })?;

                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let close =
                        Python::attach(|py| stream.aclose(py).map(|awaitable| awaitable.unbind()))?;
                    Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(close.into_bound(py))
                    })?
                    .await?;

                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await;
                    match result {
                        Ok(_) => panic!("expected stop async iteration"),
                        Err(err) => {
                            let is_stop =
                                Python::attach(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                            assert!(is_stop);
                        }
                    }

                    Ok(())
                })
            })
            .unwrap();
        }

        #[test]
        fn subscription_stream_handles_empty_and_missing_stream() {
            use async_graphql::futures_util::stream;
            use async_graphql::futures_util::StreamExt;
            use std::sync::atomic::AtomicBool;
            use std::sync::Arc;
            use tokio::sync::Mutex;

            crate::with_py(|py| {
                pyo3_async_runtimes::tokio::run(py, async move {
                    let missing = SubscriptionStream {
                        stream: Arc::new(Mutex::new(None)),
                        closed: Arc::new(AtomicBool::new(false)),
                    };
                    let next =
                        Python::attach(|py| missing.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await;
                    assert!(result.is_err());

                    let empty_stream = stream::empty::<async_graphql::Response>().boxed();
                    let empty = SubscriptionStream {
                        stream: Arc::new(Mutex::new(Some(empty_stream))),
                        closed: Arc::new(AtomicBool::new(false)),
                    };
                    let next = Python::attach(|py| empty.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await;
                    assert!(result.is_err());
                    Ok(())
                })
            })
            .unwrap();
        }

        #[test]
        fn schema_wrapper_resolves_from_parent_and_requires_root() {
            crate::with_py(|py| {
                let query_field = PyDict::new(py);
                query_field.set_item("name", "value").unwrap();
                query_field.set_item("source", "value").unwrap();
                query_field.set_item("type", "Int!").unwrap();

                let query_def = PyDict::new(py);
                query_def.set_item("kind", "object").unwrap();
                query_def.set_item("name", "Query").unwrap();
                let query_fields = PyList::new(py, [query_field]).unwrap();
                query_def.set_item("fields", query_fields).unwrap();

                let schema = PyDict::new(py);
                schema.set_item("query", "Query").unwrap();

                let definition = PyDict::new(py);
                definition.set_item("schema", schema).unwrap();
                let types = PyList::new(py, [query_def]).unwrap();
                definition.set_item("types", types).unwrap();

                let resolvers = PyDict::new(py);

                let root = PyDict::new(py);
                root.set_item("value", 5).unwrap();
                let root = root.into_any().unbind();

                let definition = definition.into_any().unbind();
                let resolvers = resolvers.unbind();

                pyo3_async_runtimes::tokio::run(py, async move {
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;

                    let awaitable = Python::attach(|py| {
                        wrapper
                            .execute(py, "{ value }".to_string(), None, Some(root), None)
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let with_root = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| {
                        let dict = with_root.bind(py).cast::<PyDict>().unwrap();
                        let data = dict.get_item("data").unwrap().unwrap();
                        let data = data.cast::<PyDict>().unwrap();
                        assert_eq!(
                            data.get_item("value")
                                .unwrap()
                                .unwrap()
                                .extract::<i64>()
                                .unwrap(),
                            5
                        );
                    });

                    let awaitable = Python::attach(|py| {
                        wrapper
                            .execute(py, "{ value }".to_string(), None, None, None)
                            .map(|awaitable| awaitable.unbind())
                    })?;
                    let without_root = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| {
                        assert_response_has_errors(without_root.bind(py));
                    });

                    Ok(())
                })
            })
            .unwrap();
        }

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
                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_only_anext.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;
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
                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_not_async.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| {
                        assert_response_has_errors(result.bind(py));
                    });
                    Ok(())
                })
            })
            .unwrap();
        }

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
                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_raise.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let _ = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await;

                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_non_awaitable.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));

                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_stop.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await;
                    if let Err(err) = result {
                        let is_stop =
                            Python::attach(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                        assert!(is_stop);
                    } else {
                        panic!("expected stop async iteration");
                    }

                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_error.bind(py),
                            "Int!",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));

                    let (definition, resolvers) = Python::attach(|py| {
                        build_subscription_definition(
                            py,
                            &noop.bind(py),
                            &sub_wrong_type.bind(py),
                            "[Int]",
                        )
                    });
                    let wrapper = Python::attach(|py| {
                        SchemaWrapper::new(
                            py,
                            &definition.bind(py),
                            Some(&resolvers.bind(py)),
                            None,
                        )
                    })?;
                    let stream = Python::attach(|py| {
                        wrapper.subscribe(py, "subscription { tick }".to_string(), None, None, None)
                    })?;
                    let next = Python::attach(|py| stream.__anext__(py).unwrap().unwrap().unbind());
                    let result = Python::attach(|py| {
                        pyo3_async_runtimes::tokio::into_future(next.into_bound(py))
                    })?
                    .await?;
                    Python::attach(|py| assert_response_has_errors(result.bind(py)));

                    Ok(())
                })
            })
            .unwrap();
        }

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

#[test]
fn end_to_end_build_and_execute() {
    let (schema_tuple, scalar_bindings) = crate::with_py(|py| {
        let locals = PyDict::new(py);
        py.run(
            pyo3::ffi::c_str!(
                r#"
class ScalarType:
    def __init__(self, value):
        self.value = value

def serialize(value):
    return value.value

def parse_value(value):
    return ScalarType(value)

async def greet(parent, info, name: str):
    return f"hi {name}"
"#
            ),
            None,
            Some(&locals),
        )
        .unwrap();

        let scalar_type = locals.get_item("ScalarType").unwrap().unwrap();
        let serialize = locals.get_item("serialize").unwrap().unwrap();
        let _parse_value = locals.get_item("parse_value").unwrap().unwrap();
        let scalar_binding = PyDict::new(py);
        scalar_binding.set_item("name", "ScalarType").unwrap();
        scalar_binding.set_item("python_type", scalar_type).unwrap();
        scalar_binding.set_item("serialize", serialize).unwrap();
        let scalars = PyList::new(py, [scalar_binding]).unwrap();

        let resolver = locals.get_item("greet").unwrap().unwrap();
        let resolvers = PyDict::new(py);
        resolvers.set_item("Query.greet", resolver).unwrap();

        let arg = PyDict::new(py);
        arg.set_item("name", "name").unwrap();
        arg.set_item("type", "String!").unwrap();

        let field = PyDict::new(py);
        field.set_item("name", "greet").unwrap();
        field.set_item("source", "greet").unwrap();
        field.set_item("type", "String!").unwrap();
        field.set_item("resolver", "Query.greet").unwrap();
        let args = PyList::new(py, [arg]).unwrap();
        field.set_item("args", args).unwrap();

        let type_def = PyDict::new(py);
        type_def.set_item("kind", "object").unwrap();
        type_def.set_item("name", "Query").unwrap();
        let fields = PyList::new(py, [field]).unwrap();
        type_def.set_item("fields", fields).unwrap();

        let schema = PyDict::new(py);
        schema.set_item("query", "Query").unwrap();

        let definition = PyDict::new(py);
        definition.set_item("schema", schema).unwrap();
        let types = PyList::new(py, [type_def]).unwrap();
        definition.set_item("types", types).unwrap();
        definition.set_item("scalars", PyList::empty(py)).unwrap();
        definition.set_item("enums", PyList::empty(py)).unwrap();
        definition.set_item("unions", PyList::empty(py)).unwrap();

        let (schema_def, type_defs, scalar_defs, enum_defs, union_defs) =
            crate::parse::parse_schema_definition(py, &definition.into_any()).unwrap();
        let resolver_map = crate::parse::parse_resolvers(py, Some(&resolvers)).unwrap();
        let scalar_bindings = crate::parse::parse_scalar_bindings(py, Some(&scalars)).unwrap();
        (
            (
                schema_def,
                type_defs,
                scalar_defs,
                enum_defs,
                union_defs,
                resolver_map,
            ),
            scalar_bindings,
        )
    });

    let (schema_def, type_defs, scalar_defs, enum_defs, union_defs, resolver_map) = schema_tuple;
    let schema: DynamicSchema = crate::build::build_schema(
        schema_def,
        type_defs,
        scalar_defs,
        enum_defs,
        union_defs,
        resolver_map,
        Arc::new(scalar_bindings),
    )
    .unwrap();

    let response = crate::with_py(|py| {
        pyo3_async_runtimes::tokio::run(py, async move {
            Ok(schema
                .execute(async_graphql::Request::new("{ greet(name: \"Bob\") }"))
                .await)
        })
    })
    .unwrap();
    assert!(response.errors.is_empty());
    assert_eq!(
        response.data,
        Value::Object(IndexMap::from([(
            Name::new("greet"),
            Value::String("hi Bob".to_string()),
        )]))
    );
}
