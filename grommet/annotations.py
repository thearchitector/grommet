from collections.abc import AsyncIterable, AsyncIterator
from dataclasses import dataclass
from types import UnionType
from typing import (
    TYPE_CHECKING,
    Annotated,
    ClassVar,
    TypeAliasType,
    Union,
    get_args,
    get_origin,
)

from .errors import (
    async_iterable_requires_parameter,
    input_type_expected,
    list_type_requires_parameter,
    not_grommet_type,
    output_type_expected,
    union_input_not_supported,
    union_member_must_be_object,
    unsupported_annotation,
)
from .metadata import _SCALARS, Context, Hidden, TypeKind, TypeMeta, TypeSpec
from .metadata import Union as UnionMetadata

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Iterator
    from typing import Any

_NONE_TYPE = type(None)


@dataclass(frozen=True, slots=True)
class AnnotationInfo:
    inner: "Any"
    optional: bool
    metadata: tuple["Any", ...]
    is_list: bool
    list_item: "Any | None"
    is_async_iterable: bool
    async_item: "Any | None"
    is_classvar: bool
    is_hidden: bool
    is_context: bool


def _unwrap_type_alias(annotation: "Any") -> "Any":
    current = annotation
    seen: set[int] = set()
    while isinstance(current, TypeAliasType):
        marker = id(current)
        if marker in seen:
            break
        seen.add(marker)
        current = current.__value__
    return current


def _is_union_origin(origin: "Any") -> bool:
    return origin in (UnionType, Union)


def _is_union_annotation(annotation: "Any") -> bool:
    return _is_union_origin(get_origin(_unwrap_type_alias(annotation)))


def _iter_union_members(annotation: "Any") -> "Iterator[Any]":
    for raw_member in get_args(annotation):
        member = _unwrap_type_alias(raw_member)
        if member is _NONE_TYPE:
            continue
        yield member


def _annotation_name(annotation: "Any") -> str:
    name = getattr(annotation, "__name__", None)
    if isinstance(name, str):
        return name
    return str(annotation)


def _get_union_metadata(metadata: tuple["Any", ...]) -> UnionMetadata | None:
    for item in metadata:
        if isinstance(item, UnionMetadata):
            return item
    return None


def analyze_annotation(annotation: "Any") -> AnnotationInfo:
    metadata: tuple["Any", ...] = ()
    inner = _unwrap_type_alias(annotation)
    origin = get_origin(inner)
    if origin is Annotated:
        args = get_args(inner)
        if args:
            inner = _unwrap_type_alias(args[0])
            metadata = args[1:]
    inner, optional = _split_optional(inner)
    inner = _unwrap_type_alias(inner)
    origin = get_origin(inner)
    args = get_args(inner)
    is_list = origin is list
    list_item = _unwrap_type_alias(args[0]) if is_list and args else None
    is_async_iterable = origin in (AsyncIterator, AsyncIterable)
    async_item = _unwrap_type_alias(args[0]) if is_async_iterable and args else None
    is_classvar = origin is ClassVar
    is_hidden = any(x is Hidden for x in metadata)
    is_context = any(x is Context for x in metadata)
    return AnnotationInfo(
        inner=inner,
        optional=optional,
        metadata=metadata,
        is_list=is_list,
        list_item=list_item,
        is_async_iterable=is_async_iterable,
        async_item=async_item,
        is_classvar=is_classvar,
        is_hidden=is_hidden,
        is_context=is_context,
    )


def unwrap_async_iterable(annotation: "Any") -> tuple["Any", bool]:
    info = analyze_annotation(annotation)
    if info.is_async_iterable:
        if info.async_item is None:
            raise async_iterable_requires_parameter()
        return info.async_item, info.optional
    return annotation, False


def _split_optional(annotation: "Any") -> tuple["Any", bool]:
    annotation = _unwrap_type_alias(annotation)
    args = get_args(annotation)
    if args:
        non_none = [_unwrap_type_alias(arg) for arg in args if arg is not _NONE_TYPE]
        if len(non_none) != len(args):
            if len(non_none) == 1:
                return non_none[0], True
            union_value = non_none[0]
            for item in non_none[1:]:
                union_value = union_value | item
            return union_value, True
    return annotation, False


def is_hidden_field(attr_name: str, annotation: "Any") -> bool:
    if attr_name.startswith("_"):
        return True
    info = analyze_annotation(annotation)
    return info.is_hidden or info.is_classvar


def walk_annotation(annotation: "Any") -> "Iterator[pytype]":
    """Yields grommet types referenced in an annotation."""
    info = analyze_annotation(annotation)
    if info.is_context:
        return
    inner = info.async_item if info.is_async_iterable else info.inner
    if inner is None:
        return
    yield from _walk_inner(inner)


def _walk_inner(inner: "Any") -> "Iterator[pytype]":
    """Recursively walk an unwrapped inner type."""
    info = analyze_annotation(inner)
    if info.is_context:
        return
    if info.is_list:
        if info.list_item is None:
            raise list_type_requires_parameter()
        yield from _walk_inner(info.list_item)
        return

    if _is_union_annotation(info.inner):
        for member in _iter_union_members(info.inner):
            yield from _walk_inner(member)
        return

    if _is_grommet_type(info.inner):
        yield info.inner


def _type_spec_from_annotation(
    annotation: "Any", *, expect_input: bool, force_nullable: bool = False
) -> TypeSpec:
    info = analyze_annotation(annotation)
    if info.is_context:
        raise unsupported_annotation(annotation)
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
    union_spec = _build_union_type_spec(
        annotation=annotation,
        inner=inner,
        metadata=info.metadata,
        expect_input=expect_input,
        nullable=nullable,
    )
    if union_spec is not None:
        return union_spec
    if inner in _SCALARS:
        return TypeSpec(kind="named", name=_SCALARS[inner], nullable=nullable)
    if _is_grommet_type(inner):
        type_meta = _get_type_meta(inner)
        if expect_input and type_meta.kind is not TypeKind.INPUT:
            raise input_type_expected(type_meta.name)
        if not expect_input and type_meta.kind is TypeKind.INPUT:
            raise output_type_expected(type_meta.name)
        return TypeSpec(kind="named", name=type_meta.name, nullable=nullable)
    raise unsupported_annotation(annotation)


def _get_type_meta(cls: "pytype") -> TypeMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, TypeMeta):
        raise not_grommet_type(cls.__name__)
    return meta


def _is_grommet_type(obj: "Any") -> bool:
    return isinstance(getattr(obj, "__grommet_meta__", None), TypeMeta)


def _is_input_type(obj: "Any") -> bool:
    return _is_grommet_type(obj) and _get_type_meta(obj).kind is TypeKind.INPUT


def _build_union_type_spec(
    *,
    annotation: "Any",
    inner: "Any",
    metadata: tuple["Any", ...],
    expect_input: bool,
    nullable: bool,
) -> TypeSpec | None:
    if not _is_union_annotation(inner):
        return None
    if expect_input:
        raise union_input_not_supported()

    union_members: list[str] = []
    for member in _iter_union_members(inner):
        if not _is_grommet_type(member):
            raise union_member_must_be_object(_annotation_name(member))
        member_meta = _get_type_meta(member)
        if member_meta.kind is not TypeKind.OBJECT:
            raise union_member_must_be_object(member_meta.name)
        union_members.append(member_meta.name)
    if not union_members:
        raise unsupported_annotation(annotation)

    union_meta = _get_union_metadata(metadata)
    union_name = (
        union_meta.name
        if union_meta is not None and union_meta.name is not None
        else "".join(union_members)
    )
    union_description = union_meta.description if union_meta is not None else None
    return TypeSpec(
        kind="union",
        name=union_name,
        union_members=tuple(union_members),
        union_description=union_description,
        nullable=nullable,
    )
