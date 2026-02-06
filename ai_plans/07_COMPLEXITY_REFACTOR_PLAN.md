# Complexity Reduction Refactor

Aggressive structural refactor of grommet targeting fundamental complexity sources across both the
Rust and Python sides. Backwards compatibility is not a concern except that the public API used by
the README examples must continue to work:

- `@grommet.type`, `@grommet.type(name=...)`, `@grommet.field`, `@grommet.input`
- `grommet.Schema(query=..., mutation=..., subscription=...)`
- `schema.execute(query)` (async), `schema.subscribe(query)` (async iter)
- `grommet.Info` with `.context`, `.root`, `.field_name`
- Resolver params: `parent`, `info`, plus GraphQL args by name
- `AsyncIterator[T]` return for subscriptions
- `str | None` optional fields, `@grommet.enum`, `@grommet.scalar`, `@grommet.union`

## 1) Eliminate the Dict Serialization FFI Boundary [x]

### Rationale

The single largest source of complexity in the codebase. The current data flow:

1. Python `build_schema_plan()` → structured `SchemaPlan` dataclasses
2. Python `_build_schema_definition()` flattens `SchemaPlan` into nested `dict[str, Any]` (~120 lines)
3. Dicts cross FFI to Rust `SchemaWrapper::new`
4. Rust `parse.rs` (~370 lines) re-parses dicts back into Rust structs via `FromPyObject` + `from_item_all`
5. Rust `build.rs` constructs `async_graphql::dynamic::Schema` from those structs

Steps 2–4 are ~490 lines of pure serialization/deserialization. Python builds structured data,
flattens it to dicts, and Rust rebuilds the structure. This is eliminated by passing the plan
dataclasses directly to Rust and reading their attributes.

### Tasks

- [ ] Move resolver wrapping (`_wrap_resolver`) from `_build_schema_definition` into
      `_build_field_plans` in `plan.py`. Each `FieldPlan` gains a `resolver_key: str | None` and
      a `wrapped_resolver: Callable | None`. The `SchemaPlan` gains a
      `resolvers: dict[str, Callable]` populated during planning.
- [ ] Change all `#[derive(FromPyObject)]` / `#[pyo3(from_item_all)]` structs in `parse.rs` to use
      `#[pyo3(from_attributes)]` so they extract from dataclass attributes via `getattr` instead of
      `__getitem__`.
- [ ] Change `SchemaWrapper::new` to accept the `SchemaPlan` dataclass directly (as a single
      `Bound<'_, PyAny>`) plus the resolvers dict. The signature becomes:
      `fn new(py, plan: &Bound<'_, PyAny>, resolvers: &Bound<'_, PyDict>) -> PyResult<Self>`.
- [ ] Rename Python `FieldPlan.graphql_type` → `type` and `ArgPlan.graphql_type` → `type` to match
      what Rust expects (or use `#[pyo3(attribute("graphql_type"))]` on the Rust side).
- [ ] Delete `_build_schema_definition` from `schema.py` entirely.
- [ ] Delete the `*Input` parse structs (`SchemaListsInput`, `SchemaBlockInput`, `TypeDefInput`,
      `FieldDefInput`, `ArgDefInput`, `EnumDefInput`, `UnionDefInput`, `ScalarDefInput`,
      `ScalarBindingInput`) and the `extract_with_missing` / `map_missing_field` / `key_error_key`
      helpers — these only exist to handle dict key errors, which are impossible when reading from
      typed dataclass attributes.
- [ ] Simplify `parse_schema_definition` to directly extract from the plan's attributes using the
      existing Rust definition structs with `from_attributes`.
- [ ] Update `_core.pyi` to match the new signature.
- [ ] Run `cargo test` and `uv run pytest`.

### Implementation Notes

The scalar bindings list can be embedded in `SchemaPlan` as well (a list of dicts or a dedicated
`ScalarBinding` dataclass), further flattening the `Schema.__init__` call to just
`_core.Schema(plan, resolvers)`.

The `*_from_input` functions in `parse.rs` (e.g. `type_def_from_input`, `field_def_from_input`)
become unnecessary once we extract directly into the existing `TypeDef`/`FieldDef` Rust structs.
If pyo3's `FromPyObject` derive can target the existing structs directly (they don't currently
derive it), add `#[derive(FromPyObject)]` to them. Otherwise, keep thin parse functions that
do `getattr` extraction.

## 2) Eliminate the TypeRef String Round-Trip [x]

### Rationale

Python's `TypeSpec.to_graphql()` builds strings like `"[String!]!"` which are passed to Rust, where
`parse_type_ref` / `parse_type_ref_uncached` in `build.rs` parse them back into `TypeRef` trees.
This string round-trip exists solely because the FFI boundary was dict-based. With the plan
dataclasses crossing directly, `TypeSpec` can be passed as a structured object and converted to
`TypeRef` without string parsing.

### Tasks

- [ ] Add a Rust extraction function `type_spec_to_type_ref(py, spec: &Bound<'_, PyAny>) -> TypeRef`
      that reads `.kind`, `.name`, `.of_type`, `.nullable` from the Python `TypeSpec` dataclass and
      builds a `TypeRef` directly.
- [ ] Replace all `parse_type_ref(field_def.type_name.as_str())` calls in `build.rs` with the new
      structured conversion.
- [ ] Remove `parse_type_ref`, `parse_type_ref_uncached`, and the `TYPE_REF_CACHE` thread-local
      from `build.rs`.
- [ ] Change `FieldDef.type_name: String` → `FieldDef.output_type: TypeRef` (compute it once during
      parsing).
- [ ] Change `FieldPlan.graphql_type: str` → `FieldPlan.type_spec: TypeSpec` on the Python side
      (stop calling `.to_graphql()` during planning).
- [ ] Similarly for `ArgPlan.graphql_type` → `ArgPlan.type_spec`.
- [ ] Run `cargo test` and `uv run pytest`.

### Implementation Notes

`TypeSpec` is a simple recursive dataclass:

```python
@dataclass(frozen=True, slots=True)
class TypeSpec:
    kind: str          # "named" or "list"
    name: str | None
    of_type: TypeSpec | None
    nullable: bool
```

On the Rust side, `type_spec_to_type_ref` is ~15 lines:

```rust
fn type_spec_to_type_ref(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<TypeRef> {
    let kind: String = spec.getattr("kind")?.extract()?;
    let nullable: bool = spec.getattr("nullable")?.extract()?;
    let ty = if kind == "list" {
        let inner = spec.getattr("of_type")?;
        TypeRef::List(Box::new(type_spec_to_type_ref(py, &inner)?))
    } else {
        let name: String = spec.getattr("name")?.extract()?;
        TypeRef::named(name)
    };
    Ok(if nullable { ty } else { TypeRef::NonNull(Box::new(ty)) })
}
```

This also opens the door to caching `TypeRef` by identity of the frozen `TypeSpec` object, which
is cheaper than string hashing.

## 3) Merge Python Modules `annotations.py` + `typespec.py` + `typing_utils.py` [x]

### Rationale

These three files form a single concern — "analyze a Python type annotation and produce a GraphQL
type spec" — split across three modules totaling ~300 lines. `typespec.py` imports from
`annotations.py` for every operation. `typing_utils.py` is 30 lines used by both. Merging them
reduces import complexity and makes the annotation→type-spec pipeline readable in one place.

### Tasks

- [ ] Create `grommet/types.py` containing the merged content of `annotations.py`, `typespec.py`,
      and `typing_utils.py`.
- [ ] Move `TypeSpec` from `metadata.py` into `types.py` (it belongs with the type-spec logic, not
      with decorator metadata).
- [ ] Move `_SCALARS` mapping from `metadata.py` into `types.py`.
- [ ] Update all internal imports across the package.
- [ ] Delete `annotations.py`, `typespec.py`, `typing_utils.py`.
- [ ] Merge `info.py` (15 lines, just the `Info` dataclass) into `metadata.py`.
- [ ] Run `uv run pytest` and `uv run mypy .`.

### Implementation Notes

The resulting module layout becomes:

```
grommet/
  __init__.py      # public API
  _core.pyi        # Rust stubs
  coercion.py      # value coercion for resolver args and defaults
  decorators.py    # @type, @field, @input, @scalar, @enum, @interface, union()
  errors.py        # error factories
  metadata.py      # GrommetMeta hierarchy, registries, Info
  plan.py          # SchemaPlan building + resolver wrapping
  resolver.py      # resolver spec building, parameter introspection
  runtime.py       # configure_runtime
  schema.py        # Schema class (thin wrapper around _core)
  types.py         # annotation analysis, TypeSpec, type-spec-from-annotation
```

11 files down from 16 (including the deleted `registry.py`). Each module owns a single concern.

## 4) Delete `registry.py` [x]

### Rationale

`grommet/registry.py` contains `_traverse_schema` and a duplicate `_get_field_meta`. Both duplicate
logic already in `plan.py::build_schema_plan`. Neither function has live call sites outside tests.
The `TraversalResult` dataclass is unused. This is ~105 lines of dead code.

### Tasks

- [ ] Delete `grommet/registry.py`.
- [ ] Delete or redirect `tests/python/test_registry_coverage.py` to test equivalent code in
      `plan.py`.
- [ ] Run `uv run pytest` and `uv run mypy .`.

## 5) Unify Python Decorator Boilerplate [x]

### Rationale

`type()`, `interface()`, and `input()` in `decorators.py` repeat the same pattern: overloaded
signatures, inner `wrap()`, dataclass check, optional field-resolver application, dataclass
rebuild with identical parameter forwarding (~10 copy-pasted lines), `TypeMeta` creation, and
registration. The dataclass rebuild block is verbatim identical between `type` and `interface`.

### Tasks

- [ ] Extract `_rebuild_dataclass(target: pytype) -> pytype` that re-invokes
      `dataclasses.dataclass` with the target's existing `__dataclass_params__`.
- [ ] Extract `_wrap_type_decorator(target, *, kind, name, description, implements,
      allow_resolvers)` that performs the full decoration sequence: check `is_dataclass` →
      optionally `_apply_field_resolvers` → optionally `_rebuild_dataclass` → create `TypeMeta` →
      `_register_type`.
- [ ] Rewrite `type`, `interface`, and `input` as thin wrappers: two `@overload` signatures plus a
      three-line body that calls `_wrap_type_decorator`.
- [ ] Run `uv run pytest` and `uv run mypy .`.

### Implementation Notes

`input` differs in two ways: it forbids `_FieldResolver` and never calls `_apply_field_resolvers`.
The shared helper handles this via `allow_resolvers=False`.

## 6) Deduplicate Rust `build.rs` Field Builders [x]

### Rationale

`build_field`, `build_interface_field`, `build_subscription_field`, and `build_input_field` share
identical argument-attachment loops and metadata-application blocks (~100 lines of duplication).

### Tasks

- [ ] Extract `fn build_input_values(args: &[ArgDef], scalars: &[ScalarBinding]) -> PyResult<Vec<InputValue>>`.
- [ ] Add a `apply_metadata!` macro for description + deprecation (the async-graphql field types
      don't share a trait):
      ```rust
      macro_rules! apply_metadata {
          ($field:expr, $def:expr) => {{
              let mut f = $field;
              if let Some(desc) = $def.description.as_ref() { f = f.description(desc.as_str()); }
              if let Some(dep) = $def.deprecation.as_ref() { f = f.deprecation(Some(dep.as_str())); }
              f
          }};
      }
      ```
- [ ] Refactor all four builders to use the shared helpers.
- [ ] Run `cargo test` and `uv run pytest`.

## 7) Deduplicate Rust `values.rs` Sequence Converters [x]

### Rationale

`convert_sequence_to_field_values`, `convert_sequence_to_field_values_untyped`, and the list/tuple
branches in `py_to_value` all duplicate the same PyList-or-PyTuple dispatch pattern.

### Tasks

- [ ] Extract `fn collect_sequence<T>(value, convert) -> PyResult<Vec<T>>` that handles the
      PyList/PyTuple dispatch and iteration with a closure.
- [ ] Collapse `convert_sequence_to_field_values` and `convert_sequence_to_field_values_untyped`
      into direct calls to the helper.
- [ ] Apply the same to the list/tuple branches in `py_to_value`.
- [ ] Run `cargo test` and `uv run pytest`.

### Implementation Notes

```rust
fn collect_sequence<T>(
    value: &Bound<'_, PyAny>,
    mut convert: impl FnMut(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Vec<T>> {
    if let Ok(seq) = value.cast::<PyList>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() { items.push(convert(&item)?); }
        Ok(items)
    } else if let Ok(seq) = value.cast::<PyTuple>() {
        let mut items = Vec::with_capacity(seq.len());
        for item in seq.iter() { items.push(convert(&item)?); }
        Ok(items)
    } else { Err(expected_list_value()) }
}
```

## 8) Merge Rust `resolve_python_value` / `resolve_subscription_value` [x]

### Rationale

These two functions in `src/resolver.rs` are ~80% identical. Both extract context, call the
resolver or resolve from parent, and optionally await. The subscription variant adds one extra
`__aiter__`/`__anext__` check. Maintaining two copies means every resolver-path change must be
applied twice.

### Tasks

- [ ] Merge into `resolve_value(ctx, resolver, arg_names, field_name, source_name,
      is_subscription) -> Result<Py<PyAny>, Error>`.
- [ ] The subscription-specific logic (check `__aiter__` before awaiting) becomes a single
      `if is_subscription { ... }` block after the shared resolver call.
- [ ] Update `resolve_field` and `resolve_subscription_stream` to call the unified function.
- [ ] Run `cargo test` and `uv run pytest`.

## 9) Consolidate Duplicate Request-Building in `api.rs` [x]

### Rationale

`execute` and `subscribe` duplicate ~25 lines of identical variable conversion, root/context
wrapping, and `Request` construction.

### Tasks

- [ ] Extract `convert_variables(variables, scalars) -> PyResult<Option<Value>>`.
- [ ] Extract `build_request(query, vars, root, context) -> Request`.
- [ ] Rewrite `execute` and `subscribe` to use the shared helpers.
- [ ] Run `cargo test` and `uv run pytest`.

## 10) Use Enums for Type Kinds on Both Sides [x]

### Rationale

`TypeDef.kind` in Rust is a `String` matched against `"object"`, `"interface"`, `"subscription"`,
`"input"`. A typo silently falls through to the error case. Similarly, `TypeMeta.kind` in Python is
a string that gets looked up in a dict during `__post_init__`. Both should be proper enums for
compile-time / runtime safety.

### Tasks

- [ ] Python: Replace `TypeMeta.kind: str` with `TypeMeta.kind: GrommetMetaType` directly. Remove
      the string→enum lookup in `__post_init__`. Update all `meta.kind == "object"` checks to use
      enum comparison.
- [ ] Rust: Define `enum TypeKind { Object, Interface, Subscription, Input }` in `types.rs` with a
      `FromPyObject` impl (parsing from the Python enum's `.value` string). Replace
      `TypeDef.kind: String` with `TypeDef.kind: TypeKind`. Replace the `match kind.as_str()` block
      in `build_schema` with an exhaustive `match kind { ... }`.
- [ ] Run `cargo test` and `uv run pytest`.

## 11) Simplify `PyObj` — Remove `Mutex` [x]

### Rationale

`PyObj` wraps every Python object in `Arc<Mutex<Py<PyAny>>>`. Under pyo3 0.28, `Py<PyAny>` is
`Send`. The `Mutex` exists solely to provide `Sync` for `FieldValue::owned_any`'s
`Send + Sync + 'static` bound. However, all access to the inner `Py<PyAny>` goes through
`Python::attach` (which holds the GIL), so `Sync` is safe — the GIL serializes all access.
The mutex lock on every `.bind()` and `.clone_ref()` adds contention in the hot path.

### Tasks

- [ ] Replace `Arc<Mutex<Py<PyAny>>>` with a newtype `SyncPyObj(Py<PyAny>)` that has
      `unsafe impl Sync for SyncPyObj {}`, wrapped in `Arc`.
- [ ] Remove all `.lock().expect(...)` calls from `bind` and `clone_ref`.
- [ ] Benchmark before/after with `benchmarks/`.
- [ ] Run `cargo test` and `uv run pytest`.

### Implementation Notes

The `unsafe impl Sync` is justified because: (a) `Py<PyAny>` is an opaque pointer with no interior
mutability, (b) all dereferences go through `Python::attach` which acquires the GIL, serializing
access. Keep the `unsafe` in a minimal newtype to limit scope:

```rust
struct SyncPyObj(Py<PyAny>);
unsafe impl Sync for SyncPyObj {}

#[derive(Clone)]
pub(crate) struct PyObj {
    inner: Arc<SyncPyObj>,
}
```

## 12) Remove Dead Code in `parse.rs` [x]

### Rationale

Several functions are annotated `#[allow(dead_code)]`. Some are actually live (`parse_type_def`,
`parse_field_def`, `parse_arg_def` — all called internally) and should lose the annotation. Others
are truly dead (`extract_optional_string`, `parse_enum_def`, `parse_union_def`, `parse_scalar_def`)
and should be deleted.

Note: If section 1 is completed first, most of `parse.rs` is deleted anyway. This section exists as
a standalone cleanup if sections are done out of order.

### Tasks

- [ ] Delete `extract_optional_string` (no callers).
- [ ] Delete `parse_enum_def`, `parse_union_def`, `parse_scalar_def` (no external callers; the
      `*_from_input` variants are used instead).
- [ ] Remove `#[allow(dead_code)]` from `parse_type_def`, `parse_field_def`, `parse_arg_def`.
- [ ] Run `cargo test` and `uv run pytest`.
