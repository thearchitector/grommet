use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_graphql::Error;
use async_graphql::dynamic::{FieldValue, ResolverContext, TypeRef};
use async_graphql::futures_util::stream::{BoxStream, StreamExt};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict};
use pyo3_async_runtimes::tokio;

use crate::errors::{no_parent_value, py_err_to_error, subscription_requires_async_iterator};
use crate::lookahead::extract_lookahead;
use crate::types::{FieldContext, PyObj, ResolverEntry, ResolverShape, ScalarHint, StateValue};
use crate::values::{py_to_field_value_for_type, value_to_py};

type BoxFut = Pin<Box<dyn Future<Output = PyResult<Py<PyAny>>> + Send>>;

// Synchronous field resolution for non-resolver fields (parent attribute access).
// Single GIL block: getattr + convert. No async overhead, no task scheduling.
pub(crate) fn resolve_field_sync<'a>(
    ctx: &ResolverContext<'a>,
    field_ctx: &FieldContext,
) -> Result<FieldValue<'a>, Error> {
    let parent = ctx
        .parent_value
        .try_downcast_ref::<PyObj>()
        .ok()
        .ok_or_else(no_parent_value)?;
    Python::attach(|py| {
        let raw = match parent.bind(py).getattr(&*field_ctx.source_name_py) {
            Ok(val) => val,
            Err(_) => py.None().into_bound(py),
        };
        py_to_field_value_for_type(py, &raw, &field_ctx.output_type, field_ctx.scalar_hint)
    })
    .map_err(py_err_to_error)
}

// Async field resolution for fields with resolvers.
pub(crate) async fn resolve_field(
    ctx: ResolverContext<'_>,
    field_ctx: Arc<FieldContext>,
) -> Result<Option<FieldValue<'_>>, Error> {
        let entry = field_ctx.resolver.as_ref().expect("resolver missing");
        let value = resolve_with_resolver(&ctx, &field_ctx, entry).await?;
        let hint = field_ctx.scalar_hint;
        let field_value = Python::attach(|py| {
            py_to_field_value_for_type(py, value.bind(py), &field_ctx.output_type, hint)
        })
        .map_err(py_err_to_error)?;
        Ok(Some(field_value))
}

pub(crate) async fn resolve_subscription_stream<'a>(
    ctx: ResolverContext<'a>,
    field_ctx: Arc<FieldContext>,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let entry = field_ctx.resolver.as_ref().expect("resolver missing");
    let value = resolve_with_resolver(&ctx, &field_ctx, entry).await?;
    let iterator =
        Python::attach(|py| subscription_iterator(value.bind(py))).map_err(py_err_to_error)?;
    subscription_stream(
        iterator,
        field_ctx.output_type.clone(),
        field_ctx.scalar_hint,
    )
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
    hint: ScalarHint,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let stream =
        Python::attach(|py| tokio::into_stream_v1(iterator.bind(py))).map_err(py_err_to_error)?;
    let stream = stream.map(move |item| match item {
        Ok(value) => {
            let value = match Python::attach(|py| {
                py_to_field_value_for_type(py, value.bind(py), &output_type, hint)
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

// Resolve a field that has a resolver entry.
// Merges call_resolver + into_future into a single GIL block for async resolvers.
// Sync resolvers skip into_future entirely.
async fn resolve_with_resolver(
    ctx: &ResolverContext<'_>,
    field_ctx: &FieldContext,
    entry: &ResolverEntry,
) -> Result<Py<PyAny>, Error> {
    async {
        // Lazy state extraction: only look up state when the resolver needs context
        let needs_state = matches!(
            entry.shape,
            ResolverShape::SelfAndContext | ResolverShape::SelfContextAndArgs
        );
        let state = if needs_state {
            ctx.data::<StateValue>().ok().map(|s| s.0.clone())
        } else {
            None
        };
        let parent = ctx.parent_value.try_downcast_ref::<PyObj>().ok().cloned();

        if entry.is_async_gen {
            // Async generators (subscriptions): call resolver, return generator directly
            Python::attach(|py| {
                call_resolver(py, ctx, field_ctx, entry, parent.as_ref(), state.as_ref())
            })
            .map_err(py_err_to_error)
        } else if entry.is_async {
            // Async coroutine: call resolver + set up future in one GIL block
            let future: BoxFut = Python::attach(|py| {
                let coroutine =
                    call_resolver(py, ctx, field_ctx, entry, parent.as_ref(), state.as_ref())?;
                let bound = coroutine.into_bound(py);
                let fut = tokio::into_future(bound)?;
                Ok(Box::pin(fut) as BoxFut)
            })
            .map_err(py_err_to_error)?;
            future.await.map_err(py_err_to_error)
        } else {
            // Sync resolver: call resolver, return result directly (no into_future)
            Python::attach(|py| {
                call_resolver(py, ctx, field_ctx, entry, parent.as_ref(), state.as_ref())
            })
            .map_err(py_err_to_error)
        }
}

fn build_context_obj(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    state: Option<&PyObj>,
    context_cls: &PyObj,
) -> PyResult<Py<PyAny>> {
    let lookahead = extract_lookahead(ctx);
    let lookahead_py = lookahead.into_pyobject(py)?.into_any().unbind();
    let state_py = match state {
        Some(s) => s.clone_ref(py),
        None => py.None(),
    };
    let kwargs = PyDict::new(py);
    kwargs.set_item("_lookahead", lookahead_py)?;
    kwargs.set_item("state", state_py)?;
    let context_obj = context_cls.bind(py).call((), Some(&kwargs))?;
    Ok(context_obj.unbind())
}

fn build_kwargs_with_coercion<'py>(
    py: Python<'py>,
    ctx: &ResolverContext<'_>,
    coercers: &[(String, Option<PyObj>)],
) -> PyResult<Bound<'py, PyDict>> {
    let kwargs = PyDict::new(py);
    for (name, coercer) in coercers {
        let value = ctx.args.try_get(name.as_str());
        if let Ok(value) = value {
            let py_value = value_to_py(py, value.as_value())?;
            let final_value = match coercer {
                Some(c) => c.bind(py).call1((py_value,))?.unbind(),
                None => py_value,
            };
            kwargs.set_item(name, final_value)?;
        }
    }
    Ok(kwargs)
}

fn call_resolver(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    field_ctx: &FieldContext,
    entry: &ResolverEntry,
    parent: Option<&PyObj>,
    state: Option<&PyObj>,
) -> PyResult<Py<PyAny>> {
    let parent_obj = match parent {
        Some(p) => p.clone_ref(py),
        None => py.None(),
    };
    let func = entry.func.bind(py);
    match entry.shape {
        ResolverShape::SelfOnly => Ok(func.call1((parent_obj,))?.unbind()),
        ResolverShape::SelfAndContext => {
            let cls = field_ctx.context_cls.as_ref().expect("context_cls missing");
            let ctx_obj = build_context_obj(py, ctx, state, cls)?;
            Ok(func.call1((parent_obj, ctx_obj))?.unbind())
        }
        ResolverShape::SelfAndArgs => {
            let kwargs = build_kwargs_with_coercion(py, ctx, &entry.arg_coercers)?;
            Ok(func.call((parent_obj,), Some(&kwargs))?.unbind())
        }
        ResolverShape::SelfContextAndArgs => {
            let cls = field_ctx.context_cls.as_ref().expect("context_cls missing");
            let ctx_obj = build_context_obj(py, ctx, state, cls)?;
            let kwargs = build_kwargs_with_coercion(py, ctx, &entry.arg_coercers)?;
            Ok(func.call((parent_obj, ctx_obj), Some(&kwargs))?.unbind())
        }
    }
}
