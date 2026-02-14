"""Tests for resolver analysis: can_syncify/syncify, decorator-time analysis, and attrgetter data fields."""

import asyncio
from collections.abc import AsyncIterator
from dataclasses import dataclass

import pytest
from noaio import can_syncify, syncify

import grommet
from grommet.decorators import (
    _analyze_and_build_field,
    _analyze_and_build_subscription_field,
)

# __grommet_field_data__ tuple indices:
# (field_name, func, shape, arg_names, is_async, return_spec, description, arg_plans)
_FD_NAME = 0
_FD_FUNC = 1
_FD_SHAPE = 2
_FD_IS_ASYNC = 4

# ---------------------------------------------------------------------------
# can_syncify tests (replaces _has_await tests with noaio equivalents)
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


def test_can_syncify_true_for_await_free():
    assert can_syncify(_await_free) is True


def test_can_syncify_false_for_await():
    assert can_syncify(_with_await) is False


def test_can_syncify_true_for_nested_await():
    assert can_syncify(_nested_await) is True


def test_can_syncify_false_for_async_for():
    assert can_syncify(_with_async_for) is False


def test_can_syncify_false_for_async_with():
    assert can_syncify(_with_async_with) is False


# ---------------------------------------------------------------------------
# syncify tests
# ---------------------------------------------------------------------------


def test_syncify_returns_value():
    async def coro(x):
        return x * 2

    sync = syncify(coro)
    assert sync(21) == 21 * 2


def test_syncify_preserves_name():
    async def my_resolver(self):
        return 1

    sync = syncify(my_resolver)
    assert sync.__name__ == "my_resolver"
    assert sync.__qualname__ == my_resolver.__qualname__


def test_syncify_propagates_exception():
    async def bad(self):
        raise ValueError("boom")

    sync = syncify(bad)
    with pytest.raises(ValueError, match="boom"):
        sync(None)


# ---------------------------------------------------------------------------
# _analyze_and_build_field: sync demotion and sync acceptance
# ---------------------------------------------------------------------------


def test_analyze_demotes_await_free_async():
    async def resolver(self) -> int:
        return 1

    result = _analyze_and_build_field(resolver, field_name="x", description=None)
    data = result.__grommet_field_data__
    assert data[_FD_IS_ASYNC] is False
    assert data[_FD_SHAPE] == "self_only"
    # func should be the syncified wrapper, not the original
    assert data[_FD_FUNC] is not resolver
    assert data[_FD_FUNC](None) == 1


def test_analyze_keeps_async_with_await():
    async def resolver(self) -> int:
        await asyncio.sleep(0)
        return 1

    result = _analyze_and_build_field(resolver, field_name="x", description=None)
    assert result.__grommet_field_data__[_FD_IS_ASYNC] is True


def test_analyze_accepts_sync_resolver():
    def resolver(self) -> int:
        return 42

    result = _analyze_and_build_field(resolver, field_name="x", description=None)
    data = result.__grommet_field_data__
    assert data[_FD_IS_ASYNC] is False
    assert data[_FD_FUNC] is resolver
    assert data[_FD_SHAPE] == "self_only"


def test_analyze_subscription_requires_async():
    def resolver(self) -> int:
        return 1

    with pytest.raises(TypeError):
        _analyze_and_build_subscription_field(
            resolver, field_name="x", description=None
        )


def test_analyze_subscription_keeps_async_gen():
    async def resolver(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i

    result = _analyze_and_build_subscription_field(
        resolver, field_name="x", description=None
    )
    # __grommet_sub_field_data__ tuple: (name, func, shape, arg_names, type_spec, desc, arg_plans)
    assert result.__grommet_sub_field_data__[2] == "self_and_args"


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
