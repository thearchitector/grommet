use async_graphql::Error;
use pyo3::PyErr;
use pyo3::exceptions::{PyTypeError, PyValueError};

pub(crate) fn py_err_to_error(err: PyErr) -> Error {
    Error::new(err.to_string())
}

pub(crate) fn py_type_error(message: impl Into<String>) -> PyErr {
    PyErr::new::<PyTypeError, _>(message.into())
}

pub(crate) fn py_value_error(message: impl Into<String>) -> PyErr {
    PyErr::new::<PyValueError, _>(message.into())
}

#[allow(dead_code)]
pub(crate) fn no_parent_value() -> Error {
    Error::new("No parent value for field")
}

pub(crate) fn subscription_requires_async_iterator() -> PyErr {
    py_type_error("Subscription resolver must return an async iterator")
}

pub(crate) fn expected_list_value() -> PyErr {
    py_type_error("Expected list for GraphQL list type")
}

pub(crate) fn unsupported_value_type() -> PyErr {
    py_type_error("Unsupported value type")
}
