# Python Refactor Plan: Compile-Once Decorators, Register-Only Schema Build

## Summary
- Refactor the Python side so all annotation/signature/dataclass analysis happens at decoration time.
- Keep schema construction (`Schema -> plan.py`) as a thin graph walk + registration path with no reflection/introspection work.
- Preserve documented public API behavior and ignore Rust-side changes.
- Remove dead/duplicated internals and split `decorators.py` by concern.

## Public APIs and Interfaces
- Public API kept stable: `grommet.type`, `grommet.input`, `grommet.field`, `grommet.subscription`, `grommet.Schema`, `grommet.Context`, `grommet.Field`, `grommet.Hidden`.
- No changes to `grommet/schema.py`, `grommet/context.py`, `grommet/_annotations.py` (`pragma: no ai`).
- Internal interfaces will change:
  - Remove tuple-based resolver caches (`__grommet_field_data__`, `__grommet_sub_field_data__`).
  - Remove dead `grommet/resolver.py`.
  - Add compiled blueprint attrs for class/function internals.

## Target Internal Design

### 1) New internal compiled model (`grommet/_compiled.py`)
Define frozen dataclasses used as the only schema-build input:
- `CompiledArg`: `name`, `type_spec`, `default`.
- `CompiledResolverField`: `kind` (`field|subscription`), `name`, `func`, `shape`, `arg_names`, `is_async`, `type_spec`, `description`, `args`, `refs`.
- `CompiledDataField`: `name`, `type_spec`, `description`, `default`, `resolver_func`, `refs`.
- `CompiledInputField`: `name`, `type_spec`, `description`, `default`, `refs`.
- `CompiledType`: `meta` (`TypeMeta`), `object_fields`, `subscription_fields`, `input_fields`, `refs`.
- `instantiate_core_type(compiled_type)` helper that builds fresh `_core.Object` / `_core.InputObject` / `_core.Subscription` from the compiled dataclasses only.

### 2) Resolver compiler (`grommet/_resolver_compiler.py`)
Move and unify resolver analysis currently duplicated in `decorators.py` and `resolver.py`:
- Single analysis pipeline for field/subscription resolvers.
- Responsibilities:
  - signature parsing and arg extraction,
  - context-param detection,
  - arg type/default planning via `TypeSpec`,
  - input coercer wrapping (`_arg_coercer`),
  - sync demotion of await-free coroutine resolvers (`noaio`) for regular fields,
  - return type compilation (including async-iterable unwrapping for subscriptions),
  - referenced-type extraction.
- Output only `CompiledResolverField`.

### 3) Type compiler (`grommet/_type_compiler.py`)
Compile class definitions at `@type` / `@input` time:
- Read class annotations once.
- Compile dataclass fields into `CompiledDataField` or `CompiledInputField`.
- Collect resolver blueprints from decorated methods.
- Enforce invariants at compile time:
  - input types cannot have resolvers,
  - object/subscription types cannot mix resolver kinds,
  - subscription types cannot have visible dataclass data fields.
- Compute and store `refs` once.
- Store compiled result on class: `__grommet_compiled_type__`.
- Keep `__grommet_meta__` and `__grommet_refs__` for compatibility with existing internal helpers.

### 4) Thin decorators (`grommet/decorators.py`)
Reduce `decorators.py` to public entrypoints and wiring:
- `@field` calls resolver compiler and attaches `__grommet_compiled_resolver__`.
- `@subscription` calls resolver compiler (subscription mode) and attaches `__grommet_compiled_resolver__`.
- `@type` / `@input` call type compiler and set class metadata.
- Keep overload signatures and existing error types/messages where practical.
- Delete old local analysis/build helpers from this file.

### 5) Register-only schema build (`grommet/plan.py`)
Rewrite `build_schema_graph()` to do only:
- root metadata lookup,
- root default validation (chosen policy: fail fast here),
- BFS/DFS over precomputed refs,
- instantiate each collected `CompiledType` via `instantiate_core_type()`,
- return `SchemaBundle(query, mutation, subscription, types)`.

Prohibited in `plan.py` after refactor:
- `inspect.signature`,
- `get_annotations`,
- `dataclasses.fields`,
- `_type_spec_from_annotation`,
- resolver coercion/syncification logic.

### 6) Dead code and dependency cleanup
- Remove `grommet/resolver.py`.
- Remove all imports/callers tied to removed tuple caches.
- Keep `annotations.py` and `coercion.py` focused and reusable by compilers.
- Keep `metadata.py` as home for `TypeMeta`, `TypeKind`, `TypeSpec`, `ArgPlan`, `Field`, `Hidden`, constants.

## File-by-File Change Plan
1. Add `grommet/_compiled.py`.
2. Add `grommet/_resolver_compiler.py`.
3. Add `grommet/_type_compiler.py`.
4. Refactor `grommet/decorators.py` into thin wrappers.
5. Refactor `grommet/plan.py` to register-only flow.
6. Delete `grommet/resolver.py`.
7. Update any internal imports in `grommet/__init__.py` only if needed (public exports unchanged).

## Test Cases and Scenarios

### Preserve existing behavior
- Keep all current Python end-to-end tests green (`basic_query`, `resolvers`, `mutations`, `subscriptions`, `context_state`, `lookahead`, `hidden_fields`, `descriptions`).

### Update internal-focused tests
- Rewrite `tests/python/test_resolver_analysis.py` to validate compiled-blueprint behavior instead of tuple internals.
- Add assertions around compiled attrs (`__grommet_compiled_resolver__`, `__grommet_compiled_type__`) only where needed.

### Add new regression tests
- Schema build does not re-run resolver inspection:
  - decorate type, then monkeypatch resolver-inspection path (`inspect.signature`) to raise, then build schema successfully.
- Schema build does not re-run annotation compilation:
  - decorate type, then monkeypatch `_type_spec_from_annotation` to raise, then build schema successfully.
- Same decorated types can back multiple schema instances.
- Root data field without default fails fast in `Schema(...)` with clear `GrommetTypeError`.
- Invalid mixes still fail:
  - input type with resolver method,
  - type mixing `@field` and `@subscription`,
  - subscription type with visible dataclass data fields.

### Verification commands
- `timeout 300s uv run pytest tests/python`
- `timeout 300s uv run mypy .`
- `timeout 300s uv run cargo test`
- `timeout 300s prek run -a`

## Execution Sequence
1. Introduce compiled dataclasses + instantiation helpers.
2. Migrate resolver compilation into dedicated module; switch `@field`/`@subscription`.
3. Migrate class compilation into dedicated module; switch `@type`/`@input`.
4. Rewrite `plan.py` to register-only using compiled blueprints.
5. Remove dead legacy internals (`resolver.py`, tuple cache paths).
6. Update/add tests and run full validation gates.

## Assumptions and Defaults
- Compatibility target is public API only; internal attributes and private helper signatures may change.
- Schema construction must be register-only (no analysis/recomputation at build time).
- Root default policy is fail-fast in `build_schema_graph`.
- Rust behavior/contracts are treated as fixed for this refactor.
- Module strategy is split-by-concern rather than keeping a monolithic `decorators.py`.
