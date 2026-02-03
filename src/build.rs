use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::dynamic::{
    Enum, Field, FieldFuture, InputObject, InputValue, Interface, InterfaceField, Object, Scalar,
    Schema, Subscription, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use pyo3::prelude::*;

use crate::errors::{py_value_error, unknown_type_kind};
use crate::resolver::{resolve_field, resolve_subscription_stream};
use crate::types::{
    EnumDef, FieldDef, PyObj, ScalarBinding, ScalarDef, SchemaDef, TypeDef, UnionDef,
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
        if type_def.kind == "interface" {
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
        match type_def.kind.as_str() {
            "object" => {
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
            "interface" => {
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
            "subscription" => {
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
            "input" => {
                let mut input = InputObject::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    input = input.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    input = input.field(build_input_field(field_def, scalar_bindings.clone())?);
                }
                builder = builder.register(input);
            }
            _ => {
                return Err(unknown_type_kind(type_def.kind.as_str()));
            }
        }
    }

    builder
        .finish()
        .map_err(|err| py_value_error(err.to_string()))
}

fn build_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
) -> PyResult<Field> {
    let resolver = field_def
        .resolver
        .as_ref()
        .and_then(|key| resolver_map.get(key).cloned());
    let arg_names: Arc<Vec<String>> =
        Arc::new(field_def.args.iter().map(|arg| arg.name.clone()).collect());
    let field_name = Arc::new(field_def.name.clone());
    let source_name = Arc::new(field_def.source.clone());
    let type_ref = parse_type_ref(field_def.type_name.as_str());
    let output_type = type_ref.clone();

    let scalars = scalar_bindings.clone();
    let mut field = Field::new(field_def.name, type_ref, move |ctx| {
        let scalars = scalars.clone();
        let resolver = resolver.clone();
        let arg_names = arg_names.clone();
        let field_name = field_name.clone();
        let source_name = source_name.clone();
        let output_type = output_type.clone();
        let abstract_types = abstract_types.clone();
        FieldFuture::new(async move {
            resolve_field(
                ctx,
                resolver,
                arg_names,
                field_name,
                source_name,
                scalars,
                output_type,
                abstract_types,
            )
            .await
        })
    });

    for arg_def in field_def.args {
        let arg_ref = parse_type_ref(arg_def.type_name.as_str());
        let mut input_value = InputValue::new(arg_def.name, arg_ref);
        if let Some(default_value) = arg_def.default_value.as_ref() {
            input_value =
                input_value.default_value(pyobj_to_value(default_value, scalar_bindings.as_ref())?);
        }
        field = field.argument(input_value);
    }
    if let Some(desc) = field_def.description.as_ref() {
        field = field.description(desc.as_str());
    }
    if let Some(dep) = field_def.deprecation.as_ref() {
        field = field.deprecation(Some(dep.as_str()));
    }
    Ok(field)
}

fn build_interface_field(
    field_def: FieldDef,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
) -> PyResult<InterfaceField> {
    let type_ref = parse_type_ref(field_def.type_name.as_str());
    let mut field = InterfaceField::new(field_def.name, type_ref);
    for arg_def in field_def.args {
        let arg_ref = parse_type_ref(arg_def.type_name.as_str());
        let mut input_value = InputValue::new(arg_def.name, arg_ref);
        if let Some(default_value) = arg_def.default_value.as_ref() {
            input_value =
                input_value.default_value(pyobj_to_value(default_value, scalar_bindings.as_ref())?);
        }
        field = field.argument(input_value);
    }
    if let Some(desc) = field_def.description.as_ref() {
        field = field.description(desc.as_str());
    }
    if let Some(dep) = field_def.deprecation.as_ref() {
        field = field.deprecation(Some(dep.as_str()));
    }
    Ok(field)
}

fn build_subscription_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
) -> PyResult<SubscriptionField> {
    let resolver = field_def
        .resolver
        .as_ref()
        .and_then(|key| resolver_map.get(key).cloned());
    let arg_names: Arc<Vec<String>> =
        Arc::new(field_def.args.iter().map(|arg| arg.name.clone()).collect());
    let field_name = Arc::new(field_def.name.clone());
    let source_name = Arc::new(field_def.source.clone());
    let type_ref = parse_type_ref(field_def.type_name.as_str());
    let output_type = type_ref.clone();

    let scalars = scalar_bindings.clone();
    let mut field = SubscriptionField::new(field_def.name, type_ref, move |ctx| {
        let scalars = scalars.clone();
        let resolver = resolver.clone();
        let arg_names = arg_names.clone();
        let field_name = field_name.clone();
        let source_name = source_name.clone();
        let output_type = output_type.clone();
        let abstract_types = abstract_types.clone();
        SubscriptionFieldFuture::new(async move {
            resolve_subscription_stream(
                ctx,
                resolver,
                arg_names,
                field_name,
                source_name,
                scalars,
                output_type,
                abstract_types,
            )
            .await
        })
    });

    for arg_def in field_def.args {
        let arg_ref = parse_type_ref(arg_def.type_name.as_str());
        let mut input_value = InputValue::new(arg_def.name, arg_ref);
        if let Some(default_value) = arg_def.default_value.as_ref() {
            let value = pyobj_to_value(default_value, scalar_bindings.as_ref())?;
            input_value = input_value.default_value(value);
        }
        field = field.argument(input_value);
    }
    if let Some(desc) = field_def.description.as_ref() {
        field = field.description(desc.as_str());
    }
    if let Some(dep) = field_def.deprecation.as_ref() {
        field = field.deprecation(Some(dep.as_str()));
    }
    Ok(field)
}

fn build_input_field(
    field_def: FieldDef,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
) -> PyResult<InputValue> {
    let arg_ref = parse_type_ref(field_def.type_name.as_str());
    let mut input_value = InputValue::new(field_def.name, arg_ref);
    if let Some(default_value) = field_def.default_value.as_ref() {
        input_value =
            input_value.default_value(pyobj_to_value(default_value, scalar_bindings.as_ref())?);
    }
    Ok(input_value)
}

fn parse_type_ref(type_name: &str) -> TypeRef {
    let mut name = type_name.trim();
    let mut non_null = false;
    if name.ends_with('!') {
        non_null = true;
        name = &name[..name.len() - 1];
    }
    let ty = if name.starts_with('[') && name.ends_with(']') {
        let inner = &name[1..name.len() - 1];
        let inner_ref = parse_type_ref(inner);
        TypeRef::List(Box::new(inner_ref))
    } else {
        TypeRef::named(name)
    };

    if non_null {
        TypeRef::NonNull(Box::new(ty))
    } else {
        ty
    }
}
