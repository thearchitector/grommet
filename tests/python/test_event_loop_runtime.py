"""Tests asyncio-native execution without Tokio runtime thread hops."""

import asyncio
import threading
from collections.abc import AsyncIterator
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(self) -> str:
        await asyncio.sleep(0)
        return "Hello!"


@grommet.type
@dataclass
class Subscription:
    @grommet.subscription
    async def counter(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            await asyncio.sleep(0)
            yield i


SCHEMA = grommet.Schema(query=Query, subscription=Subscription)


def test_no_tokio_threads_spawned():
    async def run():
        before = {thread.name for thread in threading.enumerate()}

        result = await SCHEMA.execute("{ greeting }")
        assert result.data == {"greeting": "Hello!"}

        stream = await SCHEMA.execute("subscription { counter(limit: 2) }")
        rows = []
        async for item in stream:
            rows.append(item.data)
        assert rows == [{"counter": 0}, {"counter": 1}]

        after = {thread.name for thread in threading.enumerate()}
        new_threads = after - before

        assert not any("tokio" in name.lower() for name in after)
        assert not any("tokio" in name.lower() for name in new_threads)

    asyncio.run(run())
