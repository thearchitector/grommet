#![forbid(unsafe_code)]

mod api;
mod errors;
mod resolver;
mod schema_types;
mod types;
mod values;

use pyo3::prelude::*;

use crate::api::{SchemaWrapper, SubscriptionStream};
use crate::values::OperationResult;

// pyo3 module entrypoint for the python extension
#[pymodule(gil_used = false)]
#[doc(hidden)]
pub fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SchemaWrapper>()?;
    module.add_class::<SubscriptionStream>()?;
    module.add_class::<OperationResult>()?;
    Ok(())
}
