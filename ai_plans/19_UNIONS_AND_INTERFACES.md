# Implement README-Documented Union + Interface API (Named/Automatic Unions, Interface Types)

> i update the API as documented in the readme. key changes
> - added support for named, or automatic, unions
> - added support for interfaces

## Summary
Implement the README-described API additions end-to-end without modifying `pragma: no ai` files, by extending Python annotation/type compilation, schema planning, Rust type registration, and runtime value conversion. The implementation will support:
1. Named unions via `Annotated[T1 | T2, grommet.Union(...)]`.
2. Automatic union naming for plain unions.
3. `@grommet.interface` types with automatic implementer discovery.
4. Interface method fields with inherited resolver behavior on implementers.

## Public API Changes
1. Add `grommet.interface` decorator in `grommet/decorators.py`.
2. Add `grommet.Union` metadata class in `grommet/metadata.py` and export it in `grommet/__init__.py`.
3. Extend grommet type kinds to include interface/union registration semantics.
4. Keep existing APIs backward-compatible for `@grommet.type`, `@grommet.input`, `@grommet.field`, and `@grommet.subscription`.

## Implementation Plan

### 1) Extend Python Metadata and Compiled Structures
1. Update `grommet/metadata.py`:
- Add `TypeKind.INTERFACE` and `TypeKind.UNION`.
- Add public `Union` dataclass with `name: str | None` and `description: str | None`.
- Extend `TypeSpec` with union payload fields: `union_members: tuple[str, ...]` and `union_description: str | None`.
2. Update `grommet/_compiled.py`:
- Add `implements: tuple[str, ...]` to `CompiledType`.
- Add `CompiledUnion` dataclass containing `meta: TypeMeta` and `possible_types: tuple[str, ...]`.

### 2) Add Interface Decorator and Public Exports
1. Update `grommet/decorators.py`:
- Add `interface(...)` overloads and implementation, wired to `compile_type_definition(..., kind=TypeKind.INTERFACE, ...)`.
2. Update `grommet/__init__.py`:
- Export `interface` and `Union` in imports and `__all__`.

### 3) Upgrade Annotation Analysis for Type Aliases, Unions, and Interface/Union Refs
1. Update `grommet/annotations.py`:
- Add `TypeAliasType` unwrapping so `type Alias = ...` annotations are resolved before analysis.
- Extend `walk_annotation`/`_walk_inner` to recursively walk union members.
- Extend `_type_spec_from_annotation`:
  - Parse unions in output positions.
  - Reject unions in input positions with a dedicated error.
  - Validate union members are decorated object types.
  - Build `TypeSpec(kind="union", name=..., union_members=..., union_description=..., nullable=...)`.
  - Preserve optionality behavior (`None` in union => nullable union output).
2. Update `grommet/errors.py`:
- Add explicit union-related error helpers (input disallowed, invalid members, conflicting definitions as needed).
- Update `not_grommet_type(...)` messaging to include `@grommet.interface`.

### 4) Compile Interface Types and Interface Inheritance Correctly
1. Update `grommet/_type_compiler.py`:
- Collect compiled resolvers across MRO so implementers inherit interface method fields by default.
- Derive implemented interface names from base classes and store on `CompiledType.implements`.
- Include implemented interface classes in refs.
- Allow interface field signatures from dataclass fields and `@grommet.field` methods.
- Forbid `@grommet.subscription` on interfaces.
2. Keep root default-field validation rules unchanged for query/mutation/subscription roots.

### 5) Plan Builder: Auto-Discover Implementers and Materialize Union Registrations
1. Update `grommet/plan.py`:
- Keep `build_schema_graph(...)` signature unchanged.
- In `_walk_and_collect(...)`, when visiting an interface type, recursively enqueue decorated object subclasses (`__subclasses__()` DFS) to auto-include implementers.
- Collect unions by traversing all compiled `TypeSpec` trees and extracting `kind == "union"`.
- Deduplicate unions by name and fail fast on conflicting definitions (different members/description).
- Add `CompiledUnion` entries into `SchemaBundle.types` alongside `CompiledType`.
- Keep deterministic ordering for generated union registrations.

### 6) Rust Schema Registration: Register Interfaces, Object Implements, and Unions
1. Update `src/schema_types.rs`:
- Add builders for dynamic `Interface` and `Union`.
- Extend object builder to apply `implements` from compiled metadata.
- Extend decode/register flow to handle `kind == "interface"` and `kind == "union"`.
- Keep unsupported payload error path for unknown registrations.

### 7) Rust Runtime Value Conversion: Abstract Type Dispatch
1. Update `src/values.rs`:
- Add helper to inspect Python `__grommet_meta__` kind/name at runtime.
- For named non-scalar outputs:
  - Keep concrete object output behavior unchanged when expected type name equals runtime object type name.
  - When expected type name differs and runtime value is a grommet object, wrap with `.with_type(runtime_object_name)` so interface/union dispatch works.
- Preserve recursive list conversion behavior for list of abstract outputs.

### 8) No-AI File Compliance
1. Do not modify:
- `README.md`
- `grommet/schema.py`
- `grommet/_annotations.py`
- any other file marked `pragma: no ai`.

## Test Cases and Scenarios

1. Add `tests/python/test_unions_interfaces.py`:
- Named union via `type Alias = Annotated[A | B, grommet.Union(name=..., description=...)]` works and appears in SDL.
- Automatic union naming for plain `A | B` works and appears in SDL.
- Interface type appears in SDL; object implementers auto-register and implement interface.
- Interface method field defined on interface resolves via inherited implementation on implementers.
- Union name conflict raises `TypeError` during schema build.
- Union in input position raises `TypeError`.
2. Update existing Python tests where needed for new metadata fields (`implements`) and new exports.
3. Add focused Rust test coverage in `tests/test_core.rs` for abstract output wrapping logic in `py_to_field_value_for_type`.

## Verification Steps
1. `timeout 300s uv run pytest`
2. `timeout 300s uv run cargo test`
3. `timeout 300s uv run mypy .`
4. `timeout 300s prek run -a`

## Assumptions and Defaults
1. Interface methods are supported and implementers may inherit them without overriding.
2. Interface implementers are auto-discovered from decorated object subclasses.
3. Union naming conflicts are hard errors (fail fast).
4. Automatic union names concatenate member GraphQL type names in declared order.
5. Union support is output-only; union arguments/input fields are rejected.
