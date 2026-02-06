from builtins import type as pytype
from typing import TYPE_CHECKING

from . import _core
from .errors import schema_requires_query
from .plan import build_schema_plan

if TYPE_CHECKING:
    from collections.abc import AsyncIterator
    from typing import Any, Protocol

    class SubscriptionStream(Protocol):
        def __aiter__(self) -> AsyncIterator[dict[str, Any]]: ...

        async def __anext__(self) -> dict[str, Any]: ...

        async def aclose(self) -> None: ...


class Schema:
    """Builds and executes a GraphQL schema."""

    def __init__(
        self,
        *,
        query: pytype,
        mutation: pytype | None = None,
        subscription: pytype | None = None,
    ) -> None:
        if query is None:
            raise schema_requires_query()
        plan = build_schema_plan(
            query=query, mutation=mutation, subscription=subscription
        )
        self._core = _core.Schema(plan)

    async def execute(
        self,
        query: str,
        variables: dict[str, "Any"] | None = None,
        root: "Any | None" = None,
        context: "Any | None" = None,
    ) -> dict[str, "Any"]:
        return await self._core.execute(query, variables, root, context)

    def subscribe(
        self,
        query: str,
        variables: dict[str, "Any"] | None = None,
        root: "Any | None" = None,
        context: "Any | None" = None,
    ) -> "SubscriptionStream":
        return self._core.subscribe(query, variables, root, context)

    def sdl(self) -> str:
        return self._core.sdl()

    def __repr__(self) -> str:
        return f"Schema(query={pytype(self._core).__name__})"
