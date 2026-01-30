# grommet

Dataclass-first GraphQL schema definitions in Python, executed by async-graphql in Rust via PyO3.

## Example

```python
from dataclasses import dataclass
import grommet as gm

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

- Resolvers are async-first; sync resolvers also work.
- Field arguments are derived from resolver type annotations.
- Input types must be marked with `@gm.input`.
- Use `Schema.sdl()` to inspect the generated schema.

## Build

This project is configured for uv + maturin:

```bash
uv pip install -e .
# or
maturin develop
```
