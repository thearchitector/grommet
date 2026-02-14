from dataclasses import dataclass
from typing import TYPE_CHECKING

from .annotations import _get_type_meta, _is_grommet_type
from .decorators import (
    _REFS_ATTR,
    _build_input_type,
    _build_object_type,
    _build_subscription_type,
)
from .metadata import TypeKind

if TYPE_CHECKING:
    from builtins import type as pytype
    from typing import Any


@dataclass(frozen=True, slots=True)
class SchemaBundle:
    """Bundle of Rust type objects ready for registration."""

    query: str
    mutation: str | None
    subscription: str | None
    types: list["Any"]


def build_schema_graph(
    *,
    query: "pytype",
    mutation: "pytype | None" = None,
    subscription: "pytype | None" = None,
) -> SchemaBundle:
    """Build schema graph by walking decorated types and constructing Rust objects."""
    collected = _walk_and_collect(query, mutation, subscription)

    return SchemaBundle(
        query=_get_type_meta(query).name,
        mutation=_get_type_meta(mutation).name if mutation else None,
        subscription=_get_type_meta(subscription).name if subscription else None,
        types=collected,
    )


def _build_rust_type(cls: "pytype") -> "Any":
    """Build a fresh Rust type object from the metadata stored by the decorator."""
    meta = _get_type_meta(cls)
    if meta.kind is TypeKind.INPUT:
        return _build_input_type(
            cls, type_name=meta.name, description=meta.description
        )[0]
    if meta.kind is TypeKind.SUBSCRIPTION:
        return _build_subscription_type(
            cls, type_name=meta.name, description=meta.description
        )[0]
    return _build_object_type(cls, type_name=meta.name, description=meta.description)[0]


def _walk_and_collect(
    query: "pytype", mutation: "pytype | None", subscription: "pytype | None"
) -> "list[Any]":
    """Recursively walk type refs and build fresh Rust objects."""
    pending: list[pytype] = [
        tp for tp in (query, mutation, subscription) if tp is not None
    ]
    visited: set[pytype] = set()
    rust_types: list[Any] = []

    while pending:
        cls = pending.pop()
        if cls in visited:
            continue
        visited.add(cls)
        if not _is_grommet_type(cls):
            continue

        rust_types.append(_build_rust_type(cls))

        refs: frozenset[pytype] = getattr(cls, _REFS_ATTR, frozenset())
        for ref_cls in refs:
            if ref_cls not in visited:
                pending.append(ref_cls)

    return rust_types
