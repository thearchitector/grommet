# Decorator-Driven Schema Construction

> in the current design, the `plan` phase does a lot of heavily lifting. this is not ideal, since it means types cannot be reused efficiently across multiple schemas. it also centralizes a lot of logic in a single function. a lot of this heavily lifting is done _again_ when the rust side builds the async-graphql schema structs.
>
> instead, it would make more sense to do as much of the parsing and construction up-front _in the decorators_. for `@field`, that would amount to
> - looking at the resolver signature, and determining if any of the argument needs coercion (ie. the arg is an input). if so, wrapping the function so that coercion is built-into the py function run by rust
> - analyzing the types, ensuring they're all valid, building an async-graphql `Field`, and attaching it to the python function.
>
> for types, since the class's decorator gets run after everything in the body (all the fields), it would involve scanning the entire class and collecting a list of all the known fields. it would collect pre-built `Field` instances, and also build new `Field` instances for data fields (dataclass fields). after collecting, it would create a new async-graphql `Object`, register all the collected fields to it via `.field`, and then
>
> the same pre-building-style logic would apply to inputs and subscriptions.
>
> then, when the root `Query`, `Mutation`, and `Subscription` types are passed to `Schema`, the only thing `Schema` would need to do is recursively walk through the types and `.register` them on a new async-graphql `Schema` object (which it would then attach to the python Schema).
>
> this is massive refactor of the entire codebase. draft a plan to implement this. i do not anticipate much of the existing structure or tests to remain. you can be aggressive in this plan, since nothing besides the documented APIs need to remain the same.

This plan restructures grommet so that GraphQL schema artifacts (async-graphql `Field`, `Object`, `InputObject`, `Subscription`, `SubscriptionField`) are constructed eagerly at decoration time rather than lazily in a centralized `plan` phase. The current flow is:

```
decorators (mark metadata) → plan.py (analyze everything, build FieldPlan/TypePlan/SchemaPlan)
    → parse.rs (deserialize plans) → build.rs (construct async-graphql schema)
```

The new flow will be:

```
@field/@subscription (analyze resolver, build async-graphql Field/SubscriptionField, attach to function)
    → @type/@input (scan class, collect Fields, build async-graphql Object/InputObject/Subscription)
        → Schema (recursive walk + register, finish)
```

`@subscription` is a new public decorator (distinct from `@field`) that explicitly marks a resolver as a subscription field and builds a `SubscriptionField`. `@subscription` can only decorate bare functions (not class methods or data fields).

This eliminates `parse.rs`, the entire `SchemaPlan`/`TypePlan`/`FieldPlan` layer, and most of `build.rs`. The Rust side becomes a thin registration loop + the existing resolver/value/lookahead machinery.

**Files deleted or gutted:**
- `grommet/resolver.py` — absorbed into `decorators.py`
- `grommet/metadata.py` — simplified; `FieldPlan`, `TypePlan`, `SchemaPlan`, `ArgPlan` removed
- `src/parse.rs` — deleted entirely
- `src/build.rs` — folded into `src/api.rs` as a trivial registration loop
- `src/types.rs` — `SchemaDef`, `TypeDef`, `FieldDef`, `ArgDef`, `ScalarHint` removed; `ResolverEntry`/`FieldContext`/`ResolverShape` remain

**Files modified:**
- `grommet/decorators.py` — becomes the primary workhorse
- `grommet/plan.py` — rewritten to a thin `build_schema_graph` that does a recursive walk and returns a registration-ready bundle (signature unchanged so `schema.py` remains untouched)
- `grommet/__init__.py` — re-exports adjusted (adds `subscription`)
- `grommet/_core.pyi` — updated to reflect new Rust API
- `src/api.rs` — `SchemaWrapper::new` accepts a registration-ready bundle, not a `SchemaPlan`
- `src/lib.rs` — registers new pyclass wrappers

**Files unchanged:**
- `grommet/schema.py` — `pragma: no ai`; unchanged because `build_schema_graph` keeps its signature
- `grommet/context.py` — `pragma: no ai`
- `grommet/errors.py`, `grommet/_annotations.py`, `grommet/annotations.py`, `grommet/coercion.py`
- `src/resolver.rs`, `src/values.rs`, `src/lookahead.rs`, `src/errors.rs`, `src/runtime.rs`

---

## 1) Remove `ScalarHint` [x]

`ScalarHint` is a Rust-side optimization that short-circuits Python→`FieldValue` conversion when the field's GraphQL type is a known scalar. It requires knowing all object type names at schema build time, which is incompatible with per-decorator construction. The `TypeRef`-based dispatch path in `py_to_field_value_for_type` already handles all cases correctly; `ScalarHint` is purely a branch-avoidance optimization. Remove it.

### Tasks

- [ ] Remove `ScalarHint` enum from `src/types.rs`.
- [ ] Remove `scalar_hint` field from `FieldContext`.
- [ ] Remove `compute_scalar_hint` from `src/build.rs`.
- [ ] Remove `py_to_field_value_hinted` from `src/values.rs`. All call sites go through `py_to_field_value_for_type` directly.
- [ ] Update `src/resolver.rs` to not pass `ScalarHint` to value conversion.
- [ ] Update Rust tests in `tests/test_core.rs` that reference `ScalarHint`.

---

## 2) Expose async-graphql builders to Python via new pyclass wrappers [x]

Python decorators will build async-graphql `Field`, `Object`, `InputObject`, `Subscription`, and `SubscriptionField` objects at decoration time. These Rust types need thin `#[pyclass]` wrappers exposing builder methods callable from Python.

### Tasks

- [ ] Create `src/schema_types.rs` with pyclass wrappers:
  - `PyField` wrapping `async_graphql::dynamic::Field` — exposes `.argument(name, type_spec, default_value?)`. Constructor accepts: field name, return `TypeSpec`, resolver `Callable` (already coercion-wrapped by Python), `ResolverShape` string, `arg_names: list[str]`, `is_async: bool`. Wires up the resolver closure reusing `resolve_field`/`resolve_field_sync_fast` from `resolver.rs`.
  - `PySubscriptionField` wrapping `SubscriptionField` — constructor accepts: field name, return `TypeSpec`, resolver `Callable`, `ResolverShape` string, `arg_names: list[str]`. Wires up `resolve_subscription_stream`.
  - `PyInputValue` wrapping `InputValue` — constructor takes name, `TypeSpec`, optional default value.
  - `PyObject` wrapping `Object` — exposes `.field(PyField)` and `.description(str)`.
  - `PyInputObject` wrapping `InputObject` — exposes `.field(PyInputValue)` and `.description(str)`.
  - `PySubscription` wrapping `Subscription` — exposes `.field(PySubscriptionField)` and `.description(str)`.
- [ ] Extract `type_spec_to_type_ref` from `src/parse.rs` into `src/schema_types.rs` as a standalone utility, preserving the existing `TypeSpec` dataclass as the Python→Rust transfer format.
- [ ] Register all new pyclasses in `lib.rs` `_core` module.
- [ ] Update `_core.pyi` to reflect the new classes and their signatures.

### Implementation Notes

The `PyField` constructor wires up the resolver closure exactly as `build.rs::build_field` does today: it constructs a `FieldContext` (without `ScalarHint`, removed in §1) and attaches the appropriate `resolve_field`/`resolve_field_sync_fast` closure. The `context_cls` (for `grommet.Context`) is resolved lazily on first use.

The `PySubscriptionField` constructor wires up `resolve_subscription_stream` the same way `build.rs::build_subscription_field` does today.

---

## 3) Refactor `@field` and introduce `@subscription` [x]

The `@field` decorator currently stores a `_FieldResolver` sentinel. The new design has it do all the work: analyze the resolver signature, build coercion wrappers, construct a `PyField`, and attach it to the function.

A new `@subscription` decorator is introduced as a distinct public API for subscription resolvers. `@subscription` always builds a `PySubscriptionField`; `@field` always builds a `PyField`. `@subscription` can only decorate bare functions — it cannot be applied to class methods or data fields.

### Tasks

- [ ] Move resolver analysis from `resolver.py` into `decorators.py`:
  - Absorb `_analyze_resolver`, `_resolver_params`, `_resolver_arg_info`, `_is_context_annotation`, `_wrap_with_coercion`, `_build_arg_info`.
  - The analysis produces: `func` (possibly wrapped with coercion and/or syncified), `shape`, `arg_names`, `is_async`, `args` (as `TypeSpec` + defaults for each arg), and return `TypeSpec`.
- [ ] Refactor `@field`'s `wrap()`:
  1. Run resolver analysis on the target function.
  2. Build the return `TypeSpec` from the return annotation.
  3. Construct `_core.PyField(name, type_spec, func, shape, arg_names, is_async)`, then call `.argument(...)` for each arg.
  4. Attach the resulting Rust object to a `_ResolvedField` sentinel.
- [ ] Introduce `@subscription` decorator:
  - Same overload pattern as `@field` (bare `@subscription` and `@subscription(name=..., description=...)`).
  - `@subscription` can only decorate functions. Raise `GrommetTypeError` if applied to a non-function.
  - In `wrap()`:
    1. Validate the resolver is an async generator function (raise `GrommetTypeError` if not).
    2. Run resolver analysis. Unwrap the `AsyncIterator[T]`/`AsyncIterable[T]` return annotation to get the inner type `T` and build `TypeSpec` from `T`.
    3. Construct `_core.PySubscriptionField(name, type_spec, func, shape, arg_names)`, then call `.argument(...)` for each arg.
    4. Attach the resulting Rust object to a `_ResolvedSubscription` sentinel (distinct from `_ResolvedField`).
  - Export `subscription` from `grommet/__init__.py`'s `__all__`.
- [ ] Both sentinels (`_ResolvedField` and `_ResolvedSubscription`) store:
  - `rust_field` — the pre-built `PyField` or `PySubscriptionField`
  - `name: str` — the GraphQL field name
  - `referenced_types: list[type]` — grommet types referenced in return + arg annotations (for recursive registration by `Schema`)
- [ ] Delete `grommet/resolver.py` after absorbing its logic.

### Implementation Notes

`noaio` (`can_syncify`, `syncify`) is still used for sync demotion of await-free coroutines in `@field`. `@subscription` does not need sync demotion since subscription resolvers must be async generators.

Coercion wrapping (`_wrap_with_coercion` from `coercion.py`) is applied to the resolver at decoration time. The wrapped function is what gets passed to the `PyField`/`PySubscriptionField` constructor.

Type walking (discovering referenced grommet types) stays in `annotations.py` and is called at decoration time to populate `referenced_types`.

The `@subscription` decorator uses `annotations.unwrap_async_iterable` to extract the inner type `T` from `AsyncIterator[T]`.

---

## 4) Refactor `@type` and `@input` to eagerly build `PyObject`/`PyInputObject`/`PySubscription` [x]

When `@type` or `@input` runs, all `@field`/`@subscription` decorators in the class body have already executed. The class decorator scans the class, collects pre-built sentinels, builds Rust objects for data fields, and assembles the complete async-graphql type.

### Tasks

- [ ] In `_wrap_type_decorator`:
  1. **Classify the type:** Scan `vars(cls)` for sentinels. If any `_ResolvedSubscription` sentinels exist, this is a subscription type. If any `_ResolvedField` sentinels exist, it's a regular object type. Raise `GrommetTypeError` if both sentinel types are present, or if `kind == INPUT` and any resolver sentinels exist.
  2. **Build data fields (object and input types only):**
     For each `dataclasses.fields(cls)` that isn't hidden:
     - For **object data fields**: Build a `PyField` with `operator.attrgetter(field_name)` as the resolver, shape `"self_only"`, and the `TypeSpec` derived from the annotation.
     - For **input data fields**: Build a `PyInputValue` with name, `TypeSpec`, and default value.
     - **Subscription types cannot have data fields.** Raise `GrommetTypeError` if non-hidden data fields are found on a subscription type.
  3. **Assemble the Rust object:**
     - For **regular object types**: Create `_core.PyObject(name)`, optionally `.description(desc)`, then `.field(f)` for each data field `PyField` and each `_ResolvedField`'s `rust_field`.
     - For **subscription types**: Create `_core.PySubscription(name)`, optionally `.description(desc)`, then `.field(sf)` for each `_ResolvedSubscription`'s `rust_field`.
     - For **input types**: Create `_core.PyInputObject(name)`, optionally `.description(desc)`, then `.field(iv)` for each `PyInputValue`.
  4. **Attach to the class:** Store the assembled Rust object as `cls.__grommet_object__`.
  5. **Collect referenced types:** Union all `referenced_types` from resolver sentinels + types discovered by walking data field annotations. Store as `cls.__grommet_refs__` (a `frozenset[type]`).
- [ ] Remove or simplify `TypeMeta` — the only metadata `Schema` needs from a decorated class is `__grommet_object__` and `__grommet_refs__`. The `kind` is inferred from the Rust object's Python type (`PyObject` vs `PyInputObject` vs `PySubscription`).
- [ ] Simplify `grommet/metadata.py`: Remove `FieldPlan`, `TypePlan`, `SchemaPlan`, `ArgPlan`. Keep `TypeSpec`, `TypeKind`, `Field`, `Hidden`, `_SCALARS`, `MISSING`, `NO_DEFAULT`.

### Implementation Notes

The `@type` decorator still requires the target to already be a `@dataclass`.

Root type default enforcement (currently in `plan.py::_resolve_root_default`) moves to `build_schema_graph` in `plan.py` since root-ness isn't known at `@type` time. All object data fields use `attrgetter` at decoration time. Root types are validated in `build_schema_graph`: every data field on a root type must have a dataclass default.

---

## 5) Refactor `Schema` / `plan.py` / Rust `SchemaWrapper` [x]

`schema.py` has `pragma: no ai` and cannot be modified. It imports `build_schema_graph` from `.plan` and calls `_core.Schema(graph)`. To avoid changing `schema.py`, `plan.py` keeps the `build_schema_graph` function with the same signature but rewrites its internals. The return type changes, and `_core.Schema.__init__` is updated to accept the new return type.

### Tasks

- [ ] Rewrite `grommet/plan.py` entirely. The new `build_schema_graph(query, mutation, subscription)`:
  1. Validates that root classes have `__grommet_object__` and `__grommet_refs__`.
  2. Validates root type data field defaults.
  3. Recursively walks `__grommet_refs__` (BFS) starting from each root, collecting all referenced classes.
  4. Returns a registration-ready bundle (e.g. a simple dataclass or dict) containing: query/mutation/subscription names + the list of all collected `__grommet_object__` Rust objects.
- [ ] On the Rust side, rewrite `SchemaWrapper::new` in `src/api.rs`:
  - Accept the registration-ready bundle from Python.
  - Create `async_graphql::dynamic::Schema::build(query_name, mutation_name, subscription_name)`.
  - For each type object in the bundle, call `builder = builder.register(inner)` where `inner` is the unwrapped async-graphql `Object`/`InputObject`/`Subscription`.
  - Call `builder.finish()` and store the result.
- [ ] Delete `src/parse.rs`. Remove `mod parse;` from `src/lib.rs`.
- [ ] Fold `src/build.rs` into `src/api.rs` (the registration loop is trivial). Remove `mod build;` from `src/lib.rs`.
- [ ] Simplify `src/types.rs`: Remove `SchemaDef`, `TypeDef`, `FieldDef`, `ArgDef`. Keep `PyObj`, `StateValue`, `ResolverEntry`, `ResolverShape`, `FieldContext`.

---

## 6) Rewrite tests [x]

### Tasks

- [ ] **Delete or rewrite `tests/python/test_resolver_analysis.py`:** These test `_analyze_resolver`, `_has_await`, `_syncify` from `grommet.resolver`, which is absorbed into `decorators.py`. Rewrite tests to import from the new location, or test via the `@field`/`@subscription` decorator's observable behavior.
- [ ] **Keep end-to-end tests unchanged:** `test_basic_query.py`, `test_context_state.py`, `test_descriptions.py`, `test_hidden_fields.py`, `test_lookahead.py`, `test_mutations.py`, `test_resolvers.py` — these test the public API and should pass without changes.
- [ ] **Update `test_subscriptions.py`:** Replace `@grommet.field` with `@grommet.subscription` on subscription resolvers.
- [ ] **Rewrite `tests/test_core.rs`:**
  - Delete the `parse` module tests (`parse_schema_plan_round_trip` etc.).
  - Remove `ScalarHint` references from `values` tests.
  - Update `resolver` tests if resolver internals change.
  - Add tests for the new pyclass wrappers (`PyField`, `PyObject`, etc.) if non-trivial logic exists there.
- [ ] **Add new Python unit tests for decorator-time construction:**
  - `@field` produces a `_ResolvedField` with a valid Rust field object.
  - `@subscription` produces a `_ResolvedSubscription` with a valid Rust subscription field object.
  - `@type` produces a class with `__grommet_object__` containing a `PyObject`.
  - `@input` produces `__grommet_object__` containing a `PyInputObject`.
  - Error cases: `@subscription` on a non-async-generator, data field on subscription type, mixing `@field` and `@subscription` on the same type, `@subscription` used as a class method decorator (should fail), etc.
- [ ] Run `uv run pytest` and `cargo test` to verify all tests pass.
- [ ] Run `prek run -a` for final verification.

### Implementation Notes

The README currently uses `@grommet.field` for subscription resolvers (a known typo). Tests should use `@grommet.subscription` per the corrected API.

---

## Execution order

1. **§1 — Remove `ScalarHint`** (independent Rust cleanup, reduces surface area)
2. **§2 — Expose pyclass wrappers** (Rust foundation for the new Python decorators)
3. **§3 — Refactor `@field`, introduce `@subscription`** (Python decorators become the workhorse)
4. **§4 — Refactor `@type`/`@input`** (Python class decorators assemble Rust objects)
5. **§5 — Refactor `Schema` / `plan.py` / Rust `SchemaWrapper`** (thin registration loop replaces plan consumption)
6. **§6 — Rewrite tests** (verify everything end-to-end)

Each section can be verified incrementally by building (`maturin develop --uv`) and running the subset of tests that still apply.
