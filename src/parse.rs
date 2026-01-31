use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::types::{
    ArgDef, EnumDef, FieldDef, PyObj, ScalarBinding, ScalarDef, SchemaDef, TypeDef, UnionDef,
};

// parse python dictionaries into rust structs
pub(crate) fn parse_resolvers(
    _py: Python<'_>,
    resolvers: Option<&Bound<'_, PyDict>>,
) -> PyResult<HashMap<String, PyObj>> {
    let mut map = HashMap::new();
    if let Some(resolvers) = resolvers {
        for (key, value) in resolvers.iter() {
            let key: String = key.extract()?;
            let value = PyObj {
                inner: value.unbind(),
            };
            map.insert(key, value);
        }
    }
    Ok(map)
}

pub(crate) fn parse_scalar_bindings(
    py: Python<'_>,
    scalars: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<ScalarBinding>> {
    let list = match scalars {
        Some(obj) => obj.cast::<PyList>()?.to_owned(),
        None => PyList::empty(py),
    };
    let mut bindings = Vec::with_capacity(list.len());
    for item in list.iter() {
        let dict = item.downcast::<PyDict>()?;
        let name: String = dict
            .get_item("name")?
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing scalar name"))?
            .extract()?;
        let py_type = dict
            .get_item("python_type")?
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing python_type")
            })?
            .unbind();
        let serialize = dict
            .get_item("serialize")?
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing serialize")
            })?
            .unbind();
        bindings.push(ScalarBinding {
            _name: name,
            py_type: PyObj { inner: py_type },
            serialize: PyObj { inner: serialize },
        });
    }
    Ok(bindings)
}

pub(crate) fn parse_schema_definition(
    py: Python<'_>,
    definition: &Bound<'_, PyAny>,
) -> PyResult<(SchemaDef, Vec<TypeDef>, Vec<ScalarDef>, Vec<EnumDef>, Vec<UnionDef>)> {
    let dict = definition.downcast::<PyDict>()?;
    let schema_item = dict
        .get_item("schema")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing schema"))?;
    let schema_dict = schema_item.downcast::<PyDict>()?;
    let query: String = schema_dict
        .get_item("query")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing query"))?
        .extract()?;
    let mutation = extract_optional_string(schema_dict.get_item("mutation")?);
    let subscription = extract_optional_string(schema_dict.get_item("subscription")?);
    let schema_def = SchemaDef {
        query,
        mutation,
        subscription,
    };

    let types_obj = dict
        .get_item("types")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing types"))?;
    let types_list = types_obj.downcast::<PyList>()?;
    let mut type_defs = Vec::with_capacity(types_list.len());
    for item in types_list.iter() {
        type_defs.push(parse_type_def(py, &item)?);
    }

    let scalars_obj = dict.get_item("scalars")?;
    let scalars_list = match scalars_obj {
        Some(obj) => obj.cast::<PyList>()?.to_owned(),
        None => PyList::empty(dict.py()),
    };
    let mut scalar_defs = Vec::with_capacity(scalars_list.len());
    for item in scalars_list.iter() {
        scalar_defs.push(parse_scalar_def(&item)?);
    }

    let enums_obj = dict.get_item("enums")?;
    let enums_list = match enums_obj {
        Some(obj) => obj.cast::<PyList>()?.to_owned(),
        None => PyList::empty(dict.py()),
    };
    let mut enum_defs = Vec::with_capacity(enums_list.len());
    for item in enums_list.iter() {
        enum_defs.push(parse_enum_def(&item)?);
    }

    let unions_obj = dict.get_item("unions")?;
    let unions_list = match unions_obj {
        Some(obj) => obj.cast::<PyList>()?.to_owned(),
        None => PyList::empty(dict.py()),
    };
    let mut union_defs = Vec::with_capacity(unions_list.len());
    for item in unions_list.iter() {
        union_defs.push(parse_union_def(&item)?);
    }

    Ok((schema_def, type_defs, scalar_defs, enum_defs, union_defs))
}

fn parse_type_def(py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<TypeDef> {
    let dict = item.downcast::<PyDict>()?;
    let kind: String = dict
        .get_item("kind")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing type kind"))?
        .extract()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing type name"))?
        .extract()?;
    let description = extract_optional_string(dict.get_item("description")?);
    let implements_obj = dict.get_item("implements")?;
    let implements_list = match implements_obj {
        Some(obj) => obj.cast::<PyList>()?.to_owned(),
        None => PyList::empty(dict.py()),
    };
    let mut implements = Vec::with_capacity(implements_list.len());
    for item in implements_list.iter() {
        implements.push(item.extract()?);
    }

    let fields_obj = dict
        .get_item("fields")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing fields"))?;
    let fields_list = fields_obj.downcast::<PyList>()?;
    let mut fields = Vec::with_capacity(fields_list.len());
    for field in fields_list.iter() {
        fields.push(parse_field_def(py, &field)?);
    }

    Ok(TypeDef {
        kind,
        name,
        fields,
        description,
        implements,
    })
}

fn parse_enum_def(item: &Bound<'_, PyAny>) -> PyResult<EnumDef> {
    let dict = item.downcast::<PyDict>()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing enum name"))?
        .extract()?;
    let description = extract_optional_string(dict.get_item("description")?);
    let values_obj = dict
        .get_item("values")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing enum values"))?;
    let values_list = values_obj.downcast::<PyList>()?;
    let mut values = Vec::with_capacity(values_list.len());
    for item in values_list.iter() {
        values.push(item.extract()?);
    }
    Ok(EnumDef {
        name,
        description,
        values,
    })
}

fn parse_union_def(item: &Bound<'_, PyAny>) -> PyResult<UnionDef> {
    let dict = item.downcast::<PyDict>()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing union name"))?
        .extract()?;
    let description = extract_optional_string(dict.get_item("description")?);
    let types_obj = dict
        .get_item("types")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing union types"))?;
    let types_list = types_obj.downcast::<PyList>()?;
    let mut types = Vec::with_capacity(types_list.len());
    for item in types_list.iter() {
        types.push(item.extract()?);
    }
    Ok(UnionDef {
        name,
        description,
        types,
    })
}

fn parse_scalar_def(item: &Bound<'_, PyAny>) -> PyResult<ScalarDef> {
    let dict = item.downcast::<PyDict>()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing scalar name"))?
        .extract()?;
    let description = extract_optional_string(dict.get_item("description")?);
    let specified_by_url = extract_optional_string(dict.get_item("specified_by_url")?);
    Ok(ScalarDef {
        name,
        description,
        specified_by_url,
    })
}

fn parse_field_def(py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<FieldDef> {
    let dict = item.downcast::<PyDict>()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing field name"))?
        .extract()?;
    let source = extract_optional_string(dict.get_item("source")?).unwrap_or_else(|| name.clone());
    let type_name: String = dict
        .get_item("type")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing field type"))?
        .extract()?;
    let resolver = extract_optional_string(dict.get_item("resolver")?);
    let description = extract_optional_string(dict.get_item("description")?);
    let deprecation = extract_optional_string(dict.get_item("deprecation")?);
    let default_value = match dict.get_item("default")? {
        Some(value) => Some(PyObj {
            inner: value.unbind(),
        }),
        None => None,
    };

    let args_list = match dict.get_item("args")? {
        Some(args_obj) => args_obj.cast::<PyList>()?.to_owned(),
        None => PyList::empty(dict.py()),
    };
    let mut args = Vec::with_capacity(args_list.len());
    for arg in args_list.iter() {
        args.push(parse_arg_def(py, &arg)?);
    }

    Ok(FieldDef {
        name,
        source,
        type_name,
        args,
        resolver,
        description,
        deprecation,
        default_value,
    })
}

fn parse_arg_def(_py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<ArgDef> {
    let dict = item.downcast::<PyDict>()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing arg name"))?
        .extract()?;
    let type_name: String = dict
        .get_item("type")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing arg type"))?
        .extract()?;
    let default_value = match dict.get_item("default")? {
        Some(value) => Some(PyObj {
            inner: value.unbind(),
        }),
        None => None,
    };
    Ok(ArgDef {
        name,
        type_name,
        default_value,
    })
}

fn extract_optional_string(item: Option<Bound<'_, PyAny>>) -> Option<String> {
    item.and_then(|value| {
        if value.is_none() {
            None
        } else {
            value.extract().ok()
        }
    })
}
