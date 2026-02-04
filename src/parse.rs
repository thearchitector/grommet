use std::collections::HashMap;

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyList, PyTuple};

use crate::errors::missing_field;
use crate::types::{
    ArgDef, EnumDef, FieldDef, PyObj, ScalarBinding, ScalarDef, SchemaDef, TypeDef, UnionDef,
};

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct SchemaListsInput {
    types: Vec<Py<PyAny>>,
    #[pyo3(default)]
    scalars: Option<Vec<ScalarDefInput>>,
    #[pyo3(default)]
    enums: Option<Vec<EnumDefInput>>,
    #[pyo3(default)]
    unions: Option<Vec<UnionDefInput>>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct SchemaBlockInput {
    query: String,
    #[pyo3(default)]
    mutation: Option<String>,
    #[pyo3(default)]
    subscription: Option<String>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct TypeDefInput {
    kind: String,
    name: String,
    fields: Vec<Py<PyAny>>,
    #[pyo3(default)]
    description: Option<String>,
    #[pyo3(default)]
    implements: Option<Vec<String>>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct FieldDefInput {
    name: String,
    #[pyo3(default)]
    source: Option<String>,
    r#type: String,
    #[pyo3(default)]
    args: Option<Vec<Py<PyAny>>>,
    #[pyo3(default)]
    resolver: Option<String>,
    #[pyo3(default)]
    description: Option<String>,
    #[pyo3(default)]
    deprecation: Option<String>,
    #[pyo3(default)]
    default: Option<Py<PyAny>>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct ArgDefInput {
    name: String,
    r#type: String,
    #[pyo3(default)]
    default: Option<Py<PyAny>>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct EnumDefInput {
    name: String,
    #[pyo3(default)]
    description: Option<String>,
    values: Vec<String>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct UnionDefInput {
    name: String,
    #[pyo3(default)]
    description: Option<String>,
    types: Vec<String>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct ScalarDefInput {
    name: String,
    #[pyo3(default)]
    description: Option<String>,
    #[pyo3(default)]
    specified_by_url: Option<String>,
}

#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct ScalarBindingInput {
    name: String,
    python_type: Py<PyAny>,
    serialize: Py<PyAny>,
}

fn extract_with_missing<'py, T>(item: &Bound<'py, PyAny>, mapping: &[(&str, &str)]) -> PyResult<T>
where
    for<'a> T: FromPyObject<'a, 'py, Error = PyErr>,
{
    let py = item.py();
    item.extract()
        .map_err(|err| map_missing_field(py, err, mapping))
}

fn map_missing_field(py: Python<'_>, err: PyErr, mapping: &[(&str, &str)]) -> PyErr {
    if err.is_instance_of::<PyKeyError>(py) {
        if let Some(key) = key_error_key(py, &err) {
            if let Some((_, missing)) = mapping.iter().find(|(name, _)| *name == key) {
                return missing_field(missing);
            }
        }
    }
    err
}

fn key_error_key(py: Python<'_>, err: &PyErr) -> Option<String> {
    let value = err.value(py);
    if let Ok(args) = value.getattr("args") {
        if let Ok(args) = args.cast::<PyTuple>() {
            if let Ok(arg0) = args.get_item(0) {
                if let Ok(key) = arg0.extract::<String>() {
                    return Some(key);
                }
            }
        }
    }
    if let Ok(key) = value.extract::<String>() {
        let trimmed = key
            .strip_prefix('\'')
            .and_then(|candidate| candidate.strip_suffix('\''));
        return Some(trimmed.unwrap_or(&key).to_string());
    }
    None
}

// parse python dictionaries into rust structs
pub(crate) fn parse_resolvers(
    _py: Python<'_>,
    resolvers: Option<&Bound<'_, PyDict>>,
) -> PyResult<HashMap<String, PyObj>> {
    let mut map = HashMap::new();
    if let Some(resolvers) = resolvers {
        for (key, value) in resolvers.iter() {
            let key: String = key.extract()?;
            let value = PyObj::new(value.unbind());
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
        let input: ScalarBindingInput = extract_with_missing(
            &item,
            &[
                ("name", "scalar name"),
                ("python_type", "python_type"),
                ("serialize", "serialize"),
            ],
        )?;
        let name = input.name;
        let py_type = input.python_type;
        let serialize = input.serialize;
        bindings.push(ScalarBinding {
            _name: name,
            py_type: PyObj::new(py_type),
            serialize: PyObj::new(serialize),
        });
    }
    Ok(bindings)
}

pub(crate) fn parse_schema_definition(
    py: Python<'_>,
    definition: &Bound<'_, PyAny>,
) -> PyResult<(
    SchemaDef,
    Vec<TypeDef>,
    Vec<ScalarDef>,
    Vec<EnumDef>,
    Vec<UnionDef>,
)> {
    let schema = definition
        .get_item("schema")
        .map_err(|err| map_missing_field(py, err, &[("schema", "schema")]))?;
    let schema: SchemaBlockInput = extract_with_missing(&schema, &[("query", "query")])?;
    let input: SchemaListsInput = extract_with_missing(definition, &[("types", "types")])?;
    let query = schema.query;
    let schema_def = SchemaDef {
        query,
        mutation: schema.mutation,
        subscription: schema.subscription,
    };

    let types = input.types;
    let mut type_defs = Vec::with_capacity(types.len());
    for item in types {
        type_defs.push(parse_type_def(&item.bind(py))?);
    }

    let scalars = input.scalars.unwrap_or_default();
    let mut scalar_defs = Vec::with_capacity(scalars.len());
    for item in scalars {
        scalar_defs.push(scalar_def_from_input(item)?);
    }

    let enums = input.enums.unwrap_or_default();
    let mut enum_defs = Vec::with_capacity(enums.len());
    for item in enums {
        enum_defs.push(enum_def_from_input(item)?);
    }

    let unions = input.unions.unwrap_or_default();
    let mut union_defs = Vec::with_capacity(unions.len());
    for item in unions {
        union_defs.push(union_def_from_input(item)?);
    }

    Ok((schema_def, type_defs, scalar_defs, enum_defs, union_defs))
}

#[allow(dead_code)]
fn parse_type_def(item: &Bound<'_, PyAny>) -> PyResult<TypeDef> {
    let input: TypeDefInput = extract_with_missing(
        item,
        &[
            ("kind", "type kind"),
            ("name", "type name"),
            ("fields", "fields"),
        ],
    )?;
    type_def_from_input(input)
}

fn type_def_from_input(input: TypeDefInput) -> PyResult<TypeDef> {
    let mut parsed_fields = Vec::with_capacity(input.fields.len());
    for field in input.fields {
        Python::attach(|py| {
            parsed_fields.push(parse_field_def(py, &field.bind(py))?);
            Ok::<(), PyErr>(())
        })?;
    }
    Ok(TypeDef {
        kind: input.kind,
        name: input.name,
        fields: parsed_fields,
        description: input.description,
        implements: input.implements.unwrap_or_default(),
    })
}

#[allow(dead_code)]
fn parse_enum_def(item: &Bound<'_, PyAny>) -> PyResult<EnumDef> {
    let input: EnumDefInput =
        extract_with_missing(item, &[("name", "enum name"), ("values", "enum values")])?;
    enum_def_from_input(input)
}

fn enum_def_from_input(input: EnumDefInput) -> PyResult<EnumDef> {
    Ok(EnumDef {
        name: input.name,
        description: input.description,
        values: input.values,
    })
}

#[allow(dead_code)]
fn parse_union_def(item: &Bound<'_, PyAny>) -> PyResult<UnionDef> {
    let input: UnionDefInput =
        extract_with_missing(item, &[("name", "union name"), ("types", "union types")])?;
    union_def_from_input(input)
}

fn union_def_from_input(input: UnionDefInput) -> PyResult<UnionDef> {
    Ok(UnionDef {
        name: input.name,
        description: input.description,
        types: input.types,
    })
}

#[allow(dead_code)]
fn parse_scalar_def(item: &Bound<'_, PyAny>) -> PyResult<ScalarDef> {
    let input: ScalarDefInput = extract_with_missing(item, &[("name", "scalar name")])?;
    scalar_def_from_input(input)
}

fn scalar_def_from_input(input: ScalarDefInput) -> PyResult<ScalarDef> {
    Ok(ScalarDef {
        name: input.name,
        description: input.description,
        specified_by_url: input.specified_by_url,
    })
}

#[allow(dead_code)]
fn parse_field_def(py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<FieldDef> {
    let input: FieldDefInput =
        extract_with_missing(item, &[("name", "field name"), ("type", "field type")])?;
    field_def_from_input(py, input)
}

fn field_def_from_input(py: Python<'_>, input: FieldDefInput) -> PyResult<FieldDef> {
    let source = input.source.unwrap_or_else(|| input.name.clone());
    let mut parsed_args = Vec::new();
    if let Some(args) = input.args {
        parsed_args = Vec::with_capacity(args.len());
        for arg in args {
            parsed_args.push(parse_arg_def(py, &arg.bind(py))?);
        }
    }
    Ok(FieldDef {
        name: input.name,
        source,
        type_name: input.r#type,
        args: parsed_args,
        resolver: input.resolver,
        description: input.description,
        deprecation: input.deprecation,
        default_value: input.default.map(PyObj::new),
    })
}

#[allow(dead_code)]
fn parse_arg_def(_py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<ArgDef> {
    let input: ArgDefInput =
        extract_with_missing(item, &[("name", "arg name"), ("type", "arg type")])?;
    arg_def_from_input(input)
}

fn arg_def_from_input(input: ArgDefInput) -> PyResult<ArgDef> {
    Ok(ArgDef {
        name: input.name,
        type_name: input.r#type,
        default_value: input.default.map(PyObj::new),
    })
}

#[allow(dead_code)]
fn extract_optional_string(item: Option<Bound<'_, PyAny>>) -> Option<String> {
    item.and_then(|value| {
        if value.is_none() {
            None
        } else {
            value.extract().ok()
        }
    })
}
