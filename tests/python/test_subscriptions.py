"""Tests for subscriptions from README examples."""

import asyncio
from collections.abc import AsyncIterator
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class SubQuery:
    greeting: str = "Hello!"


@grommet.type
@dataclass
class Subscription:
    @grommet.field
    async def counter(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i


SCHEMA = grommet.Schema(query=SubQuery, subscription=Subscription)


def test_subscription_stream():
    async def run():
        stream = await SCHEMA.execute("subscription { counter(limit: 3) }")
        results = []
        async for result in stream:
            results.append(result.data)
        return results

    results = asyncio.run(run())
    assert results == [{"counter": 0}, {"counter": 1}, {"counter": 2}]
