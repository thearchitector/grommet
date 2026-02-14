use std::sync::Arc;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputObject, InputValue, Object, Schema, SchemaBuilder,
    Subscription, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::futures_util::stream;
use pyo3::prelude::*;

use crate::errors::py_value_error;
use crate::resolver::{resolve_field, resolve_field_sync_fast, resolve_subscription_stream};
use crate::types::{FieldContext, PyObj, ResolverEntry, ResolverShape};
use crate::values::pyobj_to_value;

// ---------------------------------------------------------------------------
// TypeRef construction from Python TypeSpec dataclass
// ---------------------------------------------------------------------------

pub(crate) fn type_spec_to_type_ref(spec: &Bound<'_, PyAny>) -> PyResult<TypeRef> {
    let kind: String = spec.getattr("kind")?.extract()?;
    let nullable: bool = spec.getattr("nullable")?.extract()?;
    let ty = if kind == "list" {
        let inner = spec.getattr("of_type")?;
        TypeRef::List(Box::new(type_spec_to_type_ref(&inner)?))
    } else {
        let name: String = spec.getattr("name")?.extract()?;
        TypeRef::named(name)
    };
    Ok(if nullable {
        ty
    } else {
        TypeRef::NonNull(Box::new(ty))
    })
}

// ---------------------------------------------------------------------------
// Lazy context class resolution (cached per-process via OnceLock)
// ---------------------------------------------------------------------------

fn resolve_context_cls() -> PyResult<PyObj> {
    Python::attach(|py| {
        let cls = py.import("grommet.context")?.getattr("Context")?;
        Ok(PyObj::new(cls.unbind()))
    })
}

// ---------------------------------------------------------------------------
// InputValue construction helper (shared by Field and SubscriptionField args)
// ---------------------------------------------------------------------------

fn build_input_value(
    name: String,
    type_spec: &Bound<'_, PyAny>,
    default_value: Option<&Bound<'_, PyAny>>,
) -> PyResult<InputValue> {
    let type_ref = type_spec_to_type_ref(type_spec)?;
    let mut iv = InputValue::new(name, type_ref);
    if let Some(dv) = default_value {
        let py_obj = PyObj::new(dv.clone().unbind());
        iv = iv.default_value(pyobj_to_value(&py_obj)?);
    }
    Ok(iv)
}

// ---------------------------------------------------------------------------
// PyField — wraps async_graphql::dynamic::Field
// ---------------------------------------------------------------------------

#[pyclass(module = "grommet._core", name = "Field")]
pub(crate) struct PyField {
    inner: Field,
}

#[pymethods]
impl PyField {
    #[new]
    #[pyo3(signature = (name, type_spec, func, shape, arg_names, is_async, description=None, args=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: String,
        type_spec: &Bound<'_, PyAny>,
        func: Py<PyAny>,
        shape: &str,
        arg_names: Vec<String>,
        is_async: bool,
        description: Option<String>,
        args: Option<Vec<(String, Bound<'_, PyAny>, Option<Bound<'_, PyAny>>)>>,
    ) -> PyResult<Self> {
        let type_ref = type_spec_to_type_ref(type_spec)?;
        let resolver_shape = ResolverShape::from_str(shape)?;

        let needs_ctx = matches!(
            resolver_shape,
            ResolverShape::SelfAndContext | ResolverShape::SelfContextAndArgs
        );
        let context_cls = if needs_ctx {
            Some(resolve_context_cls()?)
        } else {
            None
        };

        let field_ctx = Arc::new(FieldContext {
            resolver: Some(ResolverEntry {
                func: PyObj::new(func),
                shape: resolver_shape,
                arg_names,
                is_async_gen: false,
            }),
            output_type: type_ref.clone(),
            context_cls,
        });

        let mut field = Field::new(name, type_ref, move |ctx| {
            if is_async {
                let field_ctx = field_ctx.clone();
                FieldFuture::new(async move { resolve_field(ctx, field_ctx).await })
            } else {
                let result = resolve_field_sync_fast(&ctx, &field_ctx);
                match result {
                    Ok(v) => FieldFuture::Value(Some(v)),
                    Err(e) => FieldFuture::new(async move { Err::<Option<FieldValue<'_>>, _>(e) }),
                }
            }
        });

        if let Some(desc) = description.as_deref() {
            field = field.description(desc);
        }

        if let Some(arg_list) = args {
            for (arg_name, arg_type_spec, arg_default) in &arg_list {
                let iv = build_input_value(arg_name.clone(), arg_type_spec, arg_default.as_ref())?;
                field = field.argument(iv);
            }
        }

        Ok(PyField { inner: field })
    }
}

// ---------------------------------------------------------------------------
// PySubscriptionField — wraps async_graphql::dynamic::SubscriptionField
// ---------------------------------------------------------------------------

#[pyclass(module = "grommet._core", name = "SubscriptionField")]
pub(crate) struct PySubscriptionField {
    inner: SubscriptionField,
}

#[pymethods]
impl PySubscriptionField {
    #[new]
    #[pyo3(signature = (name, type_spec, func, shape, arg_names, description=None, args=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: String,
        type_spec: &Bound<'_, PyAny>,
        func: Py<PyAny>,
        shape: &str,
        arg_names: Vec<String>,
        description: Option<String>,
        args: Option<Vec<(String, Bound<'_, PyAny>, Option<Bound<'_, PyAny>>)>>,
    ) -> PyResult<Self> {
        let type_ref = type_spec_to_type_ref(type_spec)?;
        let resolver_shape = ResolverShape::from_str(shape)?;

        let needs_ctx = matches!(
            resolver_shape,
            ResolverShape::SelfAndContext | ResolverShape::SelfContextAndArgs
        );
        let context_cls = if needs_ctx {
            Some(resolve_context_cls()?)
        } else {
            None
        };

        let field_ctx = Arc::new(FieldContext {
            resolver: Some(ResolverEntry {
                func: PyObj::new(func),
                shape: resolver_shape,
                arg_names,
                is_async_gen: true,
            }),
            output_type: type_ref.clone(),
            context_cls,
        });

        let mut field = SubscriptionField::new(name, type_ref, move |ctx| {
            let field_ctx = field_ctx.clone();
            SubscriptionFieldFuture::new(async move {
                resolve_subscription_stream(ctx, field_ctx).await
            })
        });

        if let Some(desc) = description.as_deref() {
            field = field.description(desc);
        }

        if let Some(arg_list) = args {
            for (arg_name, arg_type_spec, arg_default) in &arg_list {
                let iv = build_input_value(arg_name.clone(), arg_type_spec, arg_default.as_ref())?;
                field = field.argument(iv);
            }
        }

        Ok(PySubscriptionField { inner: field })
    }
}

// ---------------------------------------------------------------------------
// PyInputValue — wraps async_graphql::dynamic::InputValue
// ---------------------------------------------------------------------------

#[pyclass(module = "grommet._core", name = "InputValue")]
pub(crate) struct PyInputValue {
    inner: InputValue,
}

#[pymethods]
impl PyInputValue {
    #[new]
    #[pyo3(signature = (name, type_spec, default_value=None, description=None))]
    fn new(
        name: String,
        type_spec: &Bound<'_, PyAny>,
        default_value: Option<&Bound<'_, PyAny>>,
        description: Option<String>,
    ) -> PyResult<Self> {
        let type_ref = type_spec_to_type_ref(type_spec)?;
        let mut iv = InputValue::new(name, type_ref);
        if let Some(dv) = default_value {
            let py_obj = PyObj::new(dv.clone().unbind());
            iv = iv.default_value(pyobj_to_value(&py_obj)?);
        }
        if let Some(desc) = description.as_deref() {
            iv = iv.description(desc);
        }
        Ok(PyInputValue { inner: iv })
    }
}

// ---------------------------------------------------------------------------
// PyObject — wraps async_graphql::dynamic::Object
// ---------------------------------------------------------------------------

#[pyclass(module = "grommet._core", name = "Object")]
pub(crate) struct PyObject {
    inner: Object,
}

#[pymethods]
impl PyObject {
    #[new]
    #[pyo3(signature = (name, description=None, fields=None))]
    fn new(
        name: String,
        description: Option<String>,
        fields: Option<Vec<Bound<'_, PyField>>>,
    ) -> PyResult<Self> {
        let mut obj = Object::new(&name);
        if let Some(desc) = description.as_deref() {
            obj = obj.description(desc);
        }
        if let Some(field_list) = fields {
            for f in field_list {
                let mut f_mut = f.borrow_mut();
                let field = std::mem::replace(
                    &mut f_mut.inner,
                    Field::new("_", TypeRef::named("String"), |_| FieldFuture::Value(None)),
                );
                obj = obj.field(field);
            }
        }
        Ok(PyObject { inner: obj })
    }
}

impl PyObject {
    pub(crate) fn consume(&mut self) -> Object {
        std::mem::replace(&mut self.inner, Object::new("_"))
    }
}

// ---------------------------------------------------------------------------
// PyInputObject — wraps async_graphql::dynamic::InputObject
// ---------------------------------------------------------------------------

#[pyclass(module = "grommet._core", name = "InputObject")]
pub(crate) struct PyInputObject {
    inner: InputObject,
}

#[pymethods]
impl PyInputObject {
    #[new]
    #[pyo3(signature = (name, description=None, fields=None))]
    fn new(
        name: String,
        description: Option<String>,
        fields: Option<Vec<Bound<'_, PyInputValue>>>,
    ) -> PyResult<Self> {
        let mut obj = InputObject::new(&name);
        if let Some(desc) = description.as_deref() {
            obj = obj.description(desc);
        }
        if let Some(field_list) = fields {
            for iv in field_list {
                let mut iv_mut = iv.borrow_mut();
                let input_value = std::mem::replace(
                    &mut iv_mut.inner,
                    InputValue::new("_", TypeRef::named("String")),
                );
                obj = obj.field(input_value);
            }
        }
        Ok(PyInputObject { inner: obj })
    }
}

impl PyInputObject {
    pub(crate) fn consume(&mut self) -> InputObject {
        std::mem::replace(&mut self.inner, InputObject::new("_"))
    }
}

// ---------------------------------------------------------------------------
// PySubscription — wraps async_graphql::dynamic::Subscription
// ---------------------------------------------------------------------------

#[pyclass(module = "grommet._core", name = "Subscription")]
pub(crate) struct PySubscription {
    inner: Subscription,
}

#[pymethods]
impl PySubscription {
    #[new]
    #[pyo3(signature = (name, description=None, fields=None))]
    fn new(
        name: String,
        description: Option<String>,
        fields: Option<Vec<Bound<'_, PySubscriptionField>>>,
    ) -> PyResult<Self> {
        let mut obj = Subscription::new(&name);
        if let Some(desc) = description.as_deref() {
            obj = obj.description(desc);
        }
        if let Some(field_list) = fields {
            for sf in field_list {
                let mut sf_mut = sf.borrow_mut();
                let sub_field = std::mem::replace(
                    &mut sf_mut.inner,
                    SubscriptionField::new("_", TypeRef::named("String"), |_| {
                        SubscriptionFieldFuture::new(async {
                            Ok(stream::empty::<Result<FieldValue<'_>, async_graphql::Error>>())
                        })
                    }),
                );
                obj = obj.field(sub_field);
            }
        }
        Ok(PySubscription { inner: obj })
    }
}

impl PySubscription {
    pub(crate) fn consume(&mut self) -> Subscription {
        std::mem::replace(&mut self.inner, Subscription::new("_"))
    }
}

// ---------------------------------------------------------------------------
// Schema registration: accepts pre-built type objects from Python
// ---------------------------------------------------------------------------

pub(crate) enum RegistrableType {
    Object(Object),
    InputObject(InputObject),
    Subscription(Subscription),
}

pub(crate) fn register_schema(
    query: &str,
    mutation: Option<&str>,
    subscription: Option<&str>,
    types: Vec<RegistrableType>,
) -> PyResult<Schema> {
    let mut builder: SchemaBuilder = Schema::build(query, mutation, subscription);

    for ty in types {
        let b = std::mem::replace(&mut builder, Schema::build("_", None, None));
        builder = match ty {
            RegistrableType::Object(obj) => b.register(obj),
            RegistrableType::InputObject(inp) => b.register(inp),
            RegistrableType::Subscription(sub) => b.register(sub),
        };
    }

    builder
        .finish()
        .map_err(|err| py_value_error(err.to_string()))
}
