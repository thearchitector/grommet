# Input Types

Input types are used to pass complex objects as arguments to queries and mutations. Unlike object types, input types cannot have resolvers and are used exclusively for input.

## Defining Input Types

Use the `@gm.input` decorator on a dataclass:

```python
from dataclasses import dataclass

import grommet as gm


@gm.input
@dataclass
class CreateUserInput:
    name: str
    email: str
    age: int | None = None
```

This generates:

```graphql
input CreateUserInput {
    name: String!
    email: String!
    age: Int
}
```

## Using Input Types

Input types are used as resolver arguments:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    email: str


@gm.type
@dataclass
class Mutation:
    @gm.field
    @staticmethod
    async def create_user(parent, info, input: CreateUserInput) -> User:
        # input is automatically converted to CreateUserInput instance
        return User(
            id=gm.ID("1"),
            name=input.name,
            email=input.email,
        )
```

Query:

```graphql
mutation {
    createUser(input: { name: "Alice", email: "alice@example.com" }) {
        id
        name
    }
}
```

## Default Values

Input fields can have default values:

```python
@gm.input
@dataclass
class PaginationInput:
    page: int = 1
    per_page: int = 10
```

```graphql
input PaginationInput {
    page: Int = 1
    perPage: Int = 10
}
```

## Default Factories

For mutable defaults like lists, use `dataclasses.field` with `default_factory`:

```python
from dataclasses import dataclass, field

import grommet as gm


@gm.input
@dataclass
class FilterInput:
    tags: list[str] = field(default_factory=list)
    status: str = "active"
```

## Nested Input Types

Input types can contain other input types:

```python
@gm.input
@dataclass
class AddressInput:
    street: str
    city: str
    country: str


@gm.input
@dataclass
class CreateUserInput:
    name: str
    email: str
    address: AddressInput | None = None
```

## Custom Names and Descriptions

```python
@gm.input(name="UserCreationInput", description="Data required to create a new user")
@dataclass
class CreateUserInput:
    name: str
    email: str
```

## Validation

Input values are automatically validated against the schema. Invalid inputs will result in GraphQL errors:

```graphql
# This will error if email is required but not provided
mutation {
    createUser(input: { name: "Alice" }) {
        id
    }
}
```

## Input vs Object Types

| Feature | Object Type (`@gm.type`) | Input Type (`@gm.input`) |
|---------|--------------------------|--------------------------|
| Used for | Output/Response | Input/Arguments |
| Resolvers | ✅ Allowed | ❌ Not allowed |
| Circular references | ✅ Allowed | ❌ Not allowed |
| Interfaces | ✅ Can implement | ❌ Cannot implement |

## ID Fields

Use `gm.ID` for GraphQL ID fields in inputs:

```python
@gm.input
@dataclass
class UpdateUserInput:
    id: gm.ID
    name: str | None = None
    email: str | None = None
```

## Internal Fields

Like object types, input types support internal fields:

```python
@gm.input
@dataclass
class CreateUserInput:
    name: str
    email: str
    _tracking_id: gm.Internal[str] = ""  # Not exposed in GraphQL
```
