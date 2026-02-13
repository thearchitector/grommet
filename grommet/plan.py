import dataclasses
import inspect
from operator import attrgetter
from typing import TYPE_CHECKING

from ._annotations import get_annotations
from .annotations import (
    _get_type_meta,
    _is_grommet_type,
    _type_spec_from_annotation,
    analyze_annotation,
    is_hidden_field,
    unwrap_async_iterable,
    walk_annotation,
)
from .coercion import _input_field_default
from .decorators import _FieldResolver
from .metadata import MISSING, NO_DEFAULT, Field, TypeKind
from .resolver import _analyze_resolver, _resolver_arg_info

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any

    from .metadata import ArgPlan, TypeMeta, TypeSpec


@dataclasses.dataclass(frozen=True, slots=True)
class FieldPlan:
    """Planned field for a GraphQL type."""

    name: str
    source: str
    type_spec: "TypeSpec"
    func: "Callable[..., Any] | None" = None
    shape: str | None = None
    arg_names: list[str] = dataclasses.field(default_factory=list)
    is_async: bool = False
    is_async_gen: bool = False
    args: tuple["ArgPlan", ...] = ()
    default: "Any" = NO_DEFAULT
    description: str | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class TypePlan:
    """Planned GraphQL type (object, input, or subscription)."""

    kind: TypeKind
    name: str
    cls: "pytype"
    fields: tuple[FieldPlan, ...]
    description: str | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class SchemaPlan:
    """Complete planned schema."""

    query: str
    mutation: str | None
    subscription: str | None
    types: tuple[TypePlan, ...]


def _is_field_resolver(obj: object) -> bool:
    return isinstance(obj, _FieldResolver)


def _get_annotated_field_meta(annotation: "Any") -> Field | None:
    """Extract Field metadata from Annotated type if present."""
    info = analyze_annotation(annotation)
    for item in info.metadata:
        if isinstance(item, Field):
            return item
    return None


def build_schema_graph(
    *,
    query: "pytype",
    mutation: "pytype | None" = None,
    subscription: "pytype | None" = None,
) -> SchemaPlan:
    """Build a complete schema plan by traversing types once."""
    types: dict["pytype", "TypeMeta"] = {}

    entrypoints = [query, mutation, subscription]
    pending = [tp for tp in entrypoints if tp is not None]
    visited: set["pytype"] = set()

    def track_annotation(annotation: "Any") -> None:
        for ref_type in walk_annotation(annotation):
            pending.append(ref_type)

    while pending:
        cls = pending.pop()
        if cls in visited:
            continue
        visited.add(cls)
        if not _is_grommet_type(cls):
            continue
        type_meta = _get_type_meta(cls)
        types[cls] = type_meta
        hints = get_annotations(cls)
        for dc_field in dataclasses.fields(cls):
            annotation = hints.get(dc_field.name, dc_field.type)
            if is_hidden_field(dc_field.name, annotation):
                continue
            track_annotation(annotation)
        for _attr_name, attr_value in vars(cls).items():
            if not _is_field_resolver(attr_value):
                continue
            resolver_hints = get_annotations(attr_value.resolver)
            ret_ann = resolver_hints.get("return")
            if ret_ann is not None:
                track_annotation(ret_ann)
            for _, arg_ann in _resolver_arg_info(attr_value.resolver):
                track_annotation(arg_ann)

    type_plans = _build_type_plans(types, query, mutation, subscription)

    return SchemaPlan(
        query=_get_type_meta(query).name,
        mutation=_get_type_meta(mutation).name if mutation else None,
        subscription=_get_type_meta(subscription).name if subscription else None,
        types=tuple(type_plans),
    )


def _build_type_plans(
    types: dict["pytype", "TypeMeta"],
    query: "pytype",
    mutation: "pytype | None",
    subscription: "pytype | None",
) -> list[TypePlan]:
    """Build TypePlan for each discovered type."""
    type_plans: list[TypePlan] = []

    roots = {query, mutation, subscription} - {None}

    for cls, meta in types.items():
        if meta.kind is TypeKind.OBJECT:
            type_kind = (
                TypeKind.SUBSCRIPTION
                if subscription is not None and cls is subscription
                else meta.kind
            )
            field_plans = _build_field_plans(cls, meta, type_kind, is_root=cls in roots)
            type_plans.append(
                TypePlan(
                    kind=type_kind,
                    name=meta.name,
                    cls=cls,
                    fields=tuple(field_plans),
                    description=meta.description,
                )
            )
        elif meta.kind is TypeKind.INPUT:
            field_plans = _build_dataclass_field_plans(cls, expect_input=True)
            type_plans.append(
                TypePlan(
                    kind=TypeKind.INPUT,
                    name=meta.name,
                    cls=cls,
                    fields=tuple(field_plans),
                    description=meta.description,
                )
            )

    return type_plans


def _build_dataclass_field_plans(
    cls: "pytype", *, expect_input: bool, is_root: bool = False
) -> list[FieldPlan]:
    """Build FieldPlan entries for dataclass fields (shared by object and input types)."""
    field_plans: list[FieldPlan] = []
    hints = get_annotations(cls)

    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue

        if expect_input:
            force_nullable = (
                dc_field.default is not MISSING
                or dc_field.default_factory is not MISSING
            )
        else:
            force_nullable = dc_field.default is None

        type_spec = _type_spec_from_annotation(
            annotation, expect_input=expect_input, force_nullable=force_nullable
        )

        annotated_field = _get_annotated_field_meta(annotation)
        description = (
            annotated_field.description if annotated_field is not None else None
        )

        if expect_input:
            default_value = _input_field_default(dc_field, annotation)
            field_default = (
                default_value if default_value is not MISSING else NO_DEFAULT
            )
            field_plans.append(
                FieldPlan(
                    name=dc_field.name,
                    source=dc_field.name,
                    type_spec=type_spec,
                    default=field_default,
                    description=description,
                )
            )
        else:
            if is_root:
                default = _resolve_root_default(dc_field)
                func = _make_default_resolver(default)
            else:
                func = attrgetter(dc_field.name)
            field_plans.append(
                FieldPlan(
                    name=dc_field.name,
                    source=dc_field.name,
                    type_spec=type_spec,
                    func=func,
                    shape="self_only",
                    description=description,
                )
            )

    return field_plans


def _resolve_root_default(dc_field: "dataclasses.Field[Any]") -> object:
    """Extract the default value for a root type's plain dataclass field."""
    if dc_field.default is not MISSING:
        return dc_field.default
    if dc_field.default_factory is not MISSING:
        return dc_field.default_factory()
    msg = (
        f"Root type field '{dc_field.name}' has no default value. "
        "Root types have no parent object, so all plain fields must have defaults."
    )
    raise TypeError(msg)


def _make_default_resolver(value: object) -> "Callable[..., Any]":
    """Create a resolver that returns a fixed default value (for root type fields)."""

    def _resolver(self: object) -> object:  # noqa: ARG001
        return value

    return _resolver


def _build_field_plans(
    cls: "pytype", meta: "TypeMeta", type_kind: TypeKind, *, is_root: bool = False
) -> list[FieldPlan]:
    """Build FieldPlan for object/subscription fields."""
    field_plans = _build_dataclass_field_plans(cls, expect_input=False, is_root=is_root)

    # Resolver-backed fields (from @grommet.field sentinels)
    for attr_name, attr_value in vars(cls).items():
        if not _is_field_resolver(attr_value):
            continue

        resolver_hints = get_annotations(attr_value.resolver)
        annotation = resolver_hints.get("return", inspect._empty)
        if annotation is inspect._empty:
            continue

        if type_kind is TypeKind.SUBSCRIPTION:
            annotation, iterator_optional = unwrap_async_iterable(annotation)
        else:
            iterator_optional = False

        force_nullable = iterator_optional
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=False, force_nullable=force_nullable
        )

        field_name = attr_value.name or attr_name
        info = _analyze_resolver(
            attr_value.resolver, kind=type_kind, field_name=field_name
        )
        field_plans.append(
            FieldPlan(
                name=field_name,
                source=attr_name,
                type_spec=type_spec,
                func=info.func,
                shape=info.shape,
                arg_names=info.arg_names,
                is_async=info.is_async,
                is_async_gen=info.is_async_gen,
                args=info.args,
                description=attr_value.description,
            )
        )

    return field_plans
