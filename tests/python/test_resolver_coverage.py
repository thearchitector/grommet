from functools import partial
from typing import AsyncIterator

import pytest

from grommet.context import Context
from grommet.errors import GrommetTypeError
from grommet.metadata import TypeKind
from grommet.resolver import (
    _is_asyncgen_callable,
    _is_coroutine_callable,
    _resolver_arg_params,
    _resolver_name,
    _resolver_params,
    _wrap_resolver,
)


def test_wrap_resolver_missing_annotation_raises() -> None:
    """Ensures resolver wrapping fails when required annotations are missing."""

    async def resolver(self, value):  # type: ignore[no-untyped-def]
        return value

    with pytest.raises(GrommetTypeError):
        _wrap_resolver(resolver, kind=TypeKind.OBJECT, field_name="value")


async def test_wrap_resolver_with_args() -> None:
    """Verifies wrapped resolvers receive coerced args."""

    async def resolver(self, value: int) -> tuple:
        return (self, value)

    wrapper, arg_defs = _wrap_resolver(
        resolver, kind=TypeKind.OBJECT, field_name="value"
    )
    assert arg_defs[0]["name"] == "value"

    result = await wrapper("parent", None, value="3")
    assert result == ("parent", 3)


async def test_wrap_resolver_with_context() -> None:
    """Verifies wrapped resolvers receive Context when annotated."""

    async def resolver(self, ctx: Context, value: int) -> tuple:
        return (self, ctx, value)

    wrapper, arg_defs = _wrap_resolver(
        resolver, kind=TypeKind.OBJECT, field_name="value"
    )
    assert arg_defs[0]["name"] == "value"

    ctx_obj = object()
    result = await wrapper("parent", ctx_obj, value="3")
    assert result == ("parent", ctx_obj, 3)


async def test_wrap_resolver_without_args_uses_defaults() -> None:
    """Verifies wrapped resolvers use defaults when kwargs are omitted."""

    async def resolver(self, value: int = 5) -> tuple:
        return (self, value)

    wrapper, _ = _wrap_resolver(resolver, kind=TypeKind.OBJECT, field_name="value")
    result = await wrapper("parent", None)
    assert result == ("parent", 5)


def test_wrap_resolver_requires_async() -> None:
    """Ensures sync resolvers are rejected for non-subscription fields."""

    def resolver(self) -> int:
        return 1

    with pytest.raises(GrommetTypeError):
        _wrap_resolver(resolver, kind=TypeKind.OBJECT, field_name="value")


async def test_wrap_subscription_requires_async_iterator() -> None:
    """Ensures subscription resolvers must return async iterators."""

    async def resolver(self) -> int:
        return 3

    wrapper, _ = _wrap_resolver(
        resolver, kind=TypeKind.SUBSCRIPTION, field_name="ticks"
    )
    with pytest.raises(GrommetTypeError):
        await wrapper(None, None)


def test_resolver_params_handles_unhashable_callable() -> None:
    """Test _resolver_params handles callables that can't be cached."""

    class UnhashableCallable:
        __hash__ = None  # type: ignore[assignment]

        def __call__(self, x: int) -> int:
            return x

    callable_obj = UnhashableCallable()
    params = _resolver_params(callable_obj)
    assert len(params) == 1
    assert params[0].name == "x"


def test_resolver_arg_params_skips_self_and_context() -> None:
    """Verifies _resolver_arg_params excludes self and Context parameters."""

    async def resolver(self, ctx: Context[dict], value: int) -> int:
        return value

    params = _resolver_arg_params(resolver)
    assert len(params) == 1
    assert params[0].name == "value"


def test_resolver_arg_params_self_only() -> None:
    """Verifies _resolver_arg_params returns empty for self-only resolvers."""

    async def resolver(self) -> int:
        return 1

    params = _resolver_arg_params(resolver)
    assert len(params) == 0


def test_resolver_name_fallback_to_type_name() -> None:
    """Test _resolver_name falls back to type name when no __name__."""

    class CallableWithoutName:
        def __call__(self) -> None:
            pass

    obj = CallableWithoutName()
    delattr(obj.__class__, "__name__") if hasattr(obj, "__name__") else None
    name = _resolver_name(obj)
    assert name == "CallableWithoutName"


def test_resolver_name_uses_func_attr() -> None:
    """Test _resolver_name uses .func attribute when available."""

    async def my_resolver() -> int:
        return 42

    partial_resolver = partial(my_resolver)
    name = _resolver_name(partial_resolver)
    assert name == "my_resolver"


def test_is_coroutine_callable_with_func_attr() -> None:
    """Test _is_coroutine_callable checks .func attribute."""

    async def async_func() -> int:
        return 1

    partial_func = partial(async_func)
    assert _is_coroutine_callable(partial_func) is True

    def sync_func() -> int:
        return 1

    partial_sync = partial(sync_func)
    assert _is_coroutine_callable(partial_sync) is False


def test_is_asyncgen_callable_with_func_attr() -> None:
    """Test _is_asyncgen_callable checks .func attribute."""

    async def async_gen() -> AsyncIterator[int]:
        yield 1

    partial_gen = partial(async_gen)
    assert _is_asyncgen_callable(partial_gen) is True


def test_wrap_resolver_subscription_asyncgen_path() -> None:
    """Test _wrap_resolver handles subscription with async generator."""

    async def subscription_gen(self) -> AsyncIterator[int]:
        yield 1
        yield 2

    wrapper, arg_defs = _wrap_resolver(
        subscription_gen, kind=TypeKind.SUBSCRIPTION, field_name="events"
    )
    assert arg_defs == []


def test_wrap_resolver_subscription_coroutine_path() -> None:
    """Test _wrap_resolver handles subscription with coroutine returning iterator."""

    async def subscription_coro(self) -> AsyncIterator[int]:
        async def gen() -> AsyncIterator[int]:
            yield 1

        return gen()

    wrapper, arg_defs = _wrap_resolver(
        subscription_coro, kind=TypeKind.SUBSCRIPTION, field_name="events"
    )
    assert arg_defs == []


async def test_wrap_resolver_subscription_coroutine_returns_iterator() -> None:
    """Verifies subscription coroutine wrapper awaits and returns the iterator."""

    async def subscription_coro(self) -> AsyncIterator[int]:
        async def gen() -> AsyncIterator[int]:
            yield 42

        return gen()

    wrapper, _ = _wrap_resolver(
        subscription_coro, kind=TypeKind.SUBSCRIPTION, field_name="events"
    )
    result = await wrapper(None, None)
    assert hasattr(result, "__anext__")


def test_wrap_resolver_sync_subscription_rejected() -> None:
    """Ensures sync functions are rejected as subscription resolvers."""

    def sync_sub(self) -> int:  # type: ignore[return]
        return 1

    with pytest.raises(GrommetTypeError):
        _wrap_resolver(sync_sub, kind=TypeKind.SUBSCRIPTION, field_name="ticks")


def test_resolver_name_func_attr_no_name() -> None:
    """Verifies _resolver_name falls back to type name when func has no __name__."""

    class Wrapper:
        func = object()  # has .func but func has no __name__

        def __call__(self) -> None:
            pass

    name = _resolver_name(Wrapper())
    assert name == "Wrapper"
