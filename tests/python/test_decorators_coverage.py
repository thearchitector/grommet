import dataclasses
import enum
import functools
import types
from dataclasses import dataclass

import pytest

import grommet as gm
from grommet.decorators import _field_from_resolver
from grommet.errors import GrommetTypeError
from grommet.metadata import MISSING


def test_field_defaults_and_init_false() -> None:
    @dataclass
    class Query:
        @gm.field(default=5)
        @staticmethod
        async def count(parent, info) -> int:  # pragma: no cover - called via schema
            return 5

    Query = gm.type(Query)
    field = dataclasses.fields(Query)[0]
    assert field.default == 5
    assert field.init is False


def test_field_default_factory_applied() -> None:
    @dataclass
    class Query:
        @gm.field(default_factory=lambda: 7)
        @staticmethod
        async def count(parent, info) -> int:  # pragma: no cover - called via schema
            return 7

    Query = gm.type(Query)
    field = dataclasses.fields(Query)[0]
    assert field.default is MISSING
    assert callable(field.default_factory)


def test_field_from_resolver_defaults_init_false() -> None:
    def resolver(parent, info) -> int:
        return 1

    field = _field_from_resolver(
        resolver,
        description=None,
        deprecation_reason=None,
        name=None,
        default=MISSING,
        default_factory=MISSING,
        init=None,
    )
    assert field.init is False


def test_field_from_resolver_respects_init() -> None:
    def resolver(parent, info) -> int:
        return 1

    field = _field_from_resolver(
        resolver,
        description=None,
        deprecation_reason=None,
        name=None,
        default=MISSING,
        default_factory=MISSING,
        init=True,
    )
    assert field.init is True


def test_field_default_conflict_raises() -> None:
    @dataclass
    class Query:
        @gm.field(default=1, default_factory=lambda: 2)
        @staticmethod
        async def count(parent, info) -> int:  # pragma: no cover - called via schema
            return 1

    with pytest.raises(GrommetTypeError):
        gm.type(Query)


def test_missing_return_annotation_raises() -> None:
    @dataclass
    class Query:
        @gm.field
        @staticmethod
        async def count(parent, info):  # type: ignore[no-untyped-def]
            return 1

    with pytest.raises(GrommetTypeError):
        gm.type(Query)


def test_existing_annotation_skips_return_inference() -> None:
    @dataclass
    class Query:
        @gm.field
        @staticmethod
        async def count(parent, info) -> int:  # pragma: no cover - called via schema
            return 1

    Query.__annotations__ = {"count": int}
    Query = gm.type(Query)
    assert "count" in Query.__annotations__


def test_annotations_mappingproxy_is_coerced() -> None:
    @dataclass
    class Query:
        @gm.field
        @staticmethod
        async def count(parent, info) -> int:  # pragma: no cover - called via schema
            return 1

    Query.__annotations__ = types.MappingProxyType(Query.__annotations__)
    Query = gm.type(Query)
    assert isinstance(Query.__annotations__, dict)


def test_classmethod_field_binds_class() -> None:
    @dataclass
    class Query:
        @gm.field
        @classmethod
        async def typename(cls, parent, info) -> str:  # pragma: no cover - called via schema
            return cls.__name__

    Query = gm.type(Query)
    field = dataclasses.fields(Query)[0]
    meta = field.metadata["grommet"]
    assert isinstance(meta.resolver, functools.partial)
    assert meta.resolver.args[0] is Query


def test_input_with_field_resolver_not_allowed() -> None:
    @dataclass
    class Input:
        @gm.field
        @staticmethod
        async def count(parent, info) -> int:  # pragma: no cover - called via schema
            return 1

    with pytest.raises(GrommetTypeError):
        gm.input(Input)


def test_interface_with_field_resolver_rewraps_dataclass() -> None:
    @dataclass
    class Node:
        @gm.field
        @staticmethod
        async def id(parent, info) -> int:  # pragma: no cover - called via schema
            return 1

    Node = gm.interface(Node)
    assert dataclasses.is_dataclass(Node)
    assert dataclasses.fields(Node)


def test_field_requires_callable() -> None:
    with pytest.raises(GrommetTypeError):
        gm.field(123)  # type: ignore[arg-type]


def test_scalar_requires_callables() -> None:
    with pytest.raises(GrommetTypeError):
        gm.scalar()


def test_scalar_direct_call_path() -> None:
    class CustomScalar:
        pass

    CustomScalar = gm.scalar(CustomScalar, serialize=lambda v: v, parse_value=lambda v: v)
    assert hasattr(CustomScalar, "__grommet_meta__")


def test_enum_requires_enum_subclass() -> None:
    class NotEnum:
        pass

    with pytest.raises(GrommetTypeError):
        gm.enum(NotEnum)


def test_enum_decorator_returned_wrapper() -> None:
    class Color(enum.Enum):
        RED = 1

    decorator = gm.enum()
    Color = decorator(Color)
    assert hasattr(Color, "__grommet_meta__")


def test_union_validation_errors() -> None:
    @gm.type
    @dataclass
    class Obj:
        value: int

    with pytest.raises(GrommetTypeError):
        gm.union("", types=[Obj])
    with pytest.raises(GrommetTypeError):
        gm.union("Empty", types=[])

    @gm.input
    @dataclass
    class Input:
        value: int

    with pytest.raises(GrommetTypeError):
        gm.union("Bad", types=[Input])


def test_interface_decorator_without_cls() -> None:
    @dataclass
    class Node:
        value: int

    decorator = gm.interface()
    Node = decorator(Node)
    assert hasattr(Node, "__grommet_meta__")
