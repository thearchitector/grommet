# Grommet Complexity + Performance Refactor Plan

Refactor the Python/Rust schema pipeline, resolver execution, and value conversion to reduce cognitive overhead while
pushing runtime throughput. Public API compatibility is limited to dataclass decorators and async resolvers/iterators.

---

## Summary of Findings

### Python Side
| File | Primary Role | Complexity Issues |
|------|--------------|-------------------|
| `registry.py` | Schema traversal | 4 nearly-identical `_iter_*_refs` functions; walks dataclasses once |
| `schema.py` | Schema definition builder | Walks dataclasses again; duplicates type-name lookups |
| `typespec.py` | Annotation → TypeSpec | Re-analyzes annotations; `_is_*`/`_get_*_meta` scattered |
| `coercion.py` | Value coercion | Parallel annotation analysis; repeated list/optional unwrapping |
| `resolver.py` | Resolver wrapping | Heavy introspection on every wrap; builds coercer list per-call |
| `annotations.py` | `analyze_annotation()` → `AnnotationInfo` | Called repeatedly for same annotation |
| `decorators.py` | User-facing decorators | `_apply_field_resolvers` re-walks annotations |
| `metadata.py` | Metadata classes + global registries | Storing types in sets; `_get_*_meta` requires getattr |

### Rust Side
| File | Primary Role | Complexity Issues |
|------|--------------|-------------------|
| `build.rs` | Schema construction | `build_field`/`build_subscription_field` near-duplicates; 7 Arcs cloned per closure |
| `resolver.rs` | Resolver dispatch | `resolve_python_value`/`resolve_subscription_value` similar; repeated context extraction |
| `values.rs` | Python ↔ async-graphql conversion | `py_to_field_value`/`py_to_value` overlap; linear scan for scalar bindings |
| `parse.rs` | Dict → Rust structs | `parse_type_ref` called per field with no cache |
| `types.rs` | Core types (`PyObj`, etc.) | Mutex around `Py<PyAny>` adds locking overhead |

### Python–Rust Interaction
1. **Schema definition** passed as nested dicts (schema → types → fields → args).
2. **Resolvers** passed as `Dict[str, Callable]` keyed by `"TypeName.fieldName"`.
3. **Scalar bindings** passed as list of `{name, python_type, serialize, parse_value}`.
4. **At resolution time**, Rust calls Python resolvers via PyO3, converts results back.

---

## 1) Python: Unified Annotation Walker [x]

### Rationale
`_iter_type_refs`, `_iter_scalar_refs`, `_iter_enum_refs`, `_iter_union_refs` in `registry.py` are structurally
identical—differing only in the predicate. Consolidate into a single `walk_annotation()` generator that yields
`(kind, pytype)` tuples, where `kind ∈ {type, scalar, enum, union}`.

### Tasks
- [x] Add `walk_annotation(annotation) -> Iterable[tuple[str, type]]` in `annotations.py`.
- [x] Replace 4 `_iter_*` functions in `registry.py` with a single loop over `walk_annotation()`.
- [x] Update `_traverse_schema()` to collect all ref kinds in one pass.
- [x] Remove redundant `analyze_annotation()` calls inside the old `_iter_*` helpers.

### Estimated Impact
- **Complexity**: −60 lines, single concept instead of 4 parallel patterns.
- **Performance**: Fewer repeated `analyze_annotation()` calls per annotation.

---

## 2) Python: SchemaPlan Abstraction [x]

### Rationale
Currently, `_traverse_schema()` walks dataclasses to discover types, then `_build_schema_definition()` walks them
again to build field dicts. Introduce a planning layer that inspects each dataclass once and caches field metadata.

### Tasks
- [x] Create `plan.py` with `SchemaPlan`, `TypePlan`, `FieldPlan` dataclasses.
- [x] `FieldPlan` holds: `name`, `source`, `graphql_type`, `resolver`, `args`, `description`, `deprecation`.
- [x] `TypePlan` holds: `kind`, `name`, `fields: list[FieldPlan]`, `implements`, `description`.
- [x] `SchemaPlan` holds: `query`, `mutation`, `subscription`, `types`, `scalars`, `enums`, `unions`.
- [x] Refactor `_traverse_schema()` to build `SchemaPlan` directly.
- [x] Refactor `_build_schema_definition()` to consume `SchemaPlan` instead of re-walking dataclasses.
- [x] Delete now-unused helpers in `registry.py` and `schema.py`.

### Estimated Impact
- **Complexity**: Single source of truth for schema metadata; clearer data flow.
- **Performance**: Halves dataclass introspection (inspect once, use twice).

---

## 3) Python: ResolverSpec Precomputation [x]

### Rationale
`_wrap_resolver()` inspects signatures, builds arg defs, and creates coercers every time. For interfaces the same
resolver may be wrapped multiple times. Precompute once and reuse.

### Tasks
- [x] Add `ResolverSpec` dataclass in `resolver.py`: `wrapper`, `arg_defs`, `is_subscription`, `is_asyncgen`.
- [x] Cache `ResolverSpec` keyed by `(resolver, kind)` in a module-level dict.
- [x] Update `_wrap_resolver()` to check cache first and return cached spec.
- [x] Remove redundant introspection by using cached spec.

### Estimated Impact
- **Complexity**: Resolver metadata lives in one place; easier to test/debug.
- **Performance**: Avoids repeated `inspect.signature()` and `_get_type_hints()` calls.

---

## 4) Rust: FieldContext Struct [x]

### Rationale
Every field closure in `build_field()` and `build_subscription_field()` clones 7 `Arc` values. Consolidate into a
single `FieldContext` struct wrapped in one `Arc`, reducing capture churn and improving readability.

### Tasks
- [x] Define `FieldContext` in `src/types.rs`:
  ```rust
  pub(crate) struct FieldContext {
      pub resolver: Option<PyObj>,
      pub arg_names: Vec<String>,
      pub field_name: String,
      pub source_name: String,
      pub output_type: TypeRef,
      pub scalar_bindings: Arc<Vec<ScalarBinding>>,
      pub abstract_types: Arc<HashSet<String>>,
  }
  ```
- [x] Update `build_field()` to create `Arc<FieldContext>` once and clone that single Arc into the closure.
- [x] Update `build_subscription_field()` similarly.
- [x] Update `resolve_field()` and `resolve_subscription_stream()` signatures to accept `Arc<FieldContext>`.

### Estimated Impact
- **Complexity**: Closure captures become `ctx.clone()` instead of 7 separate clones.
- **Performance**: Slight reduction in Arc ref-count traffic; marginal.

---

## 5) Rust: Deduplicate Field Builders [x]

### Rationale
`build_field()` and `build_subscription_field()` share ~80% of their logic. Extract common code into a helper.

### Tasks
- [x] Create `fn build_field_context(field_def, resolver_map, scalar_bindings, abstract_types) -> Arc<FieldContext>`.
- [x] Have `build_field()` call `build_field_context()` then construct `Field`.
- [x] Have `build_subscription_field()` call `build_field_context()` then construct `SubscriptionField`.

### Estimated Impact
- **Complexity**: −40 lines of near-duplicate code.
- **Performance**: Neutral (same runtime path).

---

## 6) Rust: TypeRef Parse Cache [x]

### Rationale
`parse_type_ref()` is called for every field and argument. Many types repeat (e.g., `String!`, `Int`, `[User!]!`).
A small cache keyed by the GraphQL type string avoids repeated parsing and allocation.

### Tasks
- [x] Add `thread_local! { static TYPE_REF_CACHE: RefCell<HashMap<String, TypeRef>> = ... }` in `build.rs`.
- [x] Wrap `parse_type_ref()` to check cache first; insert on miss.
- [x] Cache persists for process lifetime (acceptable memory trade-off).

### Estimated Impact
- **Complexity**: +15 lines for cache management.
- **Performance**: Reduces allocations for repeated type strings; measurable on large schemas.

---

## 7) Rust: Unify Value Conversion [x]

### Rationale
`py_to_field_value_for_type()` and `py_to_field_value()` share logic; similarly `py_to_value()` has overlapping
patterns. Consider a single entry point that branches on whether type info is available.

### Tasks
- [x] Extract `convert_sequence_to_field_values()` helper for typed list/tuple handling.
- [x] Extract `convert_sequence_to_field_values_untyped()` helper for untyped list/tuple handling.
- [x] Simplify `py_to_field_value()` to use helpers.
- [x] Keep `py_to_value()` for input coercion (returns `Value`).

### Estimated Impact
- **Complexity**: −50 lines; single conversion path to reason about.
- **Performance**: Neutral (same runtime checks).

---

## 8) Rust: Scalar Binding Lookup Optimization [x]

### Rationale
`scalar_binding_for_value()` does a linear scan with `is_instance()` for each binding. For schemas with many scalars,
this adds up. Consider a lookup by type pointer or hash.

### Tasks
- [x] Add `is_builtin_type()` helper to short-circuit built-in types (None, bool, int, float, str, list, tuple, dict).
- [x] Call `is_builtin_type()` at start of `scalar_binding_for_value()` to skip linear scan for common cases.

### Estimated Impact
- **Complexity**: +20 lines for optimized lookup.
- **Performance**: Noticeable on scalar-heavy workloads; marginal otherwise.

---

## 9) Tests & Validation [x]

### Tasks
- [x] Add unit tests for `walk_annotation()` covering list annotations and async-iterable combinations.
- [x] Update existing tests to use new `build_schema_plan()` API.
- [x] Ensure `uv run mypy .`, `uv run pytest`, and `prek run -a` pass after each change.
- [x] All 117 tests passing with 96% coverage.

---

## Implementation Order

| Phase | Section | Rationale |
|-------|---------|-----------|
| 1 | §1 Unified Annotation Walker | Foundational; unblocks §2 |
| 2 | §2 SchemaPlan Abstraction | Largest complexity win |
| 3 | §3 ResolverSpec Precomputation | Builds on §2 |
| 4 | §4–§5 FieldContext + Deduplicate | Rust cleanup, independent of Python changes |
| 5 | §6 TypeRef Cache | Quick performance win |
| 6 | §7–§8 Value Conversion + Scalar Lookup | Lower priority; profile first |
| 7 | §9 Tests & Validation | Continuous throughout |

---

## Verification

After each phase:
```bash
uv run mypy .
uv run pytest
prek run -a
```

Optional benchmark check:
```bash
uv run python benchmarks/schema_build.py  # compare before/after
```
