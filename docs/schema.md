# Schema

The `Schema` class is the core of Grommet. It compiles your type definitions into a GraphQL schema and provides methods for executing queries and subscriptions.

## Creating a Schema

```python
import grommet as gm

schema = gm.Schema(query=Query)
```

### With Mutations

```python
schema = gm.Schema(
    query=Query,
    mutation=Mutation,
)
```

### With Subscriptions

```python
schema = gm.Schema(
    query=Query,
    mutation=Mutation,
    subscription=Subscription,
)
```

## Executing Queries

Use `execute()` to run GraphQL queries:

```python
result = await schema.execute("{ hello }")
```

### With Variables

```python
result = await schema.execute(
    """
    query ($name: String!) {
        greet(name: $name)
    }
    """,
    variables={"name": "Alice"},
)
```

### With Context

Pass request-specific data to resolvers:

```python
result = await schema.execute(
    "{ currentUser { name } }",
    context={
        "user_id": "123",
        "request": request,
        "db": database,
    },
)
```

### With Root Value

Provide a root object for top-level resolvers:

```python
result = await schema.execute(
    "{ config }",
    root={"config": app_config},
)
```

## Response Format

Execution returns a dictionary with the standard GraphQL response format:

```python
{
    "data": {
        "hello": "Hello, world!"
    }
}
```

### With Errors

```python
{
    "data": {
        "user": None
    },
    "errors": [
        {
            "message": "User not found",
            "path": ["user"],
            "locations": [{"line": 1, "column": 3}]
        }
    ]
}
```

### Partial Results

GraphQL can return partial data with errors:

```python
{
    "data": {
        "users": [
            {"name": "Alice"},
            None,  # This user's resolver failed
            {"name": "Charlie"}
        ]
    },
    "errors": [
        {"message": "Failed to fetch user", "path": ["users", 1]}
    ]
}
```

## Subscriptions

Use `subscribe()` for real-time data:

```python
stream = schema.subscribe(
    "subscription { messages { text } }"
)

async for payload in stream:
    print(payload["data"]["messages"]["text"])
```

See [Subscriptions](subscriptions.md) for details.

## Schema Introspection

### SDL Output

Get the GraphQL Schema Definition Language:

```python
print(schema.sdl())
```

Output:

```graphql
type Query {
    hello: String!
    users: [User!]!
}

type User {
    id: ID!
    name: String!
    email: String!
}
```

## Type Discovery

Grommet automatically discovers types referenced by your schema:

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    posts: list["Post"]


@gm.type
@dataclass
class Post:
    id: gm.ID
    title: str
    author: User


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def user(parent, info, id: gm.ID) -> User:
        ...


# Both User and Post are included in the schema
schema = gm.Schema(query=Query)
```

## Schema Requirements

### Query Type Required

Every schema must have a query type:

```python
# ✅ Valid
schema = gm.Schema(query=Query)

# ❌ Invalid - raises GrommetSchemaError
schema = gm.Schema(query=None)
```

### Types Must Be Decorated

All types used in the schema must be decorated:

```python
# ✅ Valid - User is decorated
@gm.type
@dataclass
class User:
    name: str

# ❌ Invalid - PlainClass is not decorated
@dataclass
class PlainClass:
    name: str
```

## Complete Example

```python
import asyncio
from dataclasses import dataclass
from typing import TYPE_CHECKING

import grommet as gm

if TYPE_CHECKING:
    from collections.abc import AsyncIterator


@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    email: str


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def user(parent, info, id: gm.ID) -> User | None:
        users = {"1": User(id=gm.ID("1"), name="Alice", email="alice@example.com")}
        return users.get(str(id))

    @gm.field
    @staticmethod
    async def users(parent, info) -> list[User]:
        return [
            User(id=gm.ID("1"), name="Alice", email="alice@example.com"),
            User(id=gm.ID("2"), name="Bob", email="bob@example.com"),
        ]


@gm.input
@dataclass
class CreateUserInput:
    name: str
    email: str


@gm.type
@dataclass
class Mutation:
    @gm.field
    @staticmethod
    async def create_user(parent, info, input: CreateUserInput) -> User:
        return User(
            id=gm.ID("3"),
            name=input.name,
            email=input.email,
        )


@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def countdown(parent, info, start: int) -> "AsyncIterator[int]":
        for i in range(start, 0, -1):
            yield i
            await asyncio.sleep(1)


schema = gm.Schema(
    query=Query,
    mutation=Mutation,
    subscription=Subscription,
)


async def main():
    # Query
    result = await schema.execute('{ user(id: "1") { name } }')
    print(result)

    # Mutation
    result = await schema.execute('''
        mutation {
            createUser(input: { name: "Charlie", email: "charlie@example.com" }) {
                id
                name
            }
        }
    ''')
    print(result)

    # Subscription
    stream = schema.subscribe("subscription { countdown(start: 3) }")
    async for payload in stream:
        print(payload)


if __name__ == "__main__":
    asyncio.run(main())
```
