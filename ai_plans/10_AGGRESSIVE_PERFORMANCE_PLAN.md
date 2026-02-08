# Aggressive Performance Optimization

> i've made significant changes to the entire codebase, including major refactors and code removal, and huge API changes.
> reexamine the entire codebase, looking for radical ways to boost performance as aggressively as possible, and create a plan

Eliminate per-field overhead in the Rust↔Python boundary. The refactored codebase has a cleaner architecture but
retains the same fundamental hot-path costs: a Python wrapper coroutine per resolver, unconditional Context/Lookahead
construction, trial-and-error type conversion, and redundant GIL acquisitions. This plan targets each independently.

**Current hot path per resolver-backed field** (3 GIL acquires, 2 coroutine creations):

```
Rust: Python::attach → build_kwargs (PyDict) → build_context_obj (import module, walk selection tree,
      construct Lookahead, construct Context dataclass) → PyTuple::new([parent, context]) →
      wrapper.call(args, kwargs)
Python wrapper: dict(self=parent) → maybe add context → coerce args → resolver(**call_kwargs) → await
Rust: Python::attach → hasattr("__await__") → into_future → await
Rust: Python::attach → py_to_field_value_for_type (try bool → try i64 → try f64 → try String → fallback)
```

## 1) Eliminate the Python Wrapper — Call User Resolver Directly from Rust [x]

### Rationale

Every resolver call passes through `_wrap_resolver`'s `async def wrapper(parent, context_obj, **kwargs)` closure
before reaching the user function. This adds: 1 Python function call, 1 coroutine creation, 1 dict allocation
(`call_kwargs`), `**kwargs` unpacking, and 1 `await`. For every single field. Removing this layer is the single
highest-impact change.

### Tasks

- [ ] Add a `ResolverShape` enum to `types.rs` describing the resolver's parameter pattern:

  ```rust
  #[derive(Clone, Copy, PartialEq, Eq)]
  pub(crate) enum ResolverShape {
      SelfOnly,          // async def value(self) -> T
      SelfAndContext,    // async def value(self, ctx: Context) -> T
      SelfAndArgs,       // async def value(self, name: str) -> T
      SelfContextAndArgs,// async def value(self, ctx: Context, name: str) -> T
  }
  ```

- [ ] Add a `ResolverEntry` struct to `types.rs`:

  ```rust
  pub(crate) struct ResolverEntry {
      pub(crate) func: PyObj,           // raw user function
      pub(crate) shape: ResolverShape,
      pub(crate) arg_coercers: Vec<(String, Option<PyObj>)>,  // (name, coercer_or_none)
  }
  ```

- [ ] In Python `resolver.py`, replace `_wrap_resolver` with `_analyze_resolver` that returns a dict/object containing:
  - `"func"`: the raw user resolver (not a wrapper)
  - `"shape"`: string tag (`"self_only"`, `"self_and_context"`, `"self_and_args"`, `"self_context_and_args"`)
  - `"arg_coercers"`: list of `(name, coercer_fn | None)` tuples for GraphQL args

  All validation (async check, annotation check) stays. Only the wrapper closure is removed.

- [ ] Update `_wrap_plan_resolvers` in `plan.py` to store `_analyze_resolver`'s output dict in the resolvers map
  instead of the wrapper callable. Update `SchemaPlan.resolvers` type accordingly.

- [ ] In `parse.rs`, update resolver map extraction to parse the new dict structure into `ResolverEntry` structs.
  Change the map type from `HashMap<String, PyObj>` to `HashMap<String, ResolverEntry>`.

- [ ] Update `FieldContext` in `types.rs` to hold `Option<ResolverEntry>` instead of `Option<PyObj>`. Remove
  `arg_names` (now inside `ResolverEntry.arg_coercers`).

- [ ] Rewrite `call_resolver` in `resolver.rs` to dispatch based on `ResolverShape`:

  ```rust
  match entry.shape {
      ResolverShape::SelfOnly => {
          entry.func.call1(py, (parent_obj,))
      }
      ResolverShape::SelfAndContext => {
          let ctx_obj = build_context_obj(py, ctx, state)?;
          entry.func.call1(py, (parent_obj, ctx_obj))
      }
      ResolverShape::SelfAndArgs => {
          let kwargs = build_kwargs_with_coercion(py, ctx, &entry.arg_coercers)?;
          entry.func.call(py, (parent_obj,), Some(&kwargs))
      }
      ResolverShape::SelfContextAndArgs => {
          let ctx_obj = build_context_obj(py, ctx, state)?;
          let kwargs = build_kwargs_with_coercion(py, ctx, &entry.arg_coercers)?;
          entry.func.call(py, (parent_obj, ctx_obj), Some(&kwargs))
      }
  }
  ```

- [ ] Write `build_kwargs_with_coercion` in `resolver.rs` that builds kwargs and applies Python coercer callables
  inline (replacing the Python-side coercion loop):

  ```rust
  fn build_kwargs_with_coercion(py, ctx, coercers) -> PyResult<PyDict> {
      let kwargs = PyDict::new(py);
      for (name, coercer) in coercers {
          if let Ok(value) = ctx.args.try_get(name) {
              let py_value = value_to_py(py, value.as_value())?;
              let final_value = match coercer {
                  Some(c) => c.call1(py, (py_value,))?,
                  None => py_value,
              };
              kwargs.set_item(name, final_value)?;
          }
      }
      Ok(kwargs)
  }
  ```

- [ ] Update `build.rs` to propagate `ResolverEntry` through `build_field_context`.

- [ ] Delete the `async def wrapper(...)` closure and `_wrap_resolver` from `resolver.py`. Rename `_analyze_resolver`
  to the public name used by `plan.py`.

### Implementation Notes

The user function is called with keyword arguments matching its parameter names. Since resolvers are instance methods,
`self` is always the first positional arg. The Rust side passes `self` positionally and everything else as kwargs.
This matches Python's calling convention: `func(parent, ctx=ctx_obj, name="foo")` works regardless of parameter name
for `self`.

**Important**: The user function's `self` parameter is just a name — it receives the parent PyObj, not an actual bound
method self. Since `@gm.field` stores the raw function (not a bound method), calling `func(parent)` is equivalent to
`instance.func()` when parent IS the instance.

For the `SelfOnly` path (most common), this collapses the entire call to a single `func.call1(py, (parent,))` — no
dicts, no kwargs, no extra coroutine.

Arg coercers are only needed for `@gm.input` types (dict→dataclass). For `str`, `int`, `float`, `bool` — no coercer
is needed since async-graphql already provides the correct Python type. The current `_arg_coercer` in `coercion.py`
already returns `None` for these cases, so most resolvers with scalar args will have all-None coercers and the kwargs
loop becomes a simple pass-through.


## 2) Lazy Context Construction — Skip When Resolver Doesn't Use It [x]

### Rationale

`build_context_obj` is called for every resolver invocation. It: imports `grommet.context`, walks the selection tree
to build a `Lookahead` (recursive `HashMap` construction), creates a Python `Context` dataclass, and builds a kwargs
dict. For resolvers that are just `async def value(self) -> str`, this is 100% wasted work.

After Section 1, the `ResolverShape` enum already encodes whether context is needed. The `SelfOnly` and `SelfAndArgs`
shapes skip context construction entirely.

### Tasks

- [ ] This is already handled by the shape dispatch in Section 1 — `SelfOnly` and `SelfAndArgs` never call
  `build_context_obj`. Verify this is the case after implementing Section 1.

- [ ] Remove the unconditional `build_context_obj` call from `call_resolver`. It should only be called in the
  `SelfAndContext` and `SelfContextAndArgs` branches.

### Implementation Notes

The `Lookahead` walk (`extract_lookahead`) recurses the entire selection subtree. For deeply nested queries, this is
non-trivial work. Skipping it for the ~80% of resolvers that don't use Context is a significant win.


## 3) Cache the Context Class Reference [x]

### Rationale

`build_context_obj` calls `py.import("grommet.context")?.getattr("Context")?` on every invocation. Python module
imports are cached by the interpreter, but the `import` + `getattr` still involves dict lookups and reference counting
per call.

### Tasks

- [ ] Add a `context_cls: PyObj` field to `SchemaWrapper` (or a module-level `OnceLock<PyObj>`).

- [ ] In `SchemaWrapper::new`, resolve and cache the `Context` class:
  ```rust
  let context_cls = py.import("grommet.context")?.getattr("Context")?.unbind();
  ```

- [ ] Pass the cached class through to `FieldContext` or make it available via a shared `Arc`.

- [ ] Update `build_context_obj` to accept the cached class instead of importing each time.

### Implementation Notes

A `OnceLock<PyObj>` at module level is simplest but requires careful GIL handling. Storing on `SchemaWrapper` and
propagating through `Arc` is safer and more explicit.


## 4) Specialize `py_to_field_value` by Known Output Type [x]

### Rationale

`py_to_field_value` tries `extract::<bool>()` → `extract::<i64>()` → `extract::<f64>()` → `extract::<String>()` in
sequence for every scalar value. When the output type is `TypeRef::Named("String")`, we already know it's a string.
Skip the other checks entirely.

For object types (the fallback `FieldValue::owned_any`), all four extractions fail before reaching the correct branch.
This is the most common case for nested types and the most wasteful.

### Tasks

- [ ] Add a `ScalarHint` enum to `types.rs`:
  ```rust
  #[derive(Clone, Copy)]
  pub(crate) enum ScalarHint {
      String,
      Int,
      Float,
      Boolean,
      Object,   // grommet-decorated type — wrap as PyObj directly
      Unknown,  // fallback to trial-and-error
  }
  ```

- [ ] Compute `ScalarHint` at schema build time from `TypeRef`. In `build_field_context` or `parse.rs`, map named
  types: `"String"` → `String`, `"Int"` → `Int`, `"Float"` → `Float`, `"Boolean"` → `Boolean`. If the name matches a
  registered object type, use `Object`. Otherwise `Unknown`.

- [ ] Store `ScalarHint` in `FieldContext` alongside `output_type`.

- [ ] Rewrite `py_to_field_value_for_type` to use the hint for `TypeRef::Named`:
  ```rust
  TypeRef::Named(_) => match field_ctx.scalar_hint {
      ScalarHint::String => Ok(FieldValue::value(Value::String(value.extract()?))),
      ScalarHint::Int => Ok(FieldValue::value(Value::from(value.extract::<i64>()?))),
      ScalarHint::Float => Ok(FieldValue::value(Value::from(value.extract::<f64>()?))),
      ScalarHint::Boolean => Ok(FieldValue::value(Value::Boolean(value.extract()?))),
      ScalarHint::Object => Ok(FieldValue::owned_any(PyObj::new(value.clone().unbind()))),
      ScalarHint::Unknown => py_to_field_value(py, value),
  }
  ```

- [ ] For `TypeRef::List(inner)`, propagate the inner hint to avoid per-element trial-and-error.

### Implementation Notes

The `Object` hint is the critical one — it skips 4 failed type extractions per nested object. For a query returning
100k rows with 5 cells each, that's 2M avoided extraction attempts (4 × 500k objects).

The hint must be computed from the set of registered type names. During `build_schema`, collect all object type names
into a `HashSet<String>`. When building `FieldContext`, check if the named type is in the set → `Object`, is a known
scalar name → specific hint, else → `Unknown`.


## 5) Skip `__await__` Check in `into_future` [x]

### Rationale

`runtime.rs:into_future` calls `bound.hasattr("__await__")?` before converting to a Rust future. Since every resolver
is validated as `async def` at schema build time (in `_analyze_resolver`), this check is redundant at runtime.

### Tasks

- [ ] Remove the `hasattr("__await__")` check from `into_future`:
  ```rust
  pub(crate) fn into_future(
      awaitable: Py<PyAny>,
  ) -> PyResult<impl Future<Output = PyResult<Py<PyAny>>> + Send + 'static> {
      Python::attach(|py| {
          let bound = awaitable.into_bound(py);
          pyo3_async_runtimes::tokio::into_future(bound)
      })
  }
  ```

- [ ] Optionally: inline `into_future` into `await_awaitable` since the function is trivial.

### Implementation Notes

`pyo3_async_runtimes::tokio::into_future` will still raise a Python error if passed a non-awaitable, so safety is
preserved. The `hasattr` check just provided a slightly better error message.


## 6) Optimize `resolve_from_parent` — Single `getattr` [x]

### Rationale

`resolve_from_parent` calls `hasattr(source_name)` then `getattr(source_name)` — two Python attribute lookups.
A single `getattr` with fallback is sufficient.

### Tasks

- [ ] Replace the two-call pattern:
  ```rust
  fn resolve_from_parent(py: Python<'_>, parent: &PyObj, source_name: &str) -> PyResult<Py<PyAny>> {
      match parent.bind(py).getattr(source_name) {
          Ok(val) => Ok(val.unbind()),
          Err(_) => Ok(py.None()),
      }
  }
  ```

### Implementation Notes

Dataclass fields always exist as attributes (either in `__dict__` or `__slots__`), so the `Err` branch is
effectively dead code for well-formed schemas. The single-call version is both faster and simpler.


## 7) Eliminate Empty kwargs Dict Allocation [x]

### Rationale

`build_kwargs` creates a `PyDict::new(py)` even when `arg_names` is empty (i.e., the resolver has no GraphQL args).
After Section 1, this is already handled by shape dispatch (`SelfOnly` and `SelfAndContext` never build kwargs). But
verify that no other code path creates an empty dict unnecessarily.

### Tasks

- [ ] After Section 1, audit all remaining `build_kwargs` / `build_kwargs_with_coercion` call sites to confirm they
  are only reached when args are actually present.

- [ ] If `call_resolver` still has a code path that builds an empty kwargs dict, guard it:
  ```rust
  let kwargs = if coercers.is_empty() { None } else { Some(build_kwargs_with_coercion(...)?) };
  ```

### Implementation Notes

This is a minor optimization on its own but compounds with Section 1. In the `SelfAndArgs` branch, if the query
doesn't actually provide any of the optional args, we'd still create an empty dict. The guard prevents this.


## 8) Reduce GIL Acquisitions per Resolver Call [x]

### Rationale

The current hot path acquires the GIL 3 times per resolver: once to call, once to convert the coroutine, once to
convert the result. After Section 5, the second acquire is slightly cheaper but still present. Merging GIL
acquisitions where possible reduces overhead.

### Tasks

- [ ] Investigate whether `call_resolver` + `into_future` can share a single `Python::attach` block. Currently:
  ```rust
  let coroutine = Python::attach(|py| call_resolver(...))?;  // GIL 1
  let future = Python::attach(|py| into_future(...))?;        // GIL 2 (inside await_awaitable)
  let result = future.await?;
  let field_value = Python::attach(|py| py_to_field_value(...))?; // GIL 3
  ```

  Merge GIL 1 and 2:
  ```rust
  let future = Python::attach(|py| {
      let coroutine = call_resolver(py, ...)?;
      pyo3_async_runtimes::tokio::into_future(coroutine.into_bound(py))
  })?;
  let result = future.await?;
  let field_value = Python::attach(|py| py_to_field_value(...))?;
  ```

  This reduces per-resolver GIL acquisitions from 3 to 2.

### Implementation Notes

`pyo3_async_runtimes::tokio::into_future` requires a `Bound<'py, PyAny>`, so the coroutine must be converted within
the same GIL block. This should work since `call_resolver` returns a `Py<PyAny>` which can be bound in the same
scope.

GIL 3 cannot be merged because we need to `await` the future first (which releases the GIL).


## 9) Pre-compute Object Type Set for `py_to_field_value` [x]

### Rationale

Related to Section 4. The `ScalarHint::Object` classification requires knowing which type names are grommet object
types at build time. This set is already implicitly constructed during `build_schema` but not exposed.

### Tasks

- [ ] During `build_schema`, collect all object type names into an `Arc<HashSet<String>>`.

- [ ] Pass this set into `build_field_context` and use it to compute `ScalarHint` for each field's output type.

- [ ] For `TypeRef::List(inner)`, recursively resolve the inner hint.

### Implementation Notes

The set only needs to contain object type names (not input types, not scalars). Input types are never output types,
and scalars map to known hints directly.
