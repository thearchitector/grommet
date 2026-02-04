# PyO3 0.28 Upgrade And Performance Plan

Upgrade PyO3 to 0.28, align with the upcoming pyo3-async-runtimes 0.28 branch, and apply
performance best practices on both the Rust and Python paths.

## 1) Dependency And Tooling Upgrade [x]

### Rationale

Aligning with PyO3 0.28 and the upcoming async runtime keeps compatibility with current APIs,
raises the MSRV appropriately, and avoids accumulating technical debt.

### Tasks

- [x] Bump `pyo3` to `0.28` in `Cargo.toml` and update `Cargo.lock`.
- [x] Switch `pyo3-async-runtimes` to a git dependency pinned to a specific commit on
  `kyle/prepare-0.28` and record the commit in `Cargo.toml`.
- [x] Confirm the toolchain meets the 0.28 MSRV (>= 1.83). (Current: 1.89.0.)
- [x] Verify `pyproject.toml`/maturin continues to enable extension-module behavior only for
  packaging builds.

### Implementation Notes

- Pinned commit: `b9749069853c06b645902a536bd0ca0dcf2804f0`.


## 2) Free-Threaded Default Audit [x]

### Rationale

PyO3 0.28 defaults to free-threaded compatibility; we must confirm thread-safety or
explicitly opt out.

### Tasks

- [x] Audit shared state and `Py<...>` usage across resolver and runtime paths.
- [x] Opt out of free-threaded mode via `#[pymodule(gil_used = true)]` and document in `README.md`.
- [x] Verify there is no `GILOnceCell` usage requiring migration.
- [x] Add a concurrency test to validate multi-task query execution.

### Implementation Notes

- New Rust test: `schema_wrapper_executes_concurrently`.


## 3) PyO3 API Deprecations And Behavioral Changes [x]

### Rationale

0.28 deprecates several APIs and changes initialization behavior; addressing these up front
prevents warnings and future breakage.

### Tasks

- [x] Confirm no usage of `PyTypeInfo::NAME`/`PyTypeInfo::MODULE` remains.
- [x] Confirm no raw-pointer constructors (`from_owned_ptr`/`from_borrowed_ptr`) are used.
- [x] Confirm no `PyClassInitializer` conversions or `PyBuffer<T>` usage require changes.
- [x] Verify no custom `PyTypeCheck` implementations need updates.

### Implementation Notes

- No occurrences found in the codebase.


## 4) pyo3-async-runtimes 0.28 Alignment [x]

### Rationale

The async runtime is not yet released; aligning early avoids mismatches once PyO3 0.28 is in use.

### Tasks

- [x] Update dependency to the 0.28 prep branch.
- [x] Validate `future_into_py`/`into_future` wrappers compile and behave as expected.
- [x] Re-run `cargo test` to confirm runtime behavior.

### Implementation Notes

- No API adjustments were required after the upgrade.


## 5) Build And Linking Best Practices [x]

### Rationale

PyO3 0.28 introduces build helpers for libpython rpath, reducing the need for ad-hoc linking
workarounds.

### Tasks

- [x] Add `build.rs` with `pyo3_build_config::add_libpython_rpath_link_args()`.
- [x] Remove `LD_LIBRARY_PATH`/`rustflags` overrides from `.cargo/config.toml`.
- [x] Keep minimal env vars in `.cargo/config.toml` for uv-managed Python.
- [x] Update `README.md` with build environment expectations.

### Implementation Notes

- `build.rs` added and `pyo3-build-config` added to build-dependencies.


## 6) Rust-Side Performance Pass [x]

### Rationale

Hot paths in schema parsing and resolver execution can be tightened to reduce interpreter
attachment overhead.

### Tasks

- [x] Reduce `Python::attach` calls in parsing (`type_def_from_input`).
- [x] Review extract/cast usage for potential hot-path improvements.
- [x] Confirm Python call sites use tuple arguments for vectorcall.
- [x] Evaluate `Python::detach` and `pyo3_disable_reference_pool` applicability.

### Implementation Notes

- `Python::attach` is now amortized over field parsing.
- No safe `detach`/reference-pool changes identified yet.


## 7) Python-Side Performance Pass [x]

### Rationale

Schema construction and resolver wrapping are introspection-heavy; caching reduces repeated
work and allocations.

### Tasks

- [x] Cache `_get_type_hints` results in `grommet/typing_utils.py`.
- [x] Cache `inspect.signature` results in `grommet/resolver.py`.
- [x] Precompute argument coercer pairs in `_wrap_resolver`.
- [x] Cache `_get_type_meta` lookups in `grommet/schema.py`.
- [x] Add lightweight microbench scripts for schema build and resolver execution.
- [x] Add a unit test covering type-hints caching.

### Implementation Notes

- New benchmark scripts: `benchmarks/bench_schema_build.py`, `benchmarks/bench_resolver_call.py`.


## 8) Validation And Benchmarks [x]

### Rationale

The upgrade changes core runtime behavior; tight test coverage plus benchmarks keep correctness
and performance stable.

### Tasks

- [x] Run `cargo test`.
- [x] Run `uv run maturin develop -q`.
- [x] Run `uv run pytest`.
- [x] Run `uv run mypy .`.
- [x] Run `uv run prek run -a`.
- [x] Capture benchmark baselines.

### Implementation Notes

- Benchmarks (local):
  - `bench_schema_build.py`: ~0.000281s per schema build (200 runs).
  - `bench_resolver_call.py`: ~0.000351s per query execution (200 runs).
