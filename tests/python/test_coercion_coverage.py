import dataclasses
import enum
from dataclasses import dataclass
from typing import List

import pytest

import grommet as gm
from grommet.coercion import (
    _arg_coercer,
    _coerce_value,
    _default_value_for_annotation,
    _input_field_default,
)
from grommet.errors import GrommetTypeError, GrommetValueError
from grommet.metadata import ID, MISSING


@gm.scalar(serialize=lambda value: value, parse_value=lambda value: f"parsed:{value}")
class CustomScalar:
    pass


@gm.enum
class Color(enum.Enum):
    RED = 1
    BLUE = 2


@gm.input
@dataclass
class Input:
    value: int


@dataclass
class WithDefaults:
    value: int = 3
    values: list[int] = dataclasses.field(default_factory=lambda: [1, 2])


@dataclass
class NoDefaults:
    value: int


def test_default_value_for_annotation_list_and_input() -> None:
    """
    Verifies default values are derived for list and input annotations.
    """
    with pytest.raises(GrommetTypeError):
        _default_value_for_annotation(List, [1])
    assert _default_value_for_annotation(list[int], (1, 2)) == [1, 2]
    assert _default_value_for_annotation(list[int], [3, 4]) == [3, 4]
    assert _default_value_for_annotation(list[int], "not-a-list") == "not-a-list"
    assert _default_value_for_annotation(int, MISSING) is MISSING

    input_instance = Input(value=5)
    assert _default_value_for_annotation(Input, input_instance) == {"value": 5}
    assert _default_value_for_annotation(Input, {"value": 6}) == {"value": 6}
    assert _default_value_for_annotation(Input, "raw") == "raw"


def test_input_field_default_variants() -> None:
    """
    Verifies input field defaults resolve from values, factories, or missing.
    """
    field_default = dataclasses.fields(WithDefaults)[0]
    field_factory = dataclasses.fields(WithDefaults)[1]
    field_missing = dataclasses.fields(NoDefaults)[0]

    assert _input_field_default(field_default, int) == 3
    assert _input_field_default(field_factory, list[int]) == [1, 2]
    assert _input_field_default(field_missing, int) is MISSING


def test_arg_coercer_object_short_circuits() -> None:
    """
    Ensures argument coercion skips object and coerces concrete scalars.
    """
    assert _arg_coercer(object) is None
    coercer = _arg_coercer(int)
    assert coercer is not None
    assert coercer("2") == 2


def test_coerce_value_branches() -> None:
    """
    Verifies value coercion covers nulls, enums, lists, and input objects.
    """
    assert _coerce_value(None, int) is None
    assert _coerce_value("3", int | None) == 3
    assert _coerce_value(["1", "2"], list[int]) == [1, 2]
    with pytest.raises(GrommetTypeError):
        _coerce_value([1], List)

    assert _coerce_value(123, ID) == "123"

    assert _coerce_value(Color.RED, Color) is Color.RED
    assert _coerce_value("BLUE", Color) is Color.BLUE
    assert _coerce_value(1, Color) is Color.RED
    with pytest.raises(GrommetValueError):
        _coerce_value("NOPE", Color)

    assert _coerce_value("value", CustomScalar) == "parsed:value"

    assert _coerce_value("7", int) == 7
    assert _coerce_value(7, str) == "7"
    assert _coerce_value("1.5", float) == 1.5
    assert _coerce_value(0, bool) is False
    assert _coerce_value({"x": 1}, object) == {"x": 1}

    input_instance = Input(value=9)
    assert _coerce_value(input_instance, Input) == input_instance
    assert _coerce_value({"value": 10}, Input) == Input(value=10)
    with pytest.raises(GrommetTypeError):
        _coerce_value("bad", Input)
