use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_graphql::dynamic::Schema;
use async_graphql::futures_util::stream::{BoxStream, StreamExt};
use async_graphql::{Request, Variables};
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use tokio::sync::Mutex;

use crate::build::build_schema;
use crate::errors::runtime_threads_conflict;
use crate::parse::{parse_resolvers, parse_scalar_bindings, parse_schema_definition};
use crate::runtime::future_into_py;
use crate::types::{ContextValue, PyObj, RootValue, ScalarBinding};
use crate::values::{py_to_const_value, response_to_py};

#[pyclass(module = "grommet._core", name = "Schema")]
pub(crate) struct SchemaWrapper {
    schema: Arc<Schema>,
    scalars: Arc<Vec<ScalarBinding>>,
}

#[pymethods]
impl SchemaWrapper {
    #[new]
    #[pyo3(signature = (definition, resolvers=None, scalars=None))]
    fn new(
        py: Python,
        definition: &Bound<'_, PyAny>,
        resolvers: Option<&Bound<'_, PyDict>>,
        scalars: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let (schema_def, type_defs, scalar_defs, enum_defs, union_defs) =
            parse_schema_definition(py, definition)?;
        let resolver_map = parse_resolvers(py, resolvers)?;
        let scalar_bindings = Arc::new(parse_scalar_bindings(py, scalars)?);
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

    fn sdl(&self) -> PyResult<String> {
        Ok(self.schema.sdl())
    }

    fn execute<'py>(
        &self,
        py: Python<'py>,
        query: String,
        variables: Option<Py<PyAny>>,
        root: Option<Py<PyAny>>,
        context: Option<Py<PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vars_value = if let Some(vars) = variables {
            let value = Python::attach(|py| {
                let bound = vars.bind(py);
                py_to_const_value(py, &bound, self.scalars.as_ref())
            })?;
            Some(value)
        } else {
            None
        };
        let root_value = root.map(|obj| RootValue(PyObj::new(obj)));
        let context_value = context.map(|obj| ContextValue(PyObj::new(obj)));
        let schema = self.schema.clone();

        future_into_py(py, async move {
            let mut request = Request::new(query);
            if let Some(vars) = vars_value {
                request = request.variables(Variables::from_value(vars));
            }
            if let Some(root) = root_value {
                request = request.data(root);
            }
            if let Some(ctx) = context_value {
                request = request.data(ctx);
            }
            let response = schema.execute(request).await;
            Python::attach(|py| response_to_py(py, response))
        })
    }

    fn subscribe(
        &self,
        _py: Python,
        query: String,
        variables: Option<Py<PyAny>>,
        root: Option<Py<PyAny>>,
        context: Option<Py<PyAny>>,
    ) -> PyResult<SubscriptionStream> {
        let vars_value = if let Some(vars) = variables {
            let value = Python::attach(|py| {
                let bound = vars.bind(py);
                py_to_const_value(py, &bound, self.scalars.as_ref())
            })?;
            Some(value)
        } else {
            None
        };
        let root_value = root.map(|obj| RootValue(PyObj::new(obj)));
        let context_value = context.map(|obj| ContextValue(PyObj::new(obj)));

        let mut request = Request::new(query);
        if let Some(vars) = vars_value {
            request = request.variables(Variables::from_value(vars));
        }
        if let Some(root) = root_value {
            request = request.data(root);
        }
        if let Some(ctx) = context_value {
            request = request.data(ctx);
        }

        let stream = self.schema.execute_stream(request);
        Ok(SubscriptionStream {
            stream: Arc::new(Mutex::new(Some(stream))),
            closed: Arc::new(AtomicBool::new(false)),
        })
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
