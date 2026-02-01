from collections.abc import AsyncIterator
from dataclasses import dataclass
import enum
from typing import List, AsyncIterator as TypingAsyncIterator

import pytest

import grommet as gm
from grommet.errors import GrommetTypeError
from grommet.registry import (
    _iter_enum_refs,
    _iter_scalar_refs,
    _iter_type_refs,
    _iter_union_refs,
    _traverse_schema,
)


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
    result = _traverse_schema([Query, Status, CustomScalar, Plain])
    assert RegistryUnion in result.unions
    assert Status in result.enums
    assert CustomScalar in result.scalars
    assert Plain not in result.types


def test_traverse_schema_entrypoint_union_populates_types() -> None:
    result = _traverse_schema([RegistryUnion])
    assert RegistryUnion in result.unions
    assert Obj in result.types


def test_traverse_schema_union_already_registered() -> None:
    result = _traverse_schema([Query, RegistryUnion])
    assert RegistryUnion in result.unions


def test_iter_refs_handle_async_iterable_and_list_errors() -> None:
    assert _iter_type_refs(TypingAsyncIterator) == []
    assert _iter_scalar_refs(TypingAsyncIterator) == []
    assert _iter_enum_refs(TypingAsyncIterator) == []
    assert _iter_union_refs(TypingAsyncIterator) == []

    with pytest.raises(GrommetTypeError):
        _iter_type_refs(List)
    with pytest.raises(GrommetTypeError):
        _iter_scalar_refs(List)
    with pytest.raises(GrommetTypeError):
        _iter_enum_refs(List)
    with pytest.raises(GrommetTypeError):
        _iter_union_refs(List)


def test_iter_refs_return_types() -> None:
    assert _iter_type_refs(list[Obj]) == [Obj]
    assert _iter_scalar_refs(list[CustomScalar]) == [CustomScalar]
    assert _iter_enum_refs(list[Status]) == [Status]
    assert _iter_union_refs(list[RegistryUnion]) == [RegistryUnion]
