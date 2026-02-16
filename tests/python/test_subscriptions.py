"""Tests for subscriptions from README examples."""

import asyncio
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Annotated

import grommet


@grommet.type
@dataclass
class SubQuery:
    greeting: str = "Hello!"


@grommet.type
@dataclass
class Subscription:
    @grommet.subscription
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


def test_subscription_context_injection():
    @grommet.type
    @dataclass
    class ContextSubscription:
        @grommet.subscription
        async def counter(
            self, limit: int, context: Annotated[dict[str, str], grommet.Context]
        ) -> AsyncIterator[str]:
            for i in range(limit):
                yield f"{context['request_id']}:{i}"

    schema = grommet.Schema(query=SubQuery, subscription=ContextSubscription)

    async def run():
        stream = await schema.execute(
            "subscription { counter(limit: 3) }", context={"request_id": "123"}
        )
        results: list[dict[str, str]] = []
        async for result in stream:
            data = result.data
            assert data is not None
            results.append(data)
        return results

    results = asyncio.run(run())
    assert results == [{"counter": "123:0"}, {"counter": "123:1"}, {"counter": "123:2"}]
