#![forbid(unsafe_code)]

mod api;
mod build;
mod errors;
mod parse;
mod resolver;
mod types;
mod values;

use pyo3::prelude::*;

use crate::api::{configure_runtime, SchemaWrapper, SubscriptionStream};

// pyo3 module entrypoint for the python extension
#[pymodule]
#[doc(hidden)]
pub fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SchemaWrapper>()?;
    module.add_class::<SubscriptionStream>()?;
    module.add_function(wrap_pyfunction!(configure_runtime, module)?)?;
    Ok(())
}
