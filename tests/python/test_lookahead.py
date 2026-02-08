"""Tests for lookahead context from README examples."""

import asyncio
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class SubObject:
    @grommet.field
    async def b(self) -> str:
        return "foo"


@grommet.type
@dataclass
class Object:
    @grommet.field
    async def a(self) -> int:
        return 1

    @grommet.field
    async def sub(self) -> SubObject:
        return SubObject()


results: list[dict[str, bool]] = []


@grommet.type
@dataclass
class LookaheadQuery:
    @grommet.field
    async def obj(self, context: grommet.Context) -> Object:
        results.append({
            "requests_a": context.field("a").exists(),
            "requests_b": context.look_ahead().field("sub").field("b").exists(),
        })
        return Object()


SCHEMA = grommet.Schema(query=LookaheadQuery)


def test_lookahead_a_only():
    results.clear()
    asyncio.run(SCHEMA.execute("{ obj { a } }"))
    assert len(results) == 1
    assert results[0]["requests_a"] is True
    assert results[0]["requests_b"] is False


def test_lookahead_sub_b_only():
    results.clear()
    asyncio.run(SCHEMA.execute("{ obj { sub { b } } }"))
    assert len(results) == 1
    assert results[0]["requests_a"] is False
    assert results[0]["requests_b"] is True
