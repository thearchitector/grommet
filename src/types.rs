use std::sync::Arc;

use async_graphql::dynamic::TypeRef;
use pyo3::prelude::*;

#[derive(Clone)]
pub(crate) struct PyObj {
    inner: Arc<Py<PyAny>>,
}

impl PyObj {
    pub(crate) fn new(inner: Py<PyAny>) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub(crate) fn bind<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.inner.clone_ref(py).into_bound(py)
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Py<PyAny> {
        self.inner.clone_ref(py)
    }
}

#[derive(Clone)]
pub(crate) struct ContextValue(pub(crate) PyObj);

#[derive(Clone)]
pub(crate) struct ResolverEntry {
    pub(crate) func: PyObj,
    pub(crate) needs_context: bool,
    pub(crate) is_async_gen: bool,
}

#[derive(Clone)]
pub(crate) struct FieldContext {
    pub(crate) resolver: Option<ResolverEntry>,
    pub(crate) output_type: TypeRef,
}
