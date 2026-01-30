# Beta Release Plan

## 1) Core GraphQL Features
- **Subscriptions (async iterator support)** (done)
  - [x] Extend Rust core to support subscription execution in async-graphql dynamic schema.
  - [x] Python resolver returns `AsyncIterator`/`AsyncIterable`; bridge via pyo3-asyncio to stream results.
  - [x] Add Python `Schema.subscribe()` returning an async generator yielding payloads.
  - [x] Tests: cancellation behavior, backpressure.

- **Custom scalars** (done)
  - [x] Allow user-defined scalar mapping in Python with parse/serialize hooks.
  - [x] Expose scalar registration in Schema builder.
  - [x] Tests: invalid input errors. (Parse/serialize roundtrip covered.)

- **Enums + Unions + Interfaces** (done)
  - [x] Add Python definitions for enum, union, interface types.
  - [x] Map to async-graphql dynamic types and resolve type selection.
  - [x] Tests: union/interface resolution, enum input/output.

- **Field argument defaults + input validation** (partial)
  - [x] Ensure GraphQL arg default values map from Python defaults.
  - [x] Improve error reporting for invalid inputs and missing required fields.
  - [x] Tests: missing required, nested input errors. (Default arg + invalid input covered.)

## 2) Python API Improvements
- **Resolver signature + context typing**
  - [x] Standardize resolver params: `parent`, `info`, `context`, `root`, `**kwargs`.
  - [x] Provide a lightweight `Info`/`Context` Python struct or mapping.
  - [x] Tests: info/context availability, root passthrough.

- **Dataclass field inference and nullability**
  - Tighten Optional/nullability handling, list nullability, nested nullability.
  - Optional fields are strictly nullable. Non-optional fields are non-nullable.
  - Tests: list vs. list-of-non-null, optional coercion.

- **Decorator ergonomics**
  - Support `@gm.field(...)` with arguments on methods (e.g., description).
  - Confirm `@gm.field` works on `@classmethod` and `@staticmethod`.
  - Tests: decorator arg usage, classmethod resolver.

## 3) Execution & Error Handling
- **Error propagation**
  - Preserve path, locations, and extensions from async-graphql to Python output.
  - Convert Python exceptions into GraphQL errors with stack traces behind a flag.
  - Tests: resolver error includes path and location.

- **Result shape consistency**
  - Ensure `data` is `None` on errors per spec.
  - Normalize response into a stable Python dict schema.
  - Tests: errors only, partial data, extensions.

## 4) Rust/Python Interop & Performance
- **Async runtime integration**
  - Make runtime init explicit and configurable (tokio runtime builder options).
  - Support running inside existing event loops without deadlocks.
  - Tests: nested event loop use.

- **Resolver call overhead**
  - Cache Python callable lookups per field.
  - Avoid repeated coercion work when not needed.
  - Benchmarks: simple query throughput.

- **Huge result size handling**
  - Benchmark: A single field that returns a large list (~100k items) of other types that each have 2 fields.

## 5) Test Coverage Expansion
- **Subscription tests** (done)
  - [x] Async iterator yields and completes.
  - [x] Cancellation semantics.
  - [x] Backpressure/sequential consumption.

- **Schema generation snapshots**
  - Snapshot SDL for a representative schema.

- **Edge cases**
  - Recursive types, input object defaults, list-of-inputs, nested optionals.

- **Maximize test coverage**
  - Use pytest-cov to measure test coverage.
  - Get to 100% coverage of all features and functionality.

## 5) Packaging & Developer Experience
- **Docs + examples**
  - Add subscription example, custom scalar example, and schema overview.
  - Provide quickstart + advanced resolver patterns.

- **CI**
  - Add Linux + macOS matrix with Python 3.13 + maturin.
  - GitHub Actions.
  - Run tests, build wheels, lint.
