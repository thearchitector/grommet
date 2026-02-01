# Getting Started

This guide will walk you through creating your first GraphQL API with Grommet.

## Prerequisites

- Python 3.11 or later
- A Python package manager (pip, uv, poetry, etc.)

## Step 1: Install Grommet

```bash
pip install grommet
```

Or with uv:

```bash
uv add grommet
```

## Step 2: Define Your First Type

Create a file called `schema.py`:

```python
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Book:
    title: str
    author: str
```

Every GraphQL type in Grommet is a Python dataclass decorated with `@gm.type`. The fields of the dataclass become fields in the GraphQL schema.

## Step 3: Create a Query Type

The Query type is the entry point for all GraphQL queries:

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def books(parent, info) -> list[Book]:
        return [
            Book(title="The Great Gatsby", author="F. Scott Fitzgerald"),
            Book(title="1984", author="George Orwell"),
        ]
```

The `@gm.field` decorator marks a method as a resolver. Resolvers are functions that return data for a field.

## Step 4: Create the Schema

```python
schema = gm.Schema(query=Query)
```

## Step 5: Execute a Query

```python
import asyncio


async def main():
    result = await schema.execute("""
        {
            books {
                title
                author
            }
        }
    """)
    print(result)


asyncio.run(main())
```

The result will be:

```python
{
    "data": {
        "books": [
            {"title": "The Great Gatsby", "author": "F. Scott Fitzgerald"},
            {"title": "1984", "author": "George Orwell"}
        ]
    }
}
```

## Complete Example

Here's the complete `schema.py`:

```python
import asyncio
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Book:
    title: str
    author: str


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def books(parent, info) -> list[Book]:
        return [
            Book(title="The Great Gatsby", author="F. Scott Fitzgerald"),
            Book(title="1984", author="George Orwell"),
        ]


schema = gm.Schema(query=Query)


async def main():
    result = await schema.execute("""
        {
            books {
                title
                author
            }
        }
    """)
    print(result)


if __name__ == "__main__":
    asyncio.run(main())
```

## Understanding Resolvers

In the example above, `books` is a resolver. Every resolver receives two positional arguments:

- **parent** - The parent object (for root queries, this is typically `None` or a root value)
- **info** - A `gm.Info` object containing resolver metadata

Additional arguments become GraphQL field arguments.

## Adding Arguments

Let's add a search parameter:

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def books(parent, info, search: str | None = None) -> list[Book]:
        all_books = [
            Book(title="The Great Gatsby", author="F. Scott Fitzgerald"),
            Book(title="1984", author="George Orwell"),
        ]
        if search:
            return [b for b in all_books if search.lower() in b.title.lower()]
        return all_books
```

Now you can query with arguments:

```graphql
{
    books(search: "gatsby") {
        title
    }
}
```

## Next Steps

- [Object Types](types/object-types.md) - Learn more about defining types
- [Input Types](types/input-types.md) - Accept complex input arguments
- [Subscriptions](subscriptions.md) - Add real-time updates
