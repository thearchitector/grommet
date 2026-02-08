# Plan 10 — GIL & Async Overhead Reduction

## Problem
grommet is ~17% slower than strawberry (pure-Python graphql-core) on a 500k-cell benchmark.
The dominant costs are:

1. **Async overhead for non-resolver fields** — every field, even `parent.attr`, is wrapped
   in `FieldFuture::new(async move { ... })` which boxes a future and enters tokio scheduling.
2. **Redundant GIL acquisitions** — `call_resolver` and `into_future` each acquire GIL
   separately (×500k for async resolvers); `resolve_from_parent` and `py_to_field_value`
   each acquire separately (×200k for attribute fields).
3. **Temporary Python string allocation in getattr** — `resolve_from_parent` converts
   `&str → PyString` on every call; the same field name is resolved millions of times.
4. **Repeated TaskLocals lookup** — `pyo3_async_runtimes::tokio::into_future` calls
   `get_current_locals` per invocation, looking up the running event loop each time.
5. **No sync resolver path** — sync `def` resolvers still go through `into_future`.

## Sections

### 1. FieldFuture::Value for non-resolver fields
- [x] In `build_field`, detect when `field_ctx.resolver.is_none()`
- [x] Return `FieldFuture::Value(Some(field_value))` with a single `Python::attach` that
  does getattr + py_to_field_value_for_type in one GIL block
- [x] Keep `FieldFuture::new(async { ... })` only for resolver fields

### 2. Merge call_resolver + into_future GIL blocks
- [x] In `resolve_value`, call `pyo3_async_runtimes::tokio::into_future` inside the same
  `Python::attach` as `call_resolver`
- [x] Use `Box::pin(future)` to erase `impl Future` type so it can escape the closure
- [x] Result: 1 GIL acquire instead of 2 per async resolver call

### 3. Intern source_name as Py<PyString>
- [x] Add `source_name_py: Py<PyString>` to `FieldContext`
- [x] Populate it at build time via `PyString::new(py, &source_name)`
- [x] Use `parent.getattr(source_name_py)` in resolve_from_parent

### 4. Cache TaskLocals
- [x] Use `into_future_with_locals` with pre-fetched `TaskLocals` instead of
  `into_future` which calls `get_current_locals` each time
- [x] Store TaskLocals in the resolver context data or compute once per execute

### 5. Sync resolver support
- [x] Add `is_async: bool` to ResolverEntry (from Python's `inspect.iscoroutinefunction`)
- [x] For sync resolvers: skip into_future, return result directly
- [x] Combine with FieldFuture::Value for sync-resolver fields too

### 6. Lazy state extraction
- [x] Only call `ctx.data::<StateValue>()` when resolver shape requires context
- [x] Only extract parent when needed (always for non-resolver, always for resolver with parent)
