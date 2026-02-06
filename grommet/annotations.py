import sys
from collections.abc import AsyncIterable, AsyncIterator
from dataclasses import dataclass
from functools import lru_cache
from typing import (
    TYPE_CHECKING,
    Annotated,
    ClassVar,
    get_args,
    get_origin,
    get_type_hints,
)

from .errors import (
    async_iterable_requires_parameter,
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
    _INTERNAL_MARKER,
    _SCALARS,
    EnumMeta,
    GrommetMetaType,
    ScalarMeta,
    TypeKind,
    TypeMeta,
    TypeSpec,
    UnionMeta,
)

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Iterator
    from typing import Any

_NONE_TYPE = type(None)


# ---------------------------------------------------------------------------
# Type-hint resolution (formerly typing_utils.py)
# ---------------------------------------------------------------------------


def _resolve_type_hints(obj: "Any") -> dict[str, "Any"]:
    try:
        globalns = vars(sys.modules[obj.__module__])
        localns = dict(vars(obj))
        return get_type_hints(
            obj, globalns=globalns, localns=localns, include_extras=True
        )
    except Exception:
        return getattr(obj, "__annotations__", {})


@lru_cache(maxsize=512)
def _cached_type_hints(obj: "Any") -> dict[str, "Any"]:
    return _resolve_type_hints(obj)


def _get_type_hints(obj: "Any") -> dict[str, "Any"]:
    try:
        return _cached_type_hints(obj)
    except TypeError:
        return _resolve_type_hints(obj)


# ---------------------------------------------------------------------------
# Annotation analysis
# ---------------------------------------------------------------------------


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


# ---------------------------------------------------------------------------
# Type-spec conversion and meta accessors (formerly typespec.py)
# ---------------------------------------------------------------------------


def _type_spec_from_annotation(
    annotation: "Any", *, expect_input: bool, force_nullable: bool = False
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


def _get_scalar_meta(cls: "pytype") -> ScalarMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, ScalarMeta):
        raise not_grommet_scalar(cls.__name__)
    return meta


def _get_enum_meta(cls: "pytype") -> EnumMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, EnumMeta):
        raise not_grommet_enum(cls.__name__)
    return meta


def _get_union_meta(cls: "pytype") -> UnionMeta:
    meta = getattr(cls, "__grommet_meta__", None)
    if not isinstance(meta, UnionMeta):
        raise not_grommet_union(cls.__name__)
    return meta


def _maybe_type_name(cls: "pytype | None") -> str | None:
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
    return _is_grommet_type(obj) and _get_type_meta(obj).kind is TypeKind.INPUT
