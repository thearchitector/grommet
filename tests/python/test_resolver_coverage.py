from functools import partial
from typing import AsyncIterator

import pytest

from grommet.errors import GrommetTypeError
from grommet.info import Info
from grommet.metadata import TypeKind
from grommet.resolver import (
    _is_asyncgen_callable,
    _is_coroutine_callable,
    _normalize_info,
    _resolver_name,
    _resolver_params,
    _wrap_resolver,
)


def test_normalize_info_variants() -> None:
    """
    Verifies info normalization accepts Info, mapping, and object variants.
    """
    info = Info(field_name="field", context="ctx", root="root")
    assert _normalize_info(info) is info

    info_dict = _normalize_info({"field_name": "name", "context": 1, "root": 2})
    assert info_dict.field_name == "name"
    assert info_dict.context == 1
    assert info_dict.root == 2

    class Dummy:
        field_name = "dummy"
        context = "ctx"
        root = "root"

    info_obj = _normalize_info(Dummy())
    assert info_obj.field_name == "dummy"
    assert info_obj.context == "ctx"
    assert info_obj.root == "root"


def test_wrap_resolver_missing_annotation_raises() -> None:
    """
    Ensures resolver wrapping fails when required annotations are missing.
    """

    async def resolver(parent, info, value):  # type: ignore[no-untyped-def]
        return value

    with pytest.raises(GrommetTypeError):
        _wrap_resolver(resolver, kind=TypeKind.OBJECT, field_name="value")


@pytest.mark.anyio
async def test_wrap_resolver_info_context_root() -> None:
    """
    Verifies wrapped resolvers receive coerced args and info/context/root.
    """

    async def resolver(parent, info, context, root, value: int) -> tuple:
        return (parent, info.field_name, context, root, value)

    wrapper, arg_defs = _wrap_resolver(
        resolver, kind=TypeKind.OBJECT, field_name="value"
    )
    assert arg_defs[0]["name"] == "value"

    result = await wrapper(
        "parent", {"field_name": "f", "context": 1, "root": 2}, value="3"
    )
    assert result == ("parent", "f", 1, 2, 3)


@pytest.mark.anyio
async def test_wrap_resolver_without_info_and_missing_kwargs() -> None:
    """
    Verifies wrapped resolvers use defaults when info kwargs are omitted.
    """

    async def resolver(parent, value: int = 5) -> tuple:
        return (parent, value)

    wrapper, _ = _wrap_resolver(resolver, kind=TypeKind.OBJECT, field_name="value")
    result = await wrapper("parent", {"field_name": "ignored"})
    assert result == ("parent", 5)


def test_wrap_resolver_requires_async() -> None:
    """
    Ensures sync resolvers are rejected for non-subscription fields.
    """

    def resolver(parent, info) -> int:
        return 1

    with pytest.raises(GrommetTypeError):
        _wrap_resolver(resolver, kind=TypeKind.OBJECT, field_name="value")


@pytest.mark.anyio
async def test_wrap_subscription_requires_async_iterator() -> None:
    """
    Ensures subscription resolvers must return async iterators.
    """

    async def resolver(parent, info) -> int:
        return 3

    wrapper, _ = _wrap_resolver(
        resolver, kind=TypeKind.SUBSCRIPTION, field_name="ticks"
    )
    with pytest.raises(GrommetTypeError):
        await wrapper("parent", {"field_name": "ticks"})


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

    async def subscription_gen(parent: None) -> AsyncIterator[int]:
        yield 1
        yield 2

    wrapper, arg_defs = _wrap_resolver(
        subscription_gen, kind=TypeKind.SUBSCRIPTION, field_name="events"
    )
    assert arg_defs == []


def test_wrap_resolver_subscription_coroutine_path() -> None:
    """Test _wrap_resolver handles subscription with coroutine returning iterator."""

    async def subscription_coro(parent: None) -> AsyncIterator[int]:
        async def gen() -> AsyncIterator[int]:
            yield 1

        return gen()

    wrapper, arg_defs = _wrap_resolver(
        subscription_coro, kind=TypeKind.SUBSCRIPTION, field_name="events"
    )
    assert arg_defs == []


@pytest.mark.anyio
async def test_wrap_resolver_subscription_coroutine_returns_iterator() -> None:
    """Verifies subscription coroutine wrapper awaits and returns the iterator."""

    async def subscription_coro(parent: None) -> AsyncIterator[int]:
        async def gen() -> AsyncIterator[int]:
            yield 42

        return gen()

    wrapper, _ = _wrap_resolver(
        subscription_coro, kind=TypeKind.SUBSCRIPTION, field_name="events"
    )
    result = await wrapper(None, {"field_name": "events"})
    assert hasattr(result, "__anext__")


def test_wrap_resolver_sync_subscription_rejected() -> None:
    """Ensures sync functions are rejected as subscription resolvers."""

    def sync_sub(parent: None) -> int:  # type: ignore[return]
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
