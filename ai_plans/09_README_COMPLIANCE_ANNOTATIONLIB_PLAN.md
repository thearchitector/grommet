# README Compliance & annotationlib Refactor

> i've made a lot of changes. the objectives now are:
> 1. bring the API up to compliance with the README.md and grommet/schema.py, including dataclass fields, resolver, and descriptions.
> 2. refactor all annotation analysis to use 3.14's `annotationlib` (https://docs.python.org/3/library/annotationlib.html) and `annotationlib.get_annotations` (https://docs.python.org/3/howto/annotations.html#annotations-howto) falling back to `typing_extensions.get_annotations` if not available.
>
> analyze the entire codebase, and write a highly aggressive plan, ensuring it clearly and explicitly outlines all the steps and stages to achieve these goals. fundemental architecture changes are expected, but the code in README and `schema.py` MUST continue to work afterwards. you MUST eliminate every feature not explicitly detailed in the README. do NOT base your architectural decisions on the existing tests.

Two objectives: (1) make the public API match README.md and schema.py exactly, eliminating all features not present in the README, and (2) replace all annotation introspection with `annotationlib.get_annotations` (3.14) / `typing_extensions.get_annotations` (3.13 fallback).

### README API Surface (Exhaustive)

Everything below is explicitly demonstrated in the README. Nothing else should be public.

| Symbol | Kind | README usage |
|---|---|---|
| `grommet.type` | decorator (bare or with `description=`) | `@grommet.type`, `@grommet.type(description="All queries")` |
| `grommet.input` | decorator (bare or with `description=`) | `@grommet.input`, `@grommet.input(description="User input.")` |
| `grommet.field` | decorator (bare or with `description=`) | `@grommet.field`, `@grommet.field(description="A simple greeting")` |
| `grommet.Field` | `Annotated` metadata class | `Annotated[str, grommet.Field(description="A simple greeting")]` |
| `grommet.Hidden` | `Annotated` marker sentinel | `Annotated[int, grommet.Hidden]` |
| `grommet.Schema` | class | `grommet.Schema(query=, mutation=, subscription=)` |
| `schema.sdl` | cached property | `grommet.Schema(query=Query).sdl` |
| `schema.execute` | async method | `await schema.execute(query, variables=, state=)` |
| `grommet.Context[T]` | generic dataclass | resolver param, `.state`, `.field()`, `.look_ahead()` |
| `ClassVar` | stdlib, hides field | `bar: ClassVar[int]` |
| `_underscore` | convention, hides field | `_foo: int` |

### Symbols to REMOVE from public API

These are currently exported or supported but absent from the README:

- `grommet.interface` — decorator and all interface support
- `grommet.scalar` — decorator and all custom scalar support
- `grommet.union` — factory and all union support
- `grommet.enum` — decorator and all enum support
- `grommet.ID` — scalar class
- `grommet.Internal` — replaced by `grommet.Hidden`
- `grommet.Private` — replaced by `grommet.Hidden`
- `grommet.configure_runtime` — runtime configuration function
- `grommet.GrommetError`, `GrommetSchemaError`, `GrommetTypeError`, `GrommetValueError` — error classes (keep internal, remove from `__all__`)

### Key Behavioral Changes Required

1. **`@grommet.input` stays input-only (no resolvers, no output position).** The README uses `@grommet.type` for output types (including the hidden-fields example) and `@grommet.input` strictly for input types (mutation arguments). `@grommet.input` continues to forbid resolvers — standard GraphQL InputObject semantics.

2. **`@grommet.input` gains `description=` kwarg.** The README mutation example uses `@grommet.input(description="User input.")`. The current `input` decorator already accepts `description=`, so this is already supported.

3. **`grommet.Field(description=...)` metadata in `Annotated`.** A new `Field` class that can be placed in `Annotated[T, grommet.Field(description="...")]` to provide field-level description metadata for plain dataclass fields (not resolver fields). The README mutation example uses this on `AddUserInput` fields.

4. **`grommet.Hidden` sentinel.** Replaces `Internal`/`Private`. A sentinel object that, when present in `Annotated` metadata, excludes the field from the GraphQL schema.

5. **`build_schema_graph` function name.** `schema.py` imports `build_schema_graph` from `.plan`, but `plan.py` currently exports `build_schema_plan`. Rename to match.

---

## 1) Annotation Analysis Refactor [ ]

### Rationale

All annotation introspection must use `annotationlib.get_annotations(obj, format=annotationlib.Format.VALUE)` on Python 3.14+, falling back to `typing_extensions.get_annotations(obj, format=typing_extensions.Format.VALUE)` on Python 3.13. This replaces all uses of `typing.get_type_hints`, raw `__annotations__` access, and `resolver.__annotations__` access. This is the foundation layer — everything else depends on annotations being resolved correctly.

### Tasks

- [ ] **Create `grommet/_annotations.py` (private module)** containing a single `get_annotations(obj) -> dict[str, Any]` function. Implementation:
  ```python
  import sys

  if sys.version_info >= (3, 14):
      from annotationlib import Format, get_annotations as _get_annotations
  else:
      from typing_extensions import Format, get_annotations as _get_annotations

  def get_annotations(obj):
      return _get_annotations(obj, format=Format.VALUE)
  ```
  This is the **single source of truth** for annotation resolution across the entire codebase.

- [ ] **Replace `_resolve_type_hints` / `_cached_type_hints` / `_get_type_hints` in `annotations.py`** with imports from the new `_annotations.py` module. Delete the `_resolve_type_hints`, `_cached_type_hints`, and `_get_type_hints` functions entirely. All call sites (`annotations.py`, `plan.py`, `resolver.py`) switch to `from ._annotations import get_annotations`.

- [ ] **Replace `resolver.__annotations__` access in `decorators.py:_apply_field_resolvers`** (line 126: `marker.resolver.__annotations__.get("return", ...)`) with `get_annotations(marker.resolver).get("return", ...)`.

- [ ] **Replace `getattr(target, "__annotations__", {})` in `decorators.py:_apply_field_resolvers`** (line 120) with `get_annotations(target)`.

- [ ] **Remove `from typing import get_type_hints` and `from functools import lru_cache`** from `annotations.py` (the caching wrapper is no longer needed; `annotationlib` and `typing_extensions` handle their own caching).

- [ ] **Verify `include_extras=True` behavior.** `annotationlib.get_annotations` with `Format.VALUE` preserves `Annotated` wrappers (unlike `typing.get_type_hints` which strips them without `include_extras=True`). Confirm this with a test.

### Implementation Notes

- `pyproject.toml` already has `typing-extensions~=4.15;python_version<'3.14'` as a dependency, so `typing_extensions.get_annotations` is available on 3.13.
- `typing_extensions.get_annotations` mirrors `annotationlib.get_annotations` API including `format=` parameter and `Format` enum.
- The `lru_cache` on `_cached_type_hints` is fragile (unhashable types cause fallback). Removing it simplifies the code. If performance is a concern, caching can be reintroduced later at a higher level.

---

## 2) Public API Surface: New Symbols [ ]

### Rationale

The README demonstrates `grommet.Field(description=...)` and `grommet.Hidden` as `Annotated` metadata. These must exist and be functional.

### Tasks

- [ ] **Create `Field` class in `metadata.py`.** A frozen dataclass or simple class:
  ```python
  @dataclasses.dataclass(frozen=True, slots=True)
  class Field:
      """Annotated metadata providing field-level GraphQL configuration."""
      description: str | None = None
  ```
  This is placed in `Annotated[T, Field(description="...")]` and detected during annotation analysis.

- [ ] **Create `Hidden` sentinel in `metadata.py`.** A simple singleton sentinel:
  ```python
  class _HiddenType:
      """Marker to exclude a field from the GraphQL schema."""
      _instance = None
      def __new__(cls):
          if cls._instance is None:
              cls._instance = super().__new__(cls)
          return cls._instance
      def __repr__(self):
          return "Hidden"

  Hidden = _HiddenType()
  ```
  Detected in `Annotated` metadata during annotation analysis. Replaces `_INTERNAL_MARKER`.

- [ ] **Update `analyze_annotation` in `annotations.py`** to detect `Hidden` and `Field` in `Annotated` metadata:
  - `is_hidden`: `True` if `Hidden` is in metadata tuple (replaces `is_internal`)
  - Extract `Field` instance from metadata for description (store as `field_meta` on `AnnotationInfo`)

- [ ] **Update `is_internal_field` → `is_hidden_field`** to check for `Hidden` sentinel instead of `_INTERNAL_MARKER`, plus retain `ClassVar` and `_underscore` checks.

- [ ] **Export `Field` and `Hidden` from `grommet/__init__.py`.**

### Implementation Notes

- `Field` in `Annotated` metadata is distinct from `@grommet.field` decorator. The decorator creates resolver-backed fields; `Field(...)` in `Annotated` provides metadata for plain dataclass fields.
- When both `@grommet.field(description=...)` AND `Annotated[T, Field(description=...)]` are present, the `@grommet.field` description takes precedence (decorator is more specific).
- The `Field` description must propagate through `FieldPlan.description` into the Rust schema builder.

---

## 3) Public API Surface: Remove Symbols [ ]

### Rationale

Every feature not in the README must be eliminated. This is a large removal touching every layer.

### Tasks

#### Python removals

- [ ] **Delete `grommet.interface` decorator** from `decorators.py` (the `interface` function and its overloads, ~40 lines). Remove `TypeKind.INTERFACE` usage. Remove `_INTERFACE_IMPLEMENTERS` dict and `_interface_implementers` function from `metadata.py`.

- [ ] **Delete `grommet.scalar` decorator** from `decorators.py` (the `scalar` function and its overloads, ~50 lines). Remove `ScalarMeta` class and `_register_scalar` / `_GROMMET_SCALARS` from `metadata.py`.

- [ ] **Delete `grommet.union` factory** from `decorators.py` (~20 lines). Remove `UnionMeta` class and `_register_union` / `_GROMMET_UNIONS` from `metadata.py`.

- [ ] **Delete `grommet.enum` decorator** from `decorators.py` (the `enum_type` function and its overloads, ~30 lines). Remove `EnumMeta` class and `_register_enum` / `_GROMMET_ENUMS` from `metadata.py`.

- [ ] **Delete `grommet.ID` class** from `metadata.py`.

- [ ] **Delete `grommet.Internal` and `grommet.Private`** from `metadata.py`. Replace with `Hidden` (see section 2).

- [ ] **Delete `grommet.configure_runtime`** from `runtime.py`. Delete `runtime.py` entirely. Remove from `__init__.py`.

- [ ] **Remove error classes from `__all__`** in `__init__.py`. The error classes can remain as internal implementation details but should not be part of the public API.

- [ ] **Delete `grommet/types.py`** — only contains a `TYPE_CHECKING`-only type alias `RootType` that is unused.

- [ ] **Clean up `annotations.py`**: Remove all scalar/enum/union/interface-related functions:
  - `_get_scalar_meta`, `_get_enum_meta`, `_get_union_meta`
  - `_is_scalar_type`, `_is_enum_type`, `_is_union_type`
  - `_is_grommet_type` (simplify to just check for `TypeMeta`)
  - `walk_annotation` / `_walk_inner` (rewrite to only walk types, no scalars/enums/unions)
  - `_type_spec_from_annotation` — remove scalar/enum/union branches

- [ ] **Clean up `coercion.py`**: Remove enum coercion, scalar coercion, ID coercion. Keep only: input type coercion, list coercion, primitive coercion (str/int/float/bool).

- [ ] **Clean up `plan.py`**: Remove `ScalarPlan`, `EnumPlan`, `UnionPlan` classes. Remove scalar/enum/union fields from `SchemaPlan`. Remove all scalar/enum/union discovery and planning code from `build_schema_plan`. Remove interface-specific code paths.

- [ ] **Clean up `metadata.py`**: Remove `GrommetMetaType` entries for SCALAR, ENUM, UNION, FIELD. Remove `_TYPE_KIND_TO_META` mapping (simplify). Remove `GrommetMeta` base class if no longer needed. Remove `_SCALARS` ID entry.

- [ ] **Clean up `errors.py`**: Remove error factory functions for eliminated features: `not_grommet_scalar`, `not_grommet_enum`, `not_grommet_union`, `union_input_not_supported`, `enum_requires_enum_subclass`, `union_requires_name`, `union_requires_types`, `union_requires_object_types`, `scalar_requires_callables`, `invalid_enum_value`. Keep errors for features that remain.

#### Rust removals

- [ ] **Remove scalar support from `src/build.rs`**: Delete `scalar_defs` parameter and scalar registration loop. Delete `ScalarBinding` usage throughout.

- [ ] **Remove enum support from `src/build.rs`**: Delete `enum_defs` parameter and enum registration loop.

- [ ] **Remove union support from `src/build.rs`**: Delete `union_defs` parameter and union registration loop.

- [ ] **Remove interface support from `src/build.rs`**: Delete `TypeKind::Interface` match arm and `build_interface_field` function.

- [ ] **Remove `ScalarBinding` from `src/types.rs`**. Remove `ScalarDef`, `EnumDef`, `UnionDef` structs. Remove `TypeKind::Interface`. Remove `scalar_bindings` and `abstract_types` from `FieldContext`.

- [ ] **Simplify `src/parse.rs`**: Remove parsing of scalars, enums, unions from `parse_schema_plan`. Remove `ScalarBinding` construction. Adjust the return type of `parse_schema_plan` to only return `(SchemaDef, Vec<TypeDef>, HashMap<String, PyObj>)`.

- [ ] **Simplify `src/values.rs`**: Remove `scalar_binding_for_value`, `is_builtin_type`, `enum_name_for_value`, `meta_type_value`, all scalar/enum serialization code. Remove `ScalarBinding` parameter from all functions. Simplify `py_to_field_value`, `py_to_value`, etc.

- [ ] **Simplify `src/resolver.rs`**: Remove `ScalarBinding` from `resolve_field`, `resolve_subscription_stream`, and all downstream functions. Remove `abstract_types` from field context.

- [ ] **Remove `configure_runtime` from `src/api.rs`** and `src/lib.rs` module registration. Delete `src/runtime.rs`... wait, `runtime.rs` contains `future_into_py` and `await_awaitable` which are still needed. Only remove `configure_runtime` from `api.rs`.

- [ ] **Update `src/lib.rs`** to remove `configure_runtime` from module registration.

### Implementation Notes

- The Rust `build_schema` function signature changes dramatically. The new signature should be roughly:
  ```rust
  pub(crate) fn build_schema(
      schema_def: SchemaDef,
      type_defs: Vec<TypeDef>,
      resolver_map: HashMap<String, PyObj>,
  ) -> PyResult<Schema>
  ```
- `FieldContext` simplifies to just `resolver`, `arg_names`, `source_name`, `output_type`.
- `_SCALARS` dict in `metadata.py` (mapping `str`→`"String"`, `int`→`"Int"`, etc.) MUST be kept — these are GraphQL built-in scalars, not custom scalars. Only the `ID` entry is removed.

---

## 4) `@grommet.input` Cleanup [ ]

### Rationale

`@grommet.input` has standard GraphQL InputObject semantics: no resolvers, input-position only. The README confirms this — the hidden-fields example uses `@grommet.type` for output types and `@grommet.input` strictly for mutation argument types. No behavior change is needed beyond ensuring `description=` propagation works (already supported by the decorator).

### Tasks

- [ ] **Keep `allow_resolvers=False` for `@grommet.input`.** No change needed — current behavior is correct.

- [ ] **Verify `@grommet.input(description=...)` works.** The `input` decorator already accepts `description=` kwarg and passes it to `_wrap_type_decorator`. Confirm this propagates to `TypePlan.description` → `TypeDef.description` → Rust `InputObject.description()`.

- [ ] **Verify `Annotated[T, grommet.Field(description=...)]` works on input type fields.** After section 5 is implemented, confirm that field descriptions on `@grommet.input` dataclass fields appear in the generated SDL.

### Implementation Notes

- The README mutation example: `AddUserInput` is `@grommet.input(description="User input.")` with fields using `Annotated[str, grommet.Field(description="The name of the user.")]`.
- `@grommet.type` is for output types (Object). `@grommet.input` is for input types (InputObject). No dual-registration needed.

---

## 5) `Field` Description Propagation [ ]

### Rationale

`Annotated[str, grommet.Field(description="A simple greeting")]` must cause the field's description to appear in the generated SDL. This requires the annotation analysis to extract the `Field` metadata and propagate it through to the Rust schema builder.

### Tasks

- [ ] **Extract `Field` metadata in `plan.py:_build_field_plans` and `_build_input_field_plans`.** When building a `FieldPlan`, check the annotation's `AnnotationInfo` for a `Field` instance in the metadata. If present and the field has no resolver-provided description, use the `Field.description`.

- [ ] **Priority: `@grommet.field(description=...)` > `Annotated[T, Field(description=...)]` > `None`.** If a field has both a resolver description and an Annotated Field description, the resolver description wins.

- [ ] **Wire through to Rust.** No Rust changes needed — `FieldDef.description` already propagates to the `apply_metadata!` macro which sets the description on the GraphQL field. The Python plan just needs to populate `FieldPlan.description` correctly.

### Implementation Notes

- The README SDL example:
  ```
  """All queries"""
  query Query {
    "A simple greeting"
    greeting: String!
  }
  ```
  The `"A simple greeting"` comes from `grommet.Field(description="A simple greeting")` on the `greeting` dataclass field.

---

## 6) Rename `build_schema_plan` → `build_schema_graph` [ ]

### Rationale

`schema.py` (pragma: no ai) imports `build_schema_graph` from `.plan`. The function must match.

### Tasks

- [ ] **Rename `build_schema_plan` to `build_schema_graph` in `plan.py`.**
- [ ] **Rename `SchemaPlan` → `SchemaGraph`** (or keep `SchemaPlan` and just rename the function — depends on how much churn is desired). Since the Rust side reads attributes off the object, the class name doesn't matter to Rust. Rename both for consistency: `SchemaPlan` → `SchemaGraph`, `TypePlan` → `TypeNode`, `FieldPlan` → `FieldNode`, `ArgPlan` → `ArgNode` to reflect the "graph" naming.
- [ ] **Update all internal references** in `plan.py`, `resolver.py`, `_core.pyi`, and anywhere else that references these names.

### Implementation Notes

- `_core.pyi` line 4 imports `SchemaPlan` — update to new name.
- The Rust `parse.rs` reads attributes by string name (`"query"`, `"types"`, `"fields"`, etc.) so renaming the Python classes has zero Rust impact.

---

## 7) `__init__.py` Final Cleanup [ ]

### Rationale

The public API must exactly match the README.

### Tasks

- [ ] **Rewrite `grommet/__init__.py`** to export exactly:
  ```python
  from .context import Context
  from .decorators import field, input, type
  from .metadata import Field, Hidden
  from .schema import Schema

  __all__ = [
      "Context",
      "Field",
      "Hidden",
      "Schema",
      "field",
      "input",
      "type",
  ]
  ```

- [ ] **Remove all other imports** — no `enum`, `interface`, `scalar`, `union`, `ID`, `Internal`, `Private`, `configure_runtime`, error classes.

### Implementation Notes

- Error classes remain importable via `grommet.errors` for advanced users but are NOT in `__all__` or the top-level namespace.
- `Field` (Annotated metadata) vs `field` (decorator) — both exported, distinguished by case.

---

## 8) Test Rewrite [ ]

### Rationale

Existing tests are based on the old API surface. All tests must be rewritten to cover the README-defined API exclusively. The user said not to base architectural decisions on existing tests.

### Tasks

- [ ] **Delete all existing test files** in `tests/python/` that test eliminated features (enum, scalar, union, interface, ID, Internal/Private, configure_runtime).

- [ ] **Create new test files** covering each README example end-to-end:
  - `test_basic_query.py` — simple greeting query with default value
  - `test_field_descriptions.py` — `grommet.Field(description=...)` and SDL output
  - `test_resolver_fields.py` — `@grommet.field` with args, optional args
  - `test_hidden_fields.py` — `_prefix`, `ClassVar`, `grommet.Hidden`
  - `test_mutations.py` — `@grommet.input` for mutation arguments
  - `test_subscriptions.py` — `AsyncIterator` subscription streaming
  - `test_context.py` — `grommet.Context[T]`, state, field lookahead

- [ ] **Update Rust tests** in `tests/test_core.rs` if they reference eliminated features.

- [ ] **Run `uv run pytest` and `uv run cargo test`** to verify all tests pass.

- [ ] **Run `prek run -a`** to verify full compliance.

### Implementation Notes

- Tests should NOT use `TYPE_CHECKING` blocks per AGENTS.md rules for unit tests.
- Each README code example should be a runnable test case.

---

## Execution Order

The sections above are ordered by dependency:

1. **Section 1** (annotationlib) — foundation, no other section depends on old annotation code
2. **Section 2** (new symbols) — `Field`, `Hidden` needed by later sections
3. **Section 3** (remove symbols) — massive cleanup, clears the way for focused work
4. **Section 6** (rename) — quick, unblocks schema.py compatibility
5. **Section 5** (Field description propagation) — depends on `Field` class and clean plan code
6. **Section 4** (input cleanup) — verification only, depends on section 5
7. **Section 7** (`__init__.py`) — final public API lockdown
8. **Section 8** (tests) — validate everything works
