# Object Types

Object types are the building blocks of a GraphQL schema. They define the structure of your data and the relationships between different types.

## Defining Object Types

In Grommet, object types are Python dataclasses decorated with `@gm.type`:

```python
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    email: str
```

This generates the following GraphQL schema:

```graphql
type User {
    id: ID!
    name: String!
    email: String!
}
```

## Type Mapping

Grommet automatically maps Python types to GraphQL types:

| Python Type | GraphQL Type |
|-------------|--------------|
| `str` | `String!` |
| `int` | `Int!` |
| `float` | `Float!` |
| `bool` | `Boolean!` |
| `gm.ID` | `ID!` |
| `list[T]` | `[T!]!` |
| `T \| None` | `T` (nullable) |

## Optional Fields

Use `None` union types to make fields nullable:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    nickname: str | None = None  # Nullable with default
```

```graphql
type User {
    id: ID!
    name: String!
    nickname: String
}
```

## Nested Types

Types can reference other types:

```python
@gm.type
@dataclass
class Address:
    street: str
    city: str
    country: str


@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    address: Address
```

## Lists

Use Python's `list` type for arrays:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    tags: list[str]
```

```graphql
type User {
    id: ID!
    name: String!
    tags: [String!]!
}
```

## Custom Type Names

Override the GraphQL type name using the `name` parameter:

```python
@gm.type(name="Person")
@dataclass
class User:
    id: gm.ID
    name: str
```

```graphql
type Person {
    id: ID!
    name: String!
}
```

## Descriptions

Add descriptions that appear in your GraphQL schema:

```python
@gm.type(description="A user in the system")
@dataclass
class User:
    id: gm.ID
    name: str
```

```graphql
"""A user in the system"""
type User {
    id: ID!
    name: String!
}
```

## Fields with Resolvers

Fields can have custom resolvers using `@gm.field`:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    first_name: str
    last_name: str

    @gm.field
    @staticmethod
    async def full_name(parent, info) -> str:
        return f"{parent.first_name} {parent.last_name}"
```

See [Resolvers](../resolvers.md) for more details.

## Internal Fields

Fields can be excluded from the GraphQL schema while remaining available in Python:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    _internal_id: int  # Excluded (underscore prefix)
    password_hash: gm.Internal[str]  # Excluded (Internal wrapper)
```

Three ways to mark fields as internal:

1. **Underscore prefix** - Fields starting with `_`
2. **`gm.Internal[T]`** - Wrap the type annotation
3. **`gm.Private[T]`** - Alias for `Internal`

## Implementing Interfaces

Types can implement interfaces:

```python
@gm.interface
@dataclass
class Node:
    id: gm.ID


@gm.type(implements=[Node])
@dataclass
class User:
    id: gm.ID
    name: str
```

See [Interfaces](interfaces.md) for more details.
