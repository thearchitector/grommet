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

pub(crate) struct SchemaDef {
    pub(crate) query: String,
    pub(crate) mutation: Option<String>,
    pub(crate) subscription: Option<String>,
}

pub(crate) struct ArgDef {
    pub(crate) name: String,
    pub(crate) type_ref: TypeRef,
    pub(crate) default_value: Option<PyObj>,
}

pub(crate) struct FieldDef {
    pub(crate) name: String,
    pub(crate) type_ref: TypeRef,
    pub(crate) args: Vec<ArgDef>,
    pub(crate) resolver: Option<ResolverEntry>,
    pub(crate) description: Option<String>,
    pub(crate) default_value: Option<PyObj>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeKind {
    Object,
    Subscription,
    Input,
}

impl TypeKind {
    pub(crate) fn from_str(s: &str) -> PyResult<Self> {
        match s {
            "object" => Ok(TypeKind::Object),
            "subscription" => Ok(TypeKind::Subscription),
            "input" => Ok(TypeKind::Input),
            _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unknown type kind: {s}"
            ))),
        }
    }
}

pub(crate) struct TypeDef {
    pub(crate) kind: TypeKind,
    pub(crate) name: String,
    pub(crate) fields: Vec<FieldDef>,
    pub(crate) description: Option<String>,
}

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
    pub(crate) is_async: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarHint {
    String,
    Int,
    Float,
    Boolean,
    ID,
    Object,
    Unknown,
}

#[derive(Clone)]
pub(crate) struct FieldContext {
    pub(crate) resolver: Option<ResolverEntry>,
    pub(crate) output_type: TypeRef,
    pub(crate) context_cls: Option<PyObj>,
    pub(crate) scalar_hint: ScalarHint,
}
