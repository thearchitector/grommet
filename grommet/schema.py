# pragma: no ai
from functools import cached_property
from typing import TYPE_CHECKING

from . import _core
from .plan import build_schema_plan

if TYPE_CHECKING:
    from collections.abc import AsyncIterator
    from typing import Any, Protocol

    from .types import RootType

    class OperationResult(Protocol):
        """The result of a non-streaming operations."""

        data: dict[str, "Any"]
        errors: list[dict[str, "Any"]] | None
        extensions: dict[str, "Any"] | None

    class SubscriptionStream(Protocol):
        """
        A streaming result from a subscription operation.

        Use as an async iterator:

        ```py
        stream = await schema.execute("subscription { counter(limit: 3) }")
        async for result in stream:
            print(result.data)
        ```

        or manually:

        ```py
        stream = await schema.execute("subscription { counter(limit: 3) }")
        result = await anext(stream)
        print(result.data)
        await stream.aclose()
        ```

        If you're iterating manually, remember to call `aclose` when you're done!
        """

        def __aiter__(self) -> AsyncIterator[OperationResult]: ...

        async def __anext__(self) -> OperationResult: ...

        async def aclose(self) -> None: ...


class Schema:
    __slots__ = ("_core",)

    def __init__(
        self,
        *,
        query: "RootType",
        mutation: "RootType | None" = None,
        subscription: "RootType | None" = None,
    ) -> None:
        plan = build_schema_plan(
            query=query, mutation=mutation, subscription=subscription
        )
        self._core = _core.Schema(plan)

    async def execute(
        self, query: str, variables: dict[str, "Any"] | None = None, state: "Any" = None
    ) -> "OperationResult | SubscriptionStream":
        """
        Execute the provided query using the given variables if present. An optional
        state may be provided, which will be shared with all resolvers in this
        execution that include a parameter of type `grommet.Context[<StateCls>]`,
        available under its `state` attribute.
        """
        return await self._core.execute(query, variables, state)

    @cached_property
    def as_sdl(self) -> str:
        """Return the SDL for this schema."""
        return self._core.as_sdl()
