import enum
from dataclasses import dataclass
from typing import AsyncIterator as TypingAsyncIterator
from typing import List

import pytest

import grommet as gm
from grommet.annotations import walk_annotation
from grommet.errors import GrommetTypeError


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
