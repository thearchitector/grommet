"""Tests for context state from README examples."""

import asyncio
from dataclasses import dataclass

import grommet


@dataclass
class MyState:
    request_id: str


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(self, context: grommet.Context[MyState]) -> str:
        return f"Hello request {context.state.request_id}!"


SCHEMA = grommet.Schema(query=Query)


def test_context_state():
    result = asyncio.run(
        SCHEMA.execute("{ greeting }", state=MyState(request_id="123"))
    )
    assert result.data == {"greeting": "Hello request 123!"}
