# Context API Refactor

Major aggressive refactor of the entire codebase around the new public API defined in `grommet/schema.py` and `README.md` (both `pragma: no ai`). No backward compatibility. Four key changes:

1. **Replace `grommet.Info` with `grommet.Context[T]`** — defined in `grommet/context.py` as a frozen dataclass wrapping user state and providing `field()` / `look_ahead()` for selection introspection. Runtime-importable.
2. **Eliminate `@staticmethod` resolvers** — resolvers are now strictly instance methods. No more `parent`, `root`, `info` parameters.
3. **Unify `execute` / `subscribe`** — `Schema.execute` is the sole entry point, returning `OperationResult | SubscriptionStream`. The `.subscribe` method is removed entirely.
4. **Migrate to `annotationlib`** — Python 3.14 minimum. Replace all `typing.get_type_hints()` with `annotationlib.get_annotations()`.

### Resolver Signature Contract

Resolvers support exactly these forms (and combinations thereof):

```python
# 1. Bare — no context, no args
async def my_field(self) -> str: ...

# 2. With context (MUST be second param, annotated gm.Context[T] or gm.Context)
async def my_field(self, ctx: gm.Context[MyState]) -> str: ...

# 3. With args (required or optional, after self or after context)
async def my_field(self, name: str) -> str: ...
async def my_field(self, name: str = "default") -> str: ...

# 4. Combinations
async def my_field(self, ctx: gm.Context[MyState], name: str, limit: int = 10) -> str: ...
```

Rules:
- `self` is always param 0 — the parent/instance value.
- If a `gm.Context` param exists, it MUST be param 1 (second positional). The name is irrelevant; detection is by **type annotation only** (`gm.Context` or `gm.Context[T]`).
- All remaining params after self (and optional context) are GraphQL arguments.
- No other reserved names exist. A param named `info`, `root`, `parent`, or `context` without a `gm.Context` type annotation is treated as a GraphQL argument.

### API Surface

- `Schema.execute(query, variables=None, state=None)` → `OperationResult | SubscriptionStream`
- No `.subscribe()` method. Subscriptions are detected by query content and return a `SubscriptionStream` from `execute`.
- No `root=` or `context=` kwargs anywhere.

## 1) Rust: Implement `Lookahead` PyO3 class [ ]

### Rationale

`grommet/context.py` defines a `Lookahead` protocol with `exists() -> bool` and `field(name) -> Lookahead`. async-graphql's `Lookahead` has a borrow lifetime tied to `ResolverContext`, so we eagerly extract the selection tree into an owned `#[pyclass]` at resolver call time.

### Tasks

- [ ] Add `src/lookahead.rs` with a `#[pyclass]` struct `Lookahead`:
  - Fields: `exists: bool`, `children: HashMap<String, Lookahead>`.
  - `#[pymethods]`: `fn exists(&self) -> bool`, `fn field(&self, name: &str) -> Lookahead` (returns empty non-existent node for missing keys).
  - Implements `Clone`.
- [ ] Implement `fn extract_lookahead(ctx: &ResolverContext) -> Lookahead` — eagerly walks `ctx.look_ahead()` via `selection_fields()` recursively. Cap depth at 32.
- [ ] Register `Lookahead` in `lib.rs` module init.

### Implementation Notes

async-graphql's `Lookahead` (from `src/look_ahead.rs`):

```rust
impl Lookahead<'_> {
    pub fn field(&self, name: &str) -> Self { ... }
    pub fn exists(&self) -> bool { !self.fields.is_empty() }
    pub fn selection_fields(&self) -> Vec<SelectionField<'a>> { ... }
}
```

For each `SelectionField`, call `field.name()` to get the field name, then recurse into its children via `Lookahead::from(selection_field).selection_fields()`.

## 2) Python: Flesh out `grommet/context.py` [ ]

### Rationale

`Context[T]` has stub methods (`...`). They need real implementations delegating to an internal `_lookahead` field (the Rust `Lookahead` PyO3 instance).

### Tasks

- [ ] Add `_lookahead: "Lookahead"` field to the dataclass (use `field(repr=False)` to keep it hidden).
- [ ] Implement `field(self, name) -> Lookahead`: `return self._lookahead.field(name)`.
- [ ] Implement `look_ahead(self) -> Lookahead`: `return self._lookahead`.

### Implementation Notes

The Rust side constructs `Context(state=user_state, _lookahead=rust_lookahead)`. The `_` prefix means `is_internal_field()` skips it. The frozen dataclass ensures immutability.

## 3) Rust: Unify `execute`/`subscribe` and replace `RootValue`/`ContextValue` [ ]

### Rationale

`schema.py` defines a single `Schema.execute(query, variables, state)` returning `OperationResult | SubscriptionStream`. The Rust `SchemaWrapper` currently has separate `execute` and `subscribe` methods with `(query, variables, root, context)` signatures. Both must be unified into a single `execute(query, variables, state)` that auto-detects subscriptions.

### Tasks

- [ ] In `types.rs`:
  - Remove `RootValue` and `ContextValue`. Add `StateValue(pub(crate) PyObj)`.
  - Add `RootInstances` struct holding `PyObj` references for query/mutation/subscription root classes.
- [ ] In `api.rs`:
  - Remove `SchemaWrapper.subscribe` entirely.
  - Change `SchemaWrapper.execute` signature to `(query, variables, state)`.
  - The single `execute` must: parse the query to detect operation type, call `schema.execute()` for queries/mutations and `schema.execute_stream()` for subscriptions, returning `OperationResult` or `SubscriptionStream` respectively.
  - Update `build_request` to accept only `state: Option<Py<PyAny>>`. Store as `request.data(StateValue(...))`. Remove all root/context data insertion.
  - Store root type class references in `SchemaWrapper` (extracted from plan at build time). At execution time, instantiate root classes via `cls()` and insert as root data.
- [ ] In `resolver.rs`:
  - `extract_context` → `extract_state`: extract `Option<PyObj>` state from `ctx.data::<StateValue>()` and `Option<PyObj>` parent from `ctx.parent_value`.
  - `call_resolver`: construct `grommet.context.Context(state=..., _lookahead=extract_lookahead(&ctx))` Python object. Pass `(parent, context_obj, **kwargs)` to the wrapper.
  - Remove the `info` dict construction entirely.
  - For top-level fields, parent falls back to the pre-instantiated root object from `ctx.data::<RootInstances>()`.

### Implementation Notes

**Subscription detection**: async-graphql's `schema.execute()` already handles queries and mutations. For subscriptions, `schema.execute_stream()` must be used. The Rust side needs to detect whether the incoming query is a subscription operation. Options:
1. Parse the query with `async_graphql::parser::parse_query()` and check the operation type.
2. Try `execute()` first and if the schema has a subscription root, try `execute_stream()` based on operation detection.
3. Always attempt both — try `execute()`, and if the operation is a subscription, call `execute_stream()` instead.

Option 1 is most reliable. Parse the query, inspect `OperationType`, and branch accordingly.

**Root instantiation**: Store root Python classes in `SchemaWrapper`. At `execute` time, call `cls()` to get a no-arg instance and insert it into the request data as a `RootInstances` value. The resolver reads this as the parent fallback. Consider caching root instances if they're stateless.

**`SubscriptionStream`**: The existing `SubscriptionStream` PyO3 class stays, but it's now returned directly from `execute` instead of from a separate `subscribe` method.

## 4) Python: Overhaul `resolver.py` [ ]

### Rationale

Complete rewrite of the resolver wrapper. Remove `Info`, `_normalize_info`, all name-based parameter detection. Adopt strict positional signature contract with type-based `Context` detection.

### Tasks

- [ ] Delete `from .info import Info` and `_normalize_info` entirely.
- [ ] Delete `_RESERVED_PARAM_NAMES` entirely. Replace with positional logic.
- [ ] Delete `_find_param` helper.
- [ ] Rewrite `_build_resolver_spec`:
  - Get params via `inspect.signature`. `params[0]` is always `self` (the parent).
  - Check `params[1]` (if it exists) for `Context` annotation: resolve via `annotationlib.get_annotations(resolver, format=Format.FORWARDREF)`, then test `annotation is Context or get_origin(annotation) is Context`. If true, it's the context param; skip it for GraphQL args.
  - `arg_params` = everything after self (and context if present). These are GraphQL arguments.
  - Build arg coercers and arg defs as before for the arg params.
- [ ] Rewrite the `wrapper` closure:
  - Signature: `async def wrapper(parent, context_obj, **kwargs)`.
  - Rust calls `wrapper(parent_pyobj, context_pyobj, **graphql_kwargs)`.
  - Build `call_kwargs`: `self` → `parent`. If context param exists, map it → `context_obj`. Map remaining → coerced kwargs.
  - Subscription paths (async-gen, coroutine-returning-iterator) remain.
- [ ] Validation: if resolver is not `async def` (and not an async generator for subscriptions), raise `GrommetTypeError` at schema build time.

### Implementation Notes

Context detection — the context param annotation must be checked at position 1 only:

```python
from .context import Context
from typing import get_origin

params = list(sig.parameters.values())
context_param = None
if len(params) >= 2:
    ann = hints.get(params[1].name)
    if ann is Context or get_origin(ann) is Context:
        context_param = params[1]

# GraphQL args start after self (index 0) and context (index 1, if present)
start = 2 if context_param else 1
arg_params = params[start:]
```

The wrapper always receives `(parent, context_obj, **kwargs)` from Rust regardless of whether the resolver uses context. If the resolver doesn't have a context param, `context_obj` is simply not forwarded.

## 5) Python: Migrate `annotations.py` to `annotationlib` and skip `Context[T]` [ ]

### Rationale

Python 3.14 minimum. `annotationlib.get_annotations()` replaces `typing.get_type_hints()`. `Context[T]` must be recognized and skipped in type resolution.

### Tasks

- [ ] Replace type-hint resolution:
  - Remove `import sys`, `from typing import get_type_hints`.
  - Add `from annotationlib import Format, get_annotations`.
  - `_resolve_type_hints(obj)` → `get_annotations(obj, format=Format.FORWARDREF)`. No manual `globalns`/`localns`.
  - Simplify `_cached_type_hints` / `_get_type_hints` accordingly.
- [ ] Add `is_context: bool` to `AnnotationInfo`. Detect via `inner is Context or get_origin(inner) is Context`.
- [ ] In `_type_spec_from_annotation`, raise if annotation is `Context` (defensive).
- [ ] In `walk_annotation` / `_walk_inner`, skip `Context` annotations.
- [ ] In `_resolver_arg_annotations` (resolver.py), exclude the context param.
- [ ] Update callers in `plan.py`, `resolver.py`, `decorators.py`.

### Implementation Notes

`annotationlib.get_annotations(obj, format=Format.FORWARDREF)` returns `ForwardRef` objects for unresolvable names instead of raising. This is more robust than the current `try`/`except` pattern. For most grommet usage, annotations reference concrete types that resolve fine with `Format.VALUE`, but `FORWARDREF` provides a safe fallback.

## 6) Python: Update `__init__.py` and remove `info.py` [ ]

### Rationale

`Info` is dead. `Context` is the replacement.

### Tasks

- [ ] `__init__.py`: remove `from .info import Info`, add `from .context import Context`, replace `"Info"` with `"Context"` in `__all__`.
- [ ] Delete `grommet/info.py`.

## 7) Python: Update `decorators.py` — remove `@staticmethod`/`@classmethod` support [ ]

### Rationale

Resolvers are strictly instance methods. `@staticmethod` and `@classmethod` support is removed entirely (aggressive break, pre-1.0).

### Tasks

- [ ] In `field()`'s `wrap()`: remove `isinstance(func, staticmethod)` and `isinstance(func, classmethod)` branches. If either is passed, raise `GrommetTypeError` with a clear message (e.g. "resolvers must be instance methods").
- [ ] Remove `bind_to_class` from `_FieldResolver.__slots__` and its `__init__`.
- [ ] In `_apply_field_resolvers`, remove `bind_to_class` handling — `resolver = marker.resolver` directly.

## 8) Python: Update `plan.py` [ ]

### Rationale

Arg filtering must match the new positional resolver contract. No `root=` concept.

### Tasks

- [ ] In `_build_field_plans`: arg filtering now uses the positional logic from Section 4. Params at index 0 (`self`) and index 1 (if `Context`-typed) are excluded. Everything else is a GraphQL arg.
- [ ] Audit `_build_type_plans` — remove any root-value assumptions.
- [ ] Verify `_resolver_arg_annotations` excludes `self` and context params correctly.

## 9) Rust: Rewrite `test_core.rs` [ ]

### Rationale

Rust tests reference old API: `execute(query, variables, root, context)`, `subscribe(...)`, resolver signatures with `(parent, info)`.

### Tasks

- [ ] Remove all `subscribe` calls — everything goes through `execute`.
- [ ] Change all `execute` calls from `(query, variables, root, context)` to `(query, variables, state)`.
- [ ] Update inline Python resolver definitions from `async def resolver(parent, info)` to `async def resolver(self)`.
- [ ] Remove test scenarios exercising `root=`/`context=` separately.

## 10) Python: Rewrite test suite [ ]

### Rationale

~18 test files use the old API. Every resolver must be converted. This is mechanical but extensive.

### Tasks

- [ ] **`test_info_context.py`** → rename to `test_context.py`. Rewrite for `gm.Context[T]`, `context.state`, `context.field()`, `context.look_ahead()`. Use `state=` instead of `root=`/`context=`.
- [ ] **`test_root.py`** → delete or rewrite. `root=` no longer exists. Test field-without-resolver via default-valued fields.
- [ ] **`test_resolver_coverage.py`** → remove `_normalize_info` tests, remove `Info` imports, rewrite wrapper tests for `(parent, context_obj, **kwargs)` signature. Update all resolver definitions to instance methods.
- [ ] **`test_subscriptions.py`** → convert `@staticmethod` to instance methods. Replace `schema.subscribe(...)` with `await schema.execute(...)`.
- [ ] **All 14+ remaining test files** — mechanical replacements:
  1. Remove all `@staticmethod` under `@gm.field`
  2. `async def foo(parent: Any, info: Any, ...)` → `async def foo(self, ...)`
  3. `async def foo(parent: Any, info: Any)` → `async def foo(self)`
  4. `(parent, info, ...)` → `(self, ...)` (untyped)
  5. `(parent, info)` → `(self)`
  6. Remove `root=...` and `context=...` from `execute()` / `subscribe()` calls
  7. Replace `schema.subscribe(...)` → `await schema.execute(...)`
  8. Replace `gm.Info` / `Info` references → `gm.Context` or remove
  9. Tests asserting `info.field_name` etc. → rewrite for `context.state` / `context.field()` / `context.look_ahead()`

### Implementation Notes

Test files affected (from grep): test_decorators_coverage (8 `@staticmethod`), test_plan_coverage (7), test_abstract_types (3), test_input_defaults (3), test_sdl_snapshot (3), test_subscriptions (3), test_input_validation (2), test_scalars (2), test_schema_coverage (2), test_errors (1), test_field_decorator (1), test_field_decorator_args (1), test_input_list_defaults (1), test_resolvers (1), test_runtime (1), test_sdl (1), test_internal_fields (1).

4 test files use `root=`: test_info_context, test_internal_fields, test_resolver_coverage, test_root.

4 test files use `schema.subscribe(`: test_subscriptions (all 4 tests).
