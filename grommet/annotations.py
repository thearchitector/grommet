from collections.abc import AsyncIterable, AsyncIterator
from dataclasses import dataclass
from typing import TYPE_CHECKING, Annotated, ClassVar, get_args, get_origin

from .errors import async_iterable_requires_parameter, list_type_requires_parameter
from .metadata import (
    _INTERNAL_MARKER,
    EnumMeta,
    GrommetMetaType,
    ScalarMeta,
    TypeMeta,
    UnionMeta,
)

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Iterator
    from typing import Any

_NONE_TYPE = type(None)


@dataclass(frozen=True, slots=True)
class AnnotationInfo:
    annotation: "Any"
    inner: "Any"
    optional: bool
    origin: "Any"
    args: tuple["Any", ...]
    metadata: tuple["Any", ...]
    is_list: bool
    list_item: "Any | None"
    is_async_iterable: bool
    async_item: "Any | None"
    is_classvar: bool
    is_internal: bool


def analyze_annotation(annotation: "Any") -> AnnotationInfo:
    metadata: tuple["Any", ...] = ()
    inner = annotation
    origin = get_origin(inner)
    if origin is Annotated:
        args = get_args(inner)
        if args:
            inner = args[0]
            metadata = args[1:]
    inner, optional = _split_optional(inner)
    origin = get_origin(inner)
    args = get_args(inner)
    is_list = origin is list
    list_item = args[0] if is_list and args else None
    is_async_iterable = origin in (AsyncIterator, AsyncIterable)
    async_item = args[0] if is_async_iterable and args else None
    is_classvar = origin is ClassVar
    is_internal = _INTERNAL_MARKER in metadata
    return AnnotationInfo(
        annotation=annotation,
        inner=inner,
        optional=optional,
        origin=origin,
        args=args,
        metadata=metadata,
        is_list=is_list,
        list_item=list_item,
        is_async_iterable=is_async_iterable,
        async_item=async_item,
        is_classvar=is_classvar,
        is_internal=is_internal,
    )


def unwrap_async_iterable(annotation: "Any") -> tuple["Any", bool]:
    info = analyze_annotation(annotation)
    if info.is_async_iterable:
        if info.async_item is None:
            raise async_iterable_requires_parameter()
        return info.async_item, info.optional
    return annotation, False


def unwrap_async_iterable_inner(annotation: "Any") -> "Any":
    info = analyze_annotation(annotation)
    if info.is_async_iterable:
        if info.async_item is None:
            raise async_iterable_requires_parameter()
        return info.async_item
    return annotation


def _split_optional(annotation: "Any") -> tuple["Any", bool]:
    args = get_args(annotation)
    if args:
        non_none = [arg for arg in args if arg is not _NONE_TYPE]
        if len(non_none) == 1 and len(non_none) != len(args):
            return non_none[0], True
    return annotation, False


def is_internal_field(attr_name: str, annotation: "Any") -> bool:
    if attr_name.startswith("_"):
        return True
    info = analyze_annotation(annotation)
    return info.is_internal or info.is_classvar


def walk_annotation(annotation: "Any") -> "Iterator[tuple[str, pytype]]":
    """
    Yields (kind, pytype) tuples for all grommet types referenced in an annotation.

    kind is one of: 'type', 'scalar', 'enum', 'union'.
    Handles optional, list, and async-iterable wrappers.
    """
    info = analyze_annotation(annotation)
    inner = info.async_item if info.is_async_iterable else info.inner
    if inner is None:
        return
    yield from _walk_inner(inner)


def _walk_inner(inner: "Any") -> "Iterator[tuple[str, pytype]]":
    """Recursively walk an unwrapped inner type."""
    info = analyze_annotation(inner)
    if info.is_list:
        if info.list_item is None:
            raise list_type_requires_parameter()
        yield from _walk_inner(info.list_item)
        return

    meta = getattr(inner, "__grommet_meta__", None)
    if meta is None:
        return

    if isinstance(meta, TypeMeta) and meta.type in (
        GrommetMetaType.TYPE,
        GrommetMetaType.INTERFACE,
        GrommetMetaType.INPUT,
    ):
        yield ("type", inner)
    elif isinstance(meta, ScalarMeta):
        yield ("scalar", inner)
    elif isinstance(meta, EnumMeta):
        yield ("enum", inner)
    elif isinstance(meta, UnionMeta):
        yield ("union", inner)
