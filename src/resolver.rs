use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use async_graphql::Error;
use async_graphql::dynamic::{FieldValue, ResolverContext, TypeRef};
use async_graphql::futures_util::stream::{self, BoxStream, StreamExt};
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyCFunction, PyDict, PyTupleMethods};

use crate::errors::{py_err_to_error, subscription_requires_async_iterator};
use crate::lookahead::extract_graph;
use crate::types::{FieldContext, PyObj, ResolverEntry, StateValue};
use crate::values::{py_to_field_value_for_type, value_to_py_bound};

type BoxFut = Pin<Box<dyn Future<Output = PyResult<Py<PyAny>>> + Send>>;

struct AwaitableState {
    started: bool,
    task: Option<Py<PyAny>>,
    result: Option<PyResult<Py<PyAny>>>,
    waker: Option<Waker>,
}

struct PythonAwaitableFuture {
    awaitable: Py<PyAny>,
    state: Arc<Mutex<AwaitableState>>,
}

impl PythonAwaitableFuture {
    fn new(awaitable: Py<PyAny>) -> Self {
        Self {
            awaitable,
            state: Arc::new(Mutex::new(AwaitableState {
                started: false,
                task: None,
                result: None,
                waker: None,
            })),
        }
    }

    fn start(&self) -> PyResult<()> {
        let callback_state = Arc::clone(&self.state);

        let task = Python::attach(|py| -> PyResult<Py<PyAny>> {
            let asyncio = py.import("asyncio")?;
            let task = match asyncio.call_method1("ensure_future", (self.awaitable.bind(py),)) {
                Ok(task) => task,
                Err(err) => {
                    let _ = self.awaitable.bind(py).call_method0("close");
                    return Err(err);
                }
            };
            let callback = PyCFunction::new_closure(
                py,
                Some(c"grommet_awaitable_done"),
                None,
                move |args, _kwargs| -> PyResult<()> {
                    let task = args.get_item(0)?;
                    let result = if task.call_method0("cancelled")?.is_truthy()? {
                        let cancelled = task
                            .py()
                            .import("asyncio")?
                            .getattr("CancelledError")?
                            .call0()?;
                        Err(PyErr::from_value(cancelled))
                    } else {
                        match task.call_method0("result") {
                            Ok(value) => Ok(value.unbind()),
                            Err(err) => Err(err),
                        }
                    };

                    let mut shared = callback_state.lock().expect("awaitable state poisoned");
                    shared.task = None;
                    shared.result = Some(result);
                    if let Some(waker) = shared.waker.take() {
                        waker.wake();
                    }
                    Ok(())
                },
            )?;
            task.call_method1("add_done_callback", (callback,))?;
            Ok(task.unbind())
        })?;

        let mut shared = self.state.lock().expect("awaitable state poisoned");
        shared.task = Some(task);
        Ok(())
    }
}

impl Future for PythonAwaitableFuture {
    type Output = PyResult<Py<PyAny>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let should_start = {
            let mut shared = self.state.lock().expect("awaitable state poisoned");
            if let Some(result) = shared.result.take() {
                return Poll::Ready(result);
            }
            shared.waker = Some(cx.waker().clone());
            if shared.started {
                false
            } else {
                shared.started = true;
                true
            }
        };

        if should_start && let Err(err) = self.start() {
            let mut shared = self.state.lock().expect("awaitable state poisoned");
            shared.result = Some(Err(err));
            if let Some(waker) = shared.waker.take() {
                waker.wake();
            }
        }

        Poll::Pending
    }
}

impl Drop for PythonAwaitableFuture {
    fn drop(&mut self) {
        let task = {
            let mut shared = self.state.lock().expect("awaitable state poisoned");
            shared.task.take()
        };
        if let Some(task) = task {
            Python::attach(|py| {
                let _ = task.bind(py).call_method0("cancel");
            });
        }
    }
}

fn awaitable_into_future(awaitable: Bound<'_, PyAny>) -> BoxFut {
    Box::pin(PythonAwaitableFuture::new(awaitable.unbind()))
}

// Synchronous fast-path for all sync fields (data fields via attrgetter and sync resolvers).
// Single GIL block: call func + convert. No async overhead, no task scheduling.
pub(crate) fn resolve_field_sync_fast<'a>(
    ctx: &ResolverContext<'a>,
    field_ctx: &FieldContext,
) -> Result<FieldValue<'a>, Error> {
    let entry = field_ctx.resolver.as_ref().expect("resolver missing");
    Python::attach(|py| {
        let result = call_resolver_sync(py, ctx, field_ctx, entry)?;
        py_to_field_value_for_type(py, result.bind(py), &field_ctx.output_type)
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
    let field_value =
        Python::attach(|py| py_to_field_value_for_type(py, value.bind(py), &field_ctx.output_type))
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
    let stream = stream::try_unfold(iterator, move |iterator| {
        let output_type = output_type.clone();
        async move {
            let next_fut: BoxFut = Python::attach(|py| {
                let anext = iterator.bind(py).call_method0("__anext__")?;
                Ok(awaitable_into_future(anext))
            })
            .map_err(py_err_to_error)?;

            match next_fut.await {
                Ok(value) => {
                    let value = Python::attach(|py| {
                        py_to_field_value_for_type(py, value.bind(py), &output_type)
                    })
                    .map_err(py_err_to_error)?;
                    let value: FieldValue<'a> = value;
                    Ok(Some((value, iterator)))
                }
                Err(err) => {
                    let is_stop =
                        Python::attach(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                    if is_stop {
                        Ok(None)
                    } else {
                        Err(py_err_to_error(err))
                    }
                }
            }
        }
    });

    Ok(stream.boxed())
}

// Resolve a field that has an async resolver entry.
// Merges call_resolver + into_future into a single GIL block for async resolvers.
async fn resolve_with_resolver(
    ctx: &ResolverContext<'_>,
    field_ctx: &FieldContext,
    entry: &ResolverEntry,
) -> Result<Py<PyAny>, Error> {
    // Lazy state extraction: only look up state when the resolver needs context
    let state = if entry.needs_context {
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
    } else {
        // Async coroutine: call resolver + set up future in one GIL block
        let future: BoxFut = Python::attach(|py| {
            let coroutine =
                call_resolver(py, ctx, field_ctx, entry, parent.as_ref(), state.as_ref())?;
            let bound = coroutine.into_bound(py);
            Ok(awaitable_into_future(bound))
        })
        .map_err(py_err_to_error)?;
        future.await.map_err(py_err_to_error)
    }
}

// Synchronous resolver call for the sync fast-path. Single GIL block, vectorcall-optimized.
fn call_resolver_sync(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    field_ctx: &FieldContext,
    entry: &ResolverEntry,
) -> PyResult<Py<PyAny>> {
    let parent = ctx.parent_value.try_downcast_ref::<PyObj>().ok().cloned();
    let state = if entry.needs_context {
        ctx.data::<StateValue>().ok().map(|s| s.0.clone())
    } else {
        None
    };
    call_resolver(py, ctx, field_ctx, entry, parent.as_ref(), state.as_ref())
}

fn build_context_obj(
    py: Python<'_>,
    ctx: &ResolverContext<'_>,
    state: Option<&PyObj>,
    context_cls: &PyObj,
) -> PyResult<Py<PyAny>> {
    let graph = extract_graph(ctx);
    let graph_py = graph.into_pyobject(py)?.into_any().unbind();
    let state_py = match state {
        Some(s) => s.clone_ref(py),
        None => py.None(),
    };
    let kwargs = PyDict::new(py);
    kwargs.set_item("graph", graph_py)?;
    kwargs.set_item("state", state_py)?;
    let context_obj = context_cls.bind(py).call((), Some(&kwargs))?;
    Ok(context_obj.unbind())
}

fn build_kwargs<'py>(py: Python<'py>, ctx: &ResolverContext<'_>) -> PyResult<Bound<'py, PyDict>> {
    let kwargs = PyDict::new(py);
    for (name, value) in ctx.args.iter() {
        let py_value = value_to_py_bound(py, value.as_value())?;
        kwargs.set_item(name.as_str(), py_value)?;
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
    let parent_obj: Py<PyAny> = match parent {
        Some(p) => p.clone_ref(py),
        None => py.None(),
    };
    let context_obj: Py<PyAny> = if entry.needs_context {
        let cls = field_ctx.context_cls.as_ref().expect("context_cls missing");
        build_context_obj(py, ctx, state, cls)?
    } else {
        py.None()
    };
    let kwargs = build_kwargs(py, ctx)?;
    let func = entry.func.bind(py);
    Ok(func.call1((parent_obj, context_obj, kwargs))?.unbind())
}
