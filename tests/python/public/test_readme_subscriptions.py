"""Public contract tests for README subscription examples."""

from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet


@grommet.type
@dataclass
class Query:
    greeting: str = "Hello!"


@grommet.type
@dataclass
class Subscription:
    @grommet.subscription
    async def counter(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i


@grommet.type
@dataclass
class ContextSubscription:
    @grommet.subscription
    async def counter(
        self, limit: int, context: Annotated[dict[str, str], grommet.Context]
    ) -> AsyncIterator[str]:
        for i in range(limit):
            yield f"{context['request_id']}:{i}"


async def test_readme_subscription_streams_results_via_async_for(collect_stream):
    """Consumes a subscription stream with async-for and validates emitted payloads."""
    schema = grommet.Schema(query=Query, subscription=Subscription)
    stream = await schema.execute("subscription { counter(limit: 3) }")
    rows = await collect_stream(stream)
    assert rows == [{"counter": 0}, {"counter": 1}, {"counter": 2}]


async def test_readme_subscription_supports_manual_iteration_and_close():
    """Consumes one item manually, closes the stream, and verifies termination semantics."""
    schema = grommet.Schema(query=Query, subscription=Subscription)
    stream = await schema.execute("subscription { counter(limit: 2) }")

    first = await anext(stream)
    assert first.data == {"counter": 0}

    await stream.aclose()
    with pytest.raises(StopAsyncIteration):
        await anext(stream)


async def test_readme_subscription_context_is_injected(collect_stream):
    """Verifies context injection works for subscription resolvers."""
    schema = grommet.Schema(query=Query, subscription=ContextSubscription)
    stream = await schema.execute(
        "subscription { counter(limit: 3) }", context={"request_id": "123"}
    )
    rows = await collect_stream(stream)
    assert rows == [{"counter": "123:0"}, {"counter": "123:1"}, {"counter": "123:2"}]
