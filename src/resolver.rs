use std::collections::HashSet;
use std::sync::Arc;

use async_graphql::Error;
use async_graphql::dynamic::{FieldValue, ResolverContext, TypeRef};
use async_graphql::futures_util::stream::{BoxStream, StreamExt};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyTuple};
use pyo3_async_runtimes::tokio;

use crate::errors::{no_parent_value, py_err_to_error, subscription_requires_async_iterator};
use crate::runtime::await_awaitable;
use crate::types::{ContextValue, PyObj, RootValue, ScalarBinding};
use crate::values::{build_kwargs, py_to_field_value_for_type};

pub(crate) async fn resolve_field(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: Arc<String>,
    source_name: Arc<String>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
) -> Result<Option<FieldValue<'_>>, Error> {
    let value = resolve_python_value(ctx, resolver, arg_names, &field_name, &source_name).await?;
    let field_value = Python::attach(|py| {
        py_to_field_value_for_type(
            py,
            value.bind(py),
            &output_type,
            scalar_bindings.as_ref(),
            abstract_types.as_ref(),
        )
    })
    .map_err(py_err_to_error)?;
    Ok(Some(field_value))
}

pub(crate) async fn resolve_subscription_stream<'a>(
    ctx: ResolverContext<'a>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: Arc<String>,
    source_name: Arc<String>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let value =
        resolve_subscription_value(ctx, resolver, arg_names, &field_name, &source_name).await?;
    let iterator =
        Python::attach(|py| subscription_iterator(value.bind(py))).map_err(py_err_to_error)?;
    subscription_stream(iterator, scalar_bindings, output_type, abstract_types)
}

fn subscription_iterator(value_ref: &Bound<'_, PyAny>) -> PyResult<PyObj> {
    if value_ref.hasattr("__aiter__")? {
        let iter = value_ref.call_method0("__aiter__")?;
        Ok(PyObj::new(iter.unbind()))
    } else if value_ref.hasattr("__anext__")? {
        Ok(PyObj::new(value_ref.clone().unbind()))
    } else {
        Err(subscription_requires_async_iterator())
    }
}

fn subscription_stream<'a>(
    iterator: PyObj,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let stream =
        Python::attach(|py| tokio::into_stream_v1(iterator.bind(py))).map_err(py_err_to_error)?;
    let stream = stream.map(move |item| match item {
        Ok(value) => {
            let value = match Python::attach(|py| {
                py_to_field_value_for_type(
                    py,
                    value.bind(py),
                    &output_type,
                    scalar_bindings.as_ref(),
                    abstract_types.as_ref(),
                )
            }) {
                Ok(value) => value,
                Err(err) => return Err(py_err_to_error(err)),
            };
            let value: FieldValue<'a> = value;
            Ok(value)
        }
        Err(err) => Err(py_err_to_error(err)),
    });

    Ok(stream.boxed())
}

async fn resolve_python_value(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: &str,
    source_name: &str,
) -> Result<Py<PyAny>, Error> {
    let (root_value, context, parent) = extract_context(&ctx);

    let has_resolver = resolver.is_some();
    let value = if let Some(resolver) = resolver {
        Python::attach(|py| {
            call_resolver(
                py,
                &ctx,
                &resolver,
                &arg_names,
                field_name,
                parent.as_ref(),
                root_value.as_ref(),
                context.as_ref(),
            )
        })
        .map_err(py_err_to_error)?
    } else {
        let parent = parent.ok_or_else(no_parent_value)?;
        Python::attach(|py| resolve_from_parent(py, &parent, source_name))
            .map_err(py_err_to_error)?
    };

    if has_resolver {
        await_value(value).await
    } else {
        Ok(value)
    }
}

async fn resolve_subscription_value(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: &str,
    source_name: &str,
) -> Result<Py<PyAny>, Error> {
    let (root_value, context, parent) = extract_context(&ctx);

    let (value, has_resolver) = if let Some(resolver) = resolver {
        let value = Python::attach(|py| {
            call_resolver(
                py,
                &ctx,
                &resolver,
                &arg_names,
                field_name,
                parent.as_ref(),
                root_value.as_ref(),
                context.as_ref(),
            )
        })
        .map_err(py_err_to_error)?;
        (value, true)
    } else {
        let parent = parent.ok_or_else(no_parent_value)?;
        let value = Python::attach(|py| resolve_from_parent(py, &parent, source_name))
            .map_err(py_err_to_error)?;
        (value, false)
    };

    if has_resolver {
        let (is_async_iter, is_awaitable) = Python::attach(|py| {
            let bound = value.bind(py);
            let is_async_iter = bound.hasattr("__aiter__")? || bound.hasattr("__anext__")?;
            let is_awaitable = bound.hasattr("__await__")?;
            Ok((is_async_iter, is_awaitable))
        })
        .map_err(py_err_to_error)?;
        if !is_async_iter && is_awaitable {
            return await_value(value).await;
        }
    }

    Ok(value)
}

fn extract_context(ctx: &ResolverContext<'_>) -> (Option<PyObj>, Option<PyObj>, Option<PyObj>) {
    let root_value = ctx.data::<RootValue>().ok().map(|root| root.0.clone());
    let context = ctx.data::<ContextValue>().ok().map(|ctx| ctx.0.clone());
    let parent = ctx
        .parent_value
        .try_downcast_ref::<PyObj>()
        .ok()
        .cloned()
        .or_else(|| root_value.clone());
    (root_value, context, parent)
}

fn call_resolver(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    resolver: &PyObj,
    arg_names: &[String],
    field_name: &str,
    parent: Option<&PyObj>,
    root_value: Option<&PyObj>,
    context: Option<&PyObj>,
) -> PyResult<Py<PyAny>> {
    let kwargs = build_kwargs(py, ctx, arg_names)?;
    let info = PyDict::new(py);
    info.set_item("field_name", field_name)?;
    if let Some(ctx_obj) = context {
        info.set_item("context", ctx_obj.bind(py))?;
    } else {
        info.set_item("context", py.None())?;
    }
    if let Some(root_obj) = root_value {
        info.set_item("root", root_obj.bind(py))?;
    } else {
        info.set_item("root", py.None())?;
    }
    let parent_obj = match parent {
        Some(parent) => parent.clone_ref(py),
        None => py.None(),
    };
    let args = PyTuple::new(py, [parent_obj, info.into_any().unbind()])?;
    let result = resolver.clone_ref(py).call(py, args, Some(&kwargs))?;
    Ok(result)
}

fn resolve_from_parent(py: Python<'_>, parent: &PyObj, source_name: &str) -> PyResult<Py<PyAny>> {
    let parent_ref = parent.bind(py);
    let value = if let Ok(dict) = parent_ref.cast::<PyDict>() {
        match dict.get_item(source_name)? {
            Some(item) => item.unbind(),
            None => py.None(),
        }
    } else if parent_ref.hasattr(source_name)? {
        parent_ref.getattr(source_name)?.unbind()
    } else if parent_ref.hasattr("__getitem__")? {
        parent_ref.get_item(source_name)?.unbind()
    } else {
        py.None()
    };
    Ok(value)
}

async fn await_value(value: Py<PyAny>) -> Result<Py<PyAny>, Error> {
    await_awaitable(value).await
}
