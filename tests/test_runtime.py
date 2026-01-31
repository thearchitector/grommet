import asyncio
from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def ping(parent: "Any", info: "Any") -> str:
        return "pong"


@pytest.mark.anyio
async def test_configure_runtime_allows_execution() -> None:
    assert gm.configure_runtime(use_current_thread=True)
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ ping }")

    assert result["data"]["ping"] == "pong"


@pytest.mark.anyio
async def test_nested_event_loop_execution() -> None:
    schema = gm.Schema(query=Query)

    async def run_query() -> str:
        payload = await schema.execute("{ ping }")
        value = payload["data"]["ping"]
        assert isinstance(value, str)
        return value

    results = list(await asyncio.gather(run_query(), run_query()))
    assert results == ["pong", "pong"]
