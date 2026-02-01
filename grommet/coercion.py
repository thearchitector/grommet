import dataclasses
from typing import TYPE_CHECKING

from .annotations import analyze_annotation
from .errors import (
    input_mapping_expected,
    invalid_enum_value,
    list_type_requires_parameter,
)
from .metadata import ID, MISSING
from .typespec import _get_scalar_meta, _is_enum_type, _is_input_type, _is_scalar_type

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any


def _default_value_for_annotation(annotation: "Any", default: "Any") -> "Any":
    if default is MISSING:
        return default
    info = analyze_annotation(annotation)
    inner = info.inner
    if info.is_list:
        if info.list_item is None:
            raise list_type_requires_parameter()
        item_type = info.list_item
        if isinstance(default, list | tuple):
            return [
                _default_value_for_annotation(item_type, item) for item in list(default)
            ]
        return default
    if _is_input_type(inner):
        if isinstance(default, inner):
            return dataclasses.asdict(default)
        if isinstance(default, dict):
            return default
    return default


def _input_field_default(
    dc_field: "dataclasses.Field[Any]", annotation: "Any"
) -> "Any":
    if dc_field.default is not MISSING:
        return _default_value_for_annotation(annotation, dc_field.default)
    if dc_field.default_factory is not MISSING:
        return _default_value_for_annotation(annotation, dc_field.default_factory())
    return MISSING


def _arg_coercer(annotation: "Any") -> "Callable[[Any], Any] | None":
    if annotation is object:
        return None
    return lambda value: _coerce_value(value, annotation)


def _coerce_value(value: "Any", annotation: "Any") -> "Any":
    if value is None:
        return None
    info = analyze_annotation(annotation)
    if info.optional:
        return _coerce_value(value, info.inner)
    inner = info.inner
    if info.is_list:
        if info.list_item is None:
            raise list_type_requires_parameter()
        return [_coerce_value(item, info.list_item) for item in value]
    if inner is ID:
        return str(value)
    if _is_enum_type(inner):
        if isinstance(value, inner):
            return value
        if isinstance(value, str):
            try:
                return inner[value]
            except KeyError as exc:
                raise invalid_enum_value(value, inner.__name__) from exc
        return inner(value)
    if _is_scalar_type(inner):
        return _get_scalar_meta(inner).parse_value(value)
    if inner in (str, int, float):
        return inner(value)
    if inner is bool:
        return bool(value)
    if _is_input_type(inner):
        if isinstance(value, inner):
            return value
        if isinstance(value, dict):
            return inner(**value)
        raise input_mapping_expected(inner.__name__)
    return value
