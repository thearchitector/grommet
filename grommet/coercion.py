import dataclasses
from typing import TYPE_CHECKING

from .annotations import _is_input_type, analyze_annotation
from .errors import input_mapping_expected, list_type_requires_parameter
from .metadata import MISSING

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
    """Return a coercer only for input types that need dict→dataclass conversion."""
    info = analyze_annotation(annotation)
    inner = info.inner
    if info.is_list:
        item_coercer = _arg_coercer(info.list_item) if info.list_item else None
        if item_coercer is not None:
            return lambda value: [item_coercer(item) for item in value]
        return None
    if info.optional:
        inner_coercer = _arg_coercer(inner)
        if inner_coercer is not None:
            return lambda value: None if value is None else inner_coercer(value)
        return None
    if _is_input_type(inner):
        return lambda value: _coerce_input(value, inner)
    return None


def _coerce_input(value: "Any", cls: type) -> "Any":
    """Convert a dict to an input type dataclass instance."""
    if isinstance(value, cls):
        return value
    if isinstance(value, dict):
        return cls(**value)
    raise input_mapping_expected(cls.__name__)
