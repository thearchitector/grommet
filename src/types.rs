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
pub(crate) struct RootValue(pub(crate) PyObj);

#[derive(Clone)]
pub(crate) struct ContextValue(pub(crate) PyObj);

pub(crate) struct SchemaDef {
    pub(crate) query: String,
    pub(crate) mutation: Option<String>,
    pub(crate) subscription: Option<String>,
}

pub(crate) struct ScalarDef {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) specified_by_url: Option<String>,
}

pub(crate) struct EnumDef {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) values: Vec<String>,
}

pub(crate) struct UnionDef {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) types: Vec<String>,
}

pub(crate) struct ArgDef {
    pub(crate) name: String,
    pub(crate) type_ref: TypeRef,
    pub(crate) default_value: Option<PyObj>,
}

pub(crate) struct FieldDef {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) type_ref: TypeRef,
    pub(crate) args: Vec<ArgDef>,
    pub(crate) resolver: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) deprecation: Option<String>,
    pub(crate) default_value: Option<PyObj>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeKind {
    Object,
    Interface,
    Subscription,
    Input,
}

impl TypeKind {
    pub(crate) fn from_str(s: &str) -> PyResult<Self> {
        match s {
            "object" => Ok(TypeKind::Object),
            "interface" => Ok(TypeKind::Interface),
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
    pub(crate) implements: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ScalarBinding {
    pub(crate) _name: String,
    pub(crate) py_type: PyObj,
    pub(crate) serialize: PyObj,
}

use std::collections::HashSet;

use async_graphql::dynamic::TypeRef;

#[derive(Clone)]
pub(crate) struct FieldContext {
    pub(crate) resolver: Option<PyObj>,
    pub(crate) arg_names: Vec<String>,
    pub(crate) field_name: String,
    pub(crate) source_name: String,
    pub(crate) output_type: TypeRef,
    pub(crate) scalar_bindings: Arc<Vec<ScalarBinding>>,
    pub(crate) abstract_types: Arc<HashSet<String>>,
}
