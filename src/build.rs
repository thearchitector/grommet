use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::dynamic::{
    Enum, Field, FieldFuture, FieldValue, InputObject, InputValue, Interface,
    InterfaceField, Object, ResolverContext, Scalar, Schema, Subscription,
    SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::futures_util::stream::{self, BoxStream, StreamExt};
use async_graphql::Error;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyTuple};

use crate::types::{
    ContextValue, EnumDef, FieldDef, PyObj, RootValue, ScalarBinding, ScalarDef, SchemaDef,
    TypeDef, UnionDef,
};
use crate::values::{
    build_kwargs, py_err_to_error, py_to_field_value_for_type, pyobj_to_value,
};

// assemble the async-graphql schema from python-provided definitions
pub(crate) fn build_schema(
    schema_def: SchemaDef,
    type_defs: Vec<TypeDef>,
    scalar_defs: Vec<ScalarDef>,
    enum_defs: Vec<EnumDef>,
    union_defs: Vec<UnionDef>,
    resolver_map: HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    debug: bool,
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
                    object = object.field(build_field(
                        field_def,
                        &resolver_map,
                        scalar_bindings.clone(),
                        abstract_types.clone(),
                        debug,
                    )?);
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
                    interface = interface.field(build_interface_field(
                        field_def,
                        scalar_bindings.clone(),
                    )?);
                }
                builder = builder.register(interface);
            }
            "subscription" => {
                let mut subscription = Subscription::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    subscription = subscription.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    subscription = subscription.field(build_subscription_field(
                        field_def,
                        &resolver_map,
                        scalar_bindings.clone(),
                        abstract_types.clone(),
                        debug,
                    )?);
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
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    format!("Unknown type kind: {}", type_def.kind),
                ))
            }
        }
    }

    builder
        .finish()
        .map_err(|err| PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string()))
}

fn build_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
    debug: bool,
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
                debug,
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

fn build_subscription_field(
    field_def: FieldDef,
    resolver_map: &HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    abstract_types: Arc<HashSet<String>>,
    debug: bool,
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
            resolve_subscription_field(
                ctx,
                resolver,
                arg_names,
                field_name,
                source_name,
                scalars,
                output_type,
                abstract_types,
                debug,
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
        let value = pyobj_to_value(default_value, scalar_bindings.as_ref())?;
        input_value = input_value.default_value(value);
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

async fn resolve_field(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: Arc<String>,
    source_name: Arc<String>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
    debug: bool,
) -> Result<Option<FieldValue<'_>>, Error> {
    let root_value = ctx.data::<RootValue>().ok().map(|root| root.0.clone());
    let parent = ctx
        .parent_value
        .try_downcast_ref::<PyObj>()
        .ok()
        .cloned()
        .or_else(|| root_value.clone());
    let context = ctx
        .data::<ContextValue>()
        .ok()
        .map(|ctx| ctx.0.clone());

    if let Some(resolver) = resolver {
        let result = Python::with_gil(|py| -> PyResult<(bool, Py<PyAny>)> {
            let kwargs = build_kwargs(py, &ctx, &arg_names)?;
            let info = PyDict::new(py);
            info.set_item("field_name", field_name.as_str())?;
            if let Some(ctx_obj) = context.as_ref() {
                info.set_item("context", ctx_obj.inner.bind(py))?;
            } else {
                info.set_item("context", py.None())?;
            }
            if let Some(root_obj) = root_value.as_ref() {
                info.set_item("root", root_obj.inner.bind(py))?;
            } else {
                info.set_item("root", py.None())?;
            }
            let parent_obj = match parent.as_ref() {
                Some(parent) => parent.inner.clone_ref(py),
                None => py.None(),
            };
            let args = PyTuple::new(py, [parent_obj, info.into_any().unbind()])?;
            let result = resolver.inner.call(py, args, Some(&kwargs))?;
            let is_awaitable = result.bind(py).hasattr("__await__")?;
            Ok((is_awaitable, result))
        });

        let (is_awaitable, result) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err, debug)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(result.into_bound(py))
            })
            .map_err(|err| py_err_to_error(err, debug))?
            .await
            .map_err(|err| py_err_to_error(err, debug))?;
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    &awaited.bind(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
            .map_err(|err| py_err_to_error(err, debug))
            .map(Some)
        } else {
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    &result.bind(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
            .map_err(|err| py_err_to_error(err, debug))
            .map(Some)
        }
    } else {
        let parent = parent.ok_or_else(|| Error::new("No parent value for field"))?;
        let result = Python::with_gil(|py| -> PyResult<(bool, Py<PyAny>)> {
            let parent_ref = parent.inner.bind(py);
            let value = if let Ok(dict) = parent_ref.downcast::<PyDict>() {
                match dict.get_item(source_name.as_str())? {
                    Some(item) => item.unbind(),
                    None => py.None(),
                }
            } else if parent_ref.hasattr(source_name.as_str())? {
                parent_ref.getattr(source_name.as_str())?.unbind()
            } else if parent_ref.hasattr("__getitem__")? {
                parent_ref.get_item(source_name.as_str())?.unbind()
            } else {
                py.None()
            };
            let is_awaitable = value.bind(py).hasattr("__await__")?;
            Ok((is_awaitable, value))
        });

        let (is_awaitable, value) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err, debug)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(value.into_bound(py))
            })
            .map_err(|err| py_err_to_error(err, debug))?
            .await
            .map_err(|err| py_err_to_error(err, debug))?;
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    &awaited.bind(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
            .map_err(|err| py_err_to_error(err, debug))
            .map(Some)
        } else {
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    &value.bind(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
            .map_err(|err| py_err_to_error(err, debug))
            .map(Some)
        }
    }
}

async fn resolve_subscription_field<'a>(
    ctx: ResolverContext<'a>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: Arc<String>,
    source_name: Arc<String>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
    debug: bool,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let root_value = ctx.data::<RootValue>().ok().map(|root| root.0.clone());
    let parent = ctx
        .parent_value
        .try_downcast_ref::<PyObj>()
        .ok()
        .cloned()
        .or_else(|| root_value.clone());
    let context = ctx
        .data::<ContextValue>()
        .ok()
        .map(|ctx| ctx.0.clone());

    let result = if let Some(resolver) = resolver {
        let result = Python::with_gil(|py| -> PyResult<(bool, Py<PyAny>)> {
            let kwargs = build_kwargs(py, &ctx, &arg_names)?;
            let info = PyDict::new(py);
            info.set_item("field_name", field_name.as_str())?;
            if let Some(ctx_obj) = context.as_ref() {
                info.set_item("context", ctx_obj.inner.bind(py))?;
            } else {
                info.set_item("context", py.None())?;
            }
            if let Some(root_obj) = root_value.as_ref() {
                info.set_item("root", root_obj.inner.bind(py))?;
            } else {
                info.set_item("root", py.None())?;
            }
            let parent_obj = match parent.as_ref() {
                Some(parent) => parent.inner.clone_ref(py),
                None => py.None(),
            };
            let args = PyTuple::new(py, [parent_obj, info.into_any().unbind()])?;
            let result = resolver.inner.call(py, args, Some(&kwargs))?;
            let is_awaitable = result.bind(py).hasattr("__await__")?;
            Ok((is_awaitable, result))
        });

        let (is_awaitable, result) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err, debug)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(result.into_bound(py))
            })
            .map_err(|err| py_err_to_error(err, debug))?
            .await
            .map_err(|err| py_err_to_error(err, debug))?;
            awaited
        } else {
            result
        }
    } else {
        let parent = parent.ok_or_else(|| Error::new("No parent value for field"))?;
        let result = Python::with_gil(|py| -> PyResult<(bool, Py<PyAny>)> {
            let parent_ref = parent.inner.bind(py);
            let value = if let Ok(dict) = parent_ref.downcast::<PyDict>() {
                match dict.get_item(source_name.as_str())? {
                    Some(item) => item.unbind(),
                    None => py.None(),
                }
            } else if parent_ref.hasattr(source_name.as_str())? {
                parent_ref.getattr(source_name.as_str())?.unbind()
            } else if parent_ref.hasattr("__getitem__")? {
                parent_ref.get_item(source_name.as_str())?.unbind()
            } else {
                py.None()
            };
            let is_awaitable = value.bind(py).hasattr("__await__")?;
            Ok((is_awaitable, value))
        });

        let (is_awaitable, value) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err, debug)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(value.into_bound(py))
            })
            .map_err(|err| py_err_to_error(err, debug))?
            .await
            .map_err(|err| py_err_to_error(err, debug))?;
            awaited
        } else {
            value
        }
    };

    let iterator = Python::with_gil(|py| -> PyResult<PyObj> {
        let value_ref = result.bind(py);
        if value_ref.hasattr("__aiter__")? {
            let iter = value_ref.call_method0("__aiter__")?;
            Ok(PyObj {
                inner: iter.unbind(),
            })
        } else if value_ref.hasattr("__anext__")? {
            Ok(PyObj {
                inner: result.clone_ref(py),
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "Subscription resolver must return an async iterator",
            ))
        }
    })
    .map_err(|err| py_err_to_error(err, debug))?;

    let scalar_bindings = scalar_bindings.clone();
    let output_type = output_type.clone();
    let abstract_types = abstract_types.clone();
    let stream = stream::unfold(Some(iterator), move |state| {
        let scalar_bindings = scalar_bindings.clone();
        let output_type = output_type.clone();
        let abstract_types = abstract_types.clone();
        async move {
            let iterator = match state {
                Some(iterator) => iterator,
                None => return None,
            };

            let awaitable = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
                let awaitable = iterator.inner.bind(py).call_method0("__anext__")?;
                Ok(awaitable.unbind())
            });
            let awaitable = match awaitable {
                Ok(value) => value,
                Err(err) => return Some((Err(py_err_to_error(err, debug)), None)),
            };

            let awaited = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(awaitable.into_bound(py))
            });
            let awaited = match awaited {
                Ok(fut) => fut.await,
                Err(err) => return Some((Err(py_err_to_error(err, debug)), None)),
            };

            let next_value = match awaited {
                Ok(value) => value,
                Err(err) => {
                    let is_stop =
                        Python::with_gil(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                    if is_stop {
                        return None;
                    }
                    return Some((Err(py_err_to_error(err, debug)), None));
                }
            };

            let value = match Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    &next_value.bind(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            }) {
                Ok(value) => value,
                Err(err) => return Some((Err(py_err_to_error(err, debug)), None)),
            };
            let value: FieldValue<'a> = value;

            Some((Ok(value), Some(iterator)))
        }
    });

    let stream: BoxStream<'a, Result<FieldValue<'a>, Error>> = stream.boxed();
    Ok(stream)
}
