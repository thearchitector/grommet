use async_graphql::dynamic::{FieldValue, ResolverContext, TypeRef};
use async_graphql::{Name, Value};
use pyo3::IntoPyObject;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBytes, PyDict, PyList, PyTuple};

use crate::errors::{expected_list_value, py_value_error, unsupported_value_type};
use crate::types::PyObj;

#[pyclass(module = "grommet._core", name = "OperationResult")]
pub(crate) struct OperationResult {
    #[pyo3(get)]
    data: Py<PyAny>,
    #[pyo3(get)]
    errors: Py<PyAny>,
    #[pyo3(get)]
    extensions: Py<PyAny>,
}

#[pymethods]
impl OperationResult {
    fn __repr__(&self) -> PyResult<String> {
        Python::attach(|py| {
            let data = self.data.bind(py);
            let errors = self.errors.bind(py);
            Ok(format!(
                "OperationResult(data={}, errors={})",
                data.repr()?,
                errors.repr()?,
            ))
        })
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match key {
            "data" => Ok(self.data.clone_ref(py)),
            "errors" => Ok(self.errors.clone_ref(py)),
            "extensions" => Ok(self.extensions.clone_ref(py)),
            _ => Err(pyo3::exceptions::PyKeyError::new_err(key.to_string())),
        }
    }
}

// translate values between python and async-graphql
pub(crate) fn build_kwargs<'py>(
    py: Python<'py>,
    ctx: &ResolverContext<'_>,
    arg_names: &[String],
) -> PyResult<Bound<'py, PyDict>> {
    let kwargs = PyDict::new(py);
    for name in arg_names {
        let value = ctx.args.try_get(name.as_str());
        if let Ok(value) = value {
            let py_value = value_to_py(py, value.as_value())?;
            kwargs.set_item(name, py_value)?;
        }
    }
    Ok(kwargs)
}

pub(crate) fn pyobj_to_value(value: &PyObj) -> PyResult<Value> {
    Python::attach(|py| {
        let bound = value.bind(py);
        py_to_value(py, &bound)
    })
}

fn input_object_as_dict<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let ty = value.get_type();
    if !ty.hasattr("__grommet_meta__")? {
        return Ok(None);
    }
    let meta = ty.getattr("__grommet_meta__")?;
    if !meta.hasattr("kind")? {
        return Ok(None);
    }
    let kind = meta.getattr("kind")?;
    let kind_value: String = kind.getattr("value")?.extract()?;
    if kind_value != "input" {
        return Ok(None);
    }
    let dataclasses = py.import("dataclasses")?;
    let dict_obj = dataclasses.call_method1("asdict", (value,))?;
    Ok(Some(dict_obj))
}

pub(crate) fn py_to_field_value_for_type(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    output_type: &TypeRef,
) -> PyResult<FieldValue<'static>> {
    if value.is_none() {
        return Ok(FieldValue::value(Value::Null));
    }
    match output_type {
        TypeRef::NonNull(inner) => py_to_field_value_for_type(py, value, inner),
        TypeRef::List(inner) => convert_sequence_to_field_values(py, value, inner),
        TypeRef::Named(_name) => py_to_field_value(py, value),
    }
}

fn py_to_field_value(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<FieldValue<'static>> {
    if value.is_none() {
        return Ok(FieldValue::value(Value::Null));
    }
    if let Ok(b) = value.extract::<bool>() {
        return Ok(FieldValue::value(Value::Boolean(b)));
    }
    if let Ok(i) = value.extract::<i64>() {
        return Ok(FieldValue::value(Value::from(i)));
    }
    if let Ok(f) = value.extract::<f64>() {
        return Ok(FieldValue::value(Value::from(f)));
    }
    if let Ok(s) = value.extract::<String>() {
        return Ok(FieldValue::value(Value::String(s)));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        return convert_sequence_to_field_values_untyped(py, value);
    }
    Ok(FieldValue::owned_any(PyObj::new(value.clone().unbind())))
}

fn collect_sequence<T>(
    value: &Bound<'_, PyAny>,
    mut convert: impl FnMut(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Vec<T>> {
    if let Ok(seq) = value.cast::<PyList>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() {
            items.push(convert(&item)?);
        }
        Ok(items)
    } else if let Ok(seq) = value.cast::<PyTuple>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() {
            items.push(convert(&item)?);
        }
        Ok(items)
    } else {
        Err(expected_list_value())
    }
}

fn convert_sequence_to_field_values(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    inner_type: &TypeRef,
) -> PyResult<FieldValue<'static>> {
    let items = collect_sequence(value, |item| {
        py_to_field_value_for_type(py, item, inner_type)
    })?;
    Ok(FieldValue::list(items))
}

fn convert_sequence_to_field_values_untyped(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<FieldValue<'static>> {
    let items = collect_sequence(value, |item| py_to_field_value(py, item))?;
    Ok(FieldValue::list(items))
}

pub(crate) fn py_to_value(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Some(dict_obj) = input_object_as_dict(py, value)? {
        return py_to_value(py, &dict_obj);
    }
    if value.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(b) = value.extract::<bool>() {
        return Ok(Value::Boolean(b));
    }
    if let Ok(i) = value.extract::<i64>() {
        return Ok(Value::from(i));
    }
    if let Ok(f) = value.extract::<f64>() {
        return Ok(Value::from(f));
    }
    if let Ok(s) = value.extract::<String>() {
        return Ok(Value::String(s));
    }
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(Value::Binary(bytes.as_bytes().to_vec().into()));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let items = collect_sequence(value, |item| py_to_value(py, item))?;
        return Ok(Value::List(items));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut map = indexmap::IndexMap::new();
        for (key, value) in dict.iter() {
            let key: String = key.extract()?;
            map.insert(Name::new(key), py_to_value(py, &value)?);
        }
        return Ok(Value::Object(map));
    }
    Err(unsupported_value_type())
}

fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Boolean(b) => Ok(b.into_pyobject(py)?.to_owned().into_any().unbind()),
        Value::Number(number) => {
            if let Some(i) = number.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(number
                    .as_f64()
                    .map(|f| f.into_pyobject(py).map(|value| value.into_any().unbind()))
                    .transpose()?
                    .unwrap_or_else(|| py.None()))
            }
        }
        Value::String(s) => Ok(s.into_pyobject(py)?.into_any().unbind()),
        Value::Enum(s) => Ok(s.as_str().into_pyobject(py)?.into_any().unbind()),
        Value::List(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(value_to_py(py, item)?)?;
            }
            Ok(list.into_any().unbind())
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (key, value) in map {
                dict.set_item(key.as_str(), value_to_py(py, value)?)?;
            }
            Ok(dict.into_any().unbind())
        }
        Value::Binary(bytes) => Ok(PyBytes::new(py, bytes).into_any().unbind()),
    }
}

pub(crate) fn response_to_py<'py>(
    py: Python<'py>,
    response: async_graphql::Response,
) -> PyResult<Py<PyAny>> {
    let data = value_to_py(py, &response.data)?;

    let extensions_dict = PyDict::new(py);
    for (key, value) in response.extensions {
        extensions_dict.set_item(key, value_to_py(py, &value)?)?;
    }
    let extensions = if extensions_dict.is_empty() {
        py.None()
    } else {
        extensions_dict.into_any().unbind()
    };

    let errors = if response.errors.is_empty() {
        py.None()
    } else {
        let errors_list = PyList::empty(py);
        for err in response.errors {
            let err_dict = PyDict::new(py);
            err_dict.set_item("message", err.message)?;
            if !err.locations.is_empty() {
                let locs = PyList::empty(py);
                for loc in err.locations {
                    let loc_dict = PyDict::new(py);
                    loc_dict.set_item("line", loc.line)?;
                    loc_dict.set_item("column", loc.column)?;
                    locs.append(loc_dict)?;
                }
                err_dict.set_item("locations", locs)?;
            }
            let path_list = PyList::empty(py);
            if !err.path.is_empty() {
                for seg in err.path {
                    match seg {
                        async_graphql::PathSegment::Field(name) => {
                            path_list.append(name)?;
                        }
                        async_graphql::PathSegment::Index(index) => {
                            path_list.append(index)?;
                        }
                    }
                }
            }
            if path_list.len() > 0 {
                err_dict.set_item("path", path_list)?;
            }
            if let Some(extensions) = err.extensions {
                let ext_value = async_graphql::to_value(extensions)
                    .map_err(|err| py_value_error(err.to_string()))?;
                if !matches!(ext_value, Value::Object(ref map) if map.is_empty()) {
                    err_dict.set_item("extensions", value_to_py(py, &ext_value)?)?;
                }
            }
            errors_list.append(err_dict)?;
        }
        errors_list.into_any().unbind()
    };

    let result = OperationResult {
        data,
        errors,
        extensions,
    };
    Ok(result.into_pyobject(py)?.into_any().unbind())
}
