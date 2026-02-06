use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::dynamic::{
    Enum, Field, FieldFuture, InputObject, InputValue, Interface, InterfaceField, Object, Scalar,
    Schema, Subscription, SubscriptionField, SubscriptionFieldFuture,
};
use pyo3::prelude::*;

use crate::errors::py_value_error;
use crate::resolver::{resolve_field, resolve_subscription_stream};
use crate::types::{
    ArgDef, EnumDef, FieldContext, FieldDef, PyObj, ScalarBinding, ScalarDef, SchemaDef, TypeDef,
    TypeKind, UnionDef,
};
use crate::values::pyobj_to_value;

// assemble the async-graphql schema from python-provided definitions
pub(crate) fn build_schema(
    schema_def: SchemaDef,
    type_defs: Vec<TypeDef>,
    scalar_defs: Vec<ScalarDef>,
    enum_defs: Vec<EnumDef>,
    union_defs: Vec<UnionDef>,
    resolver_map: HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
) -> PyResult<Schema> {
    let mut builder = Schema::build(
        schema_def.query.as_str(),
        schema_def.mutation.as_deref(),
        schema_def.subscription.as_deref(),
    );

    let mut abstract_types = HashSet::new();
    for type_def in &type_defs {
        if type_def.kind == TypeKind::Interface {
            abstract_types.insert(type_def.name.clone());
        }
    }
    for union_def in &union_defs {
        abstract_types.insert(union_def.name.clone());
    }
    let abstract_types = Arc::new(abstract_types);

    for scalar_def in scalar_defs {
        let mut scalar = Scalar::new(scalar_def.name.as_str());
        if let Some(desc) = scalar_def.description.as_ref() {
            scalar = scalar.description(desc.as_str());
        }
        if let Some(url) = scalar_def.specified_by_url.as_ref() {
            scalar = scalar.specified_by_url(url.as_str());
        }
        builder = builder.register(scalar);
    }

    for enum_def in enum_defs {
        let mut enum_type = Enum::new(enum_def.name.as_str());
        if let Some(desc) = enum_def.description.as_ref() {
            enum_type = enum_type.description(desc.as_str());
        }
        for value in enum_def.values {
            enum_type = enum_type.item(value);
        }
        builder = builder.register(enum_type);
    }

    for union_def in union_defs {
        let mut union_type = async_graphql::dynamic::Union::new(union_def.name.as_str());
        if let Some(desc) = union_def.description.as_ref() {
            union_type = union_type.description(desc.as_str());
        }
        for ty in union_def.types {
            union_type = union_type.possible_type(ty);
        }
        builder = builder.register(union_type);
    }

    for type_def in type_defs {
        match type_def.kind {
            TypeKind::Object => {
                let mut object = Object::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    object = object.description(desc.as_str());
                }
                for implement in &type_def.implements {
                    object = object.implement(implement.as_str());
                }
                for field_def in type_def.fields {
                    let field = build_field(
                        field_def,
                        &resolver_map,
                        scalar_bindings.clone(),
                        abstract_types.clone(),
                    )?;
                    object = object.field(field);
                }
                builder = builder.register(object);
            }
            TypeKind::Interface => {
                let mut interface = Interface::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    interface = interface.description(desc.as_str());
                }
                for implement in &type_def.implements {
                    interface = interface.implement(implement.as_str());
                }
                for field_def in type_def.fields {
                    interface =
                        interface.field(build_interface_field(field_def, scalar_bindings.clone())?);
                }
                builder = builder.register(interface);
            }
            TypeKind::Subscription => {
                let mut subscription = Subscription::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    subscription = subscription.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    let field = build_subscription_field(
                        field_def,
                        &resolver_map,
                        scalar_bindings.clone(),
                        abstract_types.clone(),
                    )?;
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
                    input = input.field(build_input_field(field_def, scalar_bindings.clone())?);
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
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
) -> Arc<FieldContext> {
    let resolver = field_def
        .resolver
        .as_ref()
        .and_then(|key| resolver_map.get(key).cloned());
    let arg_names: Vec<String> = field_def.args.iter().map(|arg| arg.name.clone()).collect();
    Arc::new(FieldContext {
        resolver,
        arg_names,
        field_name: field_def.name.clone(),
        source_name: field_def.source.clone(),
        output_type: field_def.type_ref.clone(),
        scalar_bindings,
        abstract_types,
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

fn build_input_values(
    args: Vec<ArgDef>,
    scalar_bindings: &[ScalarBinding],
) -> PyResult<Vec<InputValue>> {
    let mut input_values = Vec::with_capacity(args.len());
    for arg_def in args {
        let mut input_value = InputValue::new(arg_def.name, arg_def.type_ref);
        if let Some(default_value) = arg_def.default_value.as_ref() {
            input_value =
                input_value.default_value(pyobj_to_value(default_value, scalar_bindings)?);
        }
        input_values.push(input_value);
    }
    Ok(input_values)
}

fn build_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
) -> PyResult<Field> {
    let field_ctx = build_field_context(
        &field_def,
        resolver_map,
        scalar_bindings.clone(),
        abstract_types,
    );
    let type_ref = field_ctx.output_type.clone();

    let mut field = Field::new(field_def.name, type_ref, move |ctx| {
        let field_ctx = field_ctx.clone();
        FieldFuture::new(async move { resolve_field(ctx, field_ctx).await })
    });
    for iv in build_input_values(field_def.args, scalar_bindings.as_ref())? {
        field = field.argument(iv);
    }
    Ok(apply_metadata!(field, field_def))
}

fn build_interface_field(
    field_def: FieldDef,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
) -> PyResult<InterfaceField> {
    let mut field = InterfaceField::new(field_def.name, field_def.type_ref);
    for iv in build_input_values(field_def.args, scalar_bindings.as_ref())? {
        field = field.argument(iv);
    }
    Ok(apply_metadata!(field, field_def))
}

fn build_subscription_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
) -> PyResult<SubscriptionField> {
    let field_ctx = build_field_context(
        &field_def,
        resolver_map,
        scalar_bindings.clone(),
        abstract_types,
    );
    let type_ref = field_ctx.output_type.clone();

    let mut field = SubscriptionField::new(field_def.name, type_ref, move |ctx| {
        let field_ctx = field_ctx.clone();
        SubscriptionFieldFuture::new(
            async move { resolve_subscription_stream(ctx, field_ctx).await },
        )
    });
    for iv in build_input_values(field_def.args, scalar_bindings.as_ref())? {
        field = field.argument(iv);
    }
    Ok(apply_metadata!(field, field_def))
}

fn build_input_field(
    field_def: FieldDef,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
) -> PyResult<InputValue> {
    let mut input_value = InputValue::new(field_def.name, field_def.type_ref);
    if let Some(default_value) = field_def.default_value.as_ref() {
        input_value =
            input_value.default_value(pyobj_to_value(default_value, scalar_bindings.as_ref())?);
    }
    Ok(input_value)
}
