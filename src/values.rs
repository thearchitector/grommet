use async_graphql::dynamic::{FieldValue, TypeRef};
use async_graphql::{Name, Value};
use pyo3::IntoPyObject;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyAnyMethods, PyBytes, PyDict, PyList};

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

pub(crate) fn pyobj_to_value(value: &PyObj) -> PyResult<Value> {
    Python::attach(|py| {
        let bound = value.bind(py);
        py_to_value(py, &bound)
    })
}

fn dataclasses_asdict(py: Python<'_>) -> PyResult<Py<PyAny>> {
    static DATACLASSES_ASDICT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    let asdict = DATACLASSES_ASDICT.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        Ok(py.import("dataclasses")?.getattr("asdict")?.unbind())
    })?;
    Ok(asdict.clone_ref(py))
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
    let Some(kind_value) = meta_kind_value(&meta)? else {
        return Ok(None);
    };
    if kind_value != "input" {
        return Ok(None);
    }
    let asdict = dataclasses_asdict(py)?;
    let dict_obj = asdict.bind(py).call1((value,))?;
    Ok(Some(dict_obj))
}

fn meta_kind_value(meta: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if !meta.hasattr("kind")? {
        return Ok(None);
    }
    let kind = meta.getattr("kind")?;
    if kind.hasattr("value")? {
        return Ok(Some(kind.getattr("value")?.extract()?));
    }
    Ok(Some(kind.extract()?))
}

fn grommet_object_type_name(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let ty = value.get_type();
    if !ty.hasattr("__grommet_meta__")? {
        return Ok(None);
    }
    let meta = ty.getattr("__grommet_meta__")?;
    let Some(kind_value) = meta_kind_value(&meta)? else {
        return Ok(None);
    };
    if kind_value != "object" {
        return Ok(None);
    }
    if !meta.hasattr("name")? {
        return Ok(None);
    }
    Ok(Some(meta.getattr("name")?.extract()?))
}

fn is_builtin_scalar(type_name: &str) -> bool {
    matches!(type_name, "Boolean" | "Int" | "Float" | "String" | "ID")
}

fn extract_scalar_value(value: &Bound<'_, PyAny>) -> Option<Value> {
    if value.is_none() {
        return Some(Value::Null);
    }
    if let Ok(boolean) = value.extract::<bool>() {
        return Some(Value::Boolean(boolean));
    }
    if let Ok(integer) = value.extract::<i64>() {
        return Some(Value::from(integer));
    }
    if let Ok(float) = value.extract::<f64>() {
        return Some(Value::from(float));
    }
    if let Ok(string) = value.extract::<String>() {
        return Some(Value::String(string));
    }
    None
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
        TypeRef::Named(name) => {
            let type_name: &str = name;
            convert_named_field_value(value, type_name)
        }
    }
}

fn convert_named_field_value(
    value: &Bound<'_, PyAny>,
    type_name: &str,
) -> PyResult<FieldValue<'static>> {
    if value.is_none() {
        return Ok(FieldValue::value(Value::Null));
    }

    if !is_builtin_scalar(type_name)
        && let Some(runtime_type_name) = grommet_object_type_name(value)?
    {
        let field_value = FieldValue::owned_any(PyObj::new(value.clone().unbind()));
        if runtime_type_name == type_name {
            return Ok(field_value);
        }
        return Ok(field_value.with_type(runtime_type_name));
    }

    match type_name {
        "Boolean" => Ok(FieldValue::value(Value::Boolean(
            value
                .extract::<bool>()
                .map_err(|_| unsupported_value_type())?,
        ))),
        "Int" => Ok(FieldValue::value(Value::from(
            value
                .extract::<i64>()
                .map_err(|_| unsupported_value_type())?,
        ))),
        "Float" => Ok(FieldValue::value(Value::from(
            value
                .extract::<f64>()
                .map_err(|_| unsupported_value_type())?,
        ))),
        "String" => Ok(FieldValue::value(Value::String(
            value
                .extract::<String>()
                .map_err(|_| unsupported_value_type())?,
        ))),
        "ID" => {
            if let Ok(string) = value.extract::<String>() {
                return Ok(FieldValue::value(Value::String(string)));
            }
            if let Ok(integer) = value.extract::<i64>() {
                return Ok(FieldValue::value(Value::String(integer.to_string())));
            }
            Err(unsupported_value_type())
        }
        _ => Ok(FieldValue::owned_any(PyObj::new(value.clone().unbind()))),
    }
}

fn try_collect_sequence<T>(
    value: &Bound<'_, PyAny>,
    mut convert: impl FnMut(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Option<Vec<T>>> {
    if let Ok(seq) = value.cast::<PyList>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() {
            items.push(convert(&item)?);
        }
        return Ok(Some(items));
    }
    Ok(None)
}

fn collect_sequence<T>(
    value: &Bound<'_, PyAny>,
    convert: impl FnMut(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Vec<T>> {
    try_collect_sequence(value, convert)?.ok_or_else(expected_list_value)
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

pub(crate) fn py_to_value(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Some(dict_obj) = input_object_as_dict(py, value)? {
        return py_to_value(py, &dict_obj);
    }

    if let Some(scalar) = extract_scalar_value(value) {
        return Ok(scalar);
    }

    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(Value::Binary(bytes.as_bytes().to_vec().into()));
    }

    if let Some(items) = try_collect_sequence(value, |item| py_to_value(py, item))? {
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

pub(crate) fn value_to_py_bound<'py>(
    py: Python<'py>,
    value: &Value,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Boolean(b) => Ok(b.into_pyobject(py)?.to_owned().into_any()),
        Value::Number(number) => {
            if let Some(i) = number.as_i64() {
                Ok(i.into_pyobject(py)?.into_any())
            } else {
                Ok(number
                    .as_f64()
                    .map(|f| f.into_pyobject(py).map(|value| value.into_any()))
                    .transpose()?
                    .unwrap_or_else(|| py.None().into_bound(py)))
            }
        }
        Value::String(s) => Ok(s.into_pyobject(py)?.into_any()),
        Value::Enum(s) => Ok(s.as_str().into_pyobject(py)?.into_any()),
        Value::List(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(value_to_py_bound(py, item)?)?;
            }
            Ok(list.into_any())
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (key, value) in map {
                dict.set_item(key.as_str(), value_to_py_bound(py, value)?)?;
            }
            Ok(dict.into_any())
        }
        Value::Binary(bytes) => Ok(PyBytes::new(py, bytes).into_any()),
    }
}

#[cfg(test)]
pub(crate) fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    Ok(value_to_py_bound(py, value)?.unbind())
}

pub(crate) fn response_to_py<'py>(
    py: Python<'py>,
    response: async_graphql::Response,
) -> PyResult<Py<PyAny>> {
    let data = value_to_py_bound(py, &response.data)?.unbind();

    let extensions_dict = PyDict::new(py);
    for (key, value) in response.extensions {
        extensions_dict.set_item(key, value_to_py_bound(py, &value)?)?;
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
                    err_dict.set_item("extensions", value_to_py_bound(py, &ext_value)?)?;
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
