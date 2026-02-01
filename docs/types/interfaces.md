# Interfaces

Interfaces define a set of fields that multiple types can implement. They enable polymorphism in your GraphQL schema.

## Defining Interfaces

Use the `@gm.interface` decorator:

```python
from dataclasses import dataclass

import grommet as gm


@gm.interface
@dataclass
class Node:
    id: gm.ID
```

This generates:

```graphql
interface Node {
    id: ID!
}
```

## Implementing Interfaces

Types implement interfaces using the `implements` parameter:

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
    email: str


@gm.type(implements=[Node])
@dataclass
class Post:
    id: gm.ID
    title: str
    content: str
```

This generates:

```graphql
interface Node {
    id: ID!
}

type User implements Node {
    id: ID!
    name: String!
    email: String!
}

type Post implements Node {
    id: ID!
    title: String!
    content: String!
}
```

## Querying Interfaces

When querying an interface, use inline fragments to access type-specific fields:

```graphql
query {
    node(id: "1") {
        id
        ... on User {
            name
            email
        }
        ... on Post {
            title
        }
    }
}
```

## Interface Fields with Resolvers

Interfaces can have fields with resolvers:

```python
@gm.interface
@dataclass
class Timestamped:
    created_at: str
    updated_at: str

    @gm.field
    @staticmethod
    async def age_in_days(parent, info) -> int:
        # Calculate days since creation
        from datetime import datetime
        created = datetime.fromisoformat(parent.created_at)
        return (datetime.now() - created).days
```

!!! note
    Interface resolvers define the field signature but are not directly called. Each implementing type must provide its own implementation or inherit the field.

## Multiple Interfaces

Types can implement multiple interfaces:

```python
@gm.interface
@dataclass
class Node:
    id: gm.ID


@gm.interface
@dataclass
class Timestamped:
    created_at: str
    updated_at: str


@gm.type(implements=[Node, Timestamped])
@dataclass
class User:
    id: gm.ID
    created_at: str
    updated_at: str
    name: str
```

## Interface Inheritance

Interfaces can extend other interfaces:

```python
@gm.interface
@dataclass
class Node:
    id: gm.ID


@gm.interface(implements=[Node])
@dataclass
class Entity:
    id: gm.ID
    name: str
```

```graphql
interface Node {
    id: ID!
}

interface Entity implements Node {
    id: ID!
    name: String!
}
```

## Custom Names and Descriptions

```python
@gm.interface(name="Identifiable", description="An object with a unique identifier")
@dataclass
class Node:
    id: gm.ID
```

## Best Practices

1. **Keep interfaces focused** - Each interface should represent a single concept
2. **Use for shared behavior** - Interfaces work best when multiple types share common fields
3. **Prefer composition** - Types can implement multiple interfaces for flexibility
4. **Document interfaces** - Use descriptions to explain the purpose of each interface
