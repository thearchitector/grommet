"""Tests for resolver compilation and decorator-time schema metadata."""

import asyncio
import dataclasses
import inspect
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Annotated

import pytest
from noaio import can_syncify, syncify

import grommet
import grommet.annotations as annotations_module
from grommet import _core
from grommet._compiled import (
    COMPILED_RESOLVER_ATTR,
    COMPILED_TYPE_ATTR,
    CompiledResolverField,
    CompiledType,
)


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


def test_field_decorator_compiles_resolver_metadata():
    async def resolver(self) -> int:
        return 1

    result = grommet.field(resolver)
    compiled = getattr(result, COMPILED_RESOLVER_ATTR)

    assert isinstance(compiled, CompiledResolverField)
    assert compiled.kind == "field"
    assert compiled.needs_context is False
    assert compiled.is_async is False
    assert not hasattr(compiled, "shape")
    assert not hasattr(compiled, "arg_names")
    assert compiled.func is not resolver
    assert compiled.func(None, None, {}) == 1


def test_field_decorator_keeps_async_with_await():
    async def resolver(self) -> int:
        await asyncio.sleep(0)
        return 1

    result = grommet.field(resolver)
    compiled = getattr(result, COMPILED_RESOLVER_ATTR)

    assert isinstance(compiled, CompiledResolverField)
    assert compiled.kind == "field"
    assert compiled.is_async is True


def test_field_decorator_accepts_sync_resolver():
    expected = 42

    def resolver(self) -> int:
        return expected

    result = grommet.field(resolver)
    compiled = getattr(result, COMPILED_RESOLVER_ATTR)

    assert isinstance(compiled, CompiledResolverField)
    assert compiled.kind == "field"
    assert compiled.is_async is False
    assert compiled.func is not resolver
    assert compiled.func(None, None, {}) == expected


def test_subscription_decorator_requires_async_generator():
    def resolver(self) -> int:
        return 1

    with pytest.raises(TypeError):
        grommet.subscription(resolver)


def test_subscription_decorator_compiles_resolver_metadata():
    async def resolver(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i

    result = grommet.subscription(resolver)
    compiled = getattr(result, COMPILED_RESOLVER_ATTR)

    assert isinstance(compiled, CompiledResolverField)
    assert compiled.kind == "subscription"
    assert compiled.needs_context is False
    assert compiled.is_async is True
    assert not hasattr(compiled, "shape")
    assert not hasattr(compiled, "arg_names")


def test_compiled_adapter_handles_all_call_modes():
    seen: list[tuple[str, object, object, object]] = []

    @grommet.field
    async def self_only(self) -> str:
        seen.append(("self_only", self, None, None))
        return "ok"

    @grommet.field
    async def self_and_context(
        self, context: Annotated[object, grommet.Context]
    ) -> str:
        seen.append(("self_and_context", self, context, None))
        return "ok"

    @grommet.field
    async def self_and_args(self, value: int) -> str:
        seen.append(("self_and_args", self, None, value))
        return "ok"

    @grommet.field
    async def self_context_and_args(
        self, context: Annotated[object, grommet.Context], value: int
    ) -> str:
        seen.append(("self_context_and_args", self, context, value))
        return "ok"

    self_only_compiled = getattr(self_only, COMPILED_RESOLVER_ATTR)
    self_only_compiled.func("parent", object(), {"value": 1})

    self_and_context_compiled = getattr(self_and_context, COMPILED_RESOLVER_ATTR)
    context_obj = object()
    self_and_context_compiled.func("parent", context_obj, {"value": 1})

    self_and_args_compiled = getattr(self_and_args, COMPILED_RESOLVER_ATTR)
    self_and_args_compiled.func("parent", object(), {"value": 3})

    self_context_and_args_compiled = getattr(
        self_context_and_args, COMPILED_RESOLVER_ATTR
    )
    self_context_and_args_compiled.func("parent", context_obj, {"value": 7})

    assert seen == [
        ("self_only", "parent", None, None),
        ("self_and_context", "parent", context_obj, None),
        ("self_and_args", "parent", None, 3),
        ("self_context_and_args", "parent", context_obj, 7),
    ]


def test_compiled_adapter_context_order_is_irrelevant():
    seen: list[tuple[object, int, object, str]] = []

    @grommet.field
    async def resolver(
        self, value: int, context: Annotated[object, grommet.Context], label: str
    ) -> str:
        seen.append((self, value, context, label))
        return "ok"

    compiled = getattr(resolver, COMPILED_RESOLVER_ATTR)
    context_obj = object()
    compiled.func("parent", context_obj, {"value": 3, "label": "x"})
    assert seen == [("parent", 3, context_obj, "x")]


def test_compiled_adapter_injects_multiple_context_params():
    marker = object()
    expected_value = 7

    @grommet.field
    def resolver(
        self,
        first: Annotated[object, grommet.Context],
        value: int,
        second: Annotated[object, grommet.Context],
    ) -> bool:
        return (
            self == "parent"
            and value == expected_value
            and first is marker
            and second is marker
        )

    compiled = getattr(resolver, COMPILED_RESOLVER_ATTR)
    assert compiled.func("parent", marker, {"value": expected_value}) is True


def test_bare_context_annotation_is_rejected():
    def resolver(self, context: grommet.Context) -> str:
        return "hello"

    with pytest.raises(TypeError, match=r"Annotated\[T, grommet\.Context\]"):
        grommet.field(resolver)


@grommet.type
@dataclass
class AttrgQuery:
    name: str = "Gromit"
    age: int = 5


def test_attrgetter_data_field_resolves():
    schema = grommet.Schema(query=AttrgQuery)
    result = asyncio.run(schema.execute("{ name age }"))
    assert result.data == {"name": "Gromit", "age": 5}


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


@grommet.type
@dataclass
class CompiledQuery:
    @grommet.field
    async def greeting(self, name: str) -> str:
        return f"Hello {name}"


def test_type_decorator_compiles_type_metadata():
    compiled = getattr(CompiledQuery, COMPILED_TYPE_ATTR)

    assert isinstance(compiled, CompiledType)
    assert compiled.meta.name == "CompiledQuery"
    assert len(compiled.object_fields) == 1


def test_schema_build_does_not_reinspect_resolvers(monkeypatch: pytest.MonkeyPatch):
    def boom(*_args, **_kwargs):
        raise RuntimeError("inspect.signature should not run during schema build")

    monkeypatch.setattr(inspect, "signature", boom)

    schema = grommet.Schema(query=CompiledQuery)
    result = asyncio.run(schema.execute('{ greeting(name: "Gromit") }'))
    assert result.data == {"greeting": "Hello Gromit"}


def test_schema_build_does_not_recompile_annotations(monkeypatch: pytest.MonkeyPatch):
    def boom(*_args, **_kwargs):
        raise RuntimeError(
            "_type_spec_from_annotation should not run during schema build"
        )

    monkeypatch.setattr(annotations_module, "_type_spec_from_annotation", boom)

    schema = grommet.Schema(query=CompiledQuery)
    result = asyncio.run(schema.execute('{ greeting(name: "Gromit") }'))
    assert result.data == {"greeting": "Hello Gromit"}


def test_same_compiled_type_supports_multiple_schemas():
    schema1 = grommet.Schema(query=CompiledQuery)
    schema2 = grommet.Schema(query=CompiledQuery)

    result1 = asyncio.run(schema1.execute('{ greeting(name: "A") }'))
    result2 = asyncio.run(schema2.execute('{ greeting(name: "B") }'))

    assert result1.data == {"greeting": "Hello A"}
    assert result2.data == {"greeting": "Hello B"}


def test_core_schema_accepts_compiled_type_metadata_bundle():
    @dataclass
    class DirectBundle:
        query: str = "CompiledQuery"
        mutation: str | None = None
        subscription: str | None = None
        types: list[object] = dataclasses.field(
            default_factory=lambda: [getattr(CompiledQuery, COMPILED_TYPE_ATTR)]
        )

    schema = _core.Schema(DirectBundle())
    result = asyncio.run(schema.execute('{ greeting(name: "Gromit") }', None, None))
    assert result.data == {"greeting": "Hello Gromit"}


def test_core_schema_rejects_unknown_registration_type():
    @dataclass
    class BadBundle:
        query: str = "Query"
        mutation: str | None = None
        subscription: str | None = None
        types: list[object] = dataclasses.field(default_factory=lambda: [object()])

    with pytest.raises(TypeError, match="unsupported type registration object"):
        _core.Schema(BadBundle())


def test_core_builder_classes_are_not_exposed():
    for name in (
        "Field",
        "SubscriptionField",
        "InputValue",
        "Object",
        "InputObject",
        "Subscription",
    ):
        assert not hasattr(_core, name)


@grommet.type
@dataclass
class ListArgQuery:
    @grommet.field
    async def total(self, values: list[int]) -> int:
        return sum(values)


def test_tuple_variable_for_list_is_rejected():
    schema = grommet.Schema(query=ListArgQuery)
    query = "query ($values: [Int!]!) { total(values: $values) }"
    with pytest.raises(TypeError, match="Unsupported value type"):
        asyncio.run(schema.execute(query, variables={"values": (1, 2)}))


@grommet.type
@dataclass
class RootWithoutDefault:
    greeting: str


def test_root_data_field_without_default_fails_fast():
    with pytest.raises(TypeError, match="must declare a default value"):
        grommet.Schema(query=RootWithoutDefault)


def test_input_type_with_resolver_is_rejected():
    with pytest.raises(TypeError, match="Input types cannot declare field resolvers"):

        @grommet.input
        @dataclass
        class InputWithResolver:
            @grommet.field
            async def value(self) -> str:
                return "nope"


def test_type_cannot_mix_field_and_subscription_decorators():
    with pytest.raises(
        TypeError, match="A type cannot mix @field and @subscription decorators"
    ):

        @grommet.type
        @dataclass
        class InvalidMixedType:
            @grommet.field
            async def greeting(self) -> str:
                return "hello"

            @grommet.subscription
            async def ticks(self) -> AsyncIterator[int]:
                yield 1


def test_subscription_type_cannot_declare_data_fields():
    with pytest.raises(
        TypeError, match="Subscription types cannot declare data fields"
    ):

        @grommet.type
        @dataclass
        class InvalidSubscriptionType:
            count: int = 1

            @grommet.subscription
            async def ticks(self) -> AsyncIterator[int]:
                yield self.count
