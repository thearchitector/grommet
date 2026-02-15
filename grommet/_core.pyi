from collections.abc import AsyncIterator
from typing import Any

from .metadata import TypeSpec

class OperationResult:
    """Result of a GraphQL operation with data, errors, and extensions."""

    data: dict[str, Any]
    errors: list[dict[str, Any]] | None
    extensions: dict[str, Any] | None
    def __repr__(self) -> str: ...
    def __getitem__(self, key: str) -> Any: ...

class Schema:
    def __init__(self, bundle: Any) -> None: ...
    async def execute(
        self, query: str, variables: dict[str, Any] | None = None, state: Any = None
    ) -> OperationResult | SubscriptionStream: ...
    def as_sdl(self) -> str: ...

class SubscriptionStream:
    def __aiter__(self) -> AsyncIterator[OperationResult]: ...
    async def __anext__(self) -> OperationResult: ...
    async def aclose(self) -> None: ...

class Graph:
    def requests(self, name: str) -> bool: ...
    def peek(self, name: str) -> Graph: ...

class Field:
    def __init__(
        self,
        name: str,
        type_spec: TypeSpec,
        func: Any,
        needs_context: bool,
        is_async: bool,
        description: str | None = None,
        args: list[tuple[str, TypeSpec, Any | None]] | None = None,
    ) -> None: ...

class SubscriptionField:
    def __init__(
        self,
        name: str,
        type_spec: TypeSpec,
        func: Any,
        needs_context: bool,
        description: str | None = None,
        args: list[tuple[str, TypeSpec, Any | None]] | None = None,
    ) -> None: ...

class InputValue:
    def __init__(
        self,
        name: str,
        type_spec: TypeSpec,
        default_value: Any | None = None,
        description: str | None = None,
    ) -> None: ...

class Object:
    def __init__(
        self,
        name: str,
        description: str | None = None,
        fields: list[Field] | None = None,
    ) -> None: ...

class InputObject:
    def __init__(
        self,
        name: str,
        description: str | None = None,
        fields: list[InputValue] | None = None,
    ) -> None: ...

class Subscription:
    def __init__(
        self,
        name: str,
        description: str | None = None,
        fields: list[SubscriptionField] | None = None,
    ) -> None: ...
