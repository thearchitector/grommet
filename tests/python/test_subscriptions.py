import asyncio
from collections.abc import AsyncIterable, AsyncIterator
from dataclasses import dataclass

import pytest

import grommet as gm


@gm.type
@dataclass
class Query:
    ok: str = "ok"


@gm.type
@dataclass
class Subscription:
    @gm.field
    async def countdown(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i


async def test_subscription_streams_values() -> None:
    """Verifies subscriptions stream successive payloads from async iterators."""
    schema = gm.Schema(query=Query, subscription=Subscription)
    stream = await schema.execute(
        "subscription ($limit: Int!) { countdown(limit: $limit) }",
        variables={"limit": 3},
    )
    results = []
    async for payload in stream:
        results.append(payload.data["countdown"])

    assert results == [0, 1, 2]


async def test_subscription_aclose_stops_iteration() -> None:
    """Ensures closing a subscription stops further iteration."""
    schema = gm.Schema(query=Query, subscription=Subscription)
    stream = await schema.execute(
        "subscription ($limit: Int!) { countdown(limit: $limit) }",
        variables={"limit": 5},
    )
    first = await anext(stream)
    assert first.data["countdown"] == 0

    await stream.aclose()

    with pytest.raises(StopAsyncIteration):
        await anext(stream)


async def test_subscription_backpressure_serializes_anext() -> None:
    """Verifies subscription backpressure serializes concurrent anext calls."""
    queue: asyncio.Queue[int] = asyncio.Queue()

    @gm.type
    @dataclass
    class LocalQuery:
        ok: str = "ok"

    @gm.type
    @dataclass
    class LocalSubscription:
        @gm.field
        async def numbers(self) -> AsyncIterable[int]:
            for _ in range(2):
                value = await queue.get()
                yield value

    schema = gm.Schema(query=LocalQuery, subscription=LocalSubscription)
    stream = await schema.execute("subscription { numbers }")

    task1 = asyncio.ensure_future(anext(stream))
    await asyncio.sleep(0)
    assert not task1.done()

    task2 = asyncio.ensure_future(anext(stream))
    await asyncio.sleep(0)
    assert not task2.done()

    await queue.put(10)
    first = await task1
    assert first.data["numbers"] == 10

    await queue.put(20)
    second = await task2
    assert second.data["numbers"] == 20

    await stream.aclose()


async def test_subscription_stream_surfaces_errors() -> None:
    """Ensures subscription stream items include GraphQL errors when iteration fails."""

    @gm.type
    @dataclass
    class LocalQuery:
        ok: str = "ok"

    @gm.type
    @dataclass
    class LocalSubscription:
        @gm.field
        async def boom(self) -> AsyncIterator[int]:
            if False:
                yield 1
            raise ValueError("boom")

    schema = gm.Schema(query=LocalQuery, subscription=LocalSubscription)
    stream = await schema.execute("subscription { boom }")
    payload = await anext(stream)
    assert payload.errors
    await stream.aclose()
