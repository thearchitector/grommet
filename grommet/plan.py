import dataclasses
import inspect
from builtins import type as pytype
from typing import TYPE_CHECKING

from .annotations import is_internal_field, unwrap_async_iterable, walk_annotation
from .metadata import (
    MISSING,
    EnumMeta,
    FieldMeta,
    ScalarMeta,
    TypeMeta,
    UnionMeta,
    _interface_implementers,
)
from .typespec import (
    _get_enum_meta,
    _get_scalar_meta,
    _get_type_meta,
    _get_union_meta,
    _is_enum_type,
    _is_grommet_type,
    _is_scalar_type,
    _is_union_type,
    _type_spec_from_annotation,
)
from .typing_utils import _get_type_hints

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any


@dataclasses.dataclass(frozen=True, slots=True)
class ArgPlan:
    """Planned argument for a resolver field."""

    name: str
    graphql_type: str
    default: "Any" = MISSING


_NO_DEFAULT: "Any" = object()


@dataclasses.dataclass(frozen=True, slots=True)
class FieldPlan:
    """Planned field for a GraphQL type."""

    name: str
    source: str
    graphql_type: str
    resolver: "Callable[..., Any] | None" = None
    args: tuple["ArgPlan", ...] = ()
    default: "Any" = _NO_DEFAULT
    description: str | None = None
    deprecation: str | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class TypePlan:
    """Planned GraphQL type (object, interface, input, or subscription)."""

    kind: str
    name: str
    cls: pytype
    fields: tuple[FieldPlan, ...]
    implements: tuple[str, ...] = ()
    description: str | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class ScalarPlan:
    """Planned GraphQL scalar."""

    cls: pytype
    meta: ScalarMeta


@dataclasses.dataclass(frozen=True, slots=True)
class EnumPlan:
    """Planned GraphQL enum."""

    cls: pytype
    meta: EnumMeta


@dataclasses.dataclass(frozen=True, slots=True)
class UnionPlan:
    """Planned GraphQL union."""

    cls: pytype
    meta: UnionMeta


@dataclasses.dataclass(frozen=True, slots=True)
class SchemaPlan:
    """Complete planned schema with all types, scalars, enums, and unions."""

    query: str
    mutation: str | None
    subscription: str | None
    types: tuple[TypePlan, ...]
    scalars: tuple[ScalarPlan, ...]
    enums: tuple[EnumPlan, ...]
    unions: tuple[UnionPlan, ...]


def _get_field_meta(dc_field: "dataclasses.Field[Any]") -> FieldMeta:
    meta = dc_field.metadata.get("grommet") if dc_field.metadata else None
    if isinstance(meta, FieldMeta):
        return meta
    return FieldMeta()


def build_schema_plan(
    *, query: pytype, mutation: pytype | None = None, subscription: pytype | None = None
) -> SchemaPlan:
    """
    Build a complete schema plan by traversing types once.

    This consolidates both discovery (what types exist) and planning (how to
    build fields) into a single pass over the dataclasses.
    """
    types: dict[pytype, TypeMeta] = {}
    scalars: dict[pytype, ScalarMeta] = {}
    enums: dict[pytype, EnumMeta] = {}
    unions: dict[pytype, UnionMeta] = {}

    entrypoints = [query, mutation, subscription]
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
                from .resolver import _resolver_arg_annotations

                arg_types = _resolver_arg_annotations(field_meta.resolver)
                for arg_ann in arg_types.values():
                    track_annotation(arg_ann)

    type_plans = _build_type_plans(
        types, query, mutation, subscription, scalars, enums, unions
    )

    return SchemaPlan(
        query=_get_type_meta(query).name,
        mutation=_get_type_meta(mutation).name if mutation else None,
        subscription=_get_type_meta(subscription).name if subscription else None,
        types=tuple(type_plans),
        scalars=tuple(ScalarPlan(cls=cls, meta=meta) for cls, meta in scalars.items()),
        enums=tuple(EnumPlan(cls=cls, meta=meta) for cls, meta in enums.items()),
        unions=tuple(UnionPlan(cls=cls, meta=meta) for cls, meta in unions.items()),
    )


def _build_type_plans(
    types: dict[pytype, TypeMeta],
    query: pytype,
    mutation: pytype | None,
    subscription: pytype | None,
    scalars: dict[pytype, ScalarMeta],
    enums: dict[pytype, EnumMeta],
    unions: dict[pytype, UnionMeta],
) -> list[TypePlan]:
    """Build TypePlan for each discovered type."""
    type_meta_cache = types
    type_plans: list[TypePlan] = []

    def type_name(tp: pytype) -> str:
        meta = type_meta_cache.get(tp)
        if meta is None:
            meta = _get_type_meta(tp)
            type_meta_cache[tp] = meta
        return meta.name

    for cls, meta in types.items():
        if meta.kind in ("object", "interface"):
            is_interface = meta.kind == "interface"
            type_kind = (
                "subscription"
                if subscription is not None and cls is subscription
                else meta.kind
            )
            field_plans = _build_field_plans(cls, meta, type_kind, is_interface)
            type_plans.append(
                TypePlan(
                    kind=type_kind,
                    name=meta.name,
                    cls=cls,
                    fields=tuple(field_plans),
                    implements=tuple(type_name(iface) for iface in meta.implements),
                    description=meta.description,
                )
            )
        elif meta.kind == "input":
            field_plans = _build_input_field_plans(cls)
            type_plans.append(
                TypePlan(
                    kind="input",
                    name=meta.name,
                    cls=cls,
                    fields=tuple(field_plans),
                    description=meta.description,
                )
            )

    return type_plans


def _build_field_plans(
    cls: pytype, meta: TypeMeta, type_kind: str, is_interface: bool
) -> list[FieldPlan]:
    """Build FieldPlan for object/interface/subscription fields."""
    from .coercion import _default_value_for_annotation
    from .resolver import _resolver_arg_annotations

    field_plans: list[FieldPlan] = []
    hints = _get_type_hints(cls)

    for dc_field in dataclasses.fields(cls):
        field_meta = _get_field_meta(dc_field)
        field_name = field_meta.name or dc_field.name
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_internal_field(dc_field.name, annotation):
            continue

        force_nullable = dc_field.default is None
        if type_kind == "subscription" and field_meta.resolver is not None:
            annotation, iterator_optional = unwrap_async_iterable(annotation)
            force_nullable = force_nullable or iterator_optional

        gql_type = _type_spec_from_annotation(
            annotation, expect_input=False, force_nullable=force_nullable
        ).to_graphql()

        resolver = field_meta.resolver if not is_interface else None
        args: list[ArgPlan] = []

        if field_meta.resolver is not None:
            arg_annotations = _resolver_arg_annotations(field_meta.resolver)
            from .resolver import _RESERVED_PARAM_NAMES, _resolver_params

            params = _resolver_params(field_meta.resolver)
            arg_params = [p for p in params if p.name not in _RESERVED_PARAM_NAMES]

            for param in arg_params:
                arg_annotation = arg_annotations.get(param.name, param.annotation)
                if arg_annotation is inspect._empty:
                    continue
                arg_force_nullable = param.default is not inspect._empty
                arg_spec = _type_spec_from_annotation(
                    arg_annotation, expect_input=True, force_nullable=arg_force_nullable
                )
                arg_default = MISSING
                if param.default is not inspect._empty:
                    arg_default = _default_value_for_annotation(
                        arg_annotation, param.default
                    )
                args.append(
                    ArgPlan(
                        name=param.name,
                        graphql_type=arg_spec.to_graphql(),
                        default=arg_default,
                    )
                )

        field_plans.append(
            FieldPlan(
                name=field_name,
                source=dc_field.name,
                graphql_type=gql_type,
                resolver=resolver,
                args=tuple(args),
                description=field_meta.description,
                deprecation=field_meta.deprecation_reason,
            )
        )

    return field_plans


def _build_input_field_plans(cls: pytype) -> list[FieldPlan]:
    """Build FieldPlan for input type fields."""
    from .coercion import _input_field_default

    field_plans: list[FieldPlan] = []
    hints = _get_type_hints(cls)

    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_internal_field(dc_field.name, annotation):
            continue

        force_nullable = (
            dc_field.default is not MISSING or dc_field.default_factory is not MISSING
        )
        gql_type = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=force_nullable
        ).to_graphql()

        default_value = _input_field_default(dc_field, annotation)
        field_default = default_value if default_value is not MISSING else _NO_DEFAULT

        field_plans.append(
            FieldPlan(
                name=dc_field.name,
                source=dc_field.name,
                graphql_type=gql_type,
                default=field_default,
            )
        )

    return field_plans
