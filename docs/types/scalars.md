# Custom Scalars

Scalars are the leaf values in GraphQL. While GraphQL provides built-in scalars (`String`, `Int`, `Float`, `Boolean`, `ID`), you can define custom scalars for specialized data types.

## Built-in Scalars

Grommet maps Python types to GraphQL scalars automatically:

| Python Type | GraphQL Scalar |
|-------------|----------------|
| `str` | `String` |
| `int` | `Int` |
| `float` | `Float` |
| `bool` | `Boolean` |
| `gm.ID` | `ID` |

## Defining Custom Scalars

Use the `@gm.scalar` decorator with `serialize` and `parse_value` functions:

```python
from dataclasses import dataclass
from datetime import date

import grommet as gm


@gm.scalar(
    serialize=lambda value: value.isoformat(),
    parse_value=lambda value: date.fromisoformat(value),
)
@dataclass
class Date:
    _value: date

    def __init__(self, value: date | str):
        if isinstance(value, str):
            value = date.fromisoformat(value)
        object.__setattr__(self, "_value", value)

    def isoformat(self) -> str:
        return self._value.isoformat()
```

- **`serialize`** - Converts Python value to JSON-compatible output
- **`parse_value`** - Converts JSON input to Python value

## Using Custom Scalars

Once defined, use scalars like any other type:

```python
@gm.type
@dataclass
class Event:
    id: gm.ID
    name: str
    date: Date


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def event(parent, info, id: gm.ID) -> Event:
        return Event(
            id=id,
            name="Conference",
            date=Date(date(2024, 6, 15)),
        )
```

Query:

```graphql
{
    event(id: "1") {
        name
        date  # Returns "2024-06-15"
    }
}
```

## Scalar Parameters

### name

Override the GraphQL scalar name:

```python
@gm.scalar(
    name="DateTime",
    serialize=lambda v: v.isoformat(),
    parse_value=lambda v: datetime.fromisoformat(v),
)
@dataclass
class MyDateTime:
    ...
```

### description

Add documentation to the scalar:

```python
@gm.scalar(
    description="ISO 8601 date string (YYYY-MM-DD)",
    serialize=lambda v: v.isoformat(),
    parse_value=lambda v: date.fromisoformat(v),
)
@dataclass
class Date:
    ...
```

### specified_by_url

Reference a specification URL:

```python
@gm.scalar(
    specified_by_url="https://tools.ietf.org/html/rfc3339",
    serialize=lambda v: v.isoformat(),
    parse_value=lambda v: datetime.fromisoformat(v),
)
@dataclass
class DateTime:
    ...
```

## Common Scalar Examples

### DateTime

```python
from datetime import datetime


@gm.scalar(
    name="DateTime",
    description="ISO 8601 datetime",
    serialize=lambda v: v.isoformat(),
    parse_value=lambda v: datetime.fromisoformat(v),
)
@dataclass
class DateTime:
    value: datetime

    def isoformat(self) -> str:
        return self.value.isoformat()
```

### JSON

```python
import json
from typing import Any


@gm.scalar(
    name="JSON",
    description="Arbitrary JSON value",
    serialize=lambda v: v,  # Pass through
    parse_value=lambda v: v,  # Pass through
)
@dataclass
class JSON:
    value: Any
```

### UUID

```python
from uuid import UUID


@gm.scalar(
    name="UUID",
    description="UUID string",
    serialize=lambda v: str(v.value),
    parse_value=lambda v: UUIDScalar(UUID(v)),
)
@dataclass
class UUIDScalar:
    value: UUID
```

### Decimal

```python
from decimal import Decimal


@gm.scalar(
    name="Decimal",
    description="Arbitrary precision decimal",
    serialize=lambda v: str(v.value),
    parse_value=lambda v: DecimalScalar(Decimal(v)),
)
@dataclass
class DecimalScalar:
    value: Decimal
```

## Scalars as Input

Custom scalars work in both input and output positions:

```python
@gm.type
@dataclass
class Mutation:
    @gm.field
    @staticmethod
    async def create_event(
        parent, info, name: str, date: Date
    ) -> Event:
        # date is automatically parsed from input string
        return Event(id=gm.ID("1"), name=name, date=date)
```

```graphql
mutation {
    createEvent(name: "Launch", date: "2024-06-15") {
        name
        date
    }
}
```

## Error Handling

If `parse_value` raises an exception, GraphQL returns a validation error:

```python
@gm.scalar(
    serialize=lambda v: v.isoformat(),
    parse_value=lambda v: date.fromisoformat(v),  # Raises ValueError for invalid dates
)
@dataclass
class Date:
    ...
```

```graphql
# Invalid date format returns an error
mutation {
    createEvent(date: "not-a-date")
}
```
