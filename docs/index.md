# Grommet

**High performance Python GraphQL server library**

Grommet is a GraphQL library for Python inspired by [Strawberry](https://strawberry.rocks/) and powered by [async-graphql](https://async-graphql.github.io/async-graphql/en/index.html) via Rust bindings. It combines Python's developer-friendly syntax with the performance of a native Rust GraphQL engine.

## Features

- **High Performance** - Powered by async-graphql through PyO3 Rust bindings
- **Type Safe** - Uses Python dataclasses and type annotations
- **Async First** - Built for async/await with full sync support
- **Subscriptions** - First-class support for GraphQL subscriptions
- **Custom Scalars** - Easy custom scalar type definitions
- **Interfaces & Unions** - Full support for abstract types

## Quick Example

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

# Execute a query
result = await schema.execute('{ hello(name: "Ada") }')
print(result["data"]["hello"])  # Hello, Ada!
```

## Installation

```bash
pip install grommet
```

Or with [uv](https://github.com/astral-sh/uv):

```bash
uv add grommet
```

## Why Grommet?

Grommet bridges the gap between Python's ease of use and the performance demands of production GraphQL APIs. By leveraging Rust's async-graphql library through PyO3, Grommet achieves significant performance improvements over pure Python implementations while maintaining a familiar, Pythonic API.

## Next Steps

- [Getting Started](getting-started.md) - Build your first GraphQL API
- [Object Types](types/object-types.md) - Learn about defining types
- [Resolvers](resolvers.md) - Understand field resolution
- [Subscriptions](subscriptions.md) - Real-time data with subscriptions
