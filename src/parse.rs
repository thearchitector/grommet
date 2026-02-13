use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;

use async_graphql::dynamic::TypeRef;

use crate::types::{
    ArgDef, FieldDef, PyObj, ResolverEntry, ResolverShape, SchemaDef, TypeDef, TypeKind,
};

// extract schema components directly from SchemaPlan dataclass attributes
pub(crate) fn parse_schema_plan(
    py: Python<'_>,
    plan: &Bound<'_, PyAny>,
) -> PyResult<(SchemaDef, Vec<TypeDef>)> {
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

    Ok((schema_def, type_defs))
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
    let type_spec = item.getattr("type_spec")?;
    let type_ref = type_spec_to_type_ref(&type_spec)?;
    let description: Option<String> = item.getattr("description")?.extract()?;

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

    // Parse inline resolver fields
    let func_obj = item.getattr("func")?;
    let resolver = if func_obj.is_none() {
        None
    } else {
        let shape_obj = item.getattr("shape")?;
        let shape_str: String = shape_obj.extract()?;
        let shape = ResolverShape::from_str(&shape_str)?;
        let is_async: bool = item.getattr("is_async")?.extract()?;
        let is_async_gen: bool = item.getattr("is_async_gen")?.extract()?;

        let arg_names: Vec<String> = item.getattr("arg_names")?.extract()?;

        Some(ResolverEntry {
            func: PyObj::new(func_obj.unbind()),
            shape,
            arg_names,
            is_async,
            is_async_gen,
        })
    };

    Ok(FieldDef {
        name,
        type_ref,
        args,
        resolver,
        description,
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
