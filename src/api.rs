use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_graphql::dynamic::Schema;
use async_graphql::futures_util::stream::{BoxStream, StreamExt};
use async_graphql::parser::{parse_query, types::OperationType};
use async_graphql::{Request, Variables};
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use tokio::sync::Mutex;

use crate::build::build_schema;
use crate::errors::runtime_threads_conflict;
use crate::parse::parse_schema_plan;
use crate::runtime::future_into_py;
use crate::types::{PyObj, ScalarBinding, StateValue};
use crate::values::{py_to_const_value, response_to_py};

#[pyclass(module = "grommet._core", name = "Schema")]
pub(crate) struct SchemaWrapper {
    schema: Arc<Schema>,
    scalars: Arc<Vec<ScalarBinding>>,
}

impl SchemaWrapper {
    fn convert_variables(
        &self,
        variables: Option<Py<PyAny>>,
    ) -> PyResult<Option<async_graphql::Value>> {
        match variables {
            Some(vars) => Python::attach(|py| {
                let bound = vars.bind(py);
                py_to_const_value(py, &bound, self.scalars.as_ref())
            })
            .map(Some),
            None => Ok(None),
        }
    }

    fn build_request(
        &self,
        query: String,
        variables: Option<Py<PyAny>>,
        state: Option<Py<PyAny>>,
    ) -> PyResult<Request> {
        let vars_value = self.convert_variables(variables)?;
        let mut request = Request::new(query);
        if let Some(vars) = vars_value {
            request = request.variables(Variables::from_value(vars));
        }
        if let Some(obj) = state {
            request = request.data(StateValue(PyObj::new(obj)));
        }
        Ok(request)
    }

    fn is_subscription(query: &str) -> bool {
        let Ok(doc) = parse_query(query) else {
            return false;
        };
        for (_name, op) in doc.operations.iter() {
            if op.node.ty == OperationType::Subscription {
                return true;
            }
        }
        false
    }
}

#[pymethods]
impl SchemaWrapper {
    #[new]
    fn new(py: Python, plan: &Bound<'_, PyAny>) -> PyResult<Self> {
        let (schema_def, type_defs, scalar_defs, enum_defs, union_defs, resolver_map, bindings) =
            parse_schema_plan(py, plan)?;
        let scalar_bindings = Arc::new(bindings);
        let schema = build_schema(
            schema_def,
            type_defs,
            scalar_defs,
            enum_defs,
            union_defs,
            resolver_map,
            scalar_bindings.clone(),
        )?;
        Ok(SchemaWrapper {
            schema: Arc::new(schema),
            scalars: scalar_bindings,
        })
    }

    fn as_sdl(&self) -> PyResult<String> {
        Ok(self.schema.sdl())
    }

    fn execute<'py>(
        &self,
        py: Python<'py>,
        query: String,
        variables: Option<Py<PyAny>>,
        state: Option<Py<PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let is_sub = Self::is_subscription(&query);
        let request = self.build_request(query, variables, state)?;
        let schema = self.schema.clone();

        if is_sub {
            future_into_py(py, async move {
                let stream = schema.execute_stream(request);
                let sub_stream = SubscriptionStream {
                    stream: Arc::new(Mutex::new(Some(stream))),
                    closed: Arc::new(AtomicBool::new(false)),
                };
                Python::attach(|py| Ok(sub_stream.into_pyobject(py)?.into_any().unbind()))
            })
        } else {
            future_into_py(py, async move {
                let response = schema.execute(request).await;
                Python::attach(|py| response_to_py(py, response))
            })
        }
    }
}

#[pyclass(module = "grommet._core", name = "SubscriptionStream")]
pub(crate) struct SubscriptionStream {
    stream: Arc<Mutex<Option<BoxStream<'static, async_graphql::Response>>>>,
    closed: Arc<AtomicBool>,
}

#[pymethods]
impl SubscriptionStream {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let stream = self.stream.clone();
        let closed = self.closed.clone();
        let awaitable = future_into_py(py, async move {
            if closed.load(Ordering::SeqCst) {
                return Err(PyErr::new::<PyStopAsyncIteration, _>(""));
            }
            let mut guard = stream.lock().await;
            let Some(stream) = guard.as_mut() else {
                return Err(PyErr::new::<PyStopAsyncIteration, _>(""));
            };
            match stream.next().await {
                Some(response) => Python::attach(|py| response_to_py(py, response)),
                None => Err(PyErr::new::<PyStopAsyncIteration, _>("")),
            }
        })?;
        Ok(Some(awaitable))
    }

    fn aclose<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let stream = self.stream.clone();
        let closed = self.closed.clone();
        future_into_py(py, async move {
            closed.store(true, Ordering::SeqCst);
            let mut guard = stream.lock().await;
            *guard = None;
            Ok(Python::attach(|py| py.None()))
        })
    }
}

#[pyfunction]
#[pyo3(signature = (use_current_thread=false, worker_threads=None))]
pub(crate) fn configure_runtime(
    use_current_thread: bool,
    worker_threads: Option<usize>,
) -> PyResult<bool> {
    if use_current_thread && worker_threads.is_some() {
        return Err(runtime_threads_conflict());
    }
    let mut builder = if use_current_thread {
        tokio::runtime::Builder::new_current_thread()
    } else {
        tokio::runtime::Builder::new_multi_thread()
    };
    builder.enable_all();
    if let Some(threads) = worker_threads {
        builder.worker_threads(threads);
    }
    pyo3_async_runtimes::tokio::init(builder);
    Ok(true)
}
