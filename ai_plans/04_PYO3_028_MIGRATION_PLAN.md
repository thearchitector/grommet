# PyO3 0.28 Upgrade And Performance Plan

Upgrade PyO3 to 0.28, align with the upcoming pyo3-async-runtimes 0.28 branch, and apply
performance best practices on both the Rust and Python paths.

## 1) Dependency And Tooling Upgrade [ ]

### Rationale

Aligning with PyO3 0.28 and the upcoming async runtime keeps compatibility with current APIs,
raises the MSRV appropriately, and avoids accumulating technical debt.

### Tasks

- [ ] Bump `pyo3` to `0.28` in `Cargo.toml` and update `Cargo.lock`.
- [ ] Switch `pyo3-async-runtimes` to a git dependency pinned to a specific commit on
  `kyle/prepare-0.28` and record the commit in the plan and `Cargo.toml` comment.
- [ ] Ensure `rust-toolchain.toml` meets the 0.28 MSRV (>= 1.83). Update if lower.
- [ ] Confirm `pyproject.toml`/maturin config still sets extension-module behavior only for
  distribution builds (not for `cargo test`).

### Implementation Notes

- References:
```
https://pyo3.rs/v0.28.0/changelog
https://github.com/PyO3/pyo3-async-runtimes/compare/kyle/prepare-0.28..v0.27.0
```


## 2) Free-Threaded Default Audit [ ]

### Rationale

PyO3 0.28 now defaults to free-threaded compatibility; we must confirm thread-safety or
explicitly opt out.

### Tasks

- [ ] Audit shared state and `Py<...>` usage in `src/api.rs`, `src/resolver.rs`, `src/runtime.rs`,
  and `src/values.rs` for thread-safety.
- [ ] Decide on free-threaded support vs. opt-out. If opting out, add
  `#[pymodule(gil_used = true)]` to `src/lib.rs` and document the decision in `README.md`.
- [ ] Replace any `GILOnceCell` usage with `PyOnceLock` or `OnceLock` if found.
- [ ] Add or update tests to exercise concurrency (multi-threaded runtime paths) to validate the
  decision.

### Implementation Notes

- References:
```
https://pyo3.rs/v0.28.0/migration.html#from-027-to-028
```


## 3) PyO3 API Deprecations And Behavioral Changes [ ]

### Rationale

0.28 deprecates several APIs and changes initialization behavior; addressing these up front
prevents warnings and future breakage.

### Tasks

- [ ] Search for `PyTypeInfo::NAME` / `PyTypeInfo::MODULE` and migrate to recommended
  alternatives (`PyClass::NAME`, `PyType::qualname/name`) where used.
- [ ] Search for `Py<T>::from_owned` / `from_borrowed` raw-pointer constructors and replace with
  the updated `Bound`/safe APIs.
- [ ] Identify `#[pyclass]` types that derive `Clone` and add explicit
  `#[pyclass(from_py_object)]` or `#[pyclass(skip_from_py_object)]` as appropriate.
- [ ] If any `PyBuffer<T>` usage exists, migrate to `pyo3::buffer::PyUntypedBuffer`.
- [ ] Ensure any custom `PyTypeCheck` implementations are `unsafe` per 0.28 requirements.
- [ ] Verify `#[pymodule]` assumptions still hold with PEP-489 multi-phase initialization.

### Implementation Notes

- References:
```
https://pyo3.rs/v0.28.0/changelog
https://pyo3.rs/v0.28.0/migration.html#from-027-to-028
```


## 4) pyo3-async-runtimes 0.28 Alignment [ ]

### Rationale

The async runtime is not yet released; aligning early avoids mismatches once PyO3 0.28 is in use.

### Tasks

- [ ] Update `src/runtime.rs` wrappers to match any API changes in the 0.28 prep branch
  (notably `tokio::future_into_py`, `tokio::into_future`, and runtime init helpers).
- [ ] Update Rust tests in `tests/test_rust.rs` that depend on async runtime behavior.
- [ ] Re-run `cargo test` after each change to isolate runtime API regressions.

### Implementation Notes

- References:
```
https://github.com/PyO3/pyo3-async-runtimes/compare/kyle/prepare-0.28..v0.27.0
```


## 5) Build And Linking Best Practices [ ]

### Rationale

PyO3 recommends using environment-variable driven linkage for extension builds and provides
build helpers for libpython rpath; using these reduces local config hacks.

### Tasks

- [ ] Replace hardcoded `.cargo/config.toml` linking overrides with a minimal `build.rs` using
  `pyo3_build_config::add_libpython_rpath_link_args`, if possible.
- [ ] Keep `extension-module` behavior only for packaging builds (prefer
  `PYO3_BUILD_EXTENSION_MODULE`), so `cargo test` continues to link to libpython.
- [ ] Update `README.md` to document the expected build environment for uv-managed Python.

### Implementation Notes

- References:
```
https://pyo3.rs/main/building-and-distribution.html
https://pyo3.rs/v0.28.0/changelog
```


## 6) Rust-Side Performance Pass [ ]

### Rationale

The hot paths in schema execution and value conversion can be tightened using PyO3 performance
best practices to reduce per-request overhead.

### Tasks

- [ ] Replace `extract()` with `cast()` where only type checking is required (e.g., in
  `src/values.rs` and `src/parse.rs`).
- [ ] Reduce `Python::attach` calls in loops by threading `Python<'_>` or using `Bound::py()`
  when a bound object is already available.
- [ ] Ensure Python call sites use Rust tuples for arguments to leverage vectorcall (review
  `call_resolver` and similar helpers).
- [ ] Identify any long-running Rust-only work and wrap with `Python::detach`.
- [ ] Evaluate `pyo3_disable_reference_pool` behind a feature flag if drop safety can be
  guaranteed; add tests to validate.

### Implementation Notes

- References:
```
https://pyo3.rs/v0.28.0/performance.html
```


## 7) Python-Side Performance Pass [ ]

### Rationale

Schema construction and resolver wrapping use introspection; caching reduces repeated work and
lowers latency for repeated schema builds.

### Tasks

- [ ] Add caching to `grommet/typing_utils.py::_get_type_hints` (e.g., `functools.lru_cache`).
- [ ] Cache `inspect.signature`/parameter parsing in `grommet/resolver.py` for repeated
  wrappers.
- [ ] Reduce per-call allocations in `_wrap_resolver` by precomputing `(name, coercer)` pairs.
- [ ] In `grommet/schema.py`, cache `_get_type_meta` lookups when building `implements` lists and
  enums/unions.
- [ ] Add microbench tests for schema build and resolver execution to measure before/after.

### Implementation Notes

- Ensure caches have clear keying and bounded size to avoid memory growth.


## 8) Validation And Benchmarks [ ]

### Rationale

The upgrade changes core runtime behavior; tight test coverage plus benchmarks keep correctness
and performance stable.

### Tasks

- [ ] Run `cargo test`.
- [ ] Run `uv run pytest`.
- [ ] Run `uv run mypy .`.
- [ ] Run `uv run prek run -a`.
- [ ] Capture benchmark baselines before and after performance changes.
