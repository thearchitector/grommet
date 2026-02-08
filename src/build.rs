use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::dynamic::{
    Field, FieldFuture, InputObject, InputValue, Object, Schema, Subscription, SubscriptionField,
    SubscriptionFieldFuture,
};
use pyo3::prelude::*;

use crate::errors::py_value_error;
use crate::resolver::{resolve_field, resolve_subscription_stream};
use crate::types::{ArgDef, FieldContext, FieldDef, PyObj, SchemaDef, TypeDef, TypeKind};
use crate::values::pyobj_to_value;

// assemble the async-graphql schema from python-provided definitions
pub(crate) fn build_schema(
    schema_def: SchemaDef,
    type_defs: Vec<TypeDef>,
    resolver_map: HashMap<String, PyObj>,
) -> PyResult<Schema> {
    let mut builder = Schema::build(
        schema_def.query.as_str(),
        schema_def.mutation.as_deref(),
        schema_def.subscription.as_deref(),
    );

    for type_def in type_defs {
        match type_def.kind {
            TypeKind::Object => {
                let mut object = Object::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    object = object.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    let field = build_field(field_def, &resolver_map)?;
                    object = object.field(field);
                }
                builder = builder.register(object);
            }
            TypeKind::Subscription => {
                let mut subscription = Subscription::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    subscription = subscription.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    let field = build_subscription_field(field_def, &resolver_map)?;
                    subscription = subscription.field(field);
                }
                builder = builder.register(subscription);
            }
            TypeKind::Input => {
                let mut input = InputObject::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    input = input.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    input = input.field(build_input_field(field_def)?);
                }
                builder = builder.register(input);
            }
        }
    }

    builder
        .finish()
        .map_err(|err| py_value_error(err.to_string()))
}

fn build_field_context(
    field_def: &FieldDef,
    resolver_map: &HashMap<String, PyObj>,
) -> Arc<FieldContext> {
    let resolver = field_def
        .resolver
        .as_ref()
        .and_then(|key| resolver_map.get(key).cloned());
    let arg_names: Vec<String> = field_def.args.iter().map(|arg| arg.name.clone()).collect();
    Arc::new(FieldContext {
        resolver,
        arg_names,
        source_name: field_def.source.clone(),
        output_type: field_def.type_ref.clone(),
    })
}

macro_rules! apply_metadata {
    ($field:expr, $def:expr) => {{
        let mut f = $field;
        if let Some(desc) = $def.description.as_ref() {
            f = f.description(desc.as_str());
        }
        if let Some(dep) = $def.deprecation.as_ref() {
            f = f.deprecation(Some(dep.as_str()));
        }
        f
    }};
}

fn build_input_values(args: Vec<ArgDef>) -> PyResult<Vec<InputValue>> {
    let mut input_values = Vec::with_capacity(args.len());
    for arg_def in args {
        let mut input_value = InputValue::new(arg_def.name, arg_def.type_ref);
        if let Some(default_value) = arg_def.default_value.as_ref() {
            input_value = input_value.default_value(pyobj_to_value(default_value)?);
        }
        input_values.push(input_value);
    }
    Ok(input_values)
}

fn build_field(field_def: FieldDef, resolver_map: &HashMap<String, PyObj>) -> PyResult<Field> {
    let field_ctx = build_field_context(&field_def, resolver_map);
    let type_ref = field_ctx.output_type.clone();

    let mut field = Field::new(field_def.name, type_ref, move |ctx| {
        let field_ctx = field_ctx.clone();
        FieldFuture::new(async move { resolve_field(ctx, field_ctx).await })
    });
    for iv in build_input_values(field_def.args)? {
        field = field.argument(iv);
    }
    Ok(apply_metadata!(field, field_def))
}

fn build_subscription_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
) -> PyResult<SubscriptionField> {
    let field_ctx = build_field_context(&field_def, resolver_map);
    let type_ref = field_ctx.output_type.clone();

    let mut field = SubscriptionField::new(field_def.name, type_ref, move |ctx| {
        let field_ctx = field_ctx.clone();
        SubscriptionFieldFuture::new(
            async move { resolve_subscription_stream(ctx, field_ctx).await },
        )
    });
    for iv in build_input_values(field_def.args)? {
        field = field.argument(iv);
    }
    Ok(apply_metadata!(field, field_def))
}

fn build_input_field(field_def: FieldDef) -> PyResult<InputValue> {
    let mut input_value = InputValue::new(field_def.name, field_def.type_ref);
    if let Some(desc) = field_def.description.as_ref() {
        input_value = input_value.description(desc.as_str());
    }
    if let Some(default_value) = field_def.default_value.as_ref() {
        input_value = input_value.default_value(pyobj_to_value(default_value)?);
    }
    Ok(input_value)
}
