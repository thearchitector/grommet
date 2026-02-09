# Resolver Unification & Sync Fast-Path

> the rust-side resolution path has a lot of branches. architecturally, i think there are 2 types of fields:
> - async
> - sync
>
> both async and sync fields can have the different resolver shapes.
> for sync fields, there are
> 1. data fields (attributes)
> 2. function fields (decorated methods)
>
> conceptually, though, data fields are just function fields that use `getattr`, which is a function with shape `SelfOnly`
>
> i think it would make sense to treat data fields and `SelfOnly` function fields identically, using some optimized non-async path for near-zero cost resolution for all sync fields (rust tuples for vectorcall: https://pyo3.rs/main/performance#calling-python-callables-__call__)
>
> i also think it makes sense to convert async function fields into sync function fields by analyzing the functional AST to determine if there are any `await` calls during python-side type building. this shouldn't mutate the function itself, but should change the associated `resolver` as stored in the plan. on this, there are currently 2 structures to hold field information: `FieldPlan` and `ResolverInfo`. there should only be one, in line with the data field generalization.
>
> draft a plan for all these changes.

Collapse the two Python-side field metadata structures (`FieldPlan` and `ResolverInfo`) into one, generalize
data fields as `SelfOnly` resolvers backed by `getattr`, add AST-based async→sync demotion, and implement
a unified sync fast-path on the Rust side using vectorcall-optimized tuple calls for all non-async fields.

## 1) Merge `FieldPlan` and `ResolverInfo` into a single `FieldPlan` [ ]

### Rationale

Currently field metadata is split: `FieldPlan` (in `plan.py`) holds the GraphQL schema shape (name, type,
args, description, deprecation, default), while `ResolverInfo` (in `resolver.py`) holds the dispatch shape
(func, shape, arg_coercers, is_async, is_async_gen). They are joined late via a `resolver_key` string that
indexes into `SchemaPlan.resolvers`. This indirection exists because data fields have no `ResolverInfo`, but
once data fields are generalized as `SelfOnly` resolvers, every non-input field has resolver metadata and the
split is unnecessary.

### Tasks

- [ ] Absorb `ResolverInfo` fields into `FieldPlan`:
  - Add `func: Callable[..., Any] | None` (the actual callable; `None` only for input fields)
  - Add `shape: str | None` (`"self_only"`, `"self_and_context"`, etc.; `None` for input fields)
  - Add `arg_coercers: list[tuple[str, Callable | None]]` (empty list for data fields / input fields)
  - Add `is_async: bool` (default `False`)
  - Add `is_async_gen: bool` (default `False`)
  - Remove `resolver_key: str | None`
- [ ] Remove `FieldPlan.resolver` (the raw callable stashed pre-analysis). The analyzed `func` replaces it.
- [ ] Delete `ResolverInfo` class from `resolver.py`.
- [ ] Remove `SchemaPlan.resolvers` dict. Resolver metadata now lives inline on each `FieldPlan`.
- [ ] Collapse `_wrap_plan_resolvers` into `_build_field_plans` — call `_analyze_resolver` inline when
  building each resolver-backed field plan, populating the new fields directly.
- [ ] Update `parse.rs` → `parse_field_plan` to read the new inline fields (`func`, `shape`,
  `arg_coercers`, `is_async`, `is_async_gen`) directly from each `FieldPlan` instead of doing a separate
  resolver-map lookup. Remove `parse_resolver_entry` and the `resolvers` dict parsing entirely.
- [ ] Update `types.rs` → remove `FieldDef.resolver: Option<String>` and inline `ResolverEntry` fields
  into `FieldDef` (or embed `Option<ResolverEntry>` directly). Eliminate the `resolver_map: HashMap` from
  `build_schema` and `build_field` signatures.
- [ ] Update all Rust-side call sites (`build.rs`, `resolver.rs`) that currently do
  `resolver_map.get(key)` to instead read from the `FieldDef` directly.

### Implementation Notes

The `FieldDef` struct on the Rust side currently stores `resolver: Option<String>` (the key). The simplest
Rust-side refactor is to change this to `resolver: Option<ResolverEntry>`, parsed directly from the
`FieldPlan` during `parse_field_plan`. This eliminates the `HashMap<String, ResolverEntry>` entirely from
`parse_schema_plan`, `build_schema`, `build_field`, and `build_field_context`. The `FieldContext` already
stores `Option<ResolverEntry>` so its shape doesn't change.

Affected files:
- `grommet/plan.py` — `FieldPlan`, `SchemaPlan`, `_wrap_plan_resolvers`, `_build_field_plans`
- `grommet/resolver.py` — delete `ResolverInfo`
- `src/parse.rs` — `parse_schema_plan`, `parse_field_plan`, delete `parse_resolver_entry`
- `src/types.rs` — `FieldDef`
- `src/build.rs` — `build_schema`, `build_field`, `build_field_context`, `build_subscription_field`
- `tests/test_core.rs` — `parse_schema_plan_round_trip` test, update Python snippet
- `tests/python/` — any tests constructing `ResolverInfo` or `SchemaPlan.resolvers`

## 2) Generalize data fields as `SelfOnly` resolvers [ ]

### Rationale

Data fields (plain dataclass attributes) currently take a completely separate code path: Rust's
`resolve_field_sync` does `getattr(parent, source_name)` and converts the result. Decorated resolver
fields with shape `SelfOnly` do `func.call1((parent,))` and convert the result. These are the same
operation — `getattr` is a callable with signature `(obj, name) -> value`, and a `SelfOnly` resolver
is a callable with signature `(self) -> value`. By assigning `getattr` as the `func` for data fields
(partially applied with the attribute name), both paths unify under a single dispatch mechanism.

### Tasks

- [ ] Use `operator.attrgetter` (a C-builtin callable) as the `func` for data fields:
  ```python
  from operator import attrgetter
  # e.g. for field "email":
  func = attrgetter("email")  # (obj) -> obj.email
  ```
  This replaces the Rust-side `getattr` in `resolve_field_sync`. No custom wrapper needed —
  `attrgetter` is faster than a Python closure and supports vectorcall natively.
- [ ] In `_build_field_plans`, for every non-root data field (where `resolver` was previously `None`),
  set `func = attrgetter(dc_field.name)`, `shape = "self_only"`, `is_async = False`.
- [ ] For root data fields, `_make_root_field_resolver` already creates a synthetic async resolver.
  Keep it as-is but ensure its `is_async` and `shape` are set correctly on the merged `FieldPlan`.
- [ ] Remove `resolve_field_sync` from `src/resolver.rs`. All fields now flow through the same
  dispatch, differentiated only by `is_async`.
- [ ] Remove the `has_resolver` branch in `build_field` (`src/build.rs` line 206). All fields use
  the same closure. Sync fields hit the sync arm of `resolve_with_resolver`; async fields hit the
  async arm.

### Implementation Notes

`operator.attrgetter` is implemented in C and supports vectorcall — it has effectively zero
overhead vs a raw `getattr` call and is faster than any Python-level closure. Unlike a hand-rolled
wrapper, it raises `AttributeError` on missing attributes (matching the current Rust-side behavior
in `resolve_field_sync`). If fallback-to-`None` semantics are desired, a thin wrapper or Rust-side
`getattr` with default can be used instead, but `AttributeError` is the correct behavior for
schema-declared fields.

## 3) Sync fast-path with vectorcall-optimized tuple calls [ ]

### Rationale

PyO3 can leverage Python's vectorcall protocol when callables are invoked with Rust tuples as positional
args — `func.call1((arg1, arg2))` compiles to a `vectorcall` rather than building a `PyTuple`. Currently
the sync branch in `resolve_with_resolver` calls `call_resolver` which uses `func.call1((...))` for
`SelfOnly` but `func.call((...), Some(&kwargs))` for shapes with args. The non-async path should be
extracted into a dedicated function that:
- Skips all async machinery (no `BoxFut`, no `into_future`, no `.await`)
- Uses `FieldFuture::Value` instead of `FieldFuture::new(async { ... })`
- Keeps kwargs construction only when `arg_coercers` is non-empty

### Tasks

- [ ] Create `resolve_field_sync_fast` in `resolver.rs` that handles all sync resolution in one
  GIL block:
  ```rust
  pub(crate) fn resolve_field_sync_fast<'a>(
      ctx: &ResolverContext<'a>,
      field_ctx: &FieldContext,
      entry: &ResolverEntry,
  ) -> Result<FieldValue<'a>, Error> {
      Python::attach(|py| {
          let result = call_resolver_sync(py, ctx, field_ctx, entry)?;
          py_to_field_value_for_type(py, result.bind(py), &field_ctx.output_type, field_ctx.scalar_hint)
      })
      .map_err(py_err_to_error)
  }
  ```
- [ ] Create `call_resolver_sync` that uses vectorcall-friendly tuple args:
  ```rust
  fn call_resolver_sync(
      py: Python<'_>,
      ctx: &ResolverContext<'_>,
      field_ctx: &FieldContext,
      entry: &ResolverEntry,
  ) -> PyResult<Py<PyAny>> {
      let parent = ctx.parent_value.try_downcast_ref::<PyObj>()
          .ok().map(|p| p.clone_ref(py)).unwrap_or_else(|| py.None());
      let func = entry.func.bind(py);
      match entry.shape {
          ResolverShape::SelfOnly => func.call1((parent,)),
          ResolverShape::SelfAndContext => {
              let ctx_obj = build_context_obj(py, ctx, ...)?;
              func.call1((parent, ctx_obj))
          }
          ResolverShape::SelfAndArgs => {
              let kwargs = build_kwargs_with_coercion(py, ctx, &entry.arg_coercers)?;
              func.call((parent,), Some(&kwargs))
          }
          ResolverShape::SelfContextAndArgs => {
              let ctx_obj = build_context_obj(py, ctx, ...)?;
              let kwargs = build_kwargs_with_coercion(py, ctx, &entry.arg_coercers)?;
              func.call((parent, ctx_obj), Some(&kwargs))
          }
      }.map(|r| r.unbind())
  }
  ```
- [ ] Update `build_field` in `build.rs` to dispatch based on `is_async`:
  ```rust
  // In the field closure:
  if entry.is_async {
      FieldFuture::new(async move { resolve_field(ctx, field_ctx).await })
  } else {
      match resolve_field_sync_fast(&ctx, &field_ctx, entry) {
          Ok(v) => FieldFuture::Value(Some(v)),
          Err(e) => FieldFuture::new(async move { Err(e) }),
      }
  }
  ```
- [ ] Simplify `resolve_with_resolver` to only handle the two async cases (`is_async` coroutine
  and `is_async_gen`). Remove its sync branch entirely.
- [ ] Delete `resolve_field_sync` (the old data-field-only path, replaced by `resolve_field_sync_fast`).

### Implementation Notes

The key performance insight from [PyO3 docs](https://pyo3.rs/main/performance#calling-python-callables-__call__)
is that `func.call1((a, b))` with a Rust tuple compiles to `vectorcall` with stack-allocated args,
avoiding `PyTuple` heap allocation. This matters for `SelfOnly` and `SelfAndContext` shapes where all
args are positional. For shapes with kwargs, we still need `func.call(positional, Some(&kwargs))`, but
the positional part still benefits from the tuple optimization.

The sync fast-path skips: `BoxFut` allocation, `Pin::new`, `into_future`, tokio task scheduling, and
the `.await` poll cycle. For simple attribute getters this should approach the cost of a raw Python
attribute access.

## 4) AST-based async→sync demotion [ ]

### Rationale

Many `@grommet.field` resolvers are declared `async def` by convention but contain no `await`
expressions — they are synchronously computable. These pay full async overhead (coroutine creation,
`into_future`, tokio task schedule, GIL re-acquire) for no benefit. By inspecting the function's AST
at plan-build time, we can detect await-free async functions and demote them to sync dispatch. This
is purely a plan-level optimization: the original function is not mutated, only the `is_async` flag
on the `FieldPlan` changes.

### Tasks

- [ ] Add `_has_await(func: Callable) -> bool` to `resolver.py`. **See implementation notes.**
- [ ] In `_analyze_resolver`, after determining `is_coroutine = True` and `is_asyncgen = False`:
  ```python
  if is_coroutine and not _has_await(resolver):
      is_async = False  # demote to sync dispatch
  ```
  The function itself remains an async def — we just tell Rust to call it synchronously (which yields
  a coroutine object that we... actually need to handle). **See implementation notes.**
- [ ] For demoted resolvers, wrap the call: since calling an `async def` synchronously yields a
  coroutine (not the return value), we need a thin sync wrapper:
  ```python
  def _syncify(func: Callable[..., Any]) -> Callable[..., Any]:
      """Wrap an await-free async function into a sync callable."""
      import asyncio

      def _wrapper(*args: Any, **kwargs: Any) -> Any:
          coro = func(*args, **kwargs)
          # No await in body, so send(None) drives to completion immediately
          try:
              coro.send(None)
          except StopIteration as e:
              return e.value
          finally:
              coro.close()

      _wrapper.__name__ = func.__name__
      _wrapper.__qualname__ = func.__qualname__
      return _wrapper
  ```
  Set `func = _syncify(resolver)` on the `FieldPlan` for demoted resolvers.
- [ ] Remove the blanket `resolver_requires_async` enforcement in `_analyze_resolver` for
  non-subscription fields. Sync decorated resolvers should be allowed natively (not just via
  demotion). Keep the async requirement only for subscriptions (`is_subscription` must still
  be async or async-gen).
- [ ] Add tests:
  - Async resolver with `await` → stays async.
  - Async resolver without `await` → demoted to sync.
  - Sync resolver (plain `def`) → stays sync, no error.
  - Subscription resolver → always requires async (no demotion).
  - `_has_await` with nested functions containing await → only top-level matters.
  - `_has_await` with dynamic/uninspectable function → conservative True.

### Implementation Notes

**Coroutine driving**: An `async def` with no `await` in its body will complete on the first
`send(None)` — the coroutine immediately raises `StopIteration` with the return value. The
`_syncify` helper exploits this to extract the value synchronously. This is safe because:
1. We verified via AST that there are no suspend points.
2. `send(None)` is how the event loop drives coroutines — we're just doing it eagerly.
3. `coro.close()` in the finally ensures cleanup even if something unexpected happens.

**`ast.walk` scope**: `ast.walk` traverses the entire AST tree, including nested function
definitions. An `await` inside a nested `async def` should NOT prevent demotion of the outer
function. To handle this correctly, `_has_await` should skip `ast.AsyncFunctionDef` nodes during
traversal (don't recurse into nested async defs):

```python
import ast
import inspect
import textwrap

def _has_await(func: Callable[..., Any]) -> bool:
    try:
        source = textwrap.dedent(inspect.getsource(func))
        tree = ast.parse(source)
    except (OSError, TypeError, SyntaxError):
        return True

    tree = ast.parse(src)
    f = next((n for n in ast.walk(tree)
              if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name == fn.__name__), None)
    if not f:
        return True

    class V(ast.NodeVisitor):
        found = False
        def visit_Await(self, n): self.found = True
        def visit_AsyncFor(self, n): self.found = True
        def visit_AsyncWith(self, n): self.found = True
        def visit_FunctionDef(self, n): return
        def visit_AsyncFunctionDef(self, n): return
        def visit_ClassDef(self, n): return
        def visit_Lambda(self, n): return

    v = V()
    for stmt in getattr(f, "body", []):
        v.visit(stmt)
        if v.found:
            return True
    return False
```

**Allowing sync resolvers natively**: Once the sync fast-path exists (Section 3), there's no
reason to reject plain `def` resolvers. A user might write `def resolve_name(self) -> str:
return self.first + " " + self.last` — this is perfectly valid and should use the sync path
without requiring `async def`. This change means removing the `resolver_requires_async` check
for non-subscription types.

**Root field synthetic resolvers**: `_make_root_field_resolver` currently generates `async def`
wrappers. These have no `await` and will be auto-demoted, but it's cleaner to generate them as
plain `def` directly and mark `is_async = False`.

## 5) Update tests and verify [ ]

### Rationale

The structural changes touch the Python↔Rust boundary contract. Every existing test must pass, and
new tests must cover the new paths.

### Tasks

- [ ] Update `tests/test_core.rs` → `parse_schema_plan_round_trip`: the Python snippet constructs
  `ResolverInfo` and `SchemaPlan(resolvers=...)`. Update to use the merged `FieldPlan` with inline
  resolver fields.
- [ ] Update Python-side tests that reference `ResolverInfo` imports or `SchemaPlan.resolvers`.
- [ ] Add Python tests for:
  - `_has_await` edge cases (nested async, lambda, uninspectable)
  - `_syncify` correctness (returns value, handles exceptions)
  - Sync resolver acceptance (plain `def` no longer raises)
  - Data field `attrgetter` integration
- [ ] Add Rust tests for `resolve_field_sync_fast` covering each `ResolverShape`.
- [ ] Run full suite: `uv run pytest && cargo test`
- [ ] Run `prek run -a` and address any failures/warnings.
