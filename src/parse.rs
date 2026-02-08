use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict};

use async_graphql::dynamic::TypeRef;

use crate::types::{
    ArgDef, FieldDef, PyObj, ResolverEntry, ResolverShape, SchemaDef, TypeDef, TypeKind,
};

// extract schema components directly from SchemaPlan dataclass attributes
pub(crate) fn parse_schema_plan(
    py: Python<'_>,
    plan: &Bound<'_, PyAny>,
) -> PyResult<(SchemaDef, Vec<TypeDef>, HashMap<String, ResolverEntry>)> {
    let schema_def = SchemaDef {
        query: plan.getattr("query")?.extract()?,
        mutation: plan.getattr("mutation")?.extract()?,
        subscription: plan.getattr("subscription")?.extract()?,
    };

    let no_default = py.import("grommet.metadata")?.getattr("NO_DEFAULT")?;
    let missing = py.import("grommet.metadata")?.getattr("MISSING")?;

    let types_list: Vec<Py<PyAny>> = plan.getattr("types")?.extract()?;
    let mut type_defs = Vec::with_capacity(types_list.len());
    for item in &types_list {
        type_defs.push(parse_type_plan(py, item.bind(py), &no_default, &missing)?);
    }

    let resolvers_dict: Bound<'_, PyDict> = plan.getattr("resolvers")?.extract()?;
    let mut resolver_map = HashMap::new();
    for (key, value) in resolvers_dict.iter() {
        resolver_map.insert(key.extract()?, parse_resolver_entry(py, &value)?);
    }

    Ok((schema_def, type_defs, resolver_map))
}

fn parse_resolver_entry(_py: Python<'_>, entry: &Bound<'_, PyAny>) -> PyResult<ResolverEntry> {
    let func = entry.getattr("func")?.unbind();
    let shape_str: String = entry.getattr("shape")?.extract()?;
    let shape = ResolverShape::from_str(&shape_str)?;
    let is_async_gen: bool = entry.getattr("is_async_gen")?.extract()?;

    let coercers_list: Vec<Py<PyAny>> = entry.getattr("arg_coercers")?.extract()?;
    let mut arg_coercers = Vec::with_capacity(coercers_list.len());
    for item in &coercers_list {
        let item = item.bind(_py);
        let name: String = item.get_item(0)?.extract()?;
        let coercer_obj = item.get_item(1)?;
        let coercer = if coercer_obj.is_none() {
            None
        } else {
            Some(PyObj::new(coercer_obj.unbind()))
        };
        arg_coercers.push((name, coercer));
    }

    Ok(ResolverEntry {
        func: PyObj::new(func),
        shape,
        arg_coercers,
        is_async_gen,
    })
}

fn parse_type_plan(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    no_default: &Bound<'_, PyAny>,
    missing: &Bound<'_, PyAny>,
) -> PyResult<TypeDef> {
    let kind_str: String = item.getattr("kind")?.getattr("value")?.extract()?;
    let kind = TypeKind::from_str(&kind_str)?;
    let name: String = item.getattr("name")?.extract()?;
    let description: Option<String> = item.getattr("description")?.extract()?;

    let fields_list: Vec<Py<PyAny>> = item.getattr("fields")?.extract()?;
    let mut fields = Vec::with_capacity(fields_list.len());
    for field_item in &fields_list {
        fields.push(parse_field_plan(
            py,
            field_item.bind(py),
            no_default,
            missing,
        )?);
    }

    Ok(TypeDef {
        kind,
        name,
        fields,
        description,
    })
}

fn parse_field_plan(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    no_default: &Bound<'_, PyAny>,
    missing: &Bound<'_, PyAny>,
) -> PyResult<FieldDef> {
    let name: String = item.getattr("name")?.extract()?;
    let source: String = item.getattr("source")?.extract()?;
    let type_spec = item.getattr("type_spec")?;
    let type_ref = type_spec_to_type_ref(&type_spec)?;
    let resolver_key: Option<String> = item.getattr("resolver_key")?.extract()?;
    let description: Option<String> = item.getattr("description")?.extract()?;
    let deprecation: Option<String> = item.getattr("deprecation")?.extract()?;

    let default_obj = item.getattr("default")?;
    let default_value = if default_obj.is(no_default) {
        None
    } else {
        Some(PyObj::new(default_obj.unbind()))
    };

    let args_list: Vec<Py<PyAny>> = item.getattr("args")?.extract()?;
    let mut args = Vec::with_capacity(args_list.len());
    for arg_item in &args_list {
        args.push(parse_arg_plan(py, arg_item.bind(py), missing)?);
    }

    Ok(FieldDef {
        name,
        source,
        type_ref,
        args,
        resolver: resolver_key,
        description,
        deprecation,
        default_value,
    })
}

fn parse_arg_plan(
    _py: Python<'_>,
    item: &Bound<'_, PyAny>,
    missing: &Bound<'_, PyAny>,
) -> PyResult<ArgDef> {
    let name: String = item.getattr("name")?.extract()?;
    let type_spec = item.getattr("type_spec")?;
    let type_ref = type_spec_to_type_ref(&type_spec)?;

    let default_obj = item.getattr("default")?;
    let default_value = if default_obj.is(missing) {
        None
    } else {
        Some(PyObj::new(default_obj.unbind()))
    };

    Ok(ArgDef {
        name,
        type_ref,
        default_value,
    })
}

fn type_spec_to_type_ref(spec: &Bound<'_, PyAny>) -> PyResult<TypeRef> {
    let kind: String = spec.getattr("kind")?.extract()?;
    let nullable: bool = spec.getattr("nullable")?.extract()?;
    let ty = if kind == "list" {
        let inner = spec.getattr("of_type")?;
        TypeRef::List(Box::new(type_spec_to_type_ref(&inner)?))
    } else {
        let name: String = spec.getattr("name")?.extract()?;
        TypeRef::named(name)
    };
    Ok(if nullable {
        ty
    } else {
        TypeRef::NonNull(Box::new(ty))
    })
}
