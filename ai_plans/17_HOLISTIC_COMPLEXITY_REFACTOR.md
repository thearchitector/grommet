# Codebase-Wide Complexity Reduction Plan: Data-Only Python/Rust Boundary (One-Shot)

## Summary
This plan performs an aggressive one-shot simplification with `README.md` as the only compatibility contract.
It removes the internal `_core` constructor object graph (`_core.Field`, `_core.Object`, etc.) and replaces it with a data-only compiled metadata payload consumed directly by Rust.
Primary goals are to cut footprint and cognitive load while preserving current documented behavior and runtime performance.

Current measured baseline on this machine:
1. `uv run python benchmarks/bench_large.py` → `grommet: 4.6858s` (500k cells), `strawberry: 17.0573s`
2. `uv run cargo run --release --manifest-path benchmarks/rust_control/Cargo.toml` → `async-graphql: 1.3683s`
3. `uv run pytest` and `uv run cargo test` are green

---

## Key Findings (Complexity Hotspots + Opportunities)
1. Python hotspots are concentrated in `grommet/_type_compiler.py` (`compile_type_definition`) and `grommet/_resolver_compiler.py` (`compile_resolver_field`), with unnecessary runtime-facing metadata (`shape`, `arg_names`) retained only for tests.
2. Rust footprint is dominated by `src/schema_types.rs` wrapper classes that exist only to shuttle Python metadata back into Rust dynamic schema objects.
3. Current boundary does redundant object construction:
   - Python: compiled dataclasses → `_core.*` wrapper instances
   - Rust: consume wrapper instances → real async-graphql types
4. Undocumented `_core` constructor behavior is exercised only by internal tests, not by documented API.
5. The highest-leverage simplification is eliminating wrapper-object registration and consuming compiled metadata directly.

---

## Public API / Interface Changes

### Public documented API (`README.md`)
1. No changes to `grommet.Schema`, decorators, `Context`, `Field` metadata annotation, or `Hidden`.
2. No behavioral changes to query/mutation/subscription execution semantics.

### Internal and undocumented interfaces (intentional breaking changes)
1. Remove internal `_core` constructor classes as supported inputs for schema registration:
   - `_core.Field`, `_core.SubscriptionField`, `_core.InputValue`, `_core.Object`, `_core.InputObject`, `_core.Subscription`
2. `grommet/_core.pyi` will expose only runtime classes actually intended for use:
   - `Schema`, `OperationResult`, `SubscriptionStream`, `Graph`
3. `grommet._compiled.CompiledResolverField` removes `shape` and `arg_names`.
4. Compiled default representation changes from sentinel-only semantics to explicit default-presence flags:
   - `has_default: bool`
   - `default: Any | None`

---

## Implementation Plan (Decision-Complete)

## Phase 1: Normalize Python Compiled Metadata for Direct Rust Consumption
1. Update `grommet/_compiled.py`:
   - Remove `_core` imports and delete `instantiate_core_type()` and `_core_args()`.
   - Keep only immutable metadata dataclasses and attribute-name constants.
   - Change default-bearing dataclasses (`CompiledArg`, `CompiledInputField`, `CompiledDataField`) to explicit `has_default` + `default`.
   - Remove `shape` and `arg_names` from `CompiledResolverField`.
2. Update `grommet/_resolver_compiler.py`:
   - Remove `_SHAPE_BY_RESOLVER_SIGNATURE`.
   - Keep `_resolver_adapter()` but retain `arg_names` only as local adapter closure input.
   - Emit `CompiledResolverField` without `shape`/`arg_names`.
   - Keep sync-demotion via `noaio` unchanged (performance safeguard).
3. Update `grommet/_type_compiler.py`:
   - Refactor `compile_type_definition()` into helpers:
     - `_compile_input_fields(...)`
     - `_compile_object_fields(...)`
     - `_compile_subscription_fields(...)`
   - Build compiled field/default payloads using new `has_default` contract.
   - Keep current validation rules unchanged.
4. Update `grommet/plan.py`:
   - `SchemaBundle.types` becomes list of `CompiledType` objects directly.
   - Remove per-schema instantiation of `_core.*` wrappers.
   - Keep root-default validation and type-graph traversal behavior unchanged.

## Phase 2: Replace Rust Wrapper-Class Registration with Metadata Decoder
1. Rewrite `src/schema_types.rs`:
   - Remove all PyO3 `#[pyclass]` wrapper constructors (`PyField`, `PyObject`, etc.).
   - Replace with pure decode/build helpers that read `CompiledType` Python objects and construct async-graphql dynamic types directly.
   - Keep `type_spec_to_type_ref()` as canonical recursive converter.
   - Decode fields by kind from `CompiledType` collections:
     - object fields (data + resolver)
     - input fields
     - subscription fields
   - For defaults, use `has_default` flag instead of sentinel identity checks.
2. Update `src/api.rs` (`SchemaWrapper::new`):
   - Stop downcasting to wrapper classes.
   - Accept raw compiled type objects from `bundle.types`.
   - Call new schema registration decoder path.
3. Update `src/lib.rs`:
   - Remove `module.add_class` registrations for deleted wrapper classes.
4. Keep `src/resolver.rs`, `src/values.rs`, `src/lookahead.rs` behavior stable except for minor signature updates required by the new registration pipeline.

## Phase 3: Type Stubs and Internal Test Realignment
1. Update `grommet/_core.pyi`:
   - Remove constructor class stubs no longer present.
   - Retain `Schema`, `OperationResult`, `SubscriptionStream`, `Graph`.
2. Update `tests/python/test_resolver_analysis.py`:
   - Remove assertions tied to removed internal metadata (`shape`, `arg_names`).
   - Remove `_core.Field/_core.Object` consumption test.
   - Add replacement coverage:
     - schema rejects invalid compiled payload object in `bundle.types`
     - runtime still executes all resolver forms correctly through adapter path.
3. Update Rust integration tests (`tests/test_core.rs`) only where references to removed wrappers appear.

## Phase 4: Final Complexity Sweep (No Behavior Changes)
1. Remove now-dead code paths/constants/imports in:
   - `grommet/decorators.py`
   - `grommet/_compiled.py`
   - `src/schema_types.rs`
2. Keep file/module boundaries aligned to responsibility:
   - Python compilers produce metadata
   - Rust builds schema/execution from metadata
3. Do not modify `pragma: no ai` files (`README.md`, `grommet/schema.py`, `grommet/context.py`, `grommet/_annotations.py`).

---

## Test Cases and Validation Scenarios

### Functional regression (must pass)
1. Full Python tests:
   - `timeout 300s uv run pytest`
2. Full Rust tests:
   - `timeout 300s uv run cargo test`
3. Typing and lint gates:
   - `timeout 300s uv run mypy .`
   - `timeout 300s prek run -a`

### New/updated targeted tests
1. Resolver metadata no longer exposes `shape`/`arg_names`.
2. Schema creation works from compiled metadata-only bundle (no `_core` builder objects).
3. Invalid bundle entries produce clear `TypeError`.
4. Existing resolver modes still pass:
   - self-only
   - self+context
   - self+args
   - self+context+args
5. Input default handling correctness under new `has_default` payload.

### Performance acceptance
1. Re-run `timeout 180s uv run python benchmarks/bench_large.py` 3 times.
2. Acceptance threshold:
   - median `grommet` runtime must be <= baseline + 5% (baseline currently `4.6858s`).
3. Stretch target:
   - median `grommet` runtime <= `4.3s`.

---

## Assumptions and Defaults
1. Compatibility target is strictly documented `README` API only.
2. Internal `_core` constructor classes are treated as unsupported and removable.
3. One-shot cutover is required (no compatibility bridge path).
4. Performance may not regress; sync fast-path and coroutine demotion stay in place.
5. No changes are made to `pragma: no ai` files.
