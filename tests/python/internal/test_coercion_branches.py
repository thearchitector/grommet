"""Targeted branch coverage tests for grommet.coercion."""

import dataclasses
from dataclasses import dataclass
from typing import List

import pytest

import grommet
from grommet.coercion import (
    _arg_coercer,
    _coerce_input,
    _default_value_for_annotation,
    _input_field_default,
)
from grommet.metadata import MISSING


@grommet.input
@dataclass
class ChildInput:
    value: int


def test_default_value_for_annotation_handles_missing_list_and_input_defaults():
    """Covers MISSING passthrough, list recursion, and input-instance conversion."""
    assert _default_value_for_annotation(int, MISSING) is MISSING

    with pytest.raises(TypeError, match="List types must be parameterized"):
        _default_value_for_annotation(List, [1])

    converted = _default_value_for_annotation(
        list[ChildInput], (ChildInput(value=1), {"value": 2})
    )
    assert converted == [{"value": 1}, {"value": 2}]
    assert _default_value_for_annotation(list[int], "raw") == "raw"

    instance_default = _default_value_for_annotation(ChildInput, ChildInput(value=3))
    dict_default = _default_value_for_annotation(ChildInput, {"value": 4})
    passthrough_default = _default_value_for_annotation(ChildInput, "raw")
    assert instance_default == {"value": 3}
    assert dict_default == {"value": 4}
    assert passthrough_default == "raw"


def test_input_field_default_handles_default_default_factory_and_missing():
    """Extracts defaults from dataclass fields across all default modes."""

    @dataclass
    class Defaults:
        required: int
        plain: int = 1
        from_factory: int = dataclasses.field(default_factory=lambda: 7)

    fields = {field.name: field for field in dataclasses.fields(Defaults)}

    assert _input_field_default(fields["plain"], int) == 1
    assert _input_field_default(fields["from_factory"], int) == 7
    assert _input_field_default(fields["required"], int) is MISSING


def test_arg_coercer_handles_list_optional_and_non_input_annotations():
    """Builds coercers only where input-object conversion is required."""
    list_coercer = _arg_coercer(list[ChildInput])
    assert list_coercer is not None
    coerced_list = list_coercer([{"value": 1}, ChildInput(value=2)])
    assert [item.value for item in coerced_list] == [1, 2]

    optional_coercer = _arg_coercer(ChildInput | None)
    assert optional_coercer is not None
    assert optional_coercer(None) is None
    assert optional_coercer({"value": 5}) == ChildInput(value=5)

    assert _arg_coercer(list[int]) is None
    assert _arg_coercer(str | None) is None


def test_coerce_input_accepts_instances_dicts_and_rejects_other_values():
    """Converts mappings to dataclass inputs and rejects unsupported values."""
    instance = ChildInput(value=9)
    assert _coerce_input(instance, ChildInput) is instance
    assert _coerce_input({"value": 10}, ChildInput) == ChildInput(value=10)

    with pytest.raises(TypeError, match="Expected mapping"):
        _coerce_input("bad", ChildInput)
