# Pydantic 2+ as an Alternative to Dataclasses in Grommet

## Executive Summary

Grommet's public API currently requires users to define GraphQL types as stdlib `@dataclass`
classes decorated with `@grommet.type` or `@grommet.input`. Internally, grommet performs manual
introspection of `dataclasses.fields()`, hand-rolls annotation analysis, and implements its own
dict→dataclass coercion for input types. This document evaluates whether Pydantic 2+ (backed by
`pydantic-core`, a Rust extension) can simplify these concerns, reduce code, and maintain or
improve performance.

**Verdict: mixed. Pydantic offers clear wins for input validation and coercion but adds
complexity, a heavy dependency, and potential performance friction for the output-type path that
dominates grommet's workload.**

---

## 1. Current Architecture

### 1.1 User-Facing API

Users write stdlib dataclasses and decorate them:

```python
@grommet.type
@dataclass
class Query:
    greeting: str = "Hello world!"

    @grommet.field
    async def user(self, name: str) -> User: ...


@grommet.input
@dataclass
class AddUserInput:
    name: Annotated[str, grommet.Field(description="Name")]
    title: str | None = None
```

### 1.2 Internal Pipeline

1. **`decorators.py`** — validates the class is a dataclass, attaches `__grommet_meta__`.
2. **`plan.py`** — traverses types via `dataclasses.fields()` and `get_annotations()`, producing
   a `SchemaPlan` tree of `TypePlan` / `FieldPlan` / `ArgPlan` frozen dataclasses.
3. **`annotations.py`** — manual `get_origin`/`get_args` unwrapping for `Optional`, `list`,
   `Annotated`, `AsyncIterator`, `ClassVar`, etc.
4. **`coercion.py`** — runtime dict→dataclass coercion for `@grommet.input` types (recursively
   handles nested inputs and lists).
5. **`parse.rs`** — Rust reads `SchemaPlan` attributes via PyO3 `getattr()` to build Rust-side
   `TypeDef`/`FieldDef`/`ArgDef` structs.
6. **`build.rs`** — assembles `async-graphql` dynamic schema from parsed definitions.
7. **`resolver.rs`** — calls Python resolvers, applies arg coercers at runtime.

### 1.3 Key Dataclass Touchpoints

| Module | Dataclass usage | LOC involved |
|---|---|---|
| `metadata.py` | `Field`, `TypeMeta`, `TypeSpec`, `ArgPlan` (frozen internal dataclasses) | ~40 |
| `annotations.py` | `AnnotationInfo` (frozen); manual annotation introspection | ~165 |
| `plan.py` | `FieldPlan`, `TypePlan`, `SchemaPlan`; `dataclasses.fields()` iteration | ~310 |
| `coercion.py` | `dataclasses.asdict()`, `cls(**value)` for dict→input coercion | ~71 |
| `decorators.py` | `dataclasses.is_dataclass()` check | ~181 |
| `resolver.py` | `ResolverResult` (frozen) | ~204 |

**Total Python LOC across these modules: ~971.**

---

## 2. What Pydantic 2+ Offers

### 2.1 Python Side (`pydantic.BaseModel`)

- **Automatic validation on `__init__`**: `BaseModel(**data)` validates and coerces all fields
  through Rust-backed `pydantic-core`. No manual coercion code needed.
- **`model_fields`**: returns `dict[str, FieldInfo]` — richer than `dataclasses.fields()`,
  includes alias, description, constraints, default, annotation, and metadata. Eliminates the
  need for separate `get_annotations()` + `dataclasses.fields()` iteration.
- **`model_json_schema()`**: generates JSON Schema automatically. While not directly GraphQL SDL,
  the field metadata it exposes (types, nullability, descriptions, defaults) overlaps heavily
  with what `plan.py` reconstructs manually.
- **`model_validate()`**: validates dicts, JSON strings, or arbitrary objects against the model.
  Directly replaces `_coerce_input()` / `_arg_coercer()`.
- **`model_construct()`**: creates instances _without_ validation — useful for the output path
  where data is already trusted (coming from resolvers).
- **`Annotated` + `Field()`**: Pydantic natively understands `Annotated[str, Field(description=...)]`,
  which maps cleanly to grommet's existing `Annotated[str, grommet.Field(description=...)]`.
- **`ConfigDict(frozen=True)`**: equivalent to `@dataclass(frozen=True, slots=True)`.
- **`create_model()`**: dynamic model creation from field definitions at runtime — could
  simplify programmatic schema generation.
- **Computed fields / `@field_validator`**: potential future features for grommet (e.g.,
  field-level validation hooks before resolver dispatch).

### 2.2 Rust Side (`pydantic-core`)

`pydantic-core` is a Rust extension (now embedded in the main pydantic repo) that provides:

- **`SchemaValidator`**: takes a `CoreSchema` dict and validates Python objects against it
  entirely in Rust. Supports nested models, lists, optionals, unions, etc.
- **`SchemaSerializer`**: Rust-side serialization (to dict or JSON).
- **`CoreSchema`**: a Python dict describing the validation/serialization schema. Built
  automatically by Pydantic's metaclass from type annotations.

The key architectural insight: pydantic-core is a _general-purpose_ Rust validation engine
driven by a schema DSL. It does not know about Pydantic models directly — it validates
arbitrary Python objects against a schema specification.

---

## 3. Analysis: Schema Building Simplification

### 3.1 Current Approach

`plan.py` (~310 LOC) manually:
- Iterates `dataclasses.fields()` to discover fields
- Calls `get_annotations()` to resolve type hints
- Unwraps `Annotated`, `Optional`, `list`, `ClassVar`, `AsyncIterator`
- Builds `TypeSpec` (a recursive `kind`/`name`/`of_type`/`nullable` tree)
- Extracts `Field.description` from `Annotated` metadata
- Determines defaults via `dc_field.default` / `dc_field.default_factory`
- Produces the `SchemaPlan` tree consumed by Rust

### 3.2 With Pydantic

`BaseModel.model_fields` provides all of this in a single attribute:

```python
for name, field_info in cls.model_fields().items():
    # field_info.annotation  → resolved type (e.g., str, list[int], Optional[Foo])
    # field_info.default     → default value (PydanticUndefined if required)
    # field_info.description → from Field(description=...)
    # field_info.metadata    → list of Annotated metadata
    # field_info.is_required() → whether the field is required
```

This could **eliminate ~60-80 LOC** of annotation unwrapping and field discovery in `plan.py`.
However, the _type mapping_ from Python types to GraphQL `TypeSpec` is domain-specific and
would still need custom code (Pydantic doesn't know about GraphQL scalars, object types vs
input types, etc.).

### 3.3 Net Impact

| Concern | Current | With Pydantic |
|---|---|---|
| Field discovery | `dataclasses.fields()` + `get_annotations()` | `model_fields` |
| Annotation unwrapping | Manual `get_origin`/`get_args` (~100 LOC) | Partially handled by Pydantic, but GraphQL-specific unwrapping still needed |
| Default extraction | Manual `dc_field.default`/`default_factory` | `field_info.default` / `field_info.is_required()` |
| Type→GraphQL mapping | Custom `_type_spec_from_annotation` | Still custom |

**Estimated reduction: ~60-80 LOC in `plan.py` and `annotations.py`.** The GraphQL-specific
type mapping logic is irreducible.

---

## 4. Analysis: Input Argument Validation

### 4.1 Current Approach

`coercion.py` (~71 LOC) manually converts incoming dicts to dataclass instances:

```python
def _coerce_input(value, cls):
    if isinstance(value, cls):
        return value
    if isinstance(value, dict):
        return cls(**value)  # no type validation!
    raise input_mapping_expected(...)
```

This performs **no runtime type validation**. If a GraphQL variable passes the wrong type for a
field, it silently constructs the dataclass with the wrong data. The only validation is what
`async-graphql` does on the GraphQL layer (which validates against the SDL types, not the Python
types).

Additionally, `resolver.py` builds arg coercers (lambdas that recursively convert nested dicts
to input dataclass instances), adding another ~40 LOC of plumbing.

### 4.2 With Pydantic

Replacing `cls(**value)` with `cls.model_validate(value)` gives:
- **Full type coercion**: strings coerced to ints, etc.
- **Structured error messages**: `ValidationError` with field-level detail
- **Nested model validation**: Pydantic recursively validates nested models from dicts
  automatically — **eliminates the entire `_arg_coercer` machinery**.

```python
# Before (current):
def _coerce_input(value, cls):
    if isinstance(value, cls):
        return value
    if isinstance(value, dict):
        return cls(**value)
    raise ...


# After (pydantic):
def _coerce_input(value, cls):
    if isinstance(value, cls):
        return value
    return cls.model_validate(value)  # validates + coerces
```

The recursive `_arg_coercer` factory in `coercion.py` (handling `list[InputType]`,
`Optional[InputType]`, nested inputs) becomes unnecessary — Pydantic handles all of this
natively.

### 4.3 Net Impact

**Estimated reduction: ~50-60 LOC across `coercion.py` and `resolver.py`.** Additionally, this
adds real runtime validation that doesn't exist today, improving correctness.

---

## 5. Analysis: Code Footprint Reduction

### 5.1 Modules Affected

| Module | Current LOC | Estimated with Pydantic | Delta |
|---|---|---|---|
| `annotations.py` | 165 | ~120 | -45 |
| `plan.py` | 310 | ~270 | -40 |
| `coercion.py` | 71 | ~15 | -56 |
| `metadata.py` | 57 | ~50 | -7 |
| `decorators.py` | 181 | ~170 | -11 |
| `resolver.py` | 204 | ~185 | -19 |
| **Total** | **988** | **~810** | **~-178** |

### 5.2 What Changes

- **Eliminated**: `_arg_coercer`, `_coerce_input`, `_default_value_for_annotation`, most of
  `_input_field_default`, `dataclasses.asdict()` usage.
- **Simplified**: field iteration loops, default extraction, annotation introspection.
- **Unchanged**: GraphQL type mapping, resolver analysis, `_FieldResolver` descriptor,
  `SchemaPlan`/`TypePlan`/`FieldPlan` structures (these are internal, not user-facing).

### 5.3 What's Added

- `pydantic>=2.0` as a runtime dependency (~4.5 MB installed, pulls in `pydantic-core`,
  `typing-extensions`, `annotated-types`).
- The need to either:
  - **(a)** require user types to inherit `BaseModel` (breaking API change), or
  - **(b)** use `pydantic.dataclasses.dataclass` as a drop-in replacement for
    `dataclasses.dataclass` (less breaking, but users still need pydantic installed), or
  - **(c)** use Pydantic internally only for input coercion, keeping user-facing types as
    stdlib dataclasses (no API change, but limited benefit).

---

## 6. Analysis: Performance

### 6.1 Instantiation Overhead

Grommet's hot paths:

1. **Schema construction** (one-time): `build_schema_graph()` → `_core.Schema()`. Pydantic's
   metaclass overhead for model creation is paid once here. Negligible.

2. **Resolver dispatch** (per-request, per-field): the Rust runtime calls Python resolvers and
   converts results. For **output types**, objects are returned from resolvers as-is — no
   validation needed. For **input types**, arguments are coerced from GraphQL values (dicts)
   into Python objects.

3. **Input coercion** (per-request, per-input-arg): currently `cls(**dict_value)` — a bare
   dataclass constructor with no validation. Pydantic's `model_validate()` would be _slower_
   here because it runs the full Rust validation pipeline.

### 6.2 Benchmarks (from public Pydantic data)

- Pydantic 2 `BaseModel(**data)`: ~2-5μs for a simple 3-field model
- Stdlib `dataclass(**data)` (no validation): ~0.3-0.5μs for equivalent
- Pydantic `model_construct()` (skip validation): ~0.5-1μs

For grommet, input coercion happens after `async-graphql` has already validated the GraphQL
types. The double-validation (GraphQL + Pydantic) is redundant but defensive.

### 6.3 Mitigation Strategies

- **Use `model_construct()` for trusted paths**: when data comes from `async-graphql`'s
  validated variables, skip Pydantic validation. This is nearly as fast as a bare dataclass.
- **Use `model_validate()` only at API boundaries**: e.g., when users construct input objects
  directly in Python (not via GraphQL).
- **Internal types stay as frozen dataclasses**: `FieldPlan`, `TypePlan`, `SchemaPlan`, etc.
  don't benefit from Pydantic and should remain stdlib dataclasses to avoid overhead.

### 6.4 Performance Verdict

For the **output path** (dominant): no change — resolver results are Python objects consumed
by Rust, Pydantic is not involved.

For the **input path**: ~3-10x slower per input coercion if using `model_validate()`. Mitigated
by `model_construct()` for trusted data. In practice, input coercion is a small fraction of
total request time (dwarfed by I/O, resolver logic, and Rust schema execution).

---

## 7. Rust-Side Implications (`pydantic-core`)

### 7.1 Could Grommet Use `pydantic-core` Directly?

`pydantic-core` exposes `SchemaValidator` and `SchemaSerializer` as Python classes backed by
Rust. In theory, grommet could:

1. Build a `CoreSchema` dict describing each input type
2. Use `SchemaValidator.validate_python()` to validate input arguments in Rust

However, this is **not practical** for grommet because:

- **grommet already has its own Rust layer** (`parse.rs`, `build.rs`, `resolver.rs`) that talks
  to `async-graphql`. The validation boundary is `async-graphql`'s type system, not Pydantic's.
- `pydantic-core` validates _Python objects against Python type schemas_. Grommet's Rust side
  works with `async-graphql::Value` (a Rust enum), not Python objects, for most of the
  pipeline.
- Adding `pydantic-core` as a Rust dependency would couple grommet to Pydantic's internal
  schema DSL, which is complex (~100 schema node types) and not designed for external
  consumption.

### 7.2 Could Grommet's Rust Side Parse Pydantic Models?

Currently, `parse.rs` reads `SchemaPlan` dataclass attributes via `getattr()`. If user types
were Pydantic models, the Rust side could alternatively read `cls.model_fields()` or
`cls.__pydantic_fields__` to discover field metadata. This would:

- Move some schema-building logic from Python to Rust
- Eliminate the `plan.py` intermediate step for field discovery

However, this would **tightly couple** grommet's Rust code to Pydantic's internal attribute
layout, making it fragile across Pydantic versions. The current `SchemaPlan` abstraction is a
cleaner boundary.

### 7.3 Rust-Side Verdict

**No benefit to using `pydantic-core` in grommet's Rust layer.** The existing Rust pipeline
works with `async-graphql` types and has its own optimized conversion paths. Pydantic's Rust
layer solves a different problem (Python type validation) that overlaps only with grommet's
input coercion, which is a small part of the system.

---

## 8. API Design Implications

### 8.1 Option A: BaseModel-Based API (Breaking Change)

```python
from pydantic import BaseModel


@grommet.type
class Query(BaseModel):
    model_config = ConfigDict(frozen=True)
    greeting: str = "Hello world!"
```

**Pros**: full Pydantic ecosystem (validators, serializers, JSON Schema), richest field metadata.
**Cons**: breaking API change, forces Pydantic on all users, `BaseModel` instances are heavier
than dataclasses, potential metaclass conflicts.

### 8.2 Option B: Pydantic Dataclasses (Minimal Change)

```python
from pydantic.dataclasses import dataclass  # or re-exported by grommet


@grommet.type
@dataclass
class Query:
    greeting: str = "Hello world!"
```

**Pros**: nearly identical user API, adds validation, compatible with `dataclasses.fields()`.
**Cons**: still requires Pydantic dependency, subtle behavioral differences from stdlib
dataclasses (e.g., validation on `__init__`), users must import from pydantic or grommet
instead of stdlib.

### 8.3 Option C: Internal-Only Pydantic (No API Change)

Keep user types as stdlib dataclasses. Use Pydantic internally only for input coercion:

```python
# Internal: dynamically create a Pydantic model mirroring the input dataclass
_pydantic_model = create_model(
    cls.__name__,
    **{name: (info.annotation, info.default) for name, info in fields.items()},
)


def _coerce_input(value, cls):
    validated = _pydantic_model.model_validate(value)
    return cls(**validated.model_dump())
```

**Pros**: zero API change, adds validation to inputs.
**Cons**: double-construction (Pydantic model → dict → dataclass), limited benefit, still
requires Pydantic dependency.

---

## 9. Comparison with Existing GraphQL Libraries

| Library | Type base | Validation | Notes |
|---|---|---|---|
| **Strawberry** | stdlib dataclass | None (relies on GraphQL layer) | Similar to grommet's current approach |
| **Ariadne** | Schema-first (SDL) | Manual | No Python type system integration |
| **Graphene** | Custom `ObjectType` class | Custom field-level | Heavy class hierarchy |
| **grommet (current)** | stdlib dataclass | GraphQL layer only | Lightweight, minimal deps |
| **grommet (pydantic)** | BaseModel or pydantic dataclass | GraphQL + Pydantic | Heavier, more correct |

No major Python GraphQL library currently uses Pydantic for its type system. Strawberry
considered it and chose to stay with stdlib dataclasses for performance and simplicity.

---

## 10. Recommendation

### Do Not Adopt Pydantic for the Core API

The cost/benefit analysis does not favor adopting Pydantic as grommet's type foundation:

1. **Dependency weight**: Pydantic 2 pulls in ~4.5 MB of compiled Rust extensions. Grommet's
   value proposition is being lightweight and fast. Adding a heavy dependency undermines this.

2. **Marginal code reduction**: ~178 LOC savings across ~988 LOC (18%). The remaining code is
   domain-specific (GraphQL type mapping) and unaffected by Pydantic.

3. **Performance regression on inputs**: even with `model_construct()`, Pydantic adds overhead
   to a path where `async-graphql` already validates types.

4. **API disruption**: any option that changes the user-facing API (A or B) is a breaking change
   with little user-visible benefit (users don't _need_ field validators on GraphQL types;
   the schema layer handles validation).

5. **Rust-side mismatch**: `pydantic-core` solves a different problem than grommet's Rust layer
   and cannot be meaningfully leveraged.

### Consider Instead

- **Targeted input validation**: if runtime validation of input types becomes a requirement,
  add a lightweight validation pass in `_coerce_input` using `isinstance` checks and type
  narrowing — no Pydantic needed. Alternatively, use `beartype` or `typeguard` for opt-in
  runtime checking with minimal overhead.

- **`Annotated` metadata expansion**: the current `grommet.Field(description=...)` pattern is
  clean and extensible. Add more metadata (constraints, deprecation) as `Annotated` markers
  without introducing Pydantic.

- **Simplify `annotations.py` independently**: the manual `get_origin`/`get_args` unwrapping
  can be refactored with a small utility (~30 LOC) without any new dependencies.

- **Pydantic compatibility layer** (optional, future): if users want to pass Pydantic models
  as input types, grommet could detect `BaseModel` subclasses and use `model_fields` for
  introspection, without requiring all types to be Pydantic models. This would be an additive,
  non-breaking enhancement.

---

## 11. Summary Table

| Criterion | Current (dataclass) | With Pydantic |
|---|---|---|
| **Dependencies** | 0 runtime deps | +pydantic (~4.5 MB) |
| **User API** | `@dataclass` + `@grommet.type` | `BaseModel` or `@pydantic.dataclass` |
| **Input validation** | GraphQL layer only | GraphQL + Pydantic (redundant) |
| **Code footprint** | ~988 LOC | ~810 LOC (-18%) |
| **Schema build perf** | Fast (one-time) | Slightly slower (Pydantic metaclass) |
| **Input coercion perf** | ~0.3-0.5μs/obj | ~2-5μs/obj (model_validate) |
| **Output path perf** | Unaffected | Unaffected |
| **Rust-side benefit** | N/A | None |
| **API breakage** | None | Moderate to significant |
| **Ecosystem alignment** | Standard Python | Pydantic ecosystem |
