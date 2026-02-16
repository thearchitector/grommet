use std::sync::Arc;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputObject, InputValue, Object, Schema, SchemaBuilder,
    Subscription, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyAnyMethods;

use crate::errors::{py_type_error, py_value_error};
use crate::resolver::{resolve_field, resolve_field_sync_fast, resolve_subscription_stream};
use crate::types::{FieldContext, PyObj, ResolverEntry};
use crate::values::pyobj_to_value;

const UNSUPPORTED_REGISTRATION_TYPE: &str =
    "Schema bundle contains an unsupported type registration object";

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

fn unsupported_registration_type() -> PyErr {
    py_type_error(UNSUPPORTED_REGISTRATION_TYPE)
}

fn resolve_context_cls(py: Python<'_>) -> PyResult<PyObj> {
    static CONTEXT_CLS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    let cls = CONTEXT_CLS.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        Ok(py.import("grommet.context")?.getattr("Context")?.unbind())
    })?;
    Ok(PyObj::new(cls.clone_ref(py)))
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

fn default_value_from_payload<'py>(
    payload: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let has_default: bool = payload.getattr("has_default")?.extract()?;
    if has_default {
        Ok(Some(payload.getattr("default")?))
    } else {
        Ok(None)
    }
}

fn build_input_value(
    name: String,
    type_spec: &Bound<'_, PyAny>,
    default_value: Option<&Bound<'_, PyAny>>,
    description: Option<&str>,
) -> PyResult<InputValue> {
    let type_ref = type_spec_to_type_ref(type_spec)?;
    let mut iv = InputValue::new(name, type_ref);
    if let Some(default_value) = default_value {
        let py_obj = PyObj::new(default_value.clone().unbind());
        iv = iv.default_value(pyobj_to_value(&py_obj)?);
    }
    if let Some(description) = description {
        iv = iv.description(description);
    }
    Ok(iv)
}

fn build_argument_input_value(arg: &Bound<'_, PyAny>) -> PyResult<InputValue> {
    let name: String = arg.getattr("name")?.extract()?;
    let type_spec = arg.getattr("type_spec")?;
    let default_value = default_value_from_payload(arg)?;
    build_input_value(name, &type_spec, default_value.as_ref(), None)
}

fn build_input_field_value(field: &Bound<'_, PyAny>) -> PyResult<InputValue> {
    let name: String = field.getattr("name")?.extract()?;
    let type_spec = field.getattr("type_spec")?;
    let description: Option<String> = field.getattr("description")?.extract()?;
    let default_value = default_value_from_payload(field)?;
    build_input_value(
        name,
        &type_spec,
        default_value.as_ref(),
        description.as_deref(),
    )
}

fn build_object_field(py: Python<'_>, field: &Bound<'_, PyAny>) -> PyResult<Field> {
    let name: String = field.getattr("name")?.extract()?;
    let type_spec = field.getattr("type_spec")?;
    let type_ref = type_spec_to_type_ref(&type_spec)?;
    let description: Option<String> = field.getattr("description")?.extract()?;
    let is_data_field = field.hasattr("resolver_func")?;

    let mut graphql_field = if is_data_field {
        let func: Py<PyAny> = field.getattr("resolver_func")?.extract()?;
        let field_ctx = build_field_context(py, func, false, false, &type_ref)?;
        Field::new(name, type_ref, move |ctx| {
            let result = resolve_field_sync_fast(&ctx, &field_ctx);
            match result {
                Ok(value) => FieldFuture::Value(Some(value)),
                Err(err) => FieldFuture::new(async move { Err::<Option<FieldValue<'_>>, _>(err) }),
            }
        })
    } else {
        let func: Py<PyAny> = field.getattr("func")?.extract()?;
        let needs_context: bool = field.getattr("needs_context")?.extract()?;
        let is_async: bool = field.getattr("is_async")?.extract()?;
        let field_ctx = build_field_context(py, func, needs_context, false, &type_ref)?;

        let mut graphql_field = Field::new(name, type_ref, move |ctx| {
            if is_async {
                let field_ctx = field_ctx.clone();
                FieldFuture::new(async move { resolve_field(ctx, field_ctx).await })
            } else {
                let result = resolve_field_sync_fast(&ctx, &field_ctx);
                match result {
                    Ok(value) => FieldFuture::Value(Some(value)),
                    Err(err) => {
                        FieldFuture::new(async move { Err::<Option<FieldValue<'_>>, _>(err) })
                    }
                }
            }
        });

        let args: Vec<Py<PyAny>> = field.getattr("args")?.extract()?;
        for arg in &args {
            let iv = build_argument_input_value(arg.bind(py))?;
            graphql_field = graphql_field.argument(iv);
        }

        graphql_field
    };

    if let Some(description) = description.as_deref() {
        graphql_field = graphql_field.description(description);
    }

    Ok(graphql_field)
}

fn build_subscription_field(
    py: Python<'_>,
    field: &Bound<'_, PyAny>,
) -> PyResult<SubscriptionField> {
    let name: String = field.getattr("name")?.extract()?;
    let type_spec = field.getattr("type_spec")?;
    let type_ref = type_spec_to_type_ref(&type_spec)?;
    let func: Py<PyAny> = field.getattr("func")?.extract()?;
    let needs_context: bool = field.getattr("needs_context")?.extract()?;
    let description: Option<String> = field.getattr("description")?.extract()?;
    let field_ctx = build_field_context(py, func, needs_context, true, &type_ref)?;

    let mut graphql_field = SubscriptionField::new(name, type_ref, move |ctx| {
        let field_ctx = field_ctx.clone();
        SubscriptionFieldFuture::new(
            async move { resolve_subscription_stream(ctx, field_ctx).await },
        )
    });

    let args: Vec<Py<PyAny>> = field.getattr("args")?.extract()?;
    for arg in &args {
        let iv = build_argument_input_value(arg.bind(py))?;
        graphql_field = graphql_field.argument(iv);
    }

    if let Some(description) = description.as_deref() {
        graphql_field = graphql_field.description(description);
    }

    Ok(graphql_field)
}

fn build_object_type(
    py: Python<'_>,
    compiled_type: &Bound<'_, PyAny>,
    type_name: &str,
    description: Option<&str>,
) -> PyResult<Object> {
    let mut object = Object::new(type_name);
    if let Some(description) = description {
        object = object.description(description);
    }

    let fields: Vec<Py<PyAny>> = compiled_type.getattr("object_fields")?.extract()?;
    for field in &fields {
        object = object.field(build_object_field(py, field.bind(py))?);
    }

    Ok(object)
}

fn build_input_object_type(
    py: Python<'_>,
    compiled_type: &Bound<'_, PyAny>,
    type_name: &str,
    description: Option<&str>,
) -> PyResult<InputObject> {
    let mut input_object = InputObject::new(type_name);
    if let Some(description) = description {
        input_object = input_object.description(description);
    }

    let fields: Vec<Py<PyAny>> = compiled_type.getattr("input_fields")?.extract()?;
    for field in &fields {
        input_object = input_object.field(build_input_field_value(field.bind(py))?);
    }

    Ok(input_object)
}

fn build_subscription_type(
    py: Python<'_>,
    compiled_type: &Bound<'_, PyAny>,
    type_name: &str,
    description: Option<&str>,
) -> PyResult<Subscription> {
    let mut subscription = Subscription::new(type_name);
    if let Some(description) = description {
        subscription = subscription.description(description);
    }

    let fields: Vec<Py<PyAny>> = compiled_type.getattr("subscription_fields")?.extract()?;
    for field in &fields {
        subscription = subscription.field(build_subscription_field(py, field.bind(py))?);
    }

    Ok(subscription)
}

pub(crate) enum RegistrableType {
    Object(Object),
    InputObject(InputObject),
    Subscription(Subscription),
}

fn decode_type_kind(meta: &Bound<'_, PyAny>) -> PyResult<String> {
    let kind = meta.getattr("kind")?;
    kind.getattr("value")?.extract()
}

fn decode_registrable_type(
    py: Python<'_>,
    compiled_type: &Bound<'_, PyAny>,
) -> PyResult<RegistrableType> {
    let meta = compiled_type
        .getattr("meta")
        .map_err(|_| unsupported_registration_type())?;
    let kind = decode_type_kind(&meta).map_err(|_| unsupported_registration_type())?;
    let type_name: String = meta
        .getattr("name")
        .and_then(|value| value.extract())
        .map_err(|_| unsupported_registration_type())?;
    let description: Option<String> = meta
        .getattr("description")
        .and_then(|value| value.extract())
        .map_err(|_| unsupported_registration_type())?;

    match kind.as_str() {
        "object" => Ok(RegistrableType::Object(build_object_type(
            py,
            compiled_type,
            &type_name,
            description.as_deref(),
        )?)),
        "input" => Ok(RegistrableType::InputObject(build_input_object_type(
            py,
            compiled_type,
            &type_name,
            description.as_deref(),
        )?)),
        "subscription" => Ok(RegistrableType::Subscription(build_subscription_type(
            py,
            compiled_type,
            &type_name,
            description.as_deref(),
        )?)),
        _ => Err(unsupported_registration_type()),
    }
}

pub(crate) fn register_schema(
    py: Python<'_>,
    query: &str,
    mutation: Option<&str>,
    subscription: Option<&str>,
    types: Vec<Py<PyAny>>,
) -> PyResult<Schema> {
    let mut builder: SchemaBuilder = Schema::build(query, mutation, subscription);

    for compiled_type in &types {
        let registrable = decode_registrable_type(py, compiled_type.bind(py))?;
        builder = match registrable {
            RegistrableType::Object(object) => builder.register(object),
            RegistrableType::InputObject(input_object) => builder.register(input_object),
            RegistrableType::Subscription(subscription) => builder.register(subscription),
        };
    }

    builder
        .finish()
        .map_err(|err| py_value_error(err.to_string()))
}
