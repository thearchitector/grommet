import asyncio
from collections.abc import AsyncIterable, AsyncIterator
from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class Query:
    ok: str = "ok"


@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def countdown(parent: "Any", info: "Any", limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i


@pytest.mark.anyio
async def test_subscription_streams_values() -> None:
    schema = gm.Schema(query=Query, subscription=Subscription)
    stream = schema.subscribe(
        "subscription ($limit: Int!) { countdown(limit: $limit) }",
        variables={"limit": 3},
    )
    results = []
    async for payload in stream:
        results.append(payload["data"]["countdown"])

    assert results == [0, 1, 2]


@pytest.mark.anyio
async def test_subscription_aclose_stops_iteration() -> None:
    schema = gm.Schema(query=Query, subscription=Subscription)
    stream = schema.subscribe(
        "subscription ($limit: Int!) { countdown(limit: $limit) }",
        variables={"limit": 5},
    )
    first = await anext(stream)
    assert first["data"]["countdown"] == 0

    await stream.aclose()

    with pytest.raises(StopAsyncIteration):
        await anext(stream)


@pytest.mark.anyio
async def test_subscription_backpressure_serializes_anext() -> None:
    queue: asyncio.Queue[int] = asyncio.Queue()

    @gm.type
    @dataclass
    class LocalQuery:
        ok: str = "ok"

    @gm.type
    @dataclass
    class LocalSubscription:
        @gm.field
        @staticmethod
        async def numbers(parent: "Any", info: "Any") -> AsyncIterable[int]:
            for _ in range(2):
                value = await queue.get()
                yield value

    schema = gm.Schema(query=LocalQuery, subscription=LocalSubscription)
    stream = schema.subscribe("subscription { numbers }")

    task1 = asyncio.ensure_future(anext(stream))
    await asyncio.sleep(0)
    assert not task1.done()

    task2 = asyncio.ensure_future(anext(stream))
    await asyncio.sleep(0)
    assert not task2.done()

    await queue.put(10)
    first = await task1
    assert first["data"]["numbers"] == 10

    await queue.put(20)
    second = await task2
    assert second["data"]["numbers"] == 20

    await stream.aclose()
