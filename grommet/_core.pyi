from collections.abc import AsyncIterator, Callable
from typing import Any

class SubscriptionStream(AsyncIterator[dict[str, Any]]):
    async def __anext__(self) -> dict[str, Any]: ...
    async def aclose(self) -> None: ...

class Schema:
    def __init__(
        self,
        definition: dict[str, Any],
        resolvers: dict[str, Callable[..., Any]],
        scalar_bindings: list[dict[str, Any]],
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
