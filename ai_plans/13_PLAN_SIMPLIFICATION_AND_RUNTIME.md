# Plan Simplification & Tokio Runtime Configuration

> - `plan.py:L218-L220` this doesn't make sense to me -- if the type is a root, you can just check to ensure the field has a default (since roots have no parent). if it does, just use the same attrgetter method. we dont need a special resolver
> - `plan.py:L241-L298` can this be simplified or removed? `resolver.py:L159-L164`
> - `plan.py:L301-L302` can this reuse the same codepath as `plan.py:L208-L209`
> - configure the tokio runtime to maximize performance: https://github.com/PyO3/pyo3-async-runtimes/blob/main/pyo3-async-runtimes-macros/src/tokio.rs#L241

Simplify the Python-side planning phase by eliminating the synthetic root resolver, deduplicating
arg-coercer logic between `_build_field_plans` and `_analyze_resolver`, unifying input/object field
plan building, and configuring the Tokio runtime for maximum throughput.

## 1) Remove synthetic root field resolver [x]

### Rationale

`_make_root_field_resolver` (L187-196) creates a closure that instantiates the root class with
`cls()` and then does `getattr(instance, attr_name)`. This is wrong for any root type whose fields
don't all have defaults, and unnecessary for those that do — a root field with a default value is
just an attribute on a default-constructed instance, which `attrgetter` can handle identically to
non-root fields. The Rust side resolves root fields by passing the parent object (already a `PyObj`)
into the resolver; all that's needed is for the field to have a default so the dataclass can be
instantiated by the framework.

### Tasks

- [ ] In `_build_field_plans`, replace the `is_root` branch (L218-222) with a validation:
  if the type is a root and the dataclass field has no default, raise an error (root types
  must have defaults for all plain fields since there is no parent to resolve from). Otherwise,
  use `attrgetter(dc_field.name)` unconditionally for all plain dataclass fields regardless of
  whether the type is a root.
- [ ] Delete `_make_root_field_resolver` (L187-196).
- [ ] Remove the `is_root` parameter from `_build_field_plans` and the `roots` set from
  `_build_type_plans` (L153).
- [ ] Update tests that exercise root plain fields to ensure they still resolve correctly via the
  `attrgetter` path.

### Implementation Notes

The existing `_make_root_field_resolver` signature is `(self: object) -> object`, which means it
already takes `self` (the parent). The closure ignores `self` and constructs a fresh `cls()`. The
correct behavior is to use `attrgetter` on `self` the same way non-root fields do. The key
assumption is that root types are always instantiated with defaults by the framework before their
fields are resolved — this is already the case in `async-graphql`'s dynamic schema, which creates
a parent value for root types automatically.

Current code:
```python
if is_root:
    func = _make_root_field_resolver(cls, dc_field.name)
else:
    func = attrgetter(dc_field.name)
```

New code:
```python
func = attrgetter(dc_field.name)
```

## 2) Deduplicate arg-coercer logic between plan and resolver [x]

### Rationale

`_build_field_plans` (L261-276) manually iterates resolver args to build `ArgPlan` entries,
computing `_type_spec_from_annotation` and `_default_value_for_annotation` per arg. Meanwhile,
`_analyze_resolver` (resolver.py L159-164) independently iterates the same params to build
`arg_coercers`. Both use `_resolver_arg_info` to get the same `(param, annotation)` pairs. This
means every resolver-backed field calls `_resolver_arg_info` twice and processes each arg twice.

The arg-coercer list built by `_analyze_resolver` is already stored on `FieldPlan.arg_coercers`.
The `ArgPlan` list provides type specs and defaults for the Rust-side schema registration. These
are orthogonal concerns, but both derive from the same `_resolver_arg_info` iteration. They should
be built in a single pass.

### Tasks

- [ ] Move the arg-building loop (L261-276) into `_analyze_resolver` (or a new helper called from
  it), so that `_analyze_resolver` returns both `ResolverResult` and a `list[ArgPlan]` in one pass.
  This eliminates the second `_resolver_arg_info` call and the duplicated annotation lookup.
  Concretely, extend `ResolverResult` with an `args: tuple[ArgPlan, ...]` field.
- [ ] In `_build_field_plans`, the resolver-backed field loop (L241-296) simplifies to:
  1. Get return annotation and type_spec (L246-258) — keep as-is.
  2. Call `_analyze_resolver(...)` — already done (L279-281).
  3. Use `info.args` directly instead of building `args` inline.
- [ ] Update all consumers of `ResolverResult` (currently only `_build_field_plans`) to use the
  new `args` field.

### Implementation Notes

`ResolverResult` currently contains:
```python
@dataclass(frozen=True, slots=True)
class ResolverResult:
    func: Callable[..., Any]
    shape: str
    arg_coercers: list[tuple[str, Callable[[Any], Any] | None]]
    is_async: bool
    is_async_gen: bool
```

Add:
```python
    args: tuple[ArgPlan, ...] = ()
```

Inside `_analyze_resolver`, after building `arg_coercers`, also build `ArgPlan` entries in the same
loop over `arg_params`. This requires importing `ArgPlan`, `_type_spec_from_annotation`,
`_default_value_for_annotation`, and `MISSING` into `resolver.py`. Alternatively, factor the
combined loop into a helper in `plan.py` that both produces `arg_coercers` and `ArgPlan` entries,
and have `_analyze_resolver` call it. The latter avoids circular imports since `plan.py` already
imports from `resolver.py`.

The cleanest approach: create a `_build_resolver_args` helper in `plan.py` that takes
`_resolver_arg_info(resolver)` output and returns `(arg_coercers, args)`. Call it from
`_analyze_resolver`-equivalent code inside `_build_field_plans`.

## 3) Unify `_build_input_field_plans` with `_build_field_plans` [x]

### Rationale

`_build_input_field_plans` (L301-338) and the dataclass-field loop in `_build_field_plans`
(L207-239) share the same structure: iterate `dataclasses.fields(cls)`, get annotation from hints,
skip hidden fields, compute `type_spec`, extract description from `Annotated` metadata, and build
a `FieldPlan`. The differences are:

1. `expect_input=True` vs `False` in `_type_spec_from_annotation`
2. Input fields compute `force_nullable` from `dc_field.default`/`default_factory` presence;
   object fields compute it from `dc_field.default is None`
3. Input fields compute a `default` value via `_input_field_default`; object fields don't
4. Object fields set `func`/`shape`; input fields don't

These are parameterizable differences, not fundamentally different codepaths.

### Tasks

- [ ] Refactor into a single `_build_dataclass_field_plans(cls, *, expect_input: bool)` that
  handles both cases:
  - `expect_input=True`: use input-style `force_nullable`, compute `default` via
    `_input_field_default`, no `func`/`shape`.
  - `expect_input=False`: use object-style `force_nullable`, set `func = attrgetter(...)`,
    `shape = "self_only"`.
- [ ] Replace calls to `_build_input_field_plans(cls)` with
  `_build_dataclass_field_plans(cls, expect_input=True)`.
- [ ] Replace the dataclass-field loop in `_build_field_plans` with
  `_build_dataclass_field_plans(cls, expect_input=False)`, then append resolver-backed fields.
- [ ] Delete `_build_input_field_plans`.

### Implementation Notes

The unified function signature:

```python
def _build_dataclass_field_plans(
    cls: "pytype", *, expect_input: bool
) -> list[FieldPlan]:
    field_plans: list[FieldPlan] = []
    hints = get_annotations(cls)

    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue

        if expect_input:
            force_nullable = (
                dc_field.default is not MISSING
                or dc_field.default_factory is not MISSING
            )
        else:
            force_nullable = dc_field.default is None

        type_spec = _type_spec_from_annotation(
            annotation, expect_input=expect_input, force_nullable=force_nullable
        )

        annotated_field = _get_annotated_field_meta(annotation)
        description = annotated_field.description if annotated_field else None

        if expect_input:
            default_value = _input_field_default(dc_field, annotation)
            field_default = default_value if default_value is not MISSING else NO_DEFAULT
            field_plans.append(
                FieldPlan(
                    name=dc_field.name, source=dc_field.name,
                    type_spec=type_spec, default=field_default, description=description,
                )
            )
        else:
            field_plans.append(
                FieldPlan(
                    name=dc_field.name, source=dc_field.name,
                    type_spec=type_spec, func=attrgetter(dc_field.name),
                    shape="self_only", description=description,
                )
            )

    return field_plans
```

## 4) Configure the Tokio runtime for maximum performance [x]

### Rationale

Currently, the project relies on `pyo3_async_runtimes::tokio::future_into_py` without ever calling
`pyo3_async_runtimes::tokio::init(builder)`. This means the runtime uses the default configuration
from `pyo3-async-runtimes`, which creates a multi-thread runtime with default settings. For a
GraphQL server handling concurrent async resolvers, we should explicitly configure the runtime with
`enable_all()` (IO + time drivers) and potentially tune `worker_threads`.

The `pyo3_async_runtimes::tokio::init` function accepts a `tokio::runtime::Builder` and must be
called before any `future_into_py` invocation. The natural place is during Python module
initialization (`_core` in `lib.rs`).

### Tasks

- [ ] Add `"rt-multi-thread"` to the tokio features in `Cargo.toml` (currently only `"sync"`).
  This is required to use `runtime::Builder::new_multi_thread()`.
- [ ] In `src/lib.rs`, in the `_core` module init function, call
  `pyo3_async_runtimes::tokio::init(builder)` with a configured `Builder`:
  ```rust
  let mut builder = pyo3_async_runtimes::tokio::re_exports::runtime::Builder::new_multi_thread();
  builder.enable_all();
  pyo3_async_runtimes::tokio::init(builder);
  ```
- [ ] Optionally expose a Python-level `configure_runtime(worker_threads=None)` function that
  must be called before schema creation. This allows users to tune worker threads. If not desired,
  hardcode sensible defaults (multi-thread + enable_all is the minimum).
- [ ] Verify with benchmarks that the explicit runtime init doesn't regress and that the IO/timer
  drivers are now available for resolvers that need them.

### Implementation Notes

From the `pyo3-async-runtimes` macro source, the generated `main` function does:

```rust
let mut builder = pyo3_async_runtimes::tokio::re_exports::runtime::Builder::new_multi_thread();
builder.enable_all();
// optionally: builder.worker_threads(N);
pyo3_async_runtimes::tokio::init(builder);
```

Since grommet is a library (not a binary with `#[pyo3_async_runtimes::tokio::main]`), we must call
`init` manually. The `init` function is idempotent in that it sets a global `OnceCell`, so calling
it multiple times is safe (only the first call takes effect).

The `enable_all()` call enables both the IO driver and the time driver. Without it, any resolver
that uses `tokio::time::sleep` or `tokio::net` would panic at runtime.

The `Cargo.toml` change:
```toml
tokio = { version = "1.49", features = ["sync", "rt-multi-thread"] }
```

## 5) Update tests and verify [x]

### Rationale

All structural changes must be validated against existing tests and new edge cases.

### Tasks

- [ ] Run `uv run pytest` and `cargo test` after each section.
- [ ] Add/update tests for:
  - Root type plain fields resolving via `attrgetter` (no synthetic resolver).
  - Root type plain fields without defaults raising an error.
  - Input field plans produced by the unified `_build_dataclass_field_plans`.
  - Tokio runtime initialization (verify `future_into_py` works after explicit init).
- [ ] Run `prek run -a` and address any failures/warnings.
