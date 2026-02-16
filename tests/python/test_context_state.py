"""Tests for context state from README examples."""

import asyncio
from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet


@dataclass
class MyState:
    request_id: str


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(self, context: Annotated[MyState, grommet.Context]) -> str:
        return f"Hello request {context.request_id}!"


SCHEMA = grommet.Schema(query=Query)


def test_context_state():
    result = asyncio.run(
        SCHEMA.execute("{ greeting }", context=MyState(request_id="123"))
    )
    assert result.data == {"greeting": "Hello request 123!"}


def test_state_kwarg_is_rejected():
    with pytest.raises(TypeError, match="state"):
        asyncio.run(SCHEMA.execute("{ greeting }", state=MyState(request_id="123")))


@grommet.type
@dataclass
class MissingContextQuery:
    @grommet.field
    def missing(self, context: Annotated[object | None, grommet.Context]) -> bool:
        return context is None


def test_missing_context_injects_none():
    schema = grommet.Schema(query=MissingContextQuery)
    result = asyncio.run(schema.execute("{ missing }"))
    assert result.data == {"missing": True}
