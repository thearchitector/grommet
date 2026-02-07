# pragma: no ai
from functools import cached_property
from typing import TYPE_CHECKING

from . import _core
from .plan import build_schema_graph

if TYPE_CHECKING:
    from collections.abc import AsyncIterator
    from typing import Any, Protocol

    class OperationResult(Protocol):
        """
        The result of a non-streaming operations. If there were errors during parsing,
        validation, or execution, they will be included in the `errors` list.
        """

        data: dict[str, "Any"]
        errors: list[dict[str, "Any"]] | None

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
    __slots__ = ("_schema",)

    def __init__(
        self,
        *,
        query: type,
        mutation: type | None = None,
        subscription: type | None = None,
    ) -> None:
        graph = build_schema_graph(
            query=query, mutation=mutation, subscription=subscription
        )
        self._schema = _core.Schema(graph)

    async def execute(
        self, query: str, variables: dict[str, "Any"] | None = None, state: "Any" = None
    ) -> "OperationResult | SubscriptionStream":
        """
        Execute the provided query using the given variables if present. An optional
        state may be provided, which will be shared with all resolvers in this
        execution that include a parameter of type `grommet.Context[<StateCls>]`,
        available under its `state` attribute.
        """
        return await self._schema.execute(query, variables, state)

    @cached_property
    def sdl(self) -> str:
        """Return the SDL for this schema."""
        return self._schema.as_sdl()
