# Rust Complexity/Cognitive-Load Refactor Plan (README-Aligned, Cross-Boundary)

## Summary
- Reduce Rust footprint by removing dead code paths and collapsing repeated resolver/value conversion logic.
- Keep documented public behavior (`README.md`, `grommet/_core.pyi` public surface) stable.
- Apply chosen defaults:
1. `README`-only behavioral contract (tighten undocumented permissive paths).
2. Delete dead Rust modules now.
3. Include Python→Rust internal contract simplification for resolver dispatch.

## Current Findings (Grounded)
1. `src/build.rs` and `src/parse.rs` are not compiled from `src/lib.rs` and are stale (reference internal types that no longer exist), adding major cognitive noise.
2. `src/schema_types.rs` duplicates constructor logic across `PyField`/`PySubscriptionField` and uses placeholder `mem::replace` patterns for ownership transfer.
3. Resolver dispatch in `src/resolver.rs` still carries shape/arg-name orchestration at runtime, creating branch-heavy paths.
4. Value conversion in `src/values.rs` still accepts undocumented forms (tuple-as-list, broad scalar extraction fallback), increasing branches and ambiguity.
5. `resolve_context_cls()` comment says cached but implementation is not cached in `src/schema_types.rs`.

## Public APIs and Interface Changes
- Public API: no documented user-facing changes (`grommet.Schema`, decorators, execution semantics).
- Internal interface changes (intentional):
1. Remove unused legacy modules: `src/build.rs`, `src/parse.rs`.
2. Change internal resolver payload from “shape + arg_names dispatch” to “uniform callable adapter + needs_context flag”.
3. Update `_core.Field` / `_core.SubscriptionField` internal constructor contract accordingly (and update `grommet/_core.pyi` internal signatures to match).
4. Keep `CompiledResolverField.shape`/`arg_names` in Python metadata only if needed for tests/introspection; Rust no longer depends on them.

## Implementation Plan

## Phase 1: Remove Dead Runtime Paths
1. Delete `src/build.rs`.
2. Delete `src/parse.rs`.
3. Confirm no runtime imports/usages remain from these modules.
4. Keep historical plan/research docs as historical; do not reintroduce dead runtime references.

## Phase 2: Cross-Boundary Resolver Contract Simplification
1. In `grommet/_resolver_compiler.py`, compile a uniform resolver adapter callable with one call contract:
   - `(parent_obj, context_obj_or_none, kwargs_dict) -> result`.
2. Adapter applies coercers and shape-specific argument mapping in Python, not Rust.
3. In `grommet/_compiled.py`, instantiate `_core.Field` / `_core.SubscriptionField` with:
   - adapter func,
   - `needs_context`,
   - `is_async` / `is_async_gen` metadata as required.
4. In `src/types.rs`:
   - remove `ResolverShape`,
   - reduce `ResolverEntry` to minimal runtime needs (`func`, `needs_context`, `is_async_gen`).
5. In `src/resolver.rs`:
   - remove shape `match` dispatch path,
   - remove `arg_names`-based kwargs building,
   - build kwargs from `ctx.args.iter()` directly,
   - keep one resolver call path for sync/async with only async-gen split retained.

## Phase 3: Simplify Schema Wrapper Ownership Patterns
1. In `src/schema_types.rs`, replace placeholder `mem::replace` sentinels with `Option<T>` + `take()` ownership model for wrapper internals.
2. Add one shared helper for “take inner value or raise clear internal misuse error”.
3. Add type alias for argument tuple shape to remove `clippy::type_complexity` warnings.
4. Cache context class with `OnceLock` in `resolve_context_cls()` (or remove caching claim if intentionally uncached; default is to cache).

## Phase 4: Value Conversion Tightening (README-Only)
1. In `src/values.rs`, enforce documented list semantics:
   - accept Python `list` for GraphQL lists,
   - drop tuple-as-list support (undocumented).
2. Keep supported input domain explicit: `None | bool | int | float | str | list | dict | grommet input dataclass`.
3. For resolver output conversion (`py_to_field_value_for_type`):
   - scalar named types: strict scalar conversion,
   - non-scalar named types: direct `FieldValue::owned_any` (no broad coercion attempts).
4. Remove dead helpers (`convert_sequence_to_field_values_untyped`), and keep `value_to_py` only where runtime-required (otherwise test-only).
5. Keep `value_to_py_bound` as canonical conversion API.

## Phase 5: Registration/Error-Path Cleanup
1. In `src/schema_types.rs::register_schema`, replace builder swap pattern with ownership-preserving fold.
2. In `src/api.rs::SchemaWrapper::new`, fail fast on unknown type entries instead of silent ignore.

## Test Cases and Scenarios

### Preserve documented behavior
1. All existing Python E2E tests pass:
   - `basic_query`, `resolvers`, `mutations`, `subscriptions`, `context_state`, `lookahead`, `hidden_fields`, `descriptions`, resolver-analysis tests.
2. Existing runtime expectations for `OperationResult` shape remain unchanged.

### New/updated regression tests
1. Resolver trampoline parity:
   - cover all resolver call modes (self-only, with context, with args, with context+args) via adapter path.
2. Runtime no longer depends on shape/arg_names in Rust:
   - monkeypatch/inspect compiled payloads to confirm Rust executes through adapter only.
3. Unknown registrable type in schema bundle raises clear `TypeError`.
4. Tuple passed where GraphQL list expected now fails with clear error.
5. Wrapper double-consume path fails fast with explicit error (instead of placeholder behavior).
6. Context class cache path exercised once and reused.

### Validation commands
1. `timeout 300s uv run pytest tests/python -q`
2. `timeout 300s cargo test -q`
3. `timeout 300s uv run mypy .`
4. `timeout 300s prek run -a`

## Assumptions and Defaults
1. Contract baseline is `README.md` + public Python API; undocumented permissive behaviors may be tightened.
2. `_core` constructor signatures are internal and can change with synchronized Python/Rust updates.
3. Direct external use of stale/dead Rust internals is out of scope.
4. PyO3 trait-implementation strategy for `async-graphql` foreign types remains out of scope for this iteration.
