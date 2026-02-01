# Unions

Unions represent a value that could be one of several object types. Unlike interfaces, union types don't share any fields.

## Defining Unions

Use `gm.union()` to create a union type:

```python
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    email: str


@gm.type
@dataclass
class Organization:
    id: gm.ID
    name: str
    member_count: int


# Create a union of User and Organization
SearchResult = gm.union("SearchResult", types=[User, Organization])
```

This generates:

```graphql
union SearchResult = User | Organization
```

## Using Unions

Return union types from resolvers:

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def search(parent, info, query: str) -> list[SearchResult]:
        results: list[User | Organization] = []
        # Search logic...
        if query.startswith("@"):
            results.append(User(id=gm.ID("1"), name="Alice", email="alice@example.com"))
        else:
            results.append(Organization(id=gm.ID("2"), name="Acme Corp", member_count=50))
        return results
```

## Querying Unions

Use inline fragments to access type-specific fields:

```graphql
{
    search(query: "@alice") {
        ... on User {
            id
            name
            email
        }
        ... on Organization {
            id
            name
            memberCount
        }
    }
}
```

## Union Parameters

### name (required)

The GraphQL type name:

```python
SearchResult = gm.union("SearchResult", types=[User, Organization])
```

### types (required)

The possible types in the union:

```python
SearchResult = gm.union(
    "SearchResult",
    types=[User, Organization, Post],
)
```

### description

Add documentation:

```python
SearchResult = gm.union(
    "SearchResult",
    types=[User, Organization],
    description="A search result can be either a user or an organization",
)
```

## Type Resolution

Grommet automatically resolves the correct type based on the Python class of the returned object. Ensure your resolver returns instances of the union member types:

```python
@gm.field
@staticmethod
async def search(parent, info, query: str) -> list[SearchResult]:
    # Return actual User or Organization instances
    return [
        User(id=gm.ID("1"), name="Alice", email="alice@example.com"),
        Organization(id=gm.ID("2"), name="Acme", member_count=10),
    ]
```

## Union vs Interface

| Feature | Union | Interface |
|---------|-------|-----------|
| Shared fields | ❌ No | ✅ Yes |
| Type annotation | Function call | Decorator |
| Use case | Unrelated types | Related types with common fields |

### When to use Unions

- Types have no common fields
- Types represent fundamentally different concepts
- You need to group unrelated types

### When to use Interfaces

- Types share common fields
- Types represent variations of the same concept
- You want to query common fields without fragments

## Nullable Unions

Make union fields optional:

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def find(parent, info, id: gm.ID) -> SearchResult | None:
        # Return None if not found
        return None
```

## Lists of Unions

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def search(parent, info) -> list[SearchResult]:
        return [
            User(id=gm.ID("1"), name="Alice", email="a@b.com"),
            Organization(id=gm.ID("2"), name="Acme", member_count=10),
        ]
```

## Requirements

Union member types must be:

1. Decorated with `@gm.type`
2. Object types (not inputs, interfaces, or other unions)

```python
# ✅ Valid - both are @gm.type object types
SearchResult = gm.union("SearchResult", types=[User, Organization])

# ❌ Invalid - Input types cannot be in unions
@gm.input
@dataclass
class UserInput:
    name: str

InvalidUnion = gm.union("Invalid", types=[UserInput])  # Raises error
```
