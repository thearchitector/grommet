import dataclasses
import inspect
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
from .coercion import _default_value_for_annotation, _input_field_default
from .decorators import _FieldResolver
from .metadata import MISSING, NO_DEFAULT, Field, TypeKind
from .resolver import _resolver_arg_info, _wrap_resolver

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any

    from .metadata import TypeMeta, TypeSpec


@dataclasses.dataclass(frozen=True, slots=True)
class ArgPlan:
    """Planned argument for a resolver field."""

    name: str
    type_spec: "TypeSpec"
    default: "Any" = NO_DEFAULT


@dataclasses.dataclass(frozen=True, slots=True)
class FieldPlan:
    """Planned field for a GraphQL type."""

    name: str
    source: str
    type_spec: "TypeSpec"
    resolver: "Callable[..., Any] | None" = None
    args: tuple["ArgPlan", ...] = ()
    default: "Any" = NO_DEFAULT
    description: str | None = None
    deprecation: str | None = None
    resolver_key: str | None = None


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
    resolvers: dict[str, "Callable[..., Any]"] = dataclasses.field(default_factory=dict)


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

    resolvers: dict[str, "Callable[..., Any]"] = {}
    resolved_type_plans = _wrap_plan_resolvers(type_plans, resolvers)

    return SchemaPlan(
        query=_get_type_meta(query).name,
        mutation=_get_type_meta(mutation).name if mutation else None,
        subscription=_get_type_meta(subscription).name if subscription else None,
        types=tuple(resolved_type_plans),
        resolvers=resolvers,
    )


def _wrap_plan_resolvers(
    type_plans: list[TypePlan], resolvers: dict[str, "Callable[..., Any]"]
) -> list[TypePlan]:
    """Wrap resolvers on field plans and populate resolver keys."""

    result: list[TypePlan] = []
    for tp in type_plans:
        if tp.kind is TypeKind.INPUT:
            result.append(tp)
            continue
        new_fields: list[FieldPlan] = []
        for fp in tp.fields:
            if fp.resolver is not None:
                wrapper = _wrap_resolver(fp.resolver, kind=tp.kind, field_name=fp.name)
                key = f"{tp.name}.{fp.name}"
                resolvers[key] = wrapper
                new_fields.append(dataclasses.replace(fp, resolver_key=key))
            else:
                new_fields.append(fp)
        result.append(dataclasses.replace(tp, fields=tuple(new_fields)))
    return result


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
            field_plans = _build_input_field_plans(cls)
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


def _make_root_field_resolver(cls: "pytype", attr_name: str) -> "Callable[..., Any]":
    """Create a synthetic async resolver for a root type's plain dataclass field."""

    async def _root_resolver(self: object) -> object:  # noqa: ARG001
        instance = cls()
        return getattr(instance, attr_name)

    _root_resolver.__name__ = f"_root_{cls.__name__}_{attr_name}"
    _root_resolver.__qualname__ = _root_resolver.__name__
    return _root_resolver


def _build_field_plans(
    cls: "pytype", meta: "TypeMeta", type_kind: TypeKind, *, is_root: bool = False
) -> list[FieldPlan]:
    """Build FieldPlan for object/subscription fields."""

    field_plans: list[FieldPlan] = []
    hints = get_annotations(cls)

    # Dataclass fields (plain data fields)
    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue

        force_nullable = dc_field.default is None
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=False, force_nullable=force_nullable
        )

        resolver = None
        # Root types have no parent in async-graphql, so plain fields need synthetic resolvers
        if is_root:
            resolver = _make_root_field_resolver(cls, dc_field.name)

        # Description from Annotated Field metadata
        annotated_field = _get_annotated_field_meta(annotation)
        description = (
            annotated_field.description if annotated_field is not None else None
        )

        field_plans.append(
            FieldPlan(
                name=dc_field.name,
                source=dc_field.name,
                type_spec=type_spec,
                resolver=resolver,
                description=description,
            )
        )

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

        args: list[ArgPlan] = []
        for param, arg_annotation in _resolver_arg_info(attr_value.resolver):
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
                ArgPlan(name=param.name, type_spec=arg_spec, default=arg_default)
            )

        field_name = attr_value.name or attr_name
        field_plans.append(
            FieldPlan(
                name=field_name,
                source=attr_name,
                type_spec=type_spec,
                resolver=attr_value.resolver,
                args=tuple(args),
                description=attr_value.description,
                deprecation=attr_value.deprecation_reason,
            )
        )

    return field_plans


def _build_input_field_plans(cls: "pytype") -> list[FieldPlan]:
    """Build FieldPlan for input type fields."""

    field_plans: list[FieldPlan] = []
    hints = get_annotations(cls)

    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue

        force_nullable = (
            dc_field.default is not MISSING or dc_field.default_factory is not MISSING
        )
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=force_nullable
        )

        default_value = _input_field_default(dc_field, annotation)
        field_default = default_value if default_value is not MISSING else NO_DEFAULT

        # Extract description from Annotated Field metadata
        annotated_field = _get_annotated_field_meta(annotation)
        description = (
            annotated_field.description if annotated_field is not None else None
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

    return field_plans
