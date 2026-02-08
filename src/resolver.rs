use std::sync::Arc;

use async_graphql::Error;
use async_graphql::dynamic::{FieldValue, ResolverContext, TypeRef};
use async_graphql::futures_util::stream::{BoxStream, StreamExt};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyTuple};
use pyo3_async_runtimes::tokio;

use crate::errors::{no_parent_value, py_err_to_error, subscription_requires_async_iterator};
use crate::lookahead::extract_lookahead;
use crate::runtime::await_awaitable;
use crate::types::{FieldContext, PyObj, StateValue};
use crate::values::{build_kwargs, py_to_field_value_for_type};

pub(crate) async fn resolve_field(
    ctx: ResolverContext<'_>,
    field_ctx: Arc<FieldContext>,
) -> Result<Option<FieldValue<'_>>, Error> {
    let value = resolve_value(
        ctx,
        field_ctx.resolver.clone(),
        &field_ctx.arg_names,
        &field_ctx.source_name,
    )
    .await?;
    let field_value =
        Python::attach(|py| py_to_field_value_for_type(py, value.bind(py), &field_ctx.output_type))
            .map_err(py_err_to_error)?;
    Ok(Some(field_value))
}

pub(crate) async fn resolve_subscription_stream<'a>(
    ctx: ResolverContext<'a>,
    field_ctx: Arc<FieldContext>,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let value = resolve_value(
        ctx,
        field_ctx.resolver.clone(),
        &field_ctx.arg_names,
        &field_ctx.source_name,
    )
    .await?;
    let iterator =
        Python::attach(|py| subscription_iterator(value.bind(py))).map_err(py_err_to_error)?;
    subscription_stream(iterator, field_ctx.output_type.clone())
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
    output_type: TypeRef,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let stream =
        Python::attach(|py| tokio::into_stream_v1(iterator.bind(py))).map_err(py_err_to_error)?;
    let stream = stream.map(move |item| match item {
        Ok(value) => {
            let value = match Python::attach(|py| {
                py_to_field_value_for_type(py, value.bind(py), &output_type)
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

async fn resolve_value(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: &[String],
    source_name: &str,
) -> Result<Py<PyAny>, Error> {
    let (state, parent) = extract_state(&ctx);

    if let Some(resolver) = resolver {
        let coroutine = Python::attach(|py| {
            call_resolver(
                py,
                &ctx,
                &resolver,
                arg_names,
                parent.as_ref(),
                state.as_ref(),
            )
        })
        .map_err(py_err_to_error)?;
        await_awaitable(coroutine).await
    } else {
        let parent = parent.ok_or_else(no_parent_value)?;
        Python::attach(|py| resolve_from_parent(py, &parent, source_name)).map_err(py_err_to_error)
    }
}

fn extract_state(ctx: &ResolverContext<'_>) -> (Option<PyObj>, Option<PyObj>) {
    let state = ctx.data::<StateValue>().ok().map(|s| s.0.clone());
    let parent = ctx.parent_value.try_downcast_ref::<PyObj>().ok().cloned();
    (state, parent)
}

fn build_context_obj(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    state: Option<&PyObj>,
) -> PyResult<Py<PyAny>> {
    let lookahead = extract_lookahead(ctx);
    let lookahead_py = lookahead.into_pyobject(py)?.into_any().unbind();
    let state_py = match state {
        Some(s) => s.clone_ref(py),
        None => py.None(),
    };
    let context_cls = py.import("grommet.context")?.getattr("Context")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("_lookahead", lookahead_py)?;
    kwargs.set_item("state", state_py)?;
    let context_obj = context_cls.call((), Some(&kwargs))?;
    Ok(context_obj.unbind())
}

fn call_resolver(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    resolver: &PyObj,
    arg_names: &[String],
    parent: Option<&PyObj>,
    state: Option<&PyObj>,
) -> PyResult<Py<PyAny>> {
    let kwargs = build_kwargs(py, ctx, arg_names)?;
    let context_obj = build_context_obj(py, ctx, state)?;
    let parent_obj = match parent {
        Some(parent) => parent.clone_ref(py),
        None => py.None(),
    };
    let args = PyTuple::new(py, [parent_obj, context_obj])?;
    let result = resolver.clone_ref(py).call(py, args, Some(&kwargs))?;
    Ok(result)
}

fn resolve_from_parent(py: Python<'_>, parent: &PyObj, source_name: &str) -> PyResult<Py<PyAny>> {
    let parent_ref = parent.bind(py);
    if parent_ref.hasattr(source_name)? {
        Ok(parent_ref.getattr(source_name)?.unbind())
    } else {
        Ok(py.None())
    }
}
