# Plan: Context Marker Migration + Lookahead Removal (Order-Independent Context Injection)

> i updated the public api to remove the entire Context and lookahead functionality. context is now supplied directly into `schema.execute`, and it is then given to all field resolvers that request it via any argument annotated with `grommet.Context`. the order of the arguments does not matter.

## Summary
Migrate resolver context handling to the new public contract and remove deprecated context/lookahead infrastructure:

1. `schema.execute(..., context=...)` is the only supported context input path (`state=` is removed).
2. Resolver context injection is based on `Annotated[T, grommet.Context]` only.
3. Context-injected parameters can appear in any position after `self`.
4. Multiple context-annotated parameters are all injected with the same execute-time context value.
5. Lookahead (`context.graph`, `Graph`, `peek`, `requests`) is fully removed.

The current repo is in a transitional broken state (notably `grommet/context.py` deleted while imports remain), so phase 1 restores importability before behavior work.

## Locked Decisions
1. Context annotation form: **Annotated-only** (`Annotated[T, grommet.Context]`).
2. Backward compatibility for execute kwargs: **hard break now** (`state=` removed).
3. Multiple context params: **inject all** with the same value.
4. Missing execute-time context: **inject `None`**.

## Public API / Behavior Changes
1. `grommet.Context` remains publicly available as a marker object, imported from `grommet`.
2. Context parameters must be declared as `Annotated[T, grommet.Context]`.
3. `Schema.execute` only accepts `context=...` (plus `query`, `variables`).
4. Any use of lookahead APIs is unsupported and removed from runtime and stubs.
5. Resolver parameter ordering rule:
   - Any parameter marked with `grommet.Context` gets injected.
   - Non-context parameters remain GraphQL args.
   - Ordering between context and GraphQL args does not matter.

## Implementation Plan

### 1. Repair imports and marker source of truth
1. Update `grommet/__init__.py` to export `Context` from `grommet.metadata` instead of `grommet.context`.
2. Replace imports of `.context.Context` with `.metadata.Context` in:
   - `grommet/annotations.py`
   - `grommet/_resolver_compiler.py`
3. Remove remaining references to `grommet.context` module paths across Python and Rust.

### 2. Annotation semantics: Annotated-only context detection
1. In `grommet/annotations.py`, change context detection to metadata-marker semantics:
   - `is_context` is true only when `Context` appears in `Annotated` metadata.
2. Add explicit validation for bare marker misuse in resolver compilation:
   - If parameter annotation is exactly `grommet.Context` (not `Annotated[...]`), raise a targeted `GrommetTypeError` explaining required syntax.
3. Keep `_type_spec_from_annotation` behavior that context-marked params are non-GraphQL params and never become schema args.

### 3. Resolver compiler: order-independent context param extraction
1. Refactor `compile_resolver_field` in `grommet/_resolver_compiler.py`:
   - Parse all parameters after `self`.
   - Split into:
     - `context_param_names` (all params marked with context metadata)
     - `graphql_arg_params` (all others)
2. Build GraphQL arg metadata/coercers only from `graphql_arg_params`.
3. Set `needs_context = bool(context_param_names)`.
4. Update adapter generation:
   - Keep runtime call shape from Rust as `(parent, context, kwargs)`.
   - Inject context value into all `context_param_names`.
   - Merge GraphQL kwargs (coerced) by name.
   - Call resolver as `func(parent, **call_kwargs)` so parameter order is irrelevant.
5. Preserve existing syncification behavior and subscription async-generator validation.

### 4. Remove lookahead/context-class runtime path in Rust
1. Remove lookahead integration entirely:
   - Stop compiling/exporting Graph from `src/lib.rs`.
   - Remove `src/lookahead.rs` module usage from build path.
2. Simplify resolver runtime in `src/resolver.rs`:
   - Remove `extract_graph` usage and context object construction wrapper.
   - Pass raw execute-time context object directly to Python adapter.
3. Simplify resolver metadata structs in `src/types.rs`:
   - Remove `context_cls` storage from `FieldContext`.
4. Simplify schema registration in `src/schema_types.rs`:
   - Remove `resolve_context_cls`.
   - Stop importing `grommet.context.Context`.
   - Keep only `needs_context` boolean-driven runtime behavior.
5. Rename internal request data carrier for clarity:
   - `StateValue` -> `ContextValue` in Rust API/resolver path (`src/types.rs`, `src/api.rs`, `src/resolver.rs`).

### 5. Execute API hard break (`state` removal)
1. In `src/api.rs`, rename third `execute` argument and request builder parameter from `state` to `context`.
2. In `grommet/_core.pyi`, update method signature to `context: Any = None`.
3. In `grommet/schema.py`, keep `context` public kwarg and align docstring text with marker-based injection.
4. Do not support aliasing `state=` anywhere in Python surface.

### 6. Tests update and replacement
1. Remove lookahead tests:
   - Delete `tests/python/test_lookahead.py`.
2. Update context behavior tests:
   - `tests/python/test_context_state.py`:
     - Use `Annotated[MyState, grommet.Context]`.
     - Pass `context=MyState(...)`.
     - Assert direct access (no `.state`, no `.graph`).
3. Extend resolver analysis tests in `tests/python/test_resolver_analysis.py`:
   - Context param in non-second position.
   - Mixed ordering of context and GraphQL args.
   - Multiple context params receive same object.
   - Bare `grommet.Context` annotation raises explicit error.
4. Add hard-break test:
   - Calling `schema.execute(..., state=...)` raises `TypeError`.
5. Subscription coverage:
   - Add or extend test to verify context injection works for subscription resolver signatures using Annotated marker.
6. Rust-side cleanup tests:
   - Remove lookahead include/use in `tests/test_core.rs`.
   - Keep resolver bridge tests intact; adjust any type imports if renamed to `ContextValue`.

## Validation / Acceptance
Run and require green:
1. `timeout 600s uv run mypy .`
2. `timeout 1200s uv run pytest`
3. `timeout 1200s uv run cargo test`
4. `timeout 1200s prek run -a`

Behavior acceptance checks:
1. `import grommet` succeeds (no `grommet.context` import path required).
2. Context injection works with `Annotated[T, grommet.Context]` regardless of parameter order.
3. Multiple context-annotated resolver params receive identical object identity.
4. Missing `context=` injects `None`.
5. `state=` is rejected.
6. No exposed lookahead API in runtime/stubs (`Graph` removed from `_core` surface).

## Assumptions and Defaults
1. `grommet.Context` is a marker object, not a runtime context wrapper type.
2. README is not modified by this implementation plan due `pragma: no ai`; user-maintained docs remain authoritative.
3. Resolver context injection applies to sync fields, async fields, and subscriptions uniformly.
4. Resolver args are keyword-dispatched in adapter path; positional-only resolver params are not part of supported resolver signature conventions.
