"""Tests for the Graph lookahead API (context.graph.requests / context.graph.peek)."""

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
            "requests_a": context.graph.requests("a"),
            "requests_b_via_sub": context.graph.peek("sub").requests("b"),
        })
        return Object()


SCHEMA = grommet.Schema(query=LookaheadQuery)


def test_requests_child_present():
    """graph.requests('a') is True when 'a' is selected inside obj."""
    results.clear()
    asyncio.run(SCHEMA.execute("{ obj { a } }"))
    assert len(results) == 1
    assert results[0]["requests_a"] is True
    assert results[0]["requests_b_via_sub"] is False


def test_requests_child_absent():
    """graph.requests('a') is False when only sub is selected."""
    results.clear()
    asyncio.run(SCHEMA.execute("{ obj { sub { b } } }"))
    assert len(results) == 1
    assert results[0]["requests_a"] is False
    assert results[0]["requests_b_via_sub"] is True


def test_peek_nested():
    """graph.peek('sub').requests('b') checks a child's subgraph."""
    results.clear()
    asyncio.run(SCHEMA.execute("{ obj { a sub { b } } }"))
    assert len(results) == 1
    assert results[0]["requests_a"] is True
    assert results[0]["requests_b_via_sub"] is True


def test_peek_missing_returns_sentinel():
    """Peeking into a non-requested field returns a MISSING graph (always False)."""
    results.clear()
    asyncio.run(SCHEMA.execute("{ obj { a } }"))
    assert len(results) == 1
    assert results[0]["requests_b_via_sub"] is False
