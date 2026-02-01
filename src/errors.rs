use async_graphql::Error;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::PyErr;

pub(crate) fn py_err_to_error(err: PyErr) -> Error {
    Error::new(err.to_string())
}

pub(crate) fn py_type_error(message: impl Into<String>) -> PyErr {
    PyErr::new::<PyTypeError, _>(message.into())
}

pub(crate) fn py_value_error(message: impl Into<String>) -> PyErr {
    PyErr::new::<PyValueError, _>(message.into())
}

pub(crate) fn missing_field(name: &str) -> PyErr {
    py_value_error(format!("Missing {name}"))
}

pub(crate) fn unknown_type_kind(kind: &str) -> PyErr {
    py_value_error(format!("Unknown type kind: {kind}"))
}

pub(crate) fn no_parent_value() -> Error {
    Error::new("No parent value for field")
}

pub(crate) fn subscription_requires_async_iterator() -> PyErr {
    py_type_error("Subscription resolver must return an async iterator")
}

pub(crate) fn expected_list_value() -> PyErr {
    py_type_error("Expected list for GraphQL list type")
}

pub(crate) fn abstract_type_requires_object() -> PyErr {
    py_type_error("Abstract types must return @grommet.type objects")
}

pub(crate) fn unsupported_value_type() -> PyErr {
    py_type_error("Unsupported value type")
}

pub(crate) fn runtime_threads_conflict() -> PyErr {
    py_value_error("worker_threads cannot be set for a current-thread runtime")
}
