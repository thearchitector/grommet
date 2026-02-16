"""Targeted branch coverage tests for decorator and compiler internals."""

import dataclasses
import inspect
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet
from grommet._compiled import COMPILED_RESOLVER_ATTR
from grommet._resolver_compiler import (
    _build_arg_info,
    _collect_refs,
    compile_resolver_field,
)
from grommet._type_compiler import (
    _data_field_resolver,
    _get_annotated_field_meta,
    _implemented_interfaces,
    _resolve_data_field_default,
)


def test_type_decorator_requires_dataclasses():
    """Rejects @grommet.type usage on classes that are not dataclasses."""

    class Plain:
        pass

    with pytest.raises(TypeError, match="requires an explicit dataclass"):
        grommet.type(Plain)


def test_field_decorator_rejects_staticmethod_and_classmethod_resolvers():
    """Rejects resolver descriptors that are not instance methods."""

    async def resolver(self) -> int:
        return 1

    with pytest.raises(TypeError, match="instance methods"):
        grommet.field(staticmethod(resolver))

    with pytest.raises(TypeError, match="instance methods"):
        grommet.field(classmethod(resolver))


def test_field_and_subscription_decorators_require_callables():
    """Rejects non-callable targets for resolver decorators."""
    with pytest.raises(TypeError, match="expects a callable"):
        grommet.field(42)

    with pytest.raises(TypeError, match="expects a callable"):
        grommet.subscription(42)


def test_compile_resolver_field_rejects_missing_parameter_annotations():
    """Raises when resolver parameters are missing required type annotations."""

    def resolver(self, value) -> int:
        return value

    with pytest.raises(TypeError, match="missing annotation"):
        compile_resolver_field(
            resolver, field_name="value", description=None, kind="field"
        )


def test_compile_resolver_field_rejects_missing_return_annotation():
    """Raises when resolver return type annotations are absent."""

    def resolver(self, value: int):
        return value

    with pytest.raises(TypeError, match="missing annotation for 'return'"):
        compile_resolver_field(
            resolver, field_name="value", description=None, kind="field"
        )


def test_compile_resolver_field_rejects_bare_context_annotations():
    """Rejects bare grommet.Context annotations without Annotated wrapping."""

    def resolver(self, context: grommet.Context) -> str:
        return "hello"

    with pytest.raises(TypeError, match=r"Annotated\[T, grommet\.Context\]"):
        compile_resolver_field(
            resolver, field_name="value", description=None, kind="field"
        )


def test_compile_subscription_field_requires_async_generator_resolvers():
    """Rejects subscription resolver functions that are not async generators."""

    def resolver(self) -> int:
        return 1

    with pytest.raises(TypeError, match="must be async"):
        compile_resolver_field(
            resolver, field_name="ticks", description=None, kind="subscription"
        )


def test_compiled_resolver_adapter_skips_missing_kwargs_and_uses_defaults():
    """Uses resolver defaults when kwargs omit optional GraphQL arguments."""

    @grommet.field
    def resolver(self, value: int = 7) -> int:
        return value

    compiled = getattr(resolver, COMPILED_RESOLVER_ATTR)
    assert compiled.func(None, None, {}) == 7


def test_build_arg_info_rejects_missing_annotations_for_graphql_args():
    """Raises from _build_arg_info when argument annotations are unavailable."""

    def resolver(value):
        return value

    params = list(inspect.signature(resolver).parameters.values())
    with pytest.raises(TypeError, match="missing annotation"):
        _build_arg_info("resolver", params, {})


def test_collect_refs_skips_unannotated_params():
    """Skips parameters with inspect._empty annotations while collecting refs."""

    def resolver(value):
        return value

    params = list(inspect.signature(resolver).parameters.values())
    refs = _collect_refs(int, params, {})
    assert refs == frozenset()


def test_interface_types_cannot_declare_subscription_resolvers():
    """Rejects interface definitions that include @grommet.subscription resolvers."""

    with pytest.raises(TypeError, match="Interface types cannot declare @subscription"):

        @grommet.interface
        @dataclass
        class InvalidInterface:
            @grommet.subscription
            async def ticker(self) -> AsyncIterator[int]:
                yield 1


def test_input_types_cannot_declare_field_resolvers():
    """Rejects input definitions that include resolver methods."""
    with pytest.raises(TypeError, match="Input types cannot declare field resolvers"):

        @grommet.input
        @dataclass
        class InvalidInput:
            @grommet.field
            def value(self) -> int:
                return 1


def test_types_cannot_mix_field_and_subscription_decorators():
    """Rejects object types that mix field and subscription decorators."""
    with pytest.raises(TypeError, match="cannot mix @field and @subscription"):

        @grommet.type
        @dataclass
        class InvalidMixedType:
            @grommet.field
            async def greeting(self) -> str:
                return "hello"

            @grommet.subscription
            async def ticks(self) -> AsyncIterator[int]:
                yield 1


def test_subscription_types_cannot_declare_data_fields():
    """Rejects subscription types that include dataclass data fields."""
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


def test_subscription_decorator_factory_returns_wrapper():
    """Covers no-arg @grommet.subscription decorator usage."""
    decorator = grommet.subscription(description="Ticker stream")

    @decorator
    async def ticks(self) -> AsyncIterator[int]:
        yield 1

    compiled = getattr(ticks, COMPILED_RESOLVER_ATTR)
    assert compiled.kind == "subscription"
    assert compiled.description == "Ticker stream"


def test_data_field_resolver_returns_default_when_parent_is_none():
    """Returns a configured default value when resolving root-level missing parent objects."""
    resolver = _data_field_resolver("missing", has_default=True, default="fallback")
    assert resolver(None, None, {}) == "fallback"


def test_data_field_resolver_reads_parent_attribute_when_present():
    """Uses attrgetter path when a parent object is available."""

    @dataclass
    class Parent:
        value: str = "ok"

    resolver = _data_field_resolver("value", has_default=True, default="fallback")
    assert resolver(Parent(), None, {}) == "ok"


def test_resolve_data_field_default_uses_default_factory_values():
    """Extracts defaults generated by dataclass default_factory callables."""

    @dataclass
    class Model:
        value: int = dataclasses.field(default_factory=lambda: 5)

    field = dataclasses.fields(Model)[0]
    assert _resolve_data_field_default(field) == (True, 5)


def test_get_annotated_field_meta_skips_unrelated_metadata_items():
    """Finds Field metadata after skipping unrelated Annotated metadata entries."""
    meta = _get_annotated_field_meta(Annotated[int, "ignored", grommet.Field("desc")])
    assert meta is not None
    assert meta.description == "desc"


def test_implemented_interfaces_ignores_non_interface_grommet_bases():
    """Skips decorated object bases when collecting implemented interfaces."""

    @grommet.type
    @dataclass
    class BaseObject:
        value: int = 1

    @grommet.type
    @dataclass
    class ChildObject(BaseObject):
        pass

    assert _implemented_interfaces(ChildObject) == ((), ())
