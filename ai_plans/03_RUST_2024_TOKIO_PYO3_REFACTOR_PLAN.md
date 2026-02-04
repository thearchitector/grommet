# Implementation Plan: Rust 2024 + Tokio 1.49 + PyO3 async runtime refactor

This plan is grounded in the current code paths:
- Rust async interop lives in `src/api.rs` (`SchemaWrapper.execute`, `SubscriptionStream.__anext__`, `SubscriptionStream.aclose`) and `src/resolver.rs` (`into_future` + awaitable checks).
- Python resolver wrapping lives in `grommet/resolver.py` and is wired by `grommet/schema.py::_build_schema_definition`.
- Manual dict parsing for schema definitions lives in `src/parse.rs` and is validated by missing-field tests in `tests/test_rust.rs`.

## 1) Rust 2024 toolchain + edition bump [x]

### Rationale
Moving to Edition 2024 unlocks modern language features and aligns with the stated target for the refactor.

### Scope
- `Cargo.toml`: bump `edition = "2024"`.
- `rust-toolchain.toml`: add a pinned toolchain (>= 1.89) to satisfy async-graphql's MSRV.
- `src/**/*.rs`: apply any compiler-driven fixes required by the edition change.

### Implementation Notes
- Run `cargo fix --edition` and address any edition-specific warnings (keyword_idents_2024, `unsafe` in `unsafe fn`, etc.).
- Re-run `cargo check` after edits to confirm no new lint failures.

### Impact
- Consistent compiler behavior across environments.
- Cleaner baseline for the rest of the refactor.

## 2) Tokio 1.49 bump + feature trim [x]

### Rationale
Tokio is only used for the runtime builder and `tokio::sync::Mutex`, so the dependency can be modernized and kept lean.

### Scope
- `Cargo.toml`: update `tokio` to `1.49` and drop the unused `macros` feature.
- `src/api.rs`: keep `configure_runtime` behavior unchanged for now (still uses `builder.enable_all()`).

### Implementation Notes
- Set `tokio = { version = "1.49", features = ["rt-multi-thread", "sync"] }`.
- `rg "tokio::(main|test)"` confirms no macro usage; remove `macros` without further code changes.
- If `cargo check` reports missing runtime features after the bump, re-add the minimum required Tokio features rather than reintroducing `macros`.

### Impact
- Updated Tokio version with a smaller feature set.
- No behavioral change in runtime initialization.

## 3) Centralize PyO3 async runtime interop [x]

### Rationale
`future_into_py` and `into_future` calls are duplicated across `src/api.rs` and `src/resolver.rs`. A shared helper keeps error handling consistent and makes future upgrades safer.

### Scope
- New file: `src/runtime.rs`.
- Update call sites in `src/api.rs`, `src/resolver.rs`, and `tests/test_rust.rs`.

### Implementation Notes
- Add helper functions such as:
  - `future_into_py(py, fut)` wrapper around `pyo3_async_runtimes::tokio::future_into_py`.
  - `into_future(py, awaitable)` wrapper around `pyo3_async_runtimes::tokio::into_future` with `PyErr -> async_graphql::Error` mapping.
- Replace direct `pyo3_async_runtimes::tokio::*` usage in:
  - `SchemaWrapper.execute`, `SubscriptionStream.__anext__`, `SubscriptionStream.aclose`.
  - `resolve_python_value` and `subscription_stream` in `src/resolver.rs`.
  - Any test helpers in `tests/test_rust.rs` that create futures from awaitables.

### Impact
- One consistent interop path and error conversion logic.
- Easier to audit runtime usage after the Tokio bump.

## 4) Enforce async-only resolver contracts in Python [x]

### Rationale
Resolver behavior is currently permissive (sync functions are allowed). Moving validation to Python makes the contract explicit and lets Rust assume awaited values.

### Scope
- `grommet/resolver.py`: update `_wrap_resolver` to enforce async-only resolvers.
- `grommet/schema.py`: pass resolver kind to `_wrap_resolver`.
- `grommet/errors.py`: add errors for non-async resolvers.
- Tests: `tests/python/test_resolver_coverage.py`, `tests/python/test_subscriptions.py`, `tests/python/test_errors_coverage.py`.
- Docs: `README.md` (Notes section).

### Implementation Notes
- Extend `_wrap_resolver` to accept a `kind: str` ("object"/"interface"/"subscription") and `field_name` for clearer errors.
- Non-subscription resolvers:
  - Require `inspect.iscoroutinefunction(resolver)`; raise `resolver_requires_async(...)` if not.
  - Call `await resolver(**kwargs)` (drop `inspect.isawaitable` for this path).
- Subscription resolvers:
  - Allow `inspect.isasyncgenfunction(resolver)` **or** `inspect.iscoroutinefunction(resolver)`.
  - If coroutine function, `await` it and return the resulting async iterator.
  - If async generator function, return the generator object directly.
- Update `_build_schema_definition` to pass the resolver kind into `_wrap_resolver` for correct enforcement.
- Add error helpers in `grommet/errors.py` and update `tests/python/test_errors_coverage.py` to cover them.
- Update `README.md` Notes section to say resolvers **must** be async (remove \"sync resolvers also work\").

### Impact
- Clear, enforceable async-only resolver contract.
- Rust can assume resolver results are awaited.

## 5) Simplify Rust resolver pipeline based on async-only contract [x]

### Rationale
Once Python enforces async-only resolvers, Rust no longer needs per-call awaitable checks for resolver paths.

### Scope
- `src/resolver.rs`: simplify resolver execution and subscription iteration.

### Implementation Notes
- Remove `is_awaitable` from `call_resolver` and `resolve_from_parent` return values.
- In `resolve_python_value`:
  - If `resolver` is present, always `await_value(...)`.
  - If `resolver` is absent, return the parent/root value directly without checking `__await__`.
- In `subscription_stream`, drop the explicit `__await__` check on `__anext__` results and call `into_future` directly.
- Keep `subscription_iterator`'s `__aiter__`/`__anext__` validation, but remove any redundant awaitable checks.

### Impact
- Less branching and fewer Python attribute lookups on hot paths.
- Behavior is clearer and aligned with the Python-side contract.

## 6) Dict parsing refactor using PyO3 derives (no `dict-derive`) [x]

### Rationale
Manual dict parsing is verbose and error-prone. The `dict-derive` crate is not viable with our current PyO3 version, so we should use PyO3's built-in derives instead.

### Decision (evaluated now)
- `dict-derive` targets older PyO3 versions (docs/examples reference PyO3 0.22 APIs), while this project uses `pyo3 = 0.27.2`.
- **Decision:** Do **not** adopt `dict-derive`. Use `#[derive(FromPyObject)]` with `#[pyo3(from_item_all)]` instead.

### Scope
- `src/types.rs` or `src/parse.rs`: add `*Input` structs that derive `FromPyObject` for schema/field/arg/scalar inputs.
- `src/parse.rs`: replace manual dict extraction with `.extract::<...>()` + mapping into existing `SchemaDef`, `TypeDef`, etc.
- Preserve error strings validated by `tests/test_rust.rs` (e.g., "Missing field name").

### Implementation Notes
- Define input structs (examples):
  - `SchemaDefInput { schema: SchemaInput, types: Vec<TypeDefInput>, scalars: Option<Vec<ScalarDefInput>>, enums: Option<Vec<EnumDefInput>>, unions: Option<Vec<UnionDefInput>> }`
  - `FieldDefInput`, `ArgDefInput`, `ScalarBindingInput`, etc.
- Use `#[pyo3(from_item_all)]` so dict keys map to fields.
- After extraction, validate required fields explicitly to preserve `missing_field(...)` error messages.
- Map Python values to `PyObj` where needed to keep ownership semantics unchanged.

### Impact
- Significantly smaller parsing code with the same error surface.
- Fewer manual `PyDict`/`PyList` manipulations.

## 7) Documentation and runtime guidance [x]

### Rationale
Users need clear guidance on async-only resolvers and event loop setup.

### Scope
- `README.md`: update Notes section and add event loop guidance.
- Optionally add a short note in `grommet/runtime.py` docstring.

### Implementation Notes
- Add a short \"Event loop compatibility\" note:
  - Call `uvloop.install()` before creating/awaiting Grommet futures.
  - Use `asyncio.run` by calling Grommet APIs inside the coroutine passed to it.

### Impact
- Clearer user-facing guidance on async contracts and loop configuration.

## 8) Verification [x]

### Commands
- `cargo check`
- `cargo test` (fails to link Python symbols in this environment)
- `uv run mypy .`
- `uv run prek run -a`

## Deliverables checklist
- [x] Edition bumped to 2024 with a pinned toolchain.
- [x] Tokio bumped to 1.49 with `macros` removed.
- [x] Runtime interop centralized in `src/runtime.rs`.
- [x] Python enforces async-only resolvers; README updated accordingly.
- [x] Rust resolver pipeline simplified to assume awaited resolver results.
- [x] Dict parsing refactored with PyO3 derives (no `dict-derive`).
- [x] Docs include event loop compatibility guidance.
- [ ] All verification commands pass.
