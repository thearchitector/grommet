# grommet

High performance Python GraphQL server library inspired by [Strawberry](https://strawberry.rocks/) and backed by [async-graphql](https://async-graphql.github.io/async-graphql/en/index.html).

This is an experiment in a nearly 100% AI-written project. I provide guidelines and design guidance through review of the generated code and curated revision plans, but AI does the heavy lifting. Features are developed as my token and usage counts reset.

## Example

```python
from dataclasses import dataclass
from typing import TYPE_CHECKING

import grommet as gm

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def hello(parent, info, name: str = "world") -> str:
        return f"Hello, {name}!"

schema = gm.Schema(query=Query)

result = await schema.execute("{ hello(name: \"Ada\") }")
print(result["data"]["hello"])
```

## Subscriptions

```python
@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def countdown(parent, info, limit: int) -> "AsyncIterator[int]":
        for i in range(limit):
            yield i

schema = gm.Schema(query=Query, subscription=Subscription)
stream = schema.subscribe("subscription ($limit: Int!) { countdown(limit: $limit) }", variables={"limit": 3})
async for payload in stream:
    print(payload["data"]["countdown"])
```

## Custom Scalars

```python
@gm.scalar(
    name="Date",
    serialize=lambda value: value.value,
    parse_value=lambda value: Date(str(value)),
)
@dataclass(frozen=True)
class Date:
    value: str

@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def today(parent, info) -> Date:
        return Date("2026-01-30")

schema = gm.Schema(query=Query)
```

## Schema Overview

```python
schema = gm.Schema(query=Query)
print(schema.sdl())
```

## Inputs

```python
@gm.input
@dataclass
class UserInput:
    id: gm.ID
    name: str | None = None
```

Use input types in resolver signatures:

```python
async def get_user(parent, info, user: UserInput) -> User:
    ...
```

## Notes

- Resolvers must be async.
- Field arguments are derived from resolver type annotations.
- Input types must be marked with `@gm.input`.
- Use `Schema.sdl()` to inspect the generated schema.
- Resolver params may include `parent`, `info` (`gm.Info`), `context`, and `root`.
- Fields prefixed with `_`, annotated as `ClassVar`, or wrapped in `gm.Internal[...]`/`gm.Private[...]` are excluded from the schema.

## Runtime Configuration

```python
# Configure the Tokio runtime before creating schemas.
gm.configure_runtime(use_current_thread=True)
```

## Event Loop Compatibility

- Call `uvloop.install()` before creating or awaiting Grommet futures.
- When using `asyncio.run`, create schemas and call Grommet APIs inside the coroutine passed to `asyncio.run`.

## Free-Threaded Python

Grommet currently opts out of free-threaded Python builds while we audit thread-safety for shared
state and resolver execution. This is implemented by setting `gil_used = true` on the PyO3 module.

## Benchmarks

```bash
uv run python scripts/bench_simple.py
uv run python scripts/bench_large_list.py
uv run python benchmarks/bench_schema_build.py
uv run python benchmarks/bench_resolver_call.py
```

## Type stubs (experimental)

PyO3 introspection data is enabled via the `experimental-inspect` feature. Stub generation is still
in progress and not wired into maturin yet, so `.pyi` files need to be generated out-of-band using
the `pyo3-introspection` crate against a built extension module.

## Build

This project is configured for uv + maturin:

```bash
uv pip install -e .
# or
maturin develop
```

For local Rust tests using uv-managed Python, ensure the `.venv` is present and that either
`.cargo/config.toml` or your shell environment provides `PYO3_PYTHON`, `PYTHONHOME`, and
`PYTHONPATH` pointing at the uv installation.
