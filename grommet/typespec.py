from builtins import type as pytype
from typing import TYPE_CHECKING

from .annotations import analyze_annotation
from .errors import (
    input_type_expected,
    list_type_requires_parameter,
    not_grommet_enum,
    not_grommet_scalar,
    not_grommet_type,
    not_grommet_union,
    output_type_expected,
    union_input_not_supported,
    unsupported_annotation,
)
from .metadata import (
    _SCALARS,
    EnumMeta,
    GrommetMetaType,
    ScalarMeta,
    TypeMeta,
    TypeSpec,
    UnionMeta,
)

if TYPE_CHECKING:
    from typing import Any


def _type_spec_from_annotation(
    annotation: "Any",
    *,
    expect_input: bool,
    force_nullable: bool = False,
) -> TypeSpec:
    info = analyze_annotation(annotation)
    inner = info.inner
    nullable = info.optional or force_nullable
    if info.is_list:
        if info.list_item is None:
            raise list_type_requires_parameter()
        return TypeSpec(
            kind="list",
            of_type=_type_spec_from_annotation(
                info.list_item, expect_input=expect_input
            ),
            nullable=nullable,
        )
    if _is_scalar_type(inner):
        scalar_meta = _get_scalar_meta(inner)
        return TypeSpec(kind="named", name=scalar_meta.name, nullable=nullable)
    if _is_enum_type(inner):
        enum_meta = _get_enum_meta(inner)
        return TypeSpec(kind="named", name=enum_meta.name, nullable=nullable)
    if _is_union_type(inner):
        if expect_input:
            raise union_input_not_supported()
        union_meta = _get_union_meta(inner)
        return TypeSpec(kind="named", name=union_meta.name, nullable=nullable)
    if inner in _SCALARS:
        return TypeSpec(kind="named", name=_SCALARS[inner], nullable=nullable)
    if _is_grommet_type(inner):
        type_meta = _get_type_meta(inner)
        if expect_input and type_meta.kind != "input":
            raise input_type_expected(type_meta.name)
        if not expect_input and type_meta.kind == "input":
            raise output_type_expected(type_meta.name)
        return TypeSpec(kind="named", name=type_meta.name, nullable=nullable)
    raise unsupported_annotation(annotation)


def _get_type_meta(cls: pytype) -> TypeMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, TypeMeta):
        raise not_grommet_type(cls.__name__)
    return meta


def _get_scalar_meta(cls: pytype) -> ScalarMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, ScalarMeta):
        raise not_grommet_scalar(cls.__name__)
    return meta


def _get_enum_meta(cls: pytype) -> EnumMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, EnumMeta):
        raise not_grommet_enum(cls.__name__)
    return meta


def _get_union_meta(cls: pytype) -> UnionMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, UnionMeta):
        raise not_grommet_union(cls.__name__)
    return meta


def _maybe_type_name(cls: pytype | None) -> str | None:
    if cls is None:
        return None
    return _get_type_meta(cls).name


def _is_grommet_type(obj: "Any") -> bool:
    meta = getattr(obj, "__grommet_meta__", None)
    return isinstance(meta, TypeMeta) and meta.type in (
        GrommetMetaType.TYPE,
        GrommetMetaType.INTERFACE,
        GrommetMetaType.INPUT,
    )


def _is_scalar_type(obj: "Any") -> bool:
    meta = getattr(obj, "__grommet_meta__", None)
    return isinstance(meta, ScalarMeta)


def _is_enum_type(obj: "Any") -> bool:
    meta = getattr(obj, "__grommet_meta__", None)
    return isinstance(meta, EnumMeta)


def _is_union_type(obj: "Any") -> bool:
    meta = getattr(obj, "__grommet_meta__", None)
    return isinstance(meta, UnionMeta)


def _is_input_type(obj: "Any") -> bool:
    return _is_grommet_type(obj) and _get_type_meta(obj).kind == "input"
