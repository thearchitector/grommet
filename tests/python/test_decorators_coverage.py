import dataclasses
import enum
import types
from dataclasses import dataclass

import pytest

import grommet as gm
from grommet.decorators import _field_from_resolver
from grommet.errors import GrommetTypeError
from grommet.metadata import MISSING


def test_field_defaults_and_init_false() -> None:
    """Verifies field defaults set init to false for grommet fields."""

    @dataclass
    class Query:
        @gm.field(default=5)
        async def count(self) -> int:  # pragma: no cover - called via schema
            return 5

    Query = gm.type(Query)
    field = dataclasses.fields(Query)[0]
    assert field.default == 5
    assert field.init is False


def test_field_default_factory_applied() -> None:
    """Verifies field default_factory is preserved on grommet fields."""

    @dataclass
    class Query:
        @gm.field(default_factory=lambda: 7)
        async def count(self) -> int:  # pragma: no cover - called via schema
            return 7

    Query = gm.type(Query)
    field = dataclasses.fields(Query)[0]
    assert field.default is MISSING
    assert callable(field.default_factory)


def test_field_from_resolver_defaults_init_false() -> None:
    """Ensures fields derived from resolvers default to init=False."""

    def resolver(self) -> int:
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
    """Ensures field generation respects an explicit init flag."""

    def resolver(self) -> int:
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
    """Ensures specifying both default and default_factory raises errors."""

    @dataclass
    class Query:
        @gm.field(default=1, default_factory=lambda: 2)
        async def count(self) -> int:  # pragma: no cover - called via schema
            return 1

    with pytest.raises(GrommetTypeError):
        gm.type(Query)


def test_missing_return_annotation_raises() -> None:
    """Ensures missing resolver return annotations raise a type error."""

    @dataclass
    class Query:
        @gm.field
        async def count(self):  # type: ignore[no-untyped-def]
            return 1

    with pytest.raises(GrommetTypeError):
        gm.type(Query)


def test_existing_annotation_skips_return_inference() -> None:
    """Verifies explicit field annotations are preserved without inference."""

    @dataclass
    class Query:
        @gm.field
        async def count(self) -> int:  # pragma: no cover - called via schema
            return 1

    Query.__annotations__ = {"count": int}
    Query = gm.type(Query)
    assert "count" in Query.__annotations__


def test_annotations_mappingproxy_is_coerced() -> None:
    """Verifies mappingproxy annotations are coerced to dictionaries."""

    @dataclass
    class Query:
        @gm.field
        async def count(self) -> int:  # pragma: no cover - called via schema
            return 1

    Query.__annotations__ = types.MappingProxyType(Query.__annotations__)
    Query = gm.type(Query)
    assert isinstance(Query.__annotations__, dict)


def test_staticmethod_field_raises() -> None:
    """Ensures @staticmethod field resolvers raise a type error."""
    with pytest.raises(GrommetTypeError):

        @dataclass
        class Query:
            @gm.field
            @staticmethod
            async def count() -> int:  # pragma: no cover
                return 1


def test_classmethod_field_raises() -> None:
    """Ensures @classmethod field resolvers raise a type error."""
    with pytest.raises(GrommetTypeError):

        @dataclass
        class Query:
            @gm.field
            @classmethod
            async def typename(cls) -> str:  # pragma: no cover
                return cls.__name__


def test_input_with_field_resolver_not_allowed() -> None:
    """Ensures input types cannot declare field resolvers."""

    @dataclass
    class Input:
        @gm.field
        async def count(self) -> int:  # pragma: no cover - called via schema
            return 1

    with pytest.raises(GrommetTypeError):
        gm.input(Input)


def test_interface_with_field_resolver_rewraps_dataclass() -> None:
    """Verifies interface decorator rewraps dataclasses with field resolvers."""

    @dataclass
    class Node:
        @gm.field
        async def id(self) -> int:  # pragma: no cover - called via schema
            return 1

    Node = gm.interface(Node)
    assert dataclasses.is_dataclass(Node)
    assert dataclasses.fields(Node)


def test_field_requires_callable() -> None:
    """
    Ensures field decorator rejects non-callable inputs.
    """
    with pytest.raises(GrommetTypeError):
        gm.field(123)  # type: ignore[arg-type]


def test_scalar_requires_callables() -> None:
    """
    Ensures scalar decorator requires serialize and parse callables.
    """
    with pytest.raises(GrommetTypeError):
        gm.scalar()


def test_scalar_direct_call_path() -> None:
    """
    Verifies scalar decorator works when invoked directly on a class.
    """

    class CustomScalar:
        pass

    CustomScalar = gm.scalar(
        CustomScalar, serialize=lambda v: v, parse_value=lambda v: v
    )
    assert hasattr(CustomScalar, "__grommet_meta__")


def test_enum_requires_enum_subclass() -> None:
    """
    Ensures enum decorator rejects non-enum classes.
    """

    class NotEnum:
        pass

    with pytest.raises(GrommetTypeError):
        gm.enum(NotEnum)


def test_enum_decorator_returned_wrapper() -> None:
    """
    Verifies enum decorator factory attaches grommet metadata.
    """

    class Color(enum.Enum):
        RED = 1

    decorator = gm.enum()
    Color = decorator(Color)
    assert hasattr(Color, "__grommet_meta__")


def test_union_validation_errors() -> None:
    """
    Ensures union decorator validates names and object type membership.
    """

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
    """
    Verifies interface decorator factory attaches grommet metadata.
    """

    @dataclass
    class Node:
        value: int

    decorator = gm.interface()
    Node = decorator(Node)
    assert hasattr(Node, "__grommet_meta__")
