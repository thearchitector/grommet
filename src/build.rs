use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputObject, InputValue, Object, Schema, Subscription,
    SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use pyo3::prelude::*;
use pyo3::types::PyString;

use crate::errors::py_value_error;
use crate::resolver::{resolve_field, resolve_field_sync, resolve_subscription_stream};
use crate::types::{
    ArgDef, FieldContext, FieldDef, PyObj, ResolverEntry, ResolverShape, ScalarHint, SchemaDef,
    TypeDef, TypeKind,
};
use crate::values::pyobj_to_value;

// assemble the async-graphql schema from python-provided definitions
pub(crate) fn build_schema(
    schema_def: SchemaDef,
    type_defs: Vec<TypeDef>,
    resolver_map: HashMap<String, ResolverEntry>,
) -> PyResult<Schema> {
    // Cache the Context class once for all fields that need it
    let context_cls = resolve_context_cls(&resolver_map)?;

    // Collect object type names for ScalarHint computation
    let object_names: HashSet<String> = type_defs
        .iter()
        .filter(|td| td.kind == TypeKind::Object || td.kind == TypeKind::Subscription)
        .map(|td| td.name.clone())
        .collect();

    let mut builder = Schema::build(
        schema_def.query.as_str(),
        schema_def.mutation.as_deref(),
        schema_def.subscription.as_deref(),
    );

    Python::attach(|py| -> PyResult<()> {
        for type_def in type_defs {
            match type_def.kind {
                TypeKind::Object => {
                    let mut object = Object::new(type_def.name.as_str());
                    if let Some(desc) = type_def.description.as_ref() {
                        object = object.description(desc.as_str());
                    }
                    for field_def in type_def.fields {
                        let field = build_field(
                            py,
                            field_def,
                            &resolver_map,
                            context_cls.as_ref(),
                            &object_names,
                        )?;
                        object = object.field(field);
                    }
                    // SAFETY: we must take and put back because Schema::build is not Copy
                    let b = std::mem::replace(&mut builder, Schema::build("_", None, None));
                    builder = b.register(object);
                }
                TypeKind::Subscription => {
                    let mut subscription = Subscription::new(type_def.name.as_str());
                    if let Some(desc) = type_def.description.as_ref() {
                        subscription = subscription.description(desc.as_str());
                    }
                    for field_def in type_def.fields {
                        let field = build_subscription_field(
                            py,
                            field_def,
                            &resolver_map,
                            context_cls.as_ref(),
                            &object_names,
                        )?;
                        subscription = subscription.field(field);
                    }
                    let b = std::mem::replace(&mut builder, Schema::build("_", None, None));
                    builder = b.register(subscription);
                }
                TypeKind::Input => {
                    let mut input = InputObject::new(type_def.name.as_str());
                    if let Some(desc) = type_def.description.as_ref() {
                        input = input.description(desc.as_str());
                    }
                    for field_def in type_def.fields {
                        input = input.field(build_input_field(field_def)?);
                    }
                    let b = std::mem::replace(&mut builder, Schema::build("_", None, None));
                    builder = b.register(input);
                }
            }
        }
        Ok(())
    })?;

    builder
        .finish()
        .map_err(|err| py_value_error(err.to_string()))
}

fn resolve_context_cls(resolver_map: &HashMap<String, ResolverEntry>) -> PyResult<Option<PyObj>> {
    let needs_context = resolver_map.values().any(|e| {
        matches!(
            e.shape,
            ResolverShape::SelfAndContext | ResolverShape::SelfContextAndArgs
        )
    });
    if !needs_context {
        return Ok(None);
    }
    Python::attach(|py| {
        let cls = py.import("grommet.context")?.getattr("Context")?;
        Ok(Some(PyObj::new(cls.unbind())))
    })
}

fn build_field_context(
    py: Python<'_>,
    field_def: &FieldDef,
    resolver_map: &HashMap<String, ResolverEntry>,
    context_cls: Option<&PyObj>,
    object_names: &HashSet<String>,
) -> Arc<FieldContext> {
    let resolver = field_def
        .resolver
        .as_ref()
        .and_then(|key| resolver_map.get(key).cloned());
    let needs_ctx = resolver.as_ref().is_some_and(|r| {
        matches!(
            r.shape,
            ResolverShape::SelfAndContext | ResolverShape::SelfContextAndArgs
        )
    });
    let scalar_hint = compute_scalar_hint(&field_def.type_ref, object_names);
    let source_name_py = std::sync::Arc::new(PyString::new(py, &field_def.source).unbind());
    Arc::new(FieldContext {
        resolver,
        source_name_py,
        output_type: field_def.type_ref.clone(),
        context_cls: if needs_ctx {
            context_cls.cloned()
        } else {
            None
        },
        scalar_hint,
    })
}

fn compute_scalar_hint(type_ref: &TypeRef, object_names: &HashSet<String>) -> ScalarHint {
    match type_ref {
        TypeRef::NonNull(inner) => compute_scalar_hint(inner, object_names),
        TypeRef::List(_) => ScalarHint::Unknown,
        TypeRef::Named(name) => {
            let s: &str = name;
            match s {
                "String" => ScalarHint::String,
                "Int" => ScalarHint::Int,
                "Float" => ScalarHint::Float,
                "Boolean" => ScalarHint::Boolean,
                "ID" => ScalarHint::ID,
                n if object_names.contains(n) => ScalarHint::Object,
                _ => ScalarHint::Unknown,
            }
        }
    }
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

fn build_field(
    py: Python<'_>,
    field_def: FieldDef,
    resolver_map: &HashMap<String, ResolverEntry>,
    context_cls: Option<&PyObj>,
    object_names: &HashSet<String>,
) -> PyResult<Field> {
    let field_ctx = build_field_context(py, &field_def, resolver_map, context_cls, object_names);
    let type_ref = field_ctx.output_type.clone();
    let has_resolver = field_ctx.resolver.is_some();

    let mut field = Field::new(field_def.name, type_ref, move |ctx| {
        if has_resolver {
            let field_ctx = field_ctx.clone();
            FieldFuture::new(async move { resolve_field(ctx, field_ctx).await })
        } else {
            // Synchronous path: resolve parent attribute + convert in one GIL block
            let result = resolve_field_sync(&ctx, &field_ctx);
            match result {
                Ok(v) => FieldFuture::Value(Some(v)),
                Err(e) => FieldFuture::new(async move { Err::<Option<FieldValue<'_>>, _>(e) }),
            }
        }
    });
    for iv in build_input_values(field_def.args)? {
        field = field.argument(iv);
    }
    Ok(apply_metadata!(field, field_def))
}

fn build_subscription_field(
    py: Python<'_>,
    field_def: FieldDef,
    resolver_map: &HashMap<String, ResolverEntry>,
    context_cls: Option<&PyObj>,
    object_names: &HashSet<String>,
) -> PyResult<SubscriptionField> {
    let field_ctx = build_field_context(py, &field_def, resolver_map, context_cls, object_names);
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
