# Implementation Plan: Rust 2024 + Tokio 1.49 + PyO3 async runtime refactor

## Research findings that drive changes
- Runtime usage is concentrated in `src/api.rs` (future_into_py + runtime init), `src/resolver.rs` (into_future), and tests (`tests/test_rust.rs`).
- Tokio usage is limited to `tokio::runtime::Builder` and `tokio::sync::Mutex` (no tokio macros in production code).
- `pyo3-async-runtimes` best practice is to:
  - Convert Python coroutines with `pyo3_async_runtimes::tokio::into_future` (await in Rust).
  - Convert Rust futures with `pyo3_async_runtimes::tokio::future_into_py` (await in Python).
  - Avoid `#[tokio::main]`/`#[async_std::main]` for PyO3 interop.
- uvloop compatibility requires installing the uvloop policy on the Python side before creating/awaiting Rust futures (call inside the coroutine passed to `asyncio.run`).
- `dict-derive` provides `FromPyObject`/`IntoPyObject` derives for mapping dicts ⇄ Rust structs, which can replace manual parsing in `src/parse.rs` if compatible with PyO3 0.27.
- Python resolver wrapping (`grommet/resolver.py`) currently checks awaitables at runtime; we can enforce async-only resolvers in Python and simplify Rust call paths.
- PyO3 conversion docs emphasize that Python-native types (`Bound<'py, PyAny>`, `PyDict`, `PyList`) are near zero-cost compared to converting into Rust std types.

## 1) Toolchain + dependency updates
**Files:** `Cargo.toml` (and optionally `rust-toolchain.toml`).

- [ ] Update `edition = "2024"` in `Cargo.toml`.
- [ ] Pin Tokio to `1.49` and trim features to those actually used:
  - Required today: runtime builder + `tokio::sync::Mutex`.
  - Plan: remove `macros`; keep `rt-multi-thread` + `sync`.
  - If we keep `builder.enable_all()`, add `time`/`io`/`net` or replace `enable_all()` with only the needed `enable_*` calls.
- [ ] Confirm `pyo3`/`pyo3-async-runtimes` versions remain compatible with Tokio 1.49.
- [ ] If CI/tooling needs it, add `rust-toolchain.toml` with a Rust version that supports Edition 2024 (1.85+).
- [ ] Rust 2024 migration pass:
  - Run `cargo fix --edition` to apply rust-2024-compatibility lints.
  - Audit for new prelude imports (`Future`, `IntoFuture`) causing method ambiguities; disambiguate with explicit trait paths if needed.
  - Check for `gen` identifiers and rename to `r#gen` if any remain (keyword_idents_2024).
  - Check for reserved guarded string syntax (`#"..."#`, `###`) in macro calls and insert spaces if needed.
  - Review any `macro_rules!` using `$e:expr` for const or `_` ambiguity; switch to `expr_2021` only when needed.
  - Review `if let` scrutinees that hold locks/borrows into an `else` block; bind temporaries before the `if let` if necessary.
  - Ensure unsafe operations inside `unsafe fn` are wrapped in explicit `unsafe {}` blocks.
  - If a workspace is introduced, set `[workspace] resolver = "3"` (edition 2024 implies this for top-level packages).

## 2) Align async runtime usage with pyo3-async-runtimes best practices
**Files:** `src/api.rs`, `src/resolver.rs`, `tests/test_rust.rs`.

- [ ] Add a small runtime helper module (e.g., `src/runtime.rs`) to centralize:
  - `future_into_py` (Rust future → Python awaitable).
  - `into_future` (Python coroutine → Rust future).
  - Error mapping from `PyErr` → `async_graphql::Error`.
- [ ] Refactor `src/api.rs` to use the helper for all `future_into_py` calls (`SchemaWrapper.execute`, `SubscriptionStream.__anext__`, `SubscriptionStream.aclose`).
- [ ] Refactor `src/resolver.rs` to use the helper for all `into_future` calls:
  - Keep the awaitable check, but centralize it in a helper to remove manual duplication.
  - Ensure `__anext__` awaitables and resolver coroutines are always passed through `into_future` as per docs.
- [ ] Update tests in `tests/test_rust.rs` (if needed) to use the same helper or `pyo3_async_runtimes::tokio::run` consistently, avoiding tokio macros.

## 3) Shift resolver assumptions to Python (less defensive Rust)
**Files:** `grommet/resolver.py`, `grommet/schema.py`, `grommet/decorators.py`, `grommet/errors.py`, `src/resolver.rs`, `src/api.rs`, tests.

- [ ] Add Python-side validation errors (e.g., `resolver_requires_async`, `subscription_requires_async_iterator`).
- [ ] Enforce async-only resolvers in Python:
  - Query/mutation/interface resolvers must be `inspect.iscoroutinefunction`.
  - Subscription resolvers must be async generators or coroutine functions returning `AsyncIterator`.
- [ ] Update `_wrap_resolver` to remove per-call `inspect.isawaitable` checks and always `await` async resolvers (or return async generators directly).
- [ ] Pass resolver kind from `schema._build_schema_definition` so `_wrap_resolver` can enforce the correct async contract.
- [ ] Simplify Rust resolver paths:
  - Remove `is_awaitable` checks and assume resolver values are awaitable.
  - Drop `__await__` validation in subscription streaming and rely on Python-side enforcement.
  - Assume subscription resolvers return async iterators; remove `__aiter__`/`__anext__` fallback logic.
- [ ] Update tests to cover Python-side validation errors and remove expectations of Rust-side defensive errors.

## 4) uvloop + asyncio.run compatibility
**Files:** `README.md` (if present), Python-facing docs, and/or module docstrings.

- [ ] Add a short “Event loop compatibility” section for Python users:
  - Call `uvloop.install()` before creating/awaiting any Grommet futures.
  - When using `asyncio.run`, call Grommet APIs inside the coroutine passed to `asyncio.run` (avoid “no running event loop”).
- [ ] Ensure `configure_runtime` does not eagerly touch the Python event loop and is safe to call after uvloop is installed.
- [ ] If needed, expose a lightweight hook in Python to set the event loop policy before any Rust async calls.

## 5) Reduce manual Rust with PyO3 helper crates
**Files:** `src/types.rs`, `src/parse.rs`, `Cargo.toml`.

- [ ] Evaluate `dict-derive` compatibility with PyO3 0.27.
  - If compatible, add `dict_derive` dependency.
  - If not, fall back to `#[derive(FromPyObject)]` with `#[pyo3(from_item_all)]` or `pyo3-serde`.
- [ ] Introduce typed input structs for schema parsing (e.g., `SchemaDefInput`, `TypeDefInput`, `FieldDefInput`, `ArgDefInput`, `ScalarDefInput`, `EnumDefInput`, `UnionDefInput`).
- [ ] Implement `FromPyObject` for `PyObj` so dict-derived structs can hold Python values without manual `.unbind()` calls.
- [ ] Replace manual dict parsing in `src/parse.rs` with `.extract::<...>()` on the new structs and map missing-field errors to existing `missing_field(...)` errors.
- [ ] Remove now-redundant parsing helpers (or keep thin wrappers if they preserve error semantics).

## 6) Conversion cost audit (PyO3)
**Files:** `src/api.rs`, `src/parse.rs`, `src/values.rs`, `src/types.rs`.

- [ ] Prefer Python-native types in PyO3 entrypoints and helpers when values are only forwarded/inspected (avoid converting to `String`/`Vec`/`HashMap` unless needed).
- [ ] Use `#[derive(FromPyObject)]` with `#[pyo3(item)]` / `#[pyo3(from_item_all)]` for dict-based inputs to reduce manual conversions.
- [ ] Consider `#[pyo3(transparent)]` newtypes for lightweight wrappers that can extract directly from the input object.
- [ ] Keep conversions to Rust std types only when the data must outlive the GIL or be fed into async-graphql structures.
- [ ] Ensure Python return values use zero-cost `Py<PyAny>`/`Bound<'py, PyAny>` handles where possible.

## 7) Verification
**Commands:**
- [ ] `cargo check`
- [ ] `cargo test`
- [ ] `uv run mypy .`
- [ ] `uv run prek run -a`

## Deliverables checklist
- [ ] `Cargo.toml` updated to Edition 2024 + Tokio 1.49 + minimal features.
- [ ] Runtime interop uses shared helpers consistent with pyo3-async-runtimes guidance.
- [ ] Python-side resolver validation enforces async-only resolvers; Rust assumes awaitables.
- [ ] uvloop/asyncio.run guidance documented for Python users.
- [ ] Manual dict parsing reduced via `dict-derive` or `FromPyObject` derivations.
- [ ] PyO3 conversion costs reduced by favoring Python-native types at boundaries.
- [ ] Rust 2024 compatibility lints applied and any edition-specific behavior changes reviewed.
- [ ] All checks pass.
