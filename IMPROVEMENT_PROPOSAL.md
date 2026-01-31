# Improvement Proposal

This document captures a set of proposed changes to tighten the public API, simplify runtime behavior, and improve typing ergonomics. Each section is a self-contained item with rationale, scope, and implementation notes.

## 1) Require explicit dataclasses

### Rationale
Decorators currently auto-apply `@dataclass` to non-dataclass classes. This hides implicit behavior and can surprise users (e.g., field defaults, init signature, repr).

### Scope
- `@gm.type`, `@gm.interface`, and `@gm.input` should **require** the target class to already be a dataclass.
- Remove auto-dataclass conversion and the “re-dataclass if resolvers were applied” pathway.

### Implementation Notes
- If a class is not a dataclass, raise a clear `TypeError` explaining that `@dataclass` is required.
- Preserve existing behavior for already-dataclass classes.

### Impact
- More explicit and predictable class behavior.
- Slightly stricter API but clearer for users and type checkers.

## 2) Remove `types` and `scalars` parameters from `Schema`

### Rationale
`Schema(types=..., scalars=...)` adds a second path for registering types. Automatic discovery already walks the type graph from entrypoints and resolver annotations, so the extra parameters are redundant and add complexity.

### Scope
- Remove `types` and `scalars` arguments from the Python `Schema` constructor.
- Collect types and scalars exclusively via graph traversal from `query`/`mutation`/`subscription` and resolver annotations.

### Implementation Notes
- Update `Schema.__init__` signature and any call sites.
- Update documentation and tests that pass explicit `types`/`scalars`.
- Ensure the collector still picks up enums/unions and custom scalars referenced in annotations.

### Impact
- Simpler API and fewer edge cases.
- Slightly stricter usage: users must reference all types in the graph rather than pass explicit lists.

## 3) Remove support for `debug`

### Rationale
`Schema(debug=True)` toggles Python tracebacks in GraphQL error extensions. This adds a mode that’s hard to reason about across environments and can leak sensitive data in production.

### Scope
- Remove the `debug` parameter from `Schema`.
- Remove any debug-specific behavior from the Rust layer.

### Implementation Notes
- Strip the `debug` flag from `Schema.__init__` and `_core.Schema` construction.
- Always omit Python tracebacks from extensions (or move to a separate, explicit mechanism later).

### Impact
- Simpler API surface.
- Reduced risk of accidental sensitive error leakage.

## 4) Add docstrings to everything exposed in `grommet.__init__`

### Rationale
Public API objects should be self-documenting. This also improves IDE hover help and aligns with the project’s docstring guidance.

### Scope
Add concise docstrings for the following public exports:
- `Schema`
- `field`, `type`, `input`, `interface`, `scalar`, `union`, `enum`
- `Info`, `ID`, `Internal`, `Private`, `configure_runtime`

### Implementation Notes
- Use one-line docstrings when possible.
- Avoid implementation details; keep them descriptive and short.

### Impact
- Better developer experience and discoverability.

## 5) Improve decorator typing for `__grommet_meta__`

### Rationale
Static typing doesn’t currently “know” that decorated classes carry grommet metadata. We can make this explicit for type checkers without changing runtime semantics.

### Scope
- Add typing constructs so that `@gm.type`, `@gm.scalar`, etc. return a class type that includes `__grommet_meta__`.

### Implementation Notes
- Introduce a `Protocol` (e.g., `GrommetAnnotated`) with a `__grommet_meta__` attribute.
- Use `TypeVar` bounds and `TypeGuard` helpers so mypy/pyright recognize the metadata.
- Maintain runtime behavior (still set `__grommet_meta__` dynamically).

### Impact
- Better static typing and fewer casts in downstream code.
- No behavioral change at runtime.

## 6) Centralize annotation normalization and coercion (Python)

### Rationale
Annotation handling is spread across multiple helpers (`_unwrap_annotated`, `_split_optional`, `_unwrap_async_iterable`, `_iter_*_refs`, `_type_spec_from_annotation`, `_coerce_value`). This makes behavior hard to audit and evolve.

### Scope
- Create a single `AnnotationInfo` (dataclass or lightweight struct) that captures:
  - base type, optionality, list/iterable wrappers, internal/private markers, and metadata
- Use that in all annotation-dependent paths.

### Implementation Notes
- Move annotation parsing to a dedicated module (e.g., `annotations.py`).
- Ensure `Info`/resolver argument coercion and schema type conversion both use the same normalization logic.

### Impact
- Fewer edge-case mismatches between coercion and schema generation.
- Easier to reason about nullability and container semantics.

## 7) Consolidate schema graph traversal (Python)

### Rationale
`_collect_types`, `_collect_scalars`, `_collect_enums`, and `_collect_unions` duplicate traversal logic with only small variations. This increases maintenance overhead and makes it easier to introduce inconsistencies when adding new type kinds.

### Scope
- Introduce a single traversal function that walks entrypoints and resolver annotations once.
- Provide callbacks or filters for what to collect (types, scalars, enums, unions).

### Implementation Notes
- Centralize internal-field skipping and annotation iteration.
- Return a structured traversal result object to reduce ad hoc loops.

### Impact
- Less duplicated logic and fewer subtle divergences.
- Easier to add new kinds (e.g., directives or future type categories).

## 8) Split `schema.py` into focused modules

### Rationale
`schema.py` is the largest Python file and mixes collection, schema assembly, coercion, and resolver wrapping. This makes navigation and edits harder.

### Scope
- Extract into modules such as:
  - `registry.py` (graph traversal + registries)
  - `annotations.py` (annotation parsing helpers)
  - `coercion.py` (input/output coercion helpers)
  - `resolver.py` (resolver wrapping and argument handling)
  - `schema.py` (public `Schema` API and orchestration)

### Implementation Notes
- Preserve public API imports via `grommet/__init__.py`.
- Keep module boundaries small and single-purpose.

### Impact
- Smaller, more focused files.
- Easier targeted changes without cross-cutting edits.

## 9) Standardize error types and messaging

### Rationale
Errors are currently created in multiple places with ad hoc strings. This makes behavior inconsistent and harder to test and document.

### Scope
- Introduce a dedicated error module (Python and Rust) with canonical error constructors and message templates.
- Ensure user-facing errors remain stable and descriptive.

### Implementation Notes
- Provide error classes in Python (e.g., `GrommetSchemaError`, `GrommetTypeError`).
- Mirror structured errors in Rust and map them consistently.

### Impact
- Predictable error messages for users.
- Easier to add tests for error cases and maintain behavior over time.

## 10) Rust safety policy: forbid unsafe code

### Rationale
The current Rust code uses `unsafe` (notably `Send`/`Sync` impls for Python objects). This makes the concurrency model harder to audit and reason about. Enforcing `#![forbid(unsafe_code)]` creates a clear maintenance contract and reduces long-term risk. The async-graphql crate itself uses `#![forbid(unsafe_code)]`, so aligning with that policy is consistent with upstream practice. citeturn1search7

### Scope
- Add `#![forbid(unsafe_code)]` at the crate root.
- Remove all `unsafe` blocks and `unsafe impl` declarations.

### Implementation Notes
- Replace `unsafe impl Send/Sync` for `PyObj` with safe alternatives:
  - avoid moving Python objects across threads, or
  - use `Py<PyAny>` behind thread-safe queues with explicit GIL-bound access patterns, or
  - refactor runtime boundaries so Python values never cross `Send` requirements.
- Revisit any async boundaries that require `Send` (e.g., streams/futures) and use `spawn_local` or non-`Send` futures where appropriate.
- async-graphql request/schema data storage requires `Any + Send + Sync`, so the refactor must ensure context/root storage remains valid without `unsafe` (e.g., isolate Python objects to non-`Send` paths or replace with IDs). citeturn3search1turn3search2

### Impact
- Stronger safety guarantees and easier auditing.
- Potential refactors to avoid `Send` requirements in the Rust/PyO3 bridge.

## 11) Rust resolver pipeline refactor

### Rationale
`build.rs` contains long, repeated resolver logic paths (field vs subscription) with similar GIL/await/error handling. This raises risk of inconsistencies and makes refactors expensive.

### Scope
- Extract shared resolver logic into a dedicated helper module.
- Standardize error conversion and GIL boundary usage.

### Implementation Notes
- Align GIL acquisition with current PyO3 guidance (`Python::attach` or a `Python<'py>` token from `#[pymethods]`/`#[pyfunction]`) and avoid awaiting while holding the GIL. PyO3 documents `Python::with_gil` as deprecated in favor of `Python::attach`. citeturn2search6turn1search6
- Introduce a shared “resolve callable” function that handles:
  - argument construction
  - awaitable detection
  - error mapping
  - conversion to `FieldValue`
- Add clear comments around the `unsafe Send/Sync` usage and evaluate whether a safer wrapper or a narrower `unsafe` boundary is possible.

### Impact
- Less duplication between field and subscription resolution.
- Clearer control flow and safer future edits.

## 12) Direct async-graphql interop (no JSON round-trip)

### Rationale
We currently serialize Python values to JSON and then feed async-graphql `Variables::from_json`, and likewise serialize error extensions to JSON before converting to Python. This adds overhead and loses some type fidelity.

### Scope
- Replace `serde_json` round-trips with direct conversion to/from async-graphql native values and error extension structures.
- Keep the public Python API identical.

### Implementation Notes
- Align the Rust implementation with the current async-graphql source and dynamic-schema APIs before making interop changes. The `async_graphql::dynamic` module is gated by the `dynamic-schema` feature and provides types like `DynamicRequest` that may be required for variable handling. citeturn1search1turn1search8
- Convert Python values directly into async-graphql native values without JSON, using the conversion points exposed by the current async-graphql version (validate exact API names in source/docs).
- Convert error extensions using async-graphql’s native extension/value structures without serializing to JSON.
- Avoid naming or relying on APIs until verified in upstream source; document any required feature flags (e.g., dynamic schema).

### Impact
- Faster variable parsing for large inputs.
- Fewer allocations and lower CPU usage.
- Less JSON-specific normalization.
