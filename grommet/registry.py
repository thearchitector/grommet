import dataclasses
from builtins import type as pytype
from typing import TYPE_CHECKING

from .annotations import analyze_annotation, is_internal_field
from .errors import list_type_requires_parameter
from .metadata import (
    EnumMeta,
    FieldMeta,
    ScalarMeta,
    TypeMeta,
    UnionMeta,
    _interface_implementers,
)
from .resolver import _resolver_arg_annotations
from .typespec import (
    _get_enum_meta,
    _get_scalar_meta,
    _get_type_meta,
    _get_union_meta,
    _is_enum_type,
    _is_grommet_type,
    _is_scalar_type,
    _is_union_type,
)
from .typing_utils import _get_type_hints

if TYPE_CHECKING:
    from collections.abc import Iterable
    from typing import Any


@dataclasses.dataclass(frozen=True)
class TraversalResult:
    types: dict[pytype, TypeMeta]
    scalars: dict[pytype, ScalarMeta]
    enums: dict[pytype, EnumMeta]
    unions: dict[pytype, UnionMeta]


def _get_field_meta(dc_field: "dataclasses.Field[Any]") -> FieldMeta:
    meta = dc_field.metadata.get("grommet") if dc_field.metadata else None
    if isinstance(meta, FieldMeta):
        return meta
    return FieldMeta()


def _traverse_schema(entrypoints: "Iterable[pytype | None]") -> TraversalResult:
    types: dict[pytype, TypeMeta] = {}
    scalars: dict[pytype, ScalarMeta] = {}
    enums: dict[pytype, EnumMeta] = {}
    unions: dict[pytype, UnionMeta] = {}
    pending = [tp for tp in entrypoints if tp is not None]
    visited: set[pytype] = set()

    def track_annotation(annotation: "Any") -> None:
        for grommet_type in _iter_type_refs(annotation):
            pending.append(grommet_type)
        for scalar_type in _iter_scalar_refs(annotation):
            scalars.setdefault(scalar_type, _get_scalar_meta(scalar_type))
        for enum_type in _iter_enum_refs(annotation):
            enums.setdefault(enum_type, _get_enum_meta(enum_type))
        for union_type in _iter_union_refs(annotation):
            if union_type not in unions:
                union_meta = _get_union_meta(union_type)
                unions[union_type] = union_meta
                pending.extend(union_meta.types)

    while pending:
        cls = pending.pop()
        if cls in visited:
            continue
        visited.add(cls)
        if _is_union_type(cls):
            if cls not in unions:
                union_meta = _get_union_meta(cls)
                unions[cls] = union_meta
                pending.extend(union_meta.types)
            continue
        if _is_enum_type(cls):
            enums.setdefault(cls, _get_enum_meta(cls))
            continue
        if _is_scalar_type(cls):
            scalars.setdefault(cls, _get_scalar_meta(cls))
            continue
        if not _is_grommet_type(cls):
            continue
        type_meta = _get_type_meta(cls)
        types[cls] = type_meta
        if type_meta.kind == "interface":
            pending.extend(_interface_implementers(cls))
        pending.extend(type_meta.implements)
        hints = _get_type_hints(cls)
        for dc_field in dataclasses.fields(cls):
            annotation = hints.get(dc_field.name, dc_field.type)
            if is_internal_field(dc_field.name, annotation):
                continue
            track_annotation(annotation)
            field_meta = _get_field_meta(dc_field)
            if field_meta.resolver is not None:
                arg_types = _resolver_arg_annotations(field_meta.resolver)
                for arg_ann in arg_types.values():
                    track_annotation(arg_ann)
    return TraversalResult(types=types, scalars=scalars, enums=enums, unions=unions)


def _iter_type_refs(annotation: "Any") -> list[pytype]:
    info = analyze_annotation(annotation)
    inner = info.async_item if info.is_async_iterable else info.inner
    if inner is None:
        return []
    inner_info = analyze_annotation(inner)
    if inner_info.is_list:
        if inner_info.list_item is None:
            raise list_type_requires_parameter()
        return _iter_type_refs(inner_info.list_item)
    if _is_grommet_type(inner):
        return [inner]
    if _is_union_type(inner):
        return [inner]
    return []


def _iter_scalar_refs(annotation: "Any") -> list[pytype]:
    info = analyze_annotation(annotation)
    inner = info.async_item if info.is_async_iterable else info.inner
    if inner is None:
        return []
    inner_info = analyze_annotation(inner)
    if inner_info.is_list:
        if inner_info.list_item is None:
            raise list_type_requires_parameter()
        return _iter_scalar_refs(inner_info.list_item)
    if _is_scalar_type(inner):
        return [inner]
    return []


def _iter_enum_refs(annotation: "Any") -> list[pytype]:
    info = analyze_annotation(annotation)
    inner = info.async_item if info.is_async_iterable else info.inner
    if inner is None:
        return []
    inner_info = analyze_annotation(inner)
    if inner_info.is_list:
        if inner_info.list_item is None:
            raise list_type_requires_parameter()
        return _iter_enum_refs(inner_info.list_item)
    if _is_enum_type(inner):
        return [inner]
    return []


def _iter_union_refs(annotation: "Any") -> list[pytype]:
    info = analyze_annotation(annotation)
    inner = info.async_item if info.is_async_iterable else info.inner
    if inner is None:
        return []
    inner_info = analyze_annotation(inner)
    if inner_info.is_list:
        if inner_info.list_item is None:
            raise list_type_requires_parameter()
        return _iter_union_refs(inner_info.list_item)
    if _is_union_type(inner):
        return [inner]
    return []
