# Subscription Streams via pyo3-async-runtimes Unstable Streams

Adopt the `pyo3-async-runtimes` `unstable-streams` feature to simplify Python async iterator handling for
GraphQL subscriptions while preserving current error/backpressure semantics.

## 1) Enable Unstable Streams + Wire New Stream Adapter [x]

### Rationale

Centralizing async-iterator bridging in `pyo3-async-runtimes` reduces custom glue code and aligns with
runtime-provided semantics.

### Tasks

- [x] Add the `unstable-streams` feature to the `pyo3-async-runtimes` dependency in `Cargo.toml` and
      ensure feature selection is documented in the crate metadata if required.
- [x] Update `src/resolver.rs` subscription stream creation to use
      `pyo3_async_runtimes::tokio::into_stream_v1` (or v2 if chosen) instead of the manual
      `__anext__` loop.
- [x] Adapt the stream mapping to `Result<FieldValue<'_>, Error>` by converting Python items to
      `FieldValue` and mapping Python errors to GraphQL errors.

### Implementation Notes

Prefer `into_stream_v1` initially because it yields `PyResult` items, which map cleanly to the current
error propagation model. If v2 is chosen, confirm how runtime errors surface and adjust mapping
accordingly.

## 2) Preserve Subscription Semantics (Errors, Stop, Backpressure) [x]

### Rationale

Behavior changes in subscriptions are user-visible; the new adapter must preserve completion, error,
and backpressure expectations.

### Tasks

- [x] Verify that `StopAsyncIteration` ends the stream (no extra errors or items).
- [x] Ensure resolver exceptions continue to surface as GraphQL errors in the subscription stream.
- [x] Validate backpressure/serialization guarantees remain intact (no concurrent `__anext__`).

### Implementation Notes

`into_stream_v1` uses `async-channel` and spawns a Rust task to call `__anext__` serially; confirm
this matches existing backpressure tests.

## 3) Tests, Docs, and Tooling Validation [x]

### Rationale

We need coverage for the new code path and to keep documentation and tooling checks green.

### Tasks

- [x] Update/add tests in `tests/python/test_subscriptions.py` and `tests/test_rust.rs` to exercise the
      new stream adapter path and error handling.
- [x] Update any relevant docs/TODOs that describe subscription behavior or runtime requirements (no
      changes required).
- [x] Run `uv run mypy .`, the Python/Rust test suites, and `prek run -a` to confirm correctness.

### Implementation Notes

If tests rely on specific timing/backpressure behavior, keep the channel size and polling behavior
aligned with existing expectations or adjust tests with justification.
