use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_graphql::dynamic::Schema;
use async_graphql::futures_util::lock::Mutex;
use async_graphql::futures_util::stream::{BoxStream, StreamExt};
use async_graphql::parser::{parse_query, types::OperationType};
use async_graphql::{Request, Variables};
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;

use crate::schema_types::register_schema;
use crate::types::{ContextValue, PyObj};
use crate::values::{py_to_value, response_to_py};

#[pyclass(module = "grommet._core", name = "Schema")]
pub(crate) struct SchemaWrapper {
    schema: Arc<Schema>,
}

impl SchemaWrapper {
    fn convert_variables(variables: Option<Py<PyAny>>) -> PyResult<Option<async_graphql::Value>> {
        match variables {
            Some(vars) => Python::attach(|py| {
                let bound = vars.bind(py);
                py_to_value(py, bound)
            })
            .map(Some),
            None => Ok(None),
        }
    }

    fn build_request(
        query: String,
        variables: Option<Py<PyAny>>,
        context: Option<Py<PyAny>>,
    ) -> PyResult<Request> {
        let vars_value = Self::convert_variables(variables)?;
        let mut request = Request::new(query);
        if let Some(vars) = vars_value {
            request = request.variables(Variables::from_value(vars));
        }
        if let Some(obj) = context {
            request = request.data(ContextValue(PyObj::new(obj)));
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
    fn new(py: Python, bundle: &Bound<'_, PyAny>) -> PyResult<Self> {
        let query: String = bundle.getattr("query")?.extract()?;
        let mutation: Option<String> = bundle.getattr("mutation")?.extract()?;
        let subscription: Option<String> = bundle.getattr("subscription")?.extract()?;
        let types_list: Vec<Py<PyAny>> = bundle.getattr("types")?.extract()?;

        let schema = register_schema(
            py,
            &query,
            mutation.as_deref(),
            subscription.as_deref(),
            types_list,
        )?;
        Ok(SchemaWrapper {
            schema: Arc::new(schema),
        })
    }

    fn as_sdl(&self) -> PyResult<String> {
        Ok(self.schema.sdl())
    }

    async fn execute(
        &self,
        query: String,
        variables: Option<Py<PyAny>>,
        context: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let is_sub = Self::is_subscription(&query);
        let request = Self::build_request(query, variables, context)?;
        let schema = self.schema.clone();

        if is_sub {
            let stream = schema.execute_stream(request);
            let sub_stream = SubscriptionStream {
                stream: Arc::new(Mutex::new(Some(stream))),
                closed: Arc::new(AtomicBool::new(false)),
            };
            Python::attach(|py| Ok(sub_stream.into_pyobject(py)?.into_any().unbind()))
        } else {
            let response = schema.execute(request).await;
            Python::attach(|py| response_to_py(py, response))
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

    fn __anext__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let slf_obj: Py<Self> = slf.into();
        slf_obj.bind(py).call_method0("_anext_impl")
    }

    #[pyo3(name = "_anext_impl")]
    async fn anext_impl(&self) -> PyResult<Py<PyAny>> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(PyErr::new::<PyStopAsyncIteration, _>(""));
        }
        let mut guard = self.stream.lock().await;
        let Some(stream) = guard.as_mut() else {
            return Err(PyErr::new::<PyStopAsyncIteration, _>(""));
        };
        match stream.next().await {
            Some(response) => Python::attach(|py| response_to_py(py, response)),
            None => Err(PyErr::new::<PyStopAsyncIteration, _>("")),
        }
    }

    async fn aclose(&self) -> PyResult<()> {
        self.closed.store(true, Ordering::SeqCst);
        let mut guard = self.stream.lock().await;
        *guard = None;
        Ok(())
    }
}
