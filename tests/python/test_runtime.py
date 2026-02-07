import asyncio
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Query:
    @gm.field
    async def ping(self) -> str:
        return "pong"


async def test_configure_runtime_allows_execution() -> None:
    """
    Verifies runtime configuration enables schema execution.
    """
    assert gm.configure_runtime(use_current_thread=True)
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ ping }")

    assert result.data["ping"] == "pong"


async def test_nested_event_loop_execution() -> None:
    """
    Ensures schema execution works within nested async scheduling.
    """
    schema = gm.Schema(query=Query)

    async def run_query() -> str:
        payload = await schema.execute("{ ping }")
        value = payload.data["ping"]
        assert isinstance(value, str)
        return value

    results = list(await asyncio.gather(run_query(), run_query()))
    assert results == ["pong", "pong"]
