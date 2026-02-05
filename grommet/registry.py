import dataclasses
from builtins import type as pytype
from typing import TYPE_CHECKING

from .annotations import is_internal_field, walk_annotation
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


@dataclasses.dataclass(frozen=True, slots=True)
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
        for kind, ref_type in walk_annotation(annotation):
            if kind == "type":
                pending.append(ref_type)
            elif kind == "scalar":
                scalars.setdefault(ref_type, _get_scalar_meta(ref_type))
            elif kind == "enum":
                enums.setdefault(ref_type, _get_enum_meta(ref_type))
            elif kind == "union":
                if ref_type not in unions:
                    union_meta = _get_union_meta(ref_type)
                    unions[ref_type] = union_meta
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
