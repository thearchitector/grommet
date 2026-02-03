import enum
from dataclasses import dataclass
from typing import List

import pytest

import grommet as gm
from grommet.errors import GrommetTypeError
from grommet.typespec import (
    _get_enum_meta,
    _get_scalar_meta,
    _get_type_meta,
    _get_union_meta,
    _maybe_type_name,
    _type_spec_from_annotation,
)


@gm.scalar(serialize=lambda value: value, parse_value=lambda value: value)
class CustomScalar:
    pass


@gm.enum
class Color(enum.Enum):
    RED = 1


@gm.type
@dataclass
class Obj:
    value: int


@gm.input
@dataclass
class Input:
    value: int


UnionType = gm.union("UnionType", types=[Obj])


class Plain:
    pass


def test_type_spec_list_requires_parameter() -> None:
    """
    Ensures list annotations require a parameterized inner type.
    """
    with pytest.raises(GrommetTypeError):
        _type_spec_from_annotation(List, expect_input=True)


def test_type_spec_scalar_and_enum() -> None:
    """
    Verifies scalar and enum annotations map to named type specs.
    """
    scalar_spec = _type_spec_from_annotation(CustomScalar, expect_input=False)
    enum_spec = _type_spec_from_annotation(Color, expect_input=False)
    assert scalar_spec.name == "CustomScalar"
    assert enum_spec.name == "Color"


def test_type_spec_union_input_not_supported() -> None:
    """
    Ensures union annotations are rejected when used for input types.
    """
    with pytest.raises(GrommetTypeError):
        _type_spec_from_annotation(UnionType, expect_input=True)


def test_type_spec_builtin_scalar_mapping() -> None:
    """
    Verifies builtin scalar annotations map to GraphQL scalar names.
    """
    spec = _type_spec_from_annotation(str, expect_input=False)
    assert spec.to_graphql() == "String!"


def test_type_spec_input_output_validation() -> None:
    """
    Ensures input/output annotations are validated against expected usage.
    """
    with pytest.raises(GrommetTypeError):
        _type_spec_from_annotation(Obj, expect_input=True)
    with pytest.raises(GrommetTypeError):
        _type_spec_from_annotation(Input, expect_input=False)


def test_type_spec_unsupported_annotation() -> None:
    """
    Ensures unsupported annotations raise a type error.
    """
    with pytest.raises(GrommetTypeError):
        _type_spec_from_annotation(set, expect_input=True)


def test_meta_getters_raise_when_missing() -> None:
    """
    Ensures meta getter helpers raise for undecorated types.
    """
    with pytest.raises(GrommetTypeError):
        _get_type_meta(Plain)
    with pytest.raises(GrommetTypeError):
        _get_scalar_meta(Plain)
    with pytest.raises(GrommetTypeError):
        _get_enum_meta(Plain)
    with pytest.raises(GrommetTypeError):
        _get_union_meta(Plain)


def test_maybe_type_name_handles_none() -> None:
    """
    Verifies _maybe_type_name handles None and decorated types.
    """
    assert _maybe_type_name(None) is None
    assert _maybe_type_name(Obj) == "Obj"
