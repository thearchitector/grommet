use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict};

use async_graphql::dynamic::TypeRef;

use crate::types::{
    ArgDef, EnumDef, FieldDef, PyObj, ScalarBinding, ScalarDef, SchemaDef, TypeDef, TypeKind,
    UnionDef,
};

// extract schema components directly from SchemaPlan dataclass attributes
pub(crate) fn parse_schema_plan(
    py: Python<'_>,
    plan: &Bound<'_, PyAny>,
) -> PyResult<(
    SchemaDef,
    Vec<TypeDef>,
    Vec<ScalarDef>,
    Vec<EnumDef>,
    Vec<UnionDef>,
    HashMap<String, PyObj>,
    Vec<ScalarBinding>,
)> {
    let schema_def = SchemaDef {
        query: plan.getattr("query")?.extract()?,
        mutation: plan.getattr("mutation")?.extract()?,
        subscription: plan.getattr("subscription")?.extract()?,
    };

    let no_default = py.import("grommet.plan")?.getattr("_NO_DEFAULT")?;
    let missing = py.import("grommet.metadata")?.getattr("MISSING")?;

    let types_list: Vec<Py<PyAny>> = plan.getattr("types")?.extract()?;
    let mut type_defs = Vec::with_capacity(types_list.len());
    for item in &types_list {
        type_defs.push(parse_type_plan(py, &item.bind(py), &no_default, &missing)?);
    }

    let scalars_list: Vec<Py<PyAny>> = plan.getattr("scalars")?.extract()?;
    let mut scalar_defs = Vec::with_capacity(scalars_list.len());
    let mut scalar_bindings = Vec::with_capacity(scalars_list.len());
    for item in &scalars_list {
        let bound = item.bind(py);
        let meta = bound.getattr("meta")?;
        scalar_defs.push(ScalarDef {
            name: meta.getattr("name")?.extract()?,
            description: meta.getattr("description")?.extract()?,
            specified_by_url: meta.getattr("specified_by_url")?.extract()?,
        });
        scalar_bindings.push(ScalarBinding {
            _name: meta.getattr("name")?.extract()?,
            py_type: PyObj::new(bound.getattr("cls")?.unbind()),
            serialize: PyObj::new(meta.getattr("serialize")?.unbind()),
        });
    }

    let enums_list: Vec<Py<PyAny>> = plan.getattr("enums")?.extract()?;
    let mut enum_defs = Vec::with_capacity(enums_list.len());
    for item in &enums_list {
        let bound = item.bind(py);
        let meta = bound.getattr("meta")?;
        let cls = bound.getattr("cls")?;
        let members = cls.getattr("__members__")?;
        let keys_list: Vec<String> = members
            .call_method0("keys")?
            .try_iter()?
            .map(|k| k?.extract())
            .collect::<PyResult<_>>()?;
        enum_defs.push(EnumDef {
            name: meta.getattr("name")?.extract()?,
            description: meta.getattr("description")?.extract()?,
            values: keys_list,
        });
    }

    let unions_list: Vec<Py<PyAny>> = plan.getattr("unions")?.extract()?;
    let mut union_defs = Vec::with_capacity(unions_list.len());
    for item in &unions_list {
        let bound = item.bind(py);
        let meta = bound.getattr("meta")?;
        let types_tuple: Vec<Py<PyAny>> = meta.getattr("types")?.extract()?;
        let mut member_names = Vec::with_capacity(types_tuple.len());
        for t in &types_tuple {
            let t_meta = t.bind(py).getattr("__grommet_meta__")?;
            member_names.push(t_meta.getattr("name")?.extract()?);
        }
        union_defs.push(UnionDef {
            name: meta.getattr("name")?.extract()?,
            description: meta.getattr("description")?.extract()?,
            types: member_names,
        });
    }

    let resolvers_dict: Bound<'_, PyDict> = plan.getattr("resolvers")?.extract()?;
    let mut resolver_map = HashMap::new();
    for (key, value) in resolvers_dict.iter() {
        resolver_map.insert(key.extract()?, PyObj::new(value.unbind()));
    }

    Ok((
        schema_def,
        type_defs,
        scalar_defs,
        enum_defs,
        union_defs,
        resolver_map,
        scalar_bindings,
    ))
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
    let implements: Vec<String> = item.getattr("implements")?.extract()?;

    let fields_list: Vec<Py<PyAny>> = item.getattr("fields")?.extract()?;
    let mut fields = Vec::with_capacity(fields_list.len());
    for field_item in &fields_list {
        fields.push(parse_field_plan(
            py,
            &field_item.bind(py),
            no_default,
            missing,
        )?);
    }

    Ok(TypeDef {
        kind,
        name,
        fields,
        description,
        implements,
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
        args.push(parse_arg_plan(py, &arg_item.bind(py), missing)?);
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
