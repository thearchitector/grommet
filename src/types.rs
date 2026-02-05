use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

// shared rust wrappers for python-owned values; Arc<Mutex<_>> keeps Send+Sync without unsafe
#[derive(Clone)]
pub(crate) struct PyObj {
    inner: Arc<Mutex<Py<PyAny>>>,
}

impl PyObj {
    pub(crate) fn new(inner: Py<PyAny>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub(crate) fn bind<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        let guard = self
            .inner
            .lock()
            .expect("PyObj mutex poisoned while binding");
        guard.clone_ref(py).into_bound(py)
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Py<PyAny> {
        let guard = self
            .inner
            .lock()
            .expect("PyObj mutex poisoned while cloning");
        guard.clone_ref(py)
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
    pub(crate) type_name: String,
    pub(crate) default_value: Option<PyObj>,
}

pub(crate) struct FieldDef {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) type_name: String,
    pub(crate) args: Vec<ArgDef>,
    pub(crate) resolver: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) deprecation: Option<String>,
    pub(crate) default_value: Option<PyObj>,
}

pub(crate) struct TypeDef {
    pub(crate) kind: String,
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
