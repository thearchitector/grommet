#![forbid(unsafe_code)]

mod api;
mod errors;
mod lookahead;
mod resolver;
mod schema_types;
mod types;
mod values;

use pyo3::prelude::*;

use crate::api::{SchemaWrapper, SubscriptionStream};
use crate::lookahead::Graph;
use crate::schema_types::{
    PyField, PyInputObject, PyInputValue, PyObject, PySubscription, PySubscriptionField,
};
use crate::values::OperationResult;

// pyo3 module entrypoint for the python extension
#[pymodule(gil_used = true)]
#[doc(hidden)]
pub fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SchemaWrapper>()?;
    module.add_class::<SubscriptionStream>()?;
    module.add_class::<OperationResult>()?;
    module.add_class::<Graph>()?;
    module.add_class::<PyField>()?;
    module.add_class::<PySubscriptionField>()?;
    module.add_class::<PyInputValue>()?;
    module.add_class::<PyObject>()?;
    module.add_class::<PyInputObject>()?;
    module.add_class::<PySubscription>()?;
    Ok(())
}
