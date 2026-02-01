# Enums

Enums represent a fixed set of allowed values. They're useful for fields that should only accept specific options.

## Defining Enums

Use the `@gm.enum` decorator on a Python `enum.Enum` subclass:

```python
import enum

import grommet as gm


@gm.enum
class Status(enum.Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"
    PENDING = "pending"
```

This generates:

```graphql
enum Status {
    ACTIVE
    INACTIVE
    PENDING
}
```

## Using Enums

Enums can be used in type fields and resolver arguments:

```python
from dataclasses import dataclass


@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    status: Status


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def users_by_status(parent, info, status: Status) -> list[User]:
        # status is a Status enum instance
        return [
            User(id=gm.ID("1"), name="Alice", status=status)
        ]
```

Query:

```graphql
{
    usersByStatus(status: ACTIVE) {
        name
        status
    }
}
```

## Enum Values

The GraphQL enum values are the Python enum member names (uppercase by convention):

```python
@gm.enum
class Priority(enum.Enum):
    LOW = 1
    MEDIUM = 2
    HIGH = 3
    CRITICAL = 4
```

```graphql
enum Priority {
    LOW
    MEDIUM
    HIGH
    CRITICAL
}
```

!!! note
    The Python enum values (`1`, `2`, `3`, `4`) are internal. GraphQL uses the member names (`LOW`, `MEDIUM`, etc.).

## Custom Names and Descriptions

```python
@gm.enum(name="TaskStatus", description="The current state of a task")
class Status(enum.Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"
```

```graphql
"""The current state of a task"""
enum TaskStatus {
    ACTIVE
    INACTIVE
}
```

## String Enums

For enums backed by string values, use `enum.StrEnum` (Python 3.11+):

```python
import enum


@gm.enum
class Color(enum.StrEnum):
    RED = "red"
    GREEN = "green"
    BLUE = "blue"
```

## Integer Enums

```python
import enum


@gm.enum
class ErrorCode(enum.IntEnum):
    NOT_FOUND = 404
    SERVER_ERROR = 500
    BAD_REQUEST = 400
```

## Nullable Enums

Make enum fields optional with union types:

```python
@gm.type
@dataclass
class Task:
    id: gm.ID
    title: str
    priority: Priority | None = None
```

## Enums in Input Types

Enums work in input types too:

```python
@gm.input
@dataclass
class CreateTaskInput:
    title: str
    priority: Priority = Priority.MEDIUM
```

```graphql
mutation {
    createTask(input: { title: "Fix bug", priority: HIGH }) {
        id
        priority
    }
}
```

## Enums as List Items

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    name: str
    permissions: list[Permission]
```

```graphql
{
    user(id: "1") {
        permissions  # Returns ["READ", "WRITE"]
    }
}
```

## Best Practices

1. **Use UPPERCASE names** - Follow GraphQL conventions for enum values
2. **Keep enums focused** - Each enum should represent a single concept
3. **Document values** - Use descriptions to explain what each value means
4. **Prefer enums over strings** - For fixed sets of values, enums provide type safety
