# API Reference

Complete reference for the Grommet public API.

## Schema

### `gm.Schema`

The main schema class that compiles types and executes GraphQL operations.

```python
class Schema:
    def __init__(
        self,
        *,
        query: type,
        mutation: type | None = None,
        subscription: type | None = None,
    ) -> None: ...

    async def execute(
        self,
        query: str,
        variables: dict[str, Any] | None = None,
        root: Any | None = None,
        context: Any | None = None,
    ) -> dict[str, Any]: ...

    def subscribe(
        self,
        query: str,
        variables: dict[str, Any] | None = None,
        root: Any | None = None,
        context: Any | None = None,
    ) -> SubscriptionStream: ...

    def sdl(self) -> str: ...
```

**Parameters:**

- `query` - The Query type (required)
- `mutation` - The Mutation type (optional)
- `subscription` - The Subscription type (optional)

**Methods:**

- `execute()` - Execute a GraphQL query or mutation
- `subscribe()` - Create a subscription stream
- `sdl()` - Return the schema in SDL format

---

## Decorators

### `@gm.type`

Marks a dataclass as a GraphQL object type.

```python
@gm.type(
    name: str | None = None,
    description: str | None = None,
    implements: Iterable[type] | None = None,
)
```

**Parameters:**

- `name` - Override the GraphQL type name
- `description` - Add documentation
- `implements` - List of interfaces to implement

---

### `@gm.input`

Marks a dataclass as a GraphQL input type.

```python
@gm.input(
    name: str | None = None,
    description: str | None = None,
)
```

**Parameters:**

- `name` - Override the GraphQL type name
- `description` - Add documentation

---

### `@gm.interface`

Marks a dataclass as a GraphQL interface.

```python
@gm.interface(
    name: str | None = None,
    description: str | None = None,
    implements: Iterable[type] | None = None,
)
```

**Parameters:**

- `name` - Override the GraphQL type name
- `description` - Add documentation
- `implements` - List of interfaces to extend

---

### `@gm.field`

Declares a resolver-backed field on a GraphQL type.

```python
@gm.field(
    name: str | None = None,
    description: str | None = None,
    deprecation_reason: str | None = None,
    default: Any = MISSING,
    default_factory: Callable[[], Any] | None = MISSING,
    init: bool | None = None,
)
```

**Parameters:**

- `name` - Override the GraphQL field name
- `description` - Add documentation
- `deprecation_reason` - Mark field as deprecated
- `default` - Default value for the dataclass field
- `default_factory` - Factory function for default value
- `init` - Include in dataclass `__init__`

---

### `@gm.scalar`

Registers a class as a GraphQL scalar type.

```python
@gm.scalar(
    name: str | None = None,
    description: str | None = None,
    specified_by_url: str | None = None,
    serialize: Callable[[Any], Any],
    parse_value: Callable[[Any], Any],
)
```

**Parameters:**

- `name` - Override the GraphQL scalar name
- `description` - Add documentation
- `specified_by_url` - Link to scalar specification
- `serialize` - Convert Python value to JSON output (required)
- `parse_value` - Convert JSON input to Python value (required)

---

### `@gm.enum`

Registers an enum.Enum subclass as a GraphQL enum.

```python
@gm.enum(
    name: str | None = None,
    description: str | None = None,
)
```

**Parameters:**

- `name` - Override the GraphQL enum name
- `description` - Add documentation

---

### `gm.union`

Creates a GraphQL union type.

```python
def union(
    name: str,
    *,
    types: Iterable[type],
    description: str | None = None,
) -> type
```

**Parameters:**

- `name` - The GraphQL union name (required)
- `types` - The possible types (required)
- `description` - Add documentation

---

## Types

### `gm.ID`

GraphQL ID scalar type. Subclass of `str`.

```python
class ID(str):
    """GraphQL ID scalar."""
```

**Usage:**

```python
@gm.type
@dataclass
class User:
    id: gm.ID
```

---

### `gm.Internal`

Marks a field as internal (excluded from GraphQL schema).

```python
Internal[T]  # Type alias
```

**Usage:**

```python
@gm.type
@dataclass
class User:
    id: gm.ID
    password_hash: gm.Internal[str]  # Not in GraphQL schema
```

---

### `gm.Private`

Alias for `Internal`.

```python
Private[T]  # Same as Internal[T]
```

---

### `gm.Info`

Resolver metadata passed to every resolver.

```python
@dataclass(frozen=True)
class Info:
    field_name: str
    context: Any | None = None
    root: Any | None = None
```

**Attributes:**

- `field_name` - Name of the field being resolved
- `context` - Context passed to `execute()` or `subscribe()`
- `root` - Root value passed to `execute()` or `subscribe()`

---

## Functions

### `gm.configure_runtime`

Configures the Tokio runtime for async execution.

```python
def configure_runtime(
    *,
    use_current_thread: bool = False,
    worker_threads: int | None = None,
) -> bool
```

**Parameters:**

- `use_current_thread` - Use single-threaded runtime
- `worker_threads` - Number of worker threads (multi-threaded only)

**Returns:** `True` on success

**Raises:** `GrommetTypeError` if `use_current_thread=True` and `worker_threads` is set

---

## Exceptions

### `gm.GrommetError`

Base exception for all Grommet errors.

```python
class GrommetError(Exception):
    """Base exception for grommet errors."""
```

---

### `gm.GrommetTypeError`

Raised for invalid types or annotations.

```python
class GrommetTypeError(TypeError, GrommetError):
    """Raised when grommet encounters an invalid type or annotation."""
```

---

### `gm.GrommetValueError`

Raised for invalid values.

```python
class GrommetValueError(ValueError, GrommetError):
    """Raised when grommet encounters an invalid value."""
```

---

### `gm.GrommetSchemaError`

Raised for invalid schema definitions.

```python
class GrommetSchemaError(GrommetValueError):
    """Raised when the schema definition is invalid."""
```
