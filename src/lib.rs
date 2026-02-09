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

use crate::api::{SchemaWrapper, SubscriptionStream};
use crate::lookahead::Lookahead;
use crate::values::OperationResult;

// pyo3 module entrypoint for the python extension
#[pymodule(gil_used = true)]
#[doc(hidden)]
pub fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialize the Tokio runtime with multi-thread scheduler and all drivers enabled.
    // Must be called before any future_into_py invocation. Uses a OnceCell internally,
    // so repeated calls are safe (only the first takes effect).
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.thread_keep_alive(tokio::time::Duration::from_secs(60));
    builder.thread_stack_size(4 * 1024 * 1024);
    pyo3_async_runtimes::tokio::init(builder);

    module.add_class::<SchemaWrapper>()?;
    module.add_class::<SubscriptionStream>()?;
    module.add_class::<OperationResult>()?;
    module.add_class::<Lookahead>()?;
    Ok(())
}
