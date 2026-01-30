use async_graphql::dynamic::{
    Enum, Field, FieldFuture, FieldValue, InputObject, InputValue, Interface, InterfaceField,
    Object, ResolverContext, Scalar, Schema, Subscription, SubscriptionField,
    SubscriptionFieldFuture, TypeRef, Union, ValueAccessor,
};
use async_graphql::futures_util::stream::{self, BoxStream, StreamExt};
use async_graphql::{Error, Name, Request, Value, Variables};
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

#[derive(Clone)]
struct PyObj {
    inner: Py<PyAny>,
}

unsafe impl Send for PyObj {}
unsafe impl Sync for PyObj {}

#[derive(Clone)]
struct RootValue(PyObj);

unsafe impl Send for RootValue {}
unsafe impl Sync for RootValue {}

#[derive(Clone)]
struct ContextValue(PyObj);

unsafe impl Send for ContextValue {}
unsafe impl Sync for ContextValue {}

struct SchemaDef {
    query: String,
    mutation: Option<String>,
    subscription: Option<String>,
}

struct ScalarDef {
    name: String,
    description: Option<String>,
    specified_by_url: Option<String>,
}

struct EnumDef {
    name: String,
    description: Option<String>,
    values: Vec<String>,
}

struct UnionDef {
    name: String,
    description: Option<String>,
    types: Vec<String>,
}

struct ArgDef {
    name: String,
    type_name: String,
    default_value: Option<PyObj>,
}

struct FieldDef {
    name: String,
    source: String,
    type_name: String,
    args: Vec<ArgDef>,
    resolver: Option<String>,
    description: Option<String>,
    deprecation: Option<String>,
    default_value: Option<PyObj>,
}

struct TypeDef {
    kind: String,
    name: String,
    fields: Vec<FieldDef>,
    description: Option<String>,
    implements: Vec<String>,
}

#[derive(Clone)]
struct ScalarBinding {
    _name: String,
    py_type: PyObj,
    serialize: PyObj,
}

#[pyclass(module = "grommet._core", name = "Schema")]
pub struct SchemaWrapper {
    schema: Arc<Schema>,
    scalars: Arc<Vec<ScalarBinding>>,
}

#[pymethods]
impl SchemaWrapper {
    #[new]
    fn new(
        py: Python,
        definition: &PyAny,
        resolvers: Option<&PyDict>,
        scalars: Option<&PyAny>,
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
        variables: Option<PyObject>,
        root: Option<PyObject>,
        context: Option<PyObject>,
    ) -> PyResult<&'py PyAny> {
        let vars_value = if let Some(vars) = variables {
            let value = Python::with_gil(|py| {
                py_to_value(py, vars.as_ref(py), self.scalars.as_ref(), true)
            })?;
            Some(value)
        } else {
            None
        };
        let root_value = root.map(|obj| RootValue(PyObj { inner: obj }));
        let context_value = context.map(|obj| ContextValue(PyObj { inner: obj }));
        let schema = self.schema.clone();

        pyo3_asyncio::tokio::future_into_py(py, async move {
            let mut request = Request::new(query);
            if let Some(vars) = vars_value {
                let json = serde_json::to_value(vars).map_err(|err| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string())
                })?;
                request = request.variables(Variables::from_json(json));
            }
            if let Some(root) = root_value {
                request = request.data(root);
            }
            if let Some(ctx) = context_value {
                request = request.data(ctx);
            }
            let response = schema.execute(request).await;
            Python::with_gil(|py| response_to_py(py, response))
        })
    }

    fn subscribe(
        &self,
        _py: Python,
        query: String,
        variables: Option<PyObject>,
        root: Option<PyObject>,
        context: Option<PyObject>,
    ) -> PyResult<SubscriptionStream> {
        let vars_value = if let Some(vars) = variables {
            let value = Python::with_gil(|py| {
                py_to_value(py, vars.as_ref(py), self.scalars.as_ref(), true)
            })?;
            Some(value)
        } else {
            None
        };
        let root_value = root.map(|obj| RootValue(PyObj { inner: obj }));
        let context_value = context.map(|obj| ContextValue(PyObj { inner: obj }));

        let mut request = Request::new(query);
        if let Some(vars) = vars_value {
            let json = serde_json::to_value(vars).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string())
            })?;
            request = request.variables(Variables::from_json(json));
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
struct SubscriptionStream {
    stream: Arc<Mutex<Option<BoxStream<'static, async_graphql::Response>>>>,
    closed: Arc<AtomicBool>,
}

#[pymethods]
impl SubscriptionStream {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let stream = self.stream.clone();
        let closed = self.closed.clone();
        let awaitable = pyo3_asyncio::tokio::future_into_py(py, async move {
            if closed.load(Ordering::SeqCst) {
                return Err(PyErr::new::<PyStopAsyncIteration, _>(""));
            }
            let mut guard = stream.lock().await;
            let Some(stream) = guard.as_mut() else {
                return Err(PyErr::new::<PyStopAsyncIteration, _>(""));
            };
            match stream.next().await {
                Some(response) => Python::with_gil(|py| response_to_py(py, response)),
                None => Err(PyErr::new::<PyStopAsyncIteration, _>("")),
            }
        })?;
        Ok(Some(awaitable.into_py(py)))
    }

    fn aclose<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
        let stream = self.stream.clone();
        let closed = self.closed.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            closed.store(true, Ordering::SeqCst);
            let mut guard = stream.lock().await;
            *guard = None;
            Ok(Python::with_gil(|py| py.None()))
        })
    }
}

#[pymodule]
fn _core(_py: Python, module: &PyModule) -> PyResult<()> {
    module.add_class::<SchemaWrapper>()?;
    module.add_class::<SubscriptionStream>()?;
    Ok(())
}

fn parse_resolvers(py: Python, resolvers: Option<&PyDict>) -> PyResult<HashMap<String, PyObj>> {
    let mut map = HashMap::new();
    if let Some(resolvers) = resolvers {
        for (key, value) in resolvers.iter() {
            let key: String = key.extract()?;
            let value = PyObj {
                inner: value.into_py(py),
            };
            map.insert(key, value);
        }
    }
    Ok(map)
}

fn parse_scalar_bindings(py: Python, scalars: Option<&PyAny>) -> PyResult<Vec<ScalarBinding>> {
    let list: &PyList = match scalars {
        Some(obj) => obj.downcast()?,
        None => PyList::empty(py),
    };
    let mut bindings = Vec::with_capacity(list.len());
    for item in list.iter() {
        let dict: &PyDict = item.downcast()?;
        let name: String = dict
            .get_item("name")?
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing scalar name"))?
            .extract()?;
        let py_type = dict
            .get_item("python_type")?
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing python_type")
            })?
            .into_py(py);
        let serialize = dict
            .get_item("serialize")?
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing serialize")
            })?
            .into_py(py);
        bindings.push(ScalarBinding {
            _name: name,
            py_type: PyObj { inner: py_type },
            serialize: PyObj { inner: serialize },
        });
    }
    Ok(bindings)
}

fn parse_schema_definition(
    py: Python,
    definition: &PyAny,
) -> PyResult<(SchemaDef, Vec<TypeDef>, Vec<ScalarDef>, Vec<EnumDef>, Vec<UnionDef>)> {
    let dict: &PyDict = definition.downcast()?;
    let schema_dict: &PyDict = dict
        .get_item("schema")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing schema"))?
        .downcast()?;
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
    let types_list: &PyList = types_obj.downcast()?;
    let mut type_defs = Vec::with_capacity(types_list.len());
    for item in types_list.iter() {
        type_defs.push(parse_type_def(py, item)?);
    }

    let scalars_obj = dict.get_item("scalars")?;
    let scalars_list: &PyList = match scalars_obj {
        Some(obj) => obj.downcast()?,
        None => PyList::empty(dict.py()),
    };
    let mut scalar_defs = Vec::with_capacity(scalars_list.len());
    for item in scalars_list.iter() {
        scalar_defs.push(parse_scalar_def(item)?);
    }

    let enums_obj = dict.get_item("enums")?;
    let enums_list: &PyList = match enums_obj {
        Some(obj) => obj.downcast()?,
        None => PyList::empty(dict.py()),
    };
    let mut enum_defs = Vec::with_capacity(enums_list.len());
    for item in enums_list.iter() {
        enum_defs.push(parse_enum_def(item)?);
    }

    let unions_obj = dict.get_item("unions")?;
    let unions_list: &PyList = match unions_obj {
        Some(obj) => obj.downcast()?,
        None => PyList::empty(dict.py()),
    };
    let mut union_defs = Vec::with_capacity(unions_list.len());
    for item in unions_list.iter() {
        union_defs.push(parse_union_def(item)?);
    }

    Ok((schema_def, type_defs, scalar_defs, enum_defs, union_defs))
}

fn parse_type_def(py: Python, item: &PyAny) -> PyResult<TypeDef> {
    let dict: &PyDict = item.downcast()?;
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
    let implements_list: &PyList = match implements_obj {
        Some(obj) => obj.downcast()?,
        None => PyList::empty(dict.py()),
    };
    let mut implements = Vec::with_capacity(implements_list.len());
    for item in implements_list.iter() {
        implements.push(item.extract()?);
    }

    let fields_obj = dict
        .get_item("fields")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing fields"))?;
    let fields_list: &PyList = fields_obj.downcast()?;
    let mut fields = Vec::with_capacity(fields_list.len());
    for field in fields_list.iter() {
        fields.push(parse_field_def(py, field)?);
    }

    Ok(TypeDef {
        kind,
        name,
        fields,
        description,
        implements,
    })
}

fn parse_enum_def(item: &PyAny) -> PyResult<EnumDef> {
    let dict: &PyDict = item.downcast()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing enum name"))?
        .extract()?;
    let description = extract_optional_string(dict.get_item("description")?);
    let values_obj = dict
        .get_item("values")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing enum values"))?;
    let values_list: &PyList = values_obj.downcast()?;
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

fn parse_union_def(item: &PyAny) -> PyResult<UnionDef> {
    let dict: &PyDict = item.downcast()?;
    let name: String = dict
        .get_item("name")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing union name"))?
        .extract()?;
    let description = extract_optional_string(dict.get_item("description")?);
    let types_obj = dict
        .get_item("types")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing union types"))?;
    let types_list: &PyList = types_obj.downcast()?;
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

fn parse_scalar_def(item: &PyAny) -> PyResult<ScalarDef> {
    let dict: &PyDict = item.downcast()?;
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

fn parse_field_def(py: Python, item: &PyAny) -> PyResult<FieldDef> {
    let dict: &PyDict = item.downcast()?;
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
            inner: value.into_py(py),
        }),
        None => None,
    };

    let args_list: &PyList = match dict.get_item("args")? {
        Some(args_obj) => args_obj.downcast()?,
        None => PyList::empty(dict.py()),
    };
    let mut args = Vec::with_capacity(args_list.len());
    for arg in args_list.iter() {
        args.push(parse_arg_def(py, arg)?);
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

fn parse_arg_def(py: Python, item: &PyAny) -> PyResult<ArgDef> {
    let dict: &PyDict = item.downcast()?;
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
            inner: value.into_py(py),
        }),
        None => None,
    };
    Ok(ArgDef {
        name,
        type_name,
        default_value,
    })
}

fn extract_optional_string(item: Option<&PyAny>) -> Option<String> {
    item.and_then(|value| {
        if value.is_none() {
            None
        } else {
            value.extract().ok()
        }
    })
}

fn build_schema(
    schema_def: SchemaDef,
    type_defs: Vec<TypeDef>,
    scalar_defs: Vec<ScalarDef>,
    enum_defs: Vec<EnumDef>,
    union_defs: Vec<UnionDef>,
    resolver_map: HashMap<String, PyObj>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
) -> PyResult<Schema> {
    let mut builder = Schema::build(
        schema_def.query.as_str(),
        schema_def.mutation.as_deref(),
        schema_def.subscription.as_deref(),
    );

    let mut abstract_types = HashSet::new();
    for type_def in &type_defs {
        if type_def.kind == "interface" {
            abstract_types.insert(type_def.name.clone());
        }
    }
    for union_def in &union_defs {
        abstract_types.insert(union_def.name.clone());
    }
    let abstract_types = Arc::new(abstract_types);

    for scalar_def in scalar_defs {
        let mut scalar = Scalar::new(scalar_def.name.as_str());
        if let Some(desc) = scalar_def.description.as_ref() {
            scalar = scalar.description(desc.as_str());
        }
        if let Some(url) = scalar_def.specified_by_url.as_ref() {
            scalar = scalar.specified_by_url(url.as_str());
        }
        builder = builder.register(scalar);
    }

    for enum_def in enum_defs {
        let mut enum_type = Enum::new(enum_def.name.as_str());
        if let Some(desc) = enum_def.description.as_ref() {
            enum_type = enum_type.description(desc.as_str());
        }
        for value in enum_def.values {
            enum_type = enum_type.item(value);
        }
        builder = builder.register(enum_type);
    }

    for union_def in union_defs {
        let mut union_type = Union::new(union_def.name.as_str());
        if let Some(desc) = union_def.description.as_ref() {
            union_type = union_type.description(desc.as_str());
        }
        for ty in union_def.types {
            union_type = union_type.possible_type(ty);
        }
        builder = builder.register(union_type);
    }

    for type_def in type_defs {
        match type_def.kind.as_str() {
            "object" => {
                let mut object = Object::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    object = object.description(desc.as_str());
                }
                for implement in &type_def.implements {
                    object = object.implement(implement.as_str());
                }
                for field_def in type_def.fields {
                    let resolver = field_def
                        .resolver
                        .as_ref()
                        .and_then(|key| resolver_map.get(key).cloned());
                    let arg_names: Arc<Vec<String>> =
                        Arc::new(field_def.args.iter().map(|arg| arg.name.clone()).collect());
                    let field_name = Arc::new(field_def.name.clone());
                    let source_name = Arc::new(field_def.source.clone());
                    let type_ref = parse_type_ref(field_def.type_name.as_str());
                    let output_type = type_ref.clone();
                    let abstract_types = abstract_types.clone();

                    let scalars = scalar_bindings.clone();
                    let mut field = Field::new(field_def.name, type_ref, move |ctx| {
                        let scalars = scalars.clone();
                        let resolver = resolver.clone();
                        let arg_names = arg_names.clone();
                        let field_name = field_name.clone();
                        let source_name = source_name.clone();
                        let output_type = output_type.clone();
                        let abstract_types = abstract_types.clone();
                        FieldFuture::new(async move {
                            resolve_field(
                                ctx,
                                resolver,
                                arg_names,
                                field_name,
                                source_name,
                                scalars,
                                output_type,
                                abstract_types,
                            )
                            .await
                        })
                    });

                    for arg_def in field_def.args {
                        let arg_ref = parse_type_ref(arg_def.type_name.as_str());
                        let mut input_value = InputValue::new(arg_def.name, arg_ref);
                        if let Some(default_value) = arg_def.default_value.as_ref() {
                            let value = pyobj_to_value(default_value, scalar_bindings.as_ref())?;
                            input_value = input_value.default_value(value);
                        }
                        field = field.argument(input_value);
                    }
                    if let Some(desc) = field_def.description.as_ref() {
                        field = field.description(desc.as_str());
                    }
                    if let Some(dep) = field_def.deprecation.as_ref() {
                        field = field.deprecation(Some(dep.as_str()));
                    }
                    object = object.field(field);
                }
                builder = builder.register(object);
            }
            "interface" => {
                let mut interface = Interface::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    interface = interface.description(desc.as_str());
                }
                for implement in &type_def.implements {
                    interface = interface.implement(implement.as_str());
                }
                for field_def in type_def.fields {
                    let type_ref = parse_type_ref(field_def.type_name.as_str());
                    let mut field = InterfaceField::new(field_def.name, type_ref);
                    for arg_def in field_def.args {
                        let arg_ref = parse_type_ref(arg_def.type_name.as_str());
                        let mut input_value = InputValue::new(arg_def.name, arg_ref);
                        if let Some(default_value) = arg_def.default_value.as_ref() {
                            let value = pyobj_to_value(default_value, scalar_bindings.as_ref())?;
                            input_value = input_value.default_value(value);
                        }
                        field = field.argument(input_value);
                    }
                    if let Some(desc) = field_def.description.as_ref() {
                        field = field.description(desc.as_str());
                    }
                    if let Some(dep) = field_def.deprecation.as_ref() {
                        field = field.deprecation(Some(dep.as_str()));
                    }
                    interface = interface.field(field);
                }
                builder = builder.register(interface);
            }
            "subscription" => {
                let mut subscription = Subscription::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    subscription = subscription.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    let resolver = field_def
                        .resolver
                        .as_ref()
                        .and_then(|key| resolver_map.get(key).cloned());
                    let arg_names: Arc<Vec<String>> =
                        Arc::new(field_def.args.iter().map(|arg| arg.name.clone()).collect());
                    let field_name = Arc::new(field_def.name.clone());
                    let source_name = Arc::new(field_def.source.clone());
                    let type_ref = parse_type_ref(field_def.type_name.as_str());
                    let output_type = type_ref.clone();
                    let abstract_types = abstract_types.clone();

                    let scalars = scalar_bindings.clone();
                    let mut field =
                        SubscriptionField::new(field_def.name, type_ref, move |ctx| {
                        let scalars = scalars.clone();
                        let resolver = resolver.clone();
                        let arg_names = arg_names.clone();
                        let field_name = field_name.clone();
                        let source_name = source_name.clone();
                        let output_type = output_type.clone();
                        let abstract_types = abstract_types.clone();
                        SubscriptionFieldFuture::new(async move {
                            resolve_subscription_field(
                                ctx,
                                resolver,
                                arg_names,
                                field_name,
                                source_name,
                                scalars,
                                output_type,
                                abstract_types,
                            )
                            .await
                        })
                    });

                    for arg_def in field_def.args {
                        let arg_ref = parse_type_ref(arg_def.type_name.as_str());
                        let mut input_value = InputValue::new(arg_def.name, arg_ref);
                        if let Some(default_value) = arg_def.default_value.as_ref() {
                            let value = pyobj_to_value(default_value, scalar_bindings.as_ref())?;
                            input_value = input_value.default_value(value);
                        }
                        field = field.argument(input_value);
                    }
                    if let Some(desc) = field_def.description.as_ref() {
                        field = field.description(desc.as_str());
                    }
                    if let Some(dep) = field_def.deprecation.as_ref() {
                        field = field.deprecation(Some(dep.as_str()));
                    }
                    subscription = subscription.field(field);
                }
                builder = builder.register(subscription);
            }
            "input" => {
                let mut input = InputObject::new(type_def.name.as_str());
                if let Some(desc) = type_def.description.as_ref() {
                    input = input.description(desc.as_str());
                }
                for field_def in type_def.fields {
                    let arg_ref = parse_type_ref(field_def.type_name.as_str());
                    let mut input_value = InputValue::new(field_def.name, arg_ref);
                    if let Some(default_value) = field_def.default_value.as_ref() {
                        let value = pyobj_to_value(default_value, scalar_bindings.as_ref())?;
                        input_value = input_value.default_value(value);
                    }
                    input = input.field(input_value);
                }
                builder = builder.register(input);
            }
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    format!("Unknown type kind: {}", type_def.kind),
                ))
            }
        }
    }

    builder
        .finish()
        .map_err(|err| PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string()))
}

fn parse_type_ref(type_name: &str) -> TypeRef {
    let mut name = type_name.trim();
    let mut non_null = false;
    if name.ends_with('!') {
        non_null = true;
        name = &name[..name.len() - 1];
    }
    let ty = if name.starts_with('[') && name.ends_with(']') {
        let inner = &name[1..name.len() - 1];
        let inner_ref = parse_type_ref(inner);
        TypeRef::List(Box::new(inner_ref))
    } else {
        TypeRef::named(name)
    };

    if non_null {
        TypeRef::NonNull(Box::new(ty))
    } else {
        ty
    }
}

async fn resolve_field(
    ctx: ResolverContext<'_>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: Arc<String>,
    source_name: Arc<String>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
) -> Result<Option<FieldValue<'_>>, Error> {
    let root_value = ctx.data::<RootValue>().ok().map(|root| root.0.inner.clone());
    let parent = ctx
        .parent_value
        .try_downcast_ref::<PyObj>()
        .ok()
        .map(|obj| obj.inner.clone())
        .or_else(|| root_value.clone());
    let context = ctx
        .data::<ContextValue>()
        .ok()
        .map(|ctx| ctx.0.inner.clone());

    if let Some(resolver) = resolver {
        let result = Python::with_gil(|py| -> PyResult<(bool, PyObject)> {
            let kwargs = build_kwargs(py, &ctx, &arg_names)?;
            let info = PyDict::new(py);
            info.set_item("field_name", field_name.as_str())?;
            if let Some(ctx_obj) = context.as_ref() {
                info.set_item("context", ctx_obj.as_ref(py))?;
            } else {
                info.set_item("context", py.None())?;
            }
            if let Some(root_obj) = root_value.as_ref() {
                info.set_item("root", root_obj.as_ref(py))?;
            } else {
                info.set_item("root", py.None())?;
            }
            let parent_obj = match parent.as_ref() {
                Some(parent) => parent.as_ref(py).to_object(py),
                None => py.None(),
            };
            let args = PyTuple::new(py, &[parent_obj, info.to_object(py)]);
            let result = resolver.inner.as_ref(py).call(args, Some(kwargs))?;
            let is_awaitable = result.hasattr("__await__")?;
            Ok((is_awaitable, result.into_py(py)))
        });

        let (is_awaitable, result) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_asyncio::tokio::into_future(result.as_ref(py))
            })
            .map_err(py_err_to_error)?
            .await
            .map_err(py_err_to_error)?;
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    awaited.as_ref(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
                .map_err(py_err_to_error)
                .map(Some)
        } else {
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    result.as_ref(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
                .map_err(py_err_to_error)
                .map(Some)
        }
    } else {
        let parent = parent.ok_or_else(|| Error::new("No parent value for field"))?;
        let result = Python::with_gil(|py| -> PyResult<(bool, PyObject)> {
            let parent_ref = parent.as_ref(py);
            let value = if let Ok(dict) = parent_ref.downcast::<PyDict>() {
                dict.get_item(source_name.as_str())?
                    .map(|item| item.into_py(py))
                    .unwrap_or_else(|| py.None())
            } else if parent_ref.hasattr(source_name.as_str())? {
                parent_ref.getattr(source_name.as_str())?.into_py(py)
            } else if parent_ref.hasattr("__getitem__")? {
                parent_ref.get_item(source_name.as_str())?.into_py(py)
            } else {
                py.None()
            };
            let is_awaitable = value.as_ref(py).hasattr("__await__")?;
            Ok((is_awaitable, value))
        });

        let (is_awaitable, value) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_asyncio::tokio::into_future(value.as_ref(py))
            })
            .map_err(py_err_to_error)?
            .await
            .map_err(py_err_to_error)?;
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    awaited.as_ref(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
                .map_err(py_err_to_error)
                .map(Some)
        } else {
            Python::with_gil(|py| {
                py_to_field_value_for_type(
                    py,
                    value.as_ref(py),
                    &output_type,
                    &scalar_bindings,
                    &abstract_types,
                )
            })
                .map_err(py_err_to_error)
                .map(Some)
        }
    }
}

async fn resolve_subscription_field<'a>(
    ctx: ResolverContext<'a>,
    resolver: Option<PyObj>,
    arg_names: Arc<Vec<String>>,
    field_name: Arc<String>,
    source_name: Arc<String>,
    scalar_bindings: Arc<Vec<ScalarBinding>>,
    output_type: TypeRef,
    abstract_types: Arc<HashSet<String>>,
) -> Result<BoxStream<'a, Result<FieldValue<'a>, Error>>, Error> {
    let root_value = ctx.data::<RootValue>().ok().map(|root| root.0.inner.clone());
    let parent = ctx
        .parent_value
        .try_downcast_ref::<PyObj>()
        .ok()
        .map(|obj| obj.inner.clone())
        .or_else(|| root_value.clone());
    let context = ctx
        .data::<ContextValue>()
        .ok()
        .map(|ctx| ctx.0.inner.clone());

    let result = if let Some(resolver) = resolver {
        let result = Python::with_gil(|py| -> PyResult<(bool, PyObject)> {
            let kwargs = build_kwargs(py, &ctx, &arg_names)?;
            let info = PyDict::new(py);
            info.set_item("field_name", field_name.as_str())?;
            if let Some(ctx_obj) = context.as_ref() {
                info.set_item("context", ctx_obj.as_ref(py))?;
            } else {
                info.set_item("context", py.None())?;
            }
            if let Some(root_obj) = root_value.as_ref() {
                info.set_item("root", root_obj.as_ref(py))?;
            } else {
                info.set_item("root", py.None())?;
            }
            let parent_obj = match parent.as_ref() {
                Some(parent) => parent.as_ref(py).to_object(py),
                None => py.None(),
            };
            let args = PyTuple::new(py, &[parent_obj, info.to_object(py)]);
            let result = resolver.inner.as_ref(py).call(args, Some(kwargs))?;
            let is_awaitable = result.hasattr("__await__")?;
            Ok((is_awaitable, result.into_py(py)))
        });

        let (is_awaitable, result) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_asyncio::tokio::into_future(result.as_ref(py))
            })
            .map_err(py_err_to_error)?
            .await
            .map_err(py_err_to_error)?;
            awaited
        } else {
            result
        }
    } else {
        let parent = parent.ok_or_else(|| Error::new("No parent value for field"))?;
        let result = Python::with_gil(|py| -> PyResult<(bool, PyObject)> {
            let parent_ref = parent.as_ref(py);
            let value = if let Ok(dict) = parent_ref.downcast::<PyDict>() {
                dict.get_item(source_name.as_str())?
                    .map(|item| item.into_py(py))
                    .unwrap_or_else(|| py.None())
            } else if parent_ref.hasattr(source_name.as_str())? {
                parent_ref.getattr(source_name.as_str())?.into_py(py)
            } else if parent_ref.hasattr("__getitem__")? {
                parent_ref.get_item(source_name.as_str())?.into_py(py)
            } else {
                py.None()
            };
            let is_awaitable = value.as_ref(py).hasattr("__await__")?;
            Ok((is_awaitable, value))
        });

        let (is_awaitable, value) = match result {
            Ok(value) => value,
            Err(err) => return Err(py_err_to_error(err)),
        };

        if is_awaitable {
            let awaited = Python::with_gil(|py| {
                pyo3_asyncio::tokio::into_future(value.as_ref(py))
            })
            .map_err(py_err_to_error)?
            .await
            .map_err(py_err_to_error)?;
            awaited
        } else {
            value
        }
    };

    let iterator = Python::with_gil(|py| -> PyResult<PyObj> {
        let value_ref = result.as_ref(py);
        if value_ref.hasattr("__aiter__")? {
            let iter = value_ref.call_method0("__aiter__")?;
            Ok(PyObj {
                inner: iter.into_py(py),
            })
        } else if value_ref.hasattr("__anext__")? {
            Ok(PyObj {
                inner: result.clone_ref(py),
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "Subscription resolver must return an async iterator",
            ))
        }
    })
    .map_err(py_err_to_error)?;

    let scalar_bindings = scalar_bindings.clone();
    let output_type = output_type.clone();
    let abstract_types = abstract_types.clone();
    let stream = stream::unfold(Some(iterator), move |state| {
        let scalar_bindings = scalar_bindings.clone();
        let output_type = output_type.clone();
        let abstract_types = abstract_types.clone();
        async move {
        let iterator = match state {
            Some(iterator) => iterator,
            None => return None,
        };

        let awaitable = Python::with_gil(|py| -> PyResult<PyObject> {
            let awaitable = iterator.inner.as_ref(py).call_method0("__anext__")?;
            Ok(awaitable.into_py(py))
        });
        let awaitable = match awaitable {
            Ok(value) => value,
            Err(err) => return Some((Err(py_err_to_error(err)), None)),
        };

        let awaited = Python::with_gil(|py| pyo3_asyncio::tokio::into_future(awaitable.as_ref(py)));
        let awaited = match awaited {
            Ok(fut) => fut.await,
            Err(err) => return Some((Err(py_err_to_error(err)), None)),
        };

        let next_value = match awaited {
            Ok(value) => value,
            Err(err) => {
                let is_stop = Python::with_gil(|py| err.is_instance_of::<PyStopAsyncIteration>(py));
                if is_stop {
                    return None;
                }
                return Some((Err(py_err_to_error(err)), None));
            }
        };

        let value = match Python::with_gil(|py| {
            py_to_field_value_for_type(
                py,
                next_value.as_ref(py),
                &output_type,
                &scalar_bindings,
                &abstract_types,
            )
        }) {
            Ok(value) => value,
            Err(err) => return Some((Err(py_err_to_error(err)), None)),
        };
        let value: FieldValue<'a> = value;

        Some((Ok(value), Some(iterator)))
        }
    });

    let stream: BoxStream<'a, Result<FieldValue<'a>, Error>> = stream.boxed();
    Ok(stream)
}

fn build_kwargs<'py>(
    py: Python<'py>,
    ctx: &ResolverContext<'_>,
    arg_names: &[String],
) -> PyResult<&'py PyDict> {
    let kwargs = PyDict::new(py);
    for name in arg_names {
        let value = ctx.args.try_get(name.as_str());
        if let Ok(value) = value {
            let value = value_accessor_to_value(&value);
            let py_value = value_to_py(py, &value)?;
            kwargs.set_item(name, py_value)?;
        }
    }
    Ok(kwargs)
}

fn value_accessor_to_value(value: &ValueAccessor<'_>) -> Value {
    value.as_value().clone()
}

fn pyobj_to_value(value: &PyObj, scalar_bindings: &[ScalarBinding]) -> PyResult<Value> {
    Python::with_gil(|py| py_to_value(py, value.inner.as_ref(py), scalar_bindings, true))
}

fn scalar_binding_for_value<'a>(
    py: Python<'_>,
    value: &PyAny,
    scalar_bindings: &'a [ScalarBinding],
) -> PyResult<Option<&'a ScalarBinding>> {
    for binding in scalar_bindings {
        let is_instance = value.is_instance(binding.py_type.inner.as_ref(py))?;
        if is_instance {
            return Ok(Some(binding));
        }
    }
    Ok(None)
}

fn grommet_type_name(_py: Python<'_>, value: &PyAny) -> PyResult<Option<String>> {
    let ty = value.get_type();
    if !ty.hasattr("__grommet__")? {
        return Ok(None);
    }
    let meta = ty.getattr("__grommet__")?;
    let name: String = meta.getattr("name")?.extract()?;
    Ok(Some(name))
}

fn enum_name_for_value(_py: Python<'_>, value: &PyAny) -> PyResult<Option<String>> {
    let ty = value.get_type();
    if !ty.hasattr("__grommet_enum__")? {
        return Ok(None);
    }
    let name: String = value.getattr("name")?.extract()?;
    Ok(Some(name))
}

fn input_object_as_dict(py: Python<'_>, value: &PyAny) -> PyResult<Option<PyObject>> {
    let ty = value.get_type();
    if !ty.hasattr("__grommet__")? {
        return Ok(None);
    }
    let meta = ty.getattr("__grommet__")?;
    let kind: String = meta.getattr("kind")?.extract()?;
    if kind != "input" {
        return Ok(None);
    }
    let dataclasses = py.import("dataclasses")?;
    let dict_obj = dataclasses.call_method1("asdict", (value,))?;
    Ok(Some(dict_obj.into_py(py)))
}

fn py_to_field_value_for_type(
    py: Python,
    value: &PyAny,
    output_type: &TypeRef,
    scalar_bindings: &[ScalarBinding],
    abstract_types: &HashSet<String>,
) -> PyResult<FieldValue<'static>> {
    if value.is_none() {
        return Ok(FieldValue::value(Value::Null));
    }
    match output_type {
        TypeRef::NonNull(inner) => {
            py_to_field_value_for_type(py, value, inner, scalar_bindings, abstract_types)
        }
        TypeRef::List(inner) => {
            if let Ok(seq) = value.extract::<&PyList>() {
                let mut items = Vec::with_capacity(seq.len());
                for item in seq.iter() {
                    items.push(py_to_field_value_for_type(
                        py,
                        item,
                        inner,
                        scalar_bindings,
                        abstract_types,
                    )?);
                }
                Ok(FieldValue::list(items))
            } else if let Ok(seq) = value.extract::<&PyTuple>() {
                let mut items = Vec::with_capacity(seq.len());
                for item in seq.iter() {
                    items.push(py_to_field_value_for_type(
                        py,
                        item,
                        inner,
                        scalar_bindings,
                        abstract_types,
                    )?);
                }
                Ok(FieldValue::list(items))
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    "Expected list for GraphQL list type",
                ))
            }
        }
        TypeRef::Named(name) => {
            if abstract_types.contains(name.as_ref()) {
                let type_name = grommet_type_name(py, value)?.ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "Abstract types must return @grommet.type objects",
                    )
                })?;
                let inner = FieldValue::owned_any(PyObj {
                    inner: value.into_py(py),
                });
                Ok(inner.with_type(type_name))
            } else {
                py_to_field_value(py, value, scalar_bindings)
            }
        }
    }
}

fn py_to_field_value(
    py: Python,
    value: &PyAny,
    scalar_bindings: &[ScalarBinding],
) -> PyResult<FieldValue<'static>> {
    if let Some(binding) = scalar_binding_for_value(py, value, scalar_bindings)? {
        let serialized = binding.serialize.inner.as_ref(py).call1((value,))?;
        let value = py_to_value(py, serialized, scalar_bindings, false)?;
        return Ok(FieldValue::value(value));
    }
    if let Some(name) = enum_name_for_value(py, value)? {
        return Ok(FieldValue::value(Value::Enum(Name::new(name))));
    }
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
    if let Ok(seq) = value.extract::<&PyList>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() {
            items.push(py_to_field_value(py, item, scalar_bindings)?);
        }
        return Ok(FieldValue::list(items));
    }
    if let Ok(seq) = value.extract::<&PyTuple>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() {
            items.push(py_to_field_value(py, item, scalar_bindings)?);
        }
        return Ok(FieldValue::list(items));
    }
    Ok(FieldValue::owned_any(PyObj {
        inner: value.into_py(py),
    }))
}

fn py_to_value(
    py: Python,
    value: &PyAny,
    scalar_bindings: &[ScalarBinding],
    allow_scalar: bool,
) -> PyResult<Value> {
    if allow_scalar {
        if let Some(binding) = scalar_binding_for_value(py, value, scalar_bindings)? {
            let serialized = binding.serialize.inner.as_ref(py).call1((value,))?;
            return py_to_value(py, serialized, scalar_bindings, false);
        }
    }
    if let Some(name) = enum_name_for_value(py, value)? {
        return Ok(Value::Enum(Name::new(name)));
    }
    if let Some(dict_obj) = input_object_as_dict(py, value)? {
        return py_to_value(py, dict_obj.as_ref(py), scalar_bindings, allow_scalar);
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
    if let Ok(bytes) = value.extract::<&PyBytes>() {
        return Ok(Value::Binary(bytes.as_bytes().to_vec().into()));
    }
    if let Ok(list) = value.extract::<&PyList>() {
        let mut items = Vec::with_capacity(list.len());
        for item in list.iter() {
            items.push(py_to_value(py, item, scalar_bindings, true)?);
        }
        return Ok(Value::List(items));
    }
    if let Ok(tuple) = value.extract::<&PyTuple>() {
        let mut items = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            items.push(py_to_value(py, item, scalar_bindings, true)?);
        }
        return Ok(Value::List(items));
    }
    if let Ok(dict) = value.downcast::<PyDict>() {
        let mut map = indexmap::IndexMap::new();
        for (key, value) in dict.iter() {
            let key: String = key.extract()?;
            map.insert(Name::new(key), py_to_value(py, value, scalar_bindings, true)?);
        }
        return Ok(Value::Object(map));
    }
    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
        "Unsupported value type",
    ))
}

fn value_to_py(py: Python, value: &Value) -> PyResult<PyObject> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Boolean(b) => Ok(b.to_object(py)),
        Value::Number(number) => {
            if let Some(i) = number.as_i64() {
                Ok(i.to_object(py))
            } else if let Some(f) = number.as_f64() {
                Ok(f.to_object(py))
            } else {
                Ok(py.None())
            }
        }
        Value::String(s) => Ok(s.to_object(py)),
        Value::Enum(s) => Ok(s.as_str().to_object(py)),
        Value::List(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(value_to_py(py, item)?)?;
            }
            Ok(list.to_object(py))
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (key, value) in map {
                dict.set_item(key.as_str(), value_to_py(py, value)?)?;
            }
            Ok(dict.to_object(py))
        }
        Value::Binary(bytes) => Ok(PyBytes::new(py, bytes).to_object(py)),
    }
}

fn response_to_py(py: Python, response: async_graphql::Response) -> PyResult<PyObject> {
    let out = PyDict::new(py);
    out.set_item("data", value_to_py(py, &response.data)?)?;

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
        if !err.path.is_empty() {
            let path_list = PyList::empty(py);
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
            err_dict.set_item("path", path_list)?;
        }
        errors_list.append(err_dict)?;
    }
    out.set_item("errors", errors_list)?;
    Ok(out.to_object(py))
}

fn py_err_to_error(err: PyErr) -> Error {
    Error::new(err.to_string())
}
