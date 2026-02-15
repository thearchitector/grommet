from dataclasses import dataclass
from typing import TYPE_CHECKING

from ._compiled import (
    COMPILED_TYPE_ATTR,
    REFS_ATTR,
    CompiledDataField,
    CompiledType,
    instantiate_core_type,
)
from .annotations import _get_type_meta, _is_grommet_type
from .errors import GrommetTypeError
from .metadata import MISSING

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
    """Build schema graph using precompiled class metadata only."""
    for root in (query, mutation, subscription):
        if root is None:
            continue
        _validate_root_defaults(root)

    collected_classes = _walk_and_collect(query, mutation, subscription)
    types = [
        instantiate_core_type(_get_compiled_type(cls)) for cls in collected_classes
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

        refs: frozenset[pytype] = getattr(cls, REFS_ATTR, frozenset())
        for ref_cls in refs:
            if ref_cls not in visited:
                pending.append(ref_cls)

    return collected


def _validate_root_defaults(root: "pytype") -> None:
    compiled = _get_compiled_type(root)
    for field in compiled.object_fields:
        if isinstance(field, CompiledDataField) and field.default is MISSING:
            raise GrommetTypeError(
                f"Root type '{compiled.meta.name}' field '{field.name}' must declare "
                "a default value or use @grommet.field."
            )
