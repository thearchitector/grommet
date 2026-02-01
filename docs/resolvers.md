# Resolvers

Resolvers are functions that return data for GraphQL fields. They're the bridge between your GraphQL schema and your data sources.

## Basic Resolvers

Use the `@gm.field` decorator to define a resolver:

```python
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def hello(parent, info) -> str:
        return "Hello, world!"
```

## Resolver Arguments

### Required Arguments

Every resolver receives two positional arguments:

- **`parent`** - The parent object. For root queries, this is the root value (or `None`)
- **`info`** - A `gm.Info` object containing resolver metadata

```python
@gm.field
@staticmethod
async def greeting(parent, info) -> str:
    print(f"Resolving field: {info.field_name}")
    return "Hello!"
```

### The Info Object

`gm.Info` provides metadata about the current resolution:

```python
@dataclass(frozen=True)
class Info:
    field_name: str        # Name of the field being resolved
    context: Any | None    # Context passed to execute/subscribe
    root: Any | None       # Root value passed to execute/subscribe
```

### Field Arguments

Additional resolver parameters become GraphQL field arguments:

```python
@gm.field
@staticmethod
async def greet(parent, info, name: str) -> str:
    return f"Hello, {name}!"
```

```graphql
{
    greet(name: "Alice")  # Returns "Hello, Alice!"
}
```

### Default Values

Parameters with defaults become optional arguments:

```python
@gm.field
@staticmethod
async def greet(parent, info, name: str = "world") -> str:
    return f"Hello, {name}!"
```

```graphql
{
    greet           # Returns "Hello, world!"
    greet(name: "Bob")  # Returns "Hello, Bob!"
}
```

## Resolver Styles

### Static Methods (Recommended)

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def users(parent, info) -> list[User]:
        return await fetch_users()
```

### Class Methods

```python
@gm.type
@dataclass
class Query:
    @gm.field
    @classmethod
    async def users(cls, parent, info) -> list[User]:
        return await fetch_users()
```

### Instance Methods

For fields that depend on instance data:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    first_name: str
    last_name: str

    @gm.field
    async def full_name(self, parent, info) -> str:
        return f"{self.first_name} {self.last_name}"
```

!!! note
    For instance methods, `self` is the instance and `parent` is the parent object from GraphQL resolution (often the same as `self`).

## Async vs Sync Resolvers

### Async Resolvers (Recommended)

```python
@gm.field
@staticmethod
async def users(parent, info) -> list[User]:
    return await database.fetch_users()
```

### Sync Resolvers

Sync resolvers are also supported:

```python
@gm.field
@staticmethod
def version(parent, info) -> str:
    return "1.0.0"
```

## Field Parameters

The `@gm.field` decorator accepts several parameters:

### name

Override the GraphQL field name:

```python
@gm.field(name="userName")
@staticmethod
async def get_user_name(parent, info) -> str:
    return "Alice"
```

### description

Add field documentation:

```python
@gm.field(description="Returns the current user's name")
@staticmethod
async def name(parent, info) -> str:
    return "Alice"
```

### deprecation_reason

Mark a field as deprecated:

```python
@gm.field(deprecation_reason="Use 'fullName' instead")
@staticmethod
async def name(parent, info) -> str:
    return "Alice"
```

### default / default_factory

Provide default values for the dataclass field:

```python
@gm.type
@dataclass
class User:
    id: gm.ID

    @gm.field(default="Anonymous")
    @staticmethod
    async def display_name(parent, info) -> str:
        return parent.name if hasattr(parent, "name") else "Anonymous"
```

## Accessing Context

Pass context when executing queries:

```python
result = await schema.execute(
    "{ currentUser { name } }",
    context={"user_id": "123", "db": database},
)
```

Access it in resolvers via `info.context`:

```python
@gm.field
@staticmethod
async def current_user(parent, info) -> User:
    user_id = info.context["user_id"]
    db = info.context["db"]
    return await db.get_user(user_id)
```

## Accessing Root Value

Pass a root value when executing:

```python
result = await schema.execute(
    "{ greeting }",
    root={"message": "Hello from root!"},
)
```

Access it via `info.root` or `parent` (for root-level fields):

```python
@gm.field
@staticmethod
async def greeting(parent, info) -> str:
    return parent["message"]  # or info.root["message"]
```

## Fields Without Resolvers

Fields without `@gm.field` are resolved from the parent object:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str  # Resolved from User instance
    email: str  # Resolved from User instance
```

## Complex Arguments

### Input Types

Use input types for complex arguments:

```python
@gm.input
@dataclass
class CreateUserInput:
    name: str
    email: str


@gm.field
@staticmethod
async def create_user(parent, info, input: CreateUserInput) -> User:
    return User(id=gm.ID("1"), name=input.name, email=input.email)
```

### Lists

```python
@gm.field
@staticmethod
async def users_by_ids(parent, info, ids: list[gm.ID]) -> list[User]:
    return await fetch_users_by_ids(ids)
```

### Enums

```python
@gm.field
@staticmethod
async def users_by_status(parent, info, status: Status) -> list[User]:
    return await fetch_users_by_status(status)
```

## Error Handling

Raise exceptions to return GraphQL errors:

```python
@gm.field
@staticmethod
async def user(parent, info, id: gm.ID) -> User:
    user = await fetch_user(id)
    if user is None:
        raise ValueError(f"User {id} not found")
    return user
```

The error appears in the response:

```json
{
    "data": { "user": null },
    "errors": [
        { "message": "User 123 not found", "path": ["user"] }
    ]
}
```
