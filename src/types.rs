use pyo3::prelude::*;

// shared rust wrappers for python-owned values
pub(crate) struct PyObj {
    pub(crate) inner: Py<PyAny>,
}

impl Clone for PyObj {
    fn clone(&self) -> Self {
        Python::with_gil(|py| Self {
            inner: self.inner.clone_ref(py),
        })
    }
}

unsafe impl Send for PyObj {}
unsafe impl Sync for PyObj {}

#[derive(Clone)]
pub(crate) struct RootValue(pub(crate) PyObj);

unsafe impl Send for RootValue {}
unsafe impl Sync for RootValue {}

#[derive(Clone)]
pub(crate) struct ContextValue(pub(crate) PyObj);

unsafe impl Send for ContextValue {}
unsafe impl Sync for ContextValue {}

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
