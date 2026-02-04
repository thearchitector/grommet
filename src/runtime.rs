use std::future::Future;

use async_graphql::Error;
use pyo3::prelude::*;

use crate::errors::{py_err_to_error, py_type_error};

pub(crate) fn future_into_py<F, T>(py: Python<'_>, fut: F) -> PyResult<Bound<'_, PyAny>>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: for<'py> IntoPyObject<'py> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py(py, fut)
}

pub(crate) fn into_future(
    awaitable: Py<PyAny>,
) -> PyResult<impl Future<Output = PyResult<Py<PyAny>>> + Send + 'static> {
    Python::attach(|py| {
        let bound = awaitable.into_bound(py);
        if !bound.hasattr("__await__")? {
            return Err(py_type_error("Expected awaitable"));
        }
        pyo3_async_runtimes::tokio::into_future(bound)
    })
}

pub(crate) async fn await_awaitable(awaitable: Py<PyAny>) -> Result<Py<PyAny>, Error> {
    let future = into_future(awaitable).map_err(py_err_to_error)?;
    let awaited = future.await.map_err(py_err_to_error)?;
    Ok(awaited)
}
