import dataclasses
import enum
from dataclasses import dataclass
from typing import AsyncIterator as TypingAsyncIterator
from typing import List

import pytest

import grommet as gm
from grommet.annotations import walk_annotation
from grommet.errors import GrommetTypeError
from grommet.registry import _get_field_meta, _traverse_schema


@gm.scalar(serialize=lambda value: value, parse_value=lambda value: value)
class CustomScalar:
    pass


@gm.enum
class Status(enum.Enum):
    OK = "OK"


@gm.type
@dataclass
class Obj:
    value: int


RegistryUnion = gm.union("RegistryUnion", types=[Obj])


@gm.type
@dataclass
class Query:
    value: Obj
    union_value: RegistryUnion


class Plain:
    pass


def test_traverse_schema_tracks_union_enum_scalar_and_ignores_plain() -> None:
    """
    Verifies schema traversal collects unions/enums/scalars and ignores plain types.
    """
    result = _traverse_schema([Query, Status, CustomScalar, Plain])
    assert RegistryUnion in result.unions
    assert Status in result.enums
    assert CustomScalar in result.scalars
    assert Plain not in result.types


def test_traverse_schema_entrypoint_union_populates_types() -> None:
    """
    Ensures traversal starting at a union includes member object types.
    """
    result = _traverse_schema([RegistryUnion])
    assert RegistryUnion in result.unions
    assert Obj in result.types


def test_traverse_schema_union_already_registered() -> None:
    """
    Verifies traversal keeps previously registered union types.
    """
    result = _traverse_schema([Query, RegistryUnion])
    assert RegistryUnion in result.unions


def test_walk_annotation_handles_async_iterable_and_list_errors() -> None:
    """
    Ensures walk_annotation ignores bare async iterables and rejects raw lists.
    """
    assert list(walk_annotation(TypingAsyncIterator)) == []

    with pytest.raises(GrommetTypeError):
        list(walk_annotation(List))


def test_walk_annotation_returns_types() -> None:
    """
    Verifies walk_annotation returns expected (kind, type) tuples for list annotations.
    """
    assert list(walk_annotation(list[Obj])) == [("type", Obj)]
    assert list(walk_annotation(list[CustomScalar])) == [("scalar", CustomScalar)]
    assert list(walk_annotation(list[Status])) == [("enum", Status)]
    assert list(walk_annotation(list[RegistryUnion])) == [("union", RegistryUnion)]


def test_traverse_schema_tracks_resolver_arg_annotations() -> None:
    """Test that resolver argument annotations are tracked."""

    @gm.type
    @dataclass
    class QueryWithResolverArgs:
        @gm.field
        @staticmethod
        async def search(status: Status, scalar: CustomScalar) -> str:
            return "result"

    result = _traverse_schema([QueryWithResolverArgs])

    enum_names = {meta.name for meta in result.enums.values()}
    scalar_names = {meta.name for meta in result.scalars.values()}

    assert "Status" in enum_names
    assert "CustomScalar" in scalar_names


def test_get_field_meta_returns_default_when_no_metadata() -> None:
    """Test _get_field_meta returns default FieldMeta when no grommet metadata."""

    @dataclass
    class PlainDataclass:
        value: int

    dc_field = next(iter(dataclasses.fields(PlainDataclass)))
    meta = _get_field_meta(dc_field)
    assert meta.resolver is None
