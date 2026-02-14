use std::sync::Arc;

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
pub(crate) struct StateValue(pub(crate) PyObj);

use async_graphql::dynamic::TypeRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolverShape {
    SelfOnly,
    SelfAndContext,
    SelfAndArgs,
    SelfContextAndArgs,
}

impl ResolverShape {
    pub(crate) fn from_str(s: &str) -> PyResult<Self> {
        match s {
            "self_only" => Ok(ResolverShape::SelfOnly),
            "self_and_context" => Ok(ResolverShape::SelfAndContext),
            "self_and_args" => Ok(ResolverShape::SelfAndArgs),
            "self_context_and_args" => Ok(ResolverShape::SelfContextAndArgs),
            _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unknown resolver shape: {s}"
            ))),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ResolverEntry {
    pub(crate) func: PyObj,
    pub(crate) shape: ResolverShape,
    pub(crate) arg_names: Vec<String>,
    pub(crate) is_async_gen: bool,
}

#[derive(Clone)]
pub(crate) struct FieldContext {
    pub(crate) resolver: Option<ResolverEntry>,
    pub(crate) output_type: TypeRef,
    pub(crate) context_cls: Option<PyObj>,
}
