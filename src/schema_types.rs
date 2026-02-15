use std::sync::{Arc, OnceLock};

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputObject, InputValue, Object, Schema, SchemaBuilder,
    Subscription, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use pyo3::prelude::*;

use crate::errors::{py_type_error, py_value_error};
use crate::resolver::{resolve_field, resolve_field_sync_fast, resolve_subscription_stream};
use crate::types::{FieldContext, PyObj, ResolverEntry};
use crate::values::pyobj_to_value;

type FieldArgSpec<'py> = (String, Bound<'py, PyAny>, Option<Bound<'py, PyAny>>);
type FieldArgSpecs<'py> = Option<Vec<FieldArgSpec<'py>>>;

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

fn resolve_context_cls(py: Python<'_>) -> PyResult<PyObj> {
    static CONTEXT_CLS: OnceLock<Py<PyAny>> = OnceLock::new();
    if let Some(cls) = CONTEXT_CLS.get() {
        return Ok(PyObj::new(cls.clone_ref(py)));
    }

    let cls = py.import("grommet.context")?.getattr("Context")?.unbind();
    let _ = CONTEXT_CLS.set(cls.clone_ref(py));
    Ok(PyObj::new(cls))
}

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

fn build_field_context(
    py: Python<'_>,
    func: Py<PyAny>,
    needs_context: bool,
    is_async_gen: bool,
    output_type: &TypeRef,
) -> PyResult<Arc<FieldContext>> {
    let context_cls = if needs_context {
        Some(resolve_context_cls(py)?)
    } else {
        None
    };

    Ok(Arc::new(FieldContext {
        resolver: Some(ResolverEntry {
            func: PyObj::new(func),
            needs_context,
            is_async_gen,
        }),
        output_type: output_type.clone(),
        context_cls,
    }))
}

fn take_inner<T>(slot: &mut Option<T>, type_name: &str) -> PyResult<T> {
    slot.take().ok_or_else(|| {
        py_type_error(format!(
            "{type_name} has already been consumed and cannot be reused"
        ))
    })
}

#[pyclass(module = "grommet._core", name = "Field")]
pub(crate) struct PyField {
    inner: Option<Field>,
}

#[pymethods]
impl PyField {
    #[new]
    #[pyo3(signature = (name, type_spec, func, needs_context, is_async, description=None, args=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        name: String,
        type_spec: &Bound<'_, PyAny>,
        func: Py<PyAny>,
        needs_context: bool,
        is_async: bool,
        description: Option<String>,
        args: FieldArgSpecs<'_>,
    ) -> PyResult<Self> {
        let type_ref = type_spec_to_type_ref(type_spec)?;
        let field_ctx = build_field_context(py, func, needs_context, false, &type_ref)?;

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

        Ok(PyField { inner: Some(field) })
    }
}

impl PyField {
    pub(crate) fn consume(&mut self) -> PyResult<Field> {
        take_inner(&mut self.inner, "Field")
    }
}

#[pyclass(module = "grommet._core", name = "SubscriptionField")]
pub(crate) struct PySubscriptionField {
    inner: Option<SubscriptionField>,
}

#[pymethods]
impl PySubscriptionField {
    #[new]
    #[pyo3(signature = (name, type_spec, func, needs_context, description=None, args=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        name: String,
        type_spec: &Bound<'_, PyAny>,
        func: Py<PyAny>,
        needs_context: bool,
        description: Option<String>,
        args: FieldArgSpecs<'_>,
    ) -> PyResult<Self> {
        let type_ref = type_spec_to_type_ref(type_spec)?;
        let field_ctx = build_field_context(py, func, needs_context, true, &type_ref)?;

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

        Ok(PySubscriptionField { inner: Some(field) })
    }
}

impl PySubscriptionField {
    pub(crate) fn consume(&mut self) -> PyResult<SubscriptionField> {
        take_inner(&mut self.inner, "SubscriptionField")
    }
}

#[pyclass(module = "grommet._core", name = "InputValue")]
pub(crate) struct PyInputValue {
    inner: Option<InputValue>,
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
        Ok(PyInputValue { inner: Some(iv) })
    }
}

impl PyInputValue {
    pub(crate) fn consume(&mut self) -> PyResult<InputValue> {
        take_inner(&mut self.inner, "InputValue")
    }
}

#[pyclass(module = "grommet._core", name = "Object")]
pub(crate) struct PyObject {
    inner: Option<Object>,
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
                let field = f.borrow_mut().consume()?;
                obj = obj.field(field);
            }
        }
        Ok(PyObject { inner: Some(obj) })
    }
}

impl PyObject {
    pub(crate) fn consume(&mut self) -> PyResult<Object> {
        take_inner(&mut self.inner, "Object")
    }
}

#[pyclass(module = "grommet._core", name = "InputObject")]
pub(crate) struct PyInputObject {
    inner: Option<InputObject>,
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
                let input_value = iv.borrow_mut().consume()?;
                obj = obj.field(input_value);
            }
        }
        Ok(PyInputObject { inner: Some(obj) })
    }
}

impl PyInputObject {
    pub(crate) fn consume(&mut self) -> PyResult<InputObject> {
        take_inner(&mut self.inner, "InputObject")
    }
}

#[pyclass(module = "grommet._core", name = "Subscription")]
pub(crate) struct PySubscription {
    inner: Option<Subscription>,
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
                let sub_field = sf.borrow_mut().consume()?;
                obj = obj.field(sub_field);
            }
        }
        Ok(PySubscription { inner: Some(obj) })
    }
}

impl PySubscription {
    pub(crate) fn consume(&mut self) -> PyResult<Subscription> {
        take_inner(&mut self.inner, "Subscription")
    }
}

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
    let builder: SchemaBuilder = types.into_iter().fold(
        Schema::build(query, mutation, subscription),
        |builder, ty| match ty {
            RegistrableType::Object(obj) => builder.register(obj),
            RegistrableType::InputObject(inp) => builder.register(inp),
            RegistrableType::Subscription(sub) => builder.register(sub),
        },
    );

    builder
        .finish()
        .map_err(|err| py_value_error(err.to_string()))
}
