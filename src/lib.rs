#![forbid(unsafe_code)]

mod api;
mod build;
mod errors;
mod lookahead;
mod parse;
mod resolver;
mod runtime;
mod types;
mod values;

use pyo3::prelude::*;

use crate::api::{SchemaWrapper, SubscriptionStream, configure_runtime};
use crate::lookahead::Lookahead;
use crate::values::OperationResult;

// pyo3 module entrypoint for the python extension
#[pymodule(gil_used = true)]
#[doc(hidden)]
pub fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SchemaWrapper>()?;
    module.add_class::<SubscriptionStream>()?;
    module.add_class::<OperationResult>()?;
    module.add_class::<Lookahead>()?;
    module.add_function(wrap_pyfunction!(configure_runtime, module)?)?;
    Ok(())
}
