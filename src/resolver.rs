use std::collections::HashSet;
use std::sync::Arc;

use async_graphql::dynamic::{FieldValue, ResolverContext, TypeRef};
use async_graphql::futures_util::stream::{self, BoxStream, StreamExt};
use async_graphql::Error;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyTuple};

use crate::errors::{
    no_parent_value, py_err_to_error, py_type_error, subscription_requires_async_iterator,
};
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
    let value = resolve_python_value(ctx, resolver, arg_names, &field_name, &source_name).await?;
    let iterator =
        Python::attach(|py| subscription_iterator(value.bind(py))).map_err(py_err_to_error)?;
    Ok(subscription_stream(
        iterator,
        scalar_bindings,
        output_type,
        abstract_types,
    ))
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
) -> BoxStream<'a, Result<FieldValue<'a>, Error>> {
    let stream = stream::unfold(Some(iterator), move |state| {
        let scalar_bindings = scalar_bindings.clone();
        let output_type = output_type.clone();
        let abstract_types = abstract_types.clone();
        async move {
            let iterator = match state {
                Some(iterator) => iterator,
                None => return None,
            };

            let awaitable = Python::attach(|py| -> PyResult<Py<PyAny>> {
                let awaitable = iterator.bind(py).call_method0("__anext__")?;
                Ok(awaitable.unbind())
            });
            let awaitable = match awaitable {
                Ok(value) => value,
                Err(err) => return Some((Err(py_err_to_error(err)), None)),
            };

            let awaited = Python::attach(|py| {
                let awaitable = awaitable.into_bound(py);
                if !awaitable.hasattr("__await__")? {
                    return Err(py_type_error(
                        "Subscription iterator __anext__ must return awaitable",
                    ));
                }
                pyo3_async_runtimes::tokio::into_future(awaitable)
            });
            let awaited = match awaited {
                Ok(fut) => fut.await,
                Err(err) => return Some((Err(py_err_to_error(err)), None)),
            };

            let next_value = match awaited {
                Ok(value) => value,
                Err(err) => {
                    let is_stop =
                        Python::attach(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                    if is_stop {
                        return None;
                    }
                    return Some((Err(py_err_to_error(err)), None));
                }
            };

            let value = match Python::attach(|py| {
                py_to_field_value_for_type(
                    py,
                    next_value.bind(py),
                    &output_type,
                    scalar_bindings.as_ref(),
                    abstract_types.as_ref(),
                )
            }) {
                Ok(value) => value,
                Err(err) => return Some((Err(py_err_to_error(err)), None)),
            };
            let value: FieldValue<'a> = value;

            Some((Ok(value), Some(iterator)))
        }
    });

    stream.boxed()
}

async fn resolve_python_value(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: &str,
    source_name: &str,
) -> Result<Py<PyAny>, Error> {
    let (root_value, context, parent) = extract_context(&ctx);

    let (is_awaitable, value) = if let Some(resolver) = resolver {
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

    if is_awaitable {
        await_value(value).await
    } else {
        Ok(value)
    }
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
) -> PyResult<(bool, Py<PyAny>)> {
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
    let is_awaitable = result.bind(py).hasattr("__await__")?;
    Ok((is_awaitable, result))
}

fn resolve_from_parent(
    py: Python<'_>,
    parent: &PyObj,
    source_name: &str,
) -> PyResult<(bool, Py<PyAny>)> {
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
    let is_awaitable = value.bind(py).hasattr("__await__")?;
    Ok((is_awaitable, value))
}

async fn await_value(value: Py<PyAny>) -> Result<Py<PyAny>, Error> {
    let awaited =
        Python::attach(|py| pyo3_async_runtimes::tokio::into_future(value.into_bound(py)))
            .map_err(py_err_to_error)?
            .await
            .map_err(py_err_to_error)?;
    Ok(awaited)
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use async_graphql::futures_util::stream::StreamExt;
    use pyo3::types::{PyAnyMethods, PyDict, PyStringMethods};
    use std::sync::Arc;

    fn with_py<F, R>(f: F) -> R
    where
        F: for<'py> FnOnce(Python<'py>) -> R,
    {
        Python::initialize();
        Python::attach(f)
    }

    #[test]
    fn subscription_iterator_branches() {
        with_py(|py| {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
async def gen():
    yield 1

class OnlyAnext:
    async def __anext__(self):
        return 1

class NotAsync:
    pass
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();

            let gen_obj = locals.get_item("gen").unwrap().unwrap().call0().unwrap();
            let _ = subscription_iterator(&gen_obj).unwrap();

            let only_anext = locals
                .get_item("OnlyAnext")
                .unwrap()
                .unwrap()
                .call0()
                .unwrap();
            let _ = subscription_iterator(&only_anext).unwrap();

            let not_async = locals
                .get_item("NotAsync")
                .unwrap()
                .unwrap()
                .call0()
                .unwrap();
            let err = subscription_iterator(&not_async).err().unwrap();
            let msg = err.value(py).str().unwrap();
            let msg = msg.to_str().unwrap();
            assert_eq!(msg, "Subscription resolver must return an async iterator");
        });
    }

    #[test]
    fn subscription_stream_error_paths() {
        with_py(|py| {
            let locals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!(
                    r#"
class RaiseInAnext:
    def __anext__(self):
        raise RuntimeError("boom")

class NonAwaitableAnext:
    def __anext__(self):
        return object()

class ErrorAsync:
    async def __anext__(self):
        raise ValueError("bad")

class OnlyAnext:
    async def __anext__(self):
        return 1
"#
                ),
                None,
                Some(&locals),
            )
            .unwrap();

            let raise_iter = PyObj::new(
                locals
                    .get_item("RaiseInAnext")
                    .unwrap()
                    .unwrap()
                    .call0()
                    .unwrap()
                    .unbind(),
            );
            let non_awaitable = PyObj::new(
                locals
                    .get_item("NonAwaitableAnext")
                    .unwrap()
                    .unwrap()
                    .call0()
                    .unwrap()
                    .unbind(),
            );
            let error_async = PyObj::new(
                locals
                    .get_item("ErrorAsync")
                    .unwrap()
                    .unwrap()
                    .call0()
                    .unwrap()
                    .unbind(),
            );
            let only_anext = PyObj::new(
                locals
                    .get_item("OnlyAnext")
                    .unwrap()
                    .unwrap()
                    .call0()
                    .unwrap()
                    .unbind(),
            );

            pyo3_async_runtimes::tokio::run(py, async move {
                use tokio::time::{timeout, Duration};

                let empty_scalars = Arc::new(Vec::new());
                let empty_abstracts = Arc::new(HashSet::new());

                let mut stream = subscription_stream(
                    raise_iter,
                    empty_scalars.clone(),
                    TypeRef::named("Int"),
                    empty_abstracts.clone(),
                );
                let first = timeout(Duration::from_secs(3), stream.next())
                    .await
                    .expect("timeout waiting for raise_iter");
                assert!(matches!(first, Some(Err(_))));
                let second = timeout(Duration::from_secs(3), stream.next())
                    .await
                    .expect("timeout waiting for raise_iter followup");
                assert!(second.is_none());

                let mut stream = subscription_stream(
                    non_awaitable,
                    empty_scalars.clone(),
                    TypeRef::named("Int"),
                    empty_abstracts.clone(),
                );
                let next = timeout(Duration::from_secs(3), stream.next())
                    .await
                    .expect("timeout waiting for non_awaitable");
                assert!(matches!(next, Some(Err(_))));

                let mut stream = subscription_stream(
                    error_async,
                    empty_scalars.clone(),
                    TypeRef::named("Int"),
                    empty_abstracts.clone(),
                );
                let next = timeout(Duration::from_secs(3), stream.next())
                    .await
                    .expect("timeout waiting for error_async");
                assert!(matches!(next, Some(Err(_))));

                let mut stream = subscription_stream(
                    only_anext,
                    empty_scalars.clone(),
                    TypeRef::List(Box::new(TypeRef::named("Int"))),
                    empty_abstracts.clone(),
                );
                let next = timeout(Duration::from_secs(3), stream.next())
                    .await
                    .expect("timeout waiting for only_anext");
                assert!(matches!(next, Some(Err(_))));

                Ok(())
            })
        })
        .unwrap();
    }
}
