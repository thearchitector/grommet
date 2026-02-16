from dataclasses import dataclass
from typing import TYPE_CHECKING

from ._compiled import (
    COMPILED_TYPE_ATTR,
    REFS_ATTR,
    CompiledDataField,
    CompiledResolverField,
    CompiledType,
    CompiledUnion,
)
from .annotations import _get_type_meta, _is_grommet_type
from .errors import GrommetTypeError, union_definition_conflict
from .metadata import TypeKind, TypeMeta

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Iterator

    from .metadata import TypeSpec


@dataclass(frozen=True, slots=True)
class SchemaBundle:
    """Bundle of compiled schema metadata ready for Rust registration."""

    query: str
    mutation: str | None
    subscription: str | None
    types: list[CompiledType | CompiledUnion]


def build_schema_graph(
    *,
    query: "pytype",
    mutation: "pytype | None" = None,
    subscription: "pytype | None" = None,
) -> SchemaBundle:
    """Build schema graph using precompiled class metadata only."""
    for root in (query, mutation, subscription):
        if root is None:
            continue
        _validate_root_defaults(root)

    collected_classes = _walk_and_collect(query, mutation, subscription)
    compiled_types = [_get_compiled_type(cls) for cls in collected_classes]
    types: list[CompiledType | CompiledUnion] = [
        *compiled_types,
        *_collect_compiled_unions(compiled_types),
    ]

    return SchemaBundle(
        query=_get_type_meta(query).name,
        mutation=_get_type_meta(mutation).name if mutation else None,
        subscription=_get_type_meta(subscription).name if subscription else None,
        types=types,
    )


def _get_compiled_type(cls: "pytype") -> CompiledType:
    compiled = getattr(cls, COMPILED_TYPE_ATTR, None)
    if isinstance(compiled, CompiledType):
        return compiled
    raise GrommetTypeError(f"{cls.__name__} is missing compiled grommet type metadata.")


def _walk_and_collect(
    query: "pytype", mutation: "pytype | None", subscription: "pytype | None"
) -> list["pytype"]:
    pending: list[pytype] = [
        tp for tp in (query, mutation, subscription) if tp is not None
    ]
    visited: set[pytype] = set()
    collected: list[pytype] = []

    while pending:
        cls = pending.pop()
        if cls in visited:
            continue

        visited.add(cls)
        if not _is_grommet_type(cls):
            continue

        collected.append(cls)
        meta = _get_type_meta(cls)
        if meta.kind is TypeKind.INTERFACE:
            for implementer in _iter_interface_implementers(cls):
                if implementer not in visited:
                    pending.append(implementer)

        refs: frozenset[pytype] = getattr(cls, REFS_ATTR, frozenset())
        for ref_cls in sorted(refs, key=_class_sort_key):
            if ref_cls not in visited:
                pending.append(ref_cls)

    return collected


def _class_sort_key(cls: "pytype") -> str:
    return f"{cls.__module__}.{cls.__qualname__}"


def _iter_interface_implementers(interface_cls: "pytype") -> "Iterator[pytype]":
    pending = sorted(interface_cls.__subclasses__(), key=_class_sort_key, reverse=True)
    seen: set[pytype] = set()
    while pending:
        cls = pending.pop()
        if cls in seen:
            continue
        seen.add(cls)

        nested = sorted(cls.__subclasses__(), key=_class_sort_key, reverse=True)
        pending.extend(nested)

        if not _is_grommet_type(cls):
            continue
        if _get_type_meta(cls).kind is TypeKind.OBJECT:
            yield cls


def _collect_compiled_unions(compiled_types: list[CompiledType]) -> list[CompiledUnion]:
    by_name: dict[str, tuple[tuple[str, ...], str | None]] = {}
    for compiled_type in compiled_types:
        for type_spec in _iter_compiled_type_specs(compiled_type):
            for union_spec in _iter_union_type_specs(type_spec):
                if not union_spec.name:
                    raise GrommetTypeError(
                        "Union definitions require a non-empty name."
                    )
                if not union_spec.union_members:
                    raise GrommetTypeError(
                        f"Union '{union_spec.name}' must contain at least one object type."
                    )
                payload = (union_spec.union_members, union_spec.union_description)
                existing = by_name.get(union_spec.name)
                if existing is not None and existing != payload:
                    raise union_definition_conflict(union_spec.name)
                by_name[union_spec.name] = payload

    unions: list[CompiledUnion] = []
    for name in sorted(by_name):
        members, description = by_name[name]
        unions.append(
            CompiledUnion(
                meta=TypeMeta(kind=TypeKind.UNION, name=name, description=description),
                possible_types=members,
            )
        )
    return unions


def _iter_compiled_type_specs(compiled_type: CompiledType) -> "Iterator[TypeSpec]":
    for object_field in compiled_type.object_fields:
        yield object_field.type_spec
        if isinstance(object_field, CompiledResolverField):
            for arg in object_field.args:
                yield arg.type_spec
    for subscription_field in compiled_type.subscription_fields:
        yield subscription_field.type_spec
        for arg in subscription_field.args:
            yield arg.type_spec
    for input_field in compiled_type.input_fields:
        yield input_field.type_spec


def _iter_union_type_specs(type_spec: "TypeSpec") -> "Iterator[TypeSpec]":
    if type_spec.kind == "union":
        yield type_spec
    if type_spec.of_type is not None:
        yield from _iter_union_type_specs(type_spec.of_type)


def _validate_root_defaults(root: "pytype") -> None:
    compiled = _get_compiled_type(root)
    for field in compiled.object_fields:
        if isinstance(field, CompiledDataField) and not field.has_default:
            raise GrommetTypeError(
                f"Root type '{compiled.meta.name}' field '{field.name}' must declare "
                "a default value or use @grommet.field."
            )
