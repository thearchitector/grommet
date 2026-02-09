"""Tests for resolver analysis: _has_await, _syncify, sync resolvers, and attrgetter data fields."""

import asyncio
from collections.abc import AsyncIterator
from dataclasses import dataclass

import pytest

import grommet
from grommet.metadata import TypeKind
from grommet.resolver import _analyze_resolver, _has_await, _syncify

# ---------------------------------------------------------------------------
# _has_await tests
# ---------------------------------------------------------------------------


async def _await_free(self):
    return 1


async def _with_await(self):
    await asyncio.sleep(0)
    return 1


async def _nested_await(self):
    async def inner():
        await asyncio.sleep(0)

    return inner


async def _with_async_for(self):
    async def gen():
        yield 1

    async for _ in gen():
        pass


async def _with_async_with(self):
    class CM:
        async def __aenter__(self):
            return self

        async def __aexit__(self, *a):
            pass

    async with CM():
        pass


def test_has_await_false_for_await_free():
    assert _has_await(_await_free) is False


def test_has_await_true_for_await():
    assert _has_await(_with_await) is True


def test_has_await_false_for_nested_await():
    assert _has_await(_nested_await) is False


def test_has_await_true_for_async_for():
    assert _has_await(_with_async_for) is True


def test_has_await_true_for_async_with():
    assert _has_await(_with_async_with) is True


def test_has_await_conservative_for_uninspectable():
    assert _has_await(print) is True


# ---------------------------------------------------------------------------
# _syncify tests
# ---------------------------------------------------------------------------


def test_syncify_returns_value():
    async def coro(x):
        return x * 2

    sync = _syncify(coro)
    assert sync(21) == 21 * 2


def test_syncify_preserves_name():
    async def my_resolver(self):
        return 1

    sync = _syncify(my_resolver)
    assert sync.__name__ == "my_resolver"
    assert sync.__qualname__ == my_resolver.__qualname__


def test_syncify_propagates_exception():
    async def bad(self):
        raise ValueError("boom")

    sync = _syncify(bad)
    with pytest.raises(ValueError, match="boom"):
        sync(None)


# ---------------------------------------------------------------------------
# _analyze_resolver: sync demotion and sync acceptance
# ---------------------------------------------------------------------------


def test_analyze_demotes_await_free_async():
    async def resolver(self) -> int:
        return 1

    result = _analyze_resolver(resolver, kind=TypeKind.OBJECT, field_name="x")
    assert result.is_async is False
    assert result.shape == "self_only"
    # func should be the syncified wrapper, not the original
    assert result.func is not resolver
    assert result.func(None) == 1


def test_analyze_keeps_async_with_await():
    async def resolver(self) -> int:
        await asyncio.sleep(0)
        return 1

    result = _analyze_resolver(resolver, kind=TypeKind.OBJECT, field_name="x")
    assert result.is_async is True


def test_analyze_accepts_sync_resolver():
    def resolver(self) -> int:
        return 42

    result = _analyze_resolver(resolver, kind=TypeKind.OBJECT, field_name="x")
    assert result.is_async is False
    assert result.func is resolver
    assert result.shape == "self_only"


def test_analyze_subscription_requires_async():
    def resolver(self) -> int:
        return 1

    with pytest.raises(TypeError):
        _analyze_resolver(resolver, kind=TypeKind.SUBSCRIPTION, field_name="x")


def test_analyze_subscription_keeps_async_gen():
    async def resolver(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i

    result = _analyze_resolver(resolver, kind=TypeKind.SUBSCRIPTION, field_name="x")
    assert result.is_async is True
    assert result.is_async_gen is True


# ---------------------------------------------------------------------------
# attrgetter integration via end-to-end schema execution
# ---------------------------------------------------------------------------


@grommet.type
@dataclass
class AttrgQuery:
    name: str = "Gromit"
    age: int = 5


def test_attrgetter_data_field_resolves():
    schema = grommet.Schema(query=AttrgQuery)
    result = asyncio.run(schema.execute("{ name age }"))
    assert result.data == {"name": "Gromit", "age": 5}


# ---------------------------------------------------------------------------
# Sync resolver end-to-end
# ---------------------------------------------------------------------------


@grommet.type
@dataclass
class SyncQuery:
    @grommet.field
    def computed(self) -> str:
        return "sync-value"


def test_sync_resolver_end_to_end():
    schema = grommet.Schema(query=SyncQuery)
    result = asyncio.run(schema.execute("{ computed }"))
    assert result.data == {"computed": "sync-value"}


@grommet.type
@dataclass
class DemotedQuery:
    @grommet.field
    async def computed(self) -> str:
        return "demoted-value"


def test_demoted_async_resolver_end_to_end():
    schema = grommet.Schema(query=DemotedQuery)
    result = asyncio.run(schema.execute("{ computed }"))
    assert result.data == {"computed": "demoted-value"}
