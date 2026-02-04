import dataclasses
from builtins import type as pytype
from typing import TYPE_CHECKING

from . import _core
from .annotations import is_internal_field, unwrap_async_iterable
from .coercion import _input_field_default
from .errors import schema_requires_query, unknown_type_kind
from .metadata import MISSING, EnumMeta, ScalarMeta, TypeMeta, UnionMeta
from .registry import _get_field_meta, _traverse_schema
from .resolver import _wrap_resolver
from .typespec import _get_type_meta, _maybe_type_name, _type_spec_from_annotation
from .typing_utils import _get_type_hints

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Callable
    from typing import Any, Protocol

    class SubscriptionStream(Protocol):
        def __aiter__(self) -> AsyncIterator[dict[str, Any]]: ...

        async def __anext__(self) -> dict[str, Any]: ...

        async def aclose(self) -> None: ...


class Schema:
    """Builds and executes a GraphQL schema."""

    def __init__(
        self,
        *,
        query: pytype,
        mutation: pytype | None = None,
        subscription: pytype | None = None,
    ) -> None:
        if query is None:
            raise schema_requires_query()
        entrypoints = [query, mutation, subscription]
        traversal = _traverse_schema(entrypoints)
        registry = traversal.types
        scalar_registry = traversal.scalars
        enum_registry = traversal.enums
        union_registry = traversal.unions
        definition, resolvers, scalar_bindings = _build_schema_definition(
            query=query,
            mutation=mutation,
            subscription=subscription,
            registry=registry,
            scalars=scalar_registry,
            enums=enum_registry,
            unions=union_registry,
        )
        self._core = _core.Schema(definition, resolvers, scalar_bindings)

    async def execute(
        self,
        query: str,
        variables: dict[str, "Any"] | None = None,
        root: "Any | None" = None,
        context: "Any | None" = None,
    ) -> dict[str, "Any"]:
        return await self._core.execute(query, variables, root, context)

    def subscribe(
        self,
        query: str,
        variables: dict[str, "Any"] | None = None,
        root: "Any | None" = None,
        context: "Any | None" = None,
    ) -> "SubscriptionStream":
        return self._core.subscribe(query, variables, root, context)

    def sdl(self) -> str:
        return self._core.sdl()

    def __repr__(self) -> str:
        return f"Schema(query={pytype(self._core).__name__})"


def _build_schema_definition(
    *,
    query: pytype,
    mutation: pytype | None,
    subscription: pytype | None,
    registry: dict[pytype, TypeMeta],
    scalars: dict[pytype, ScalarMeta],
    enums: dict[pytype, EnumMeta],
    unions: dict[pytype, UnionMeta],
) -> "tuple[dict[str, Any], dict[str, Callable[..., Any]], list[dict[str, Any]]]":
    types_def: list[dict[str, "Any"]] = []
    resolvers: dict[str, "Callable[..., Any]"] = {}

    for cls, meta in registry.items():
        if meta.kind in ("object", "interface"):
            is_interface = meta.kind == "interface"
            type_kind = (
                "subscription"
                if subscription is not None and cls is subscription
                else meta.kind
            )
            fields_def: list[dict[str, "Any"]] = []
            hints = _get_type_hints(cls)
            for dc_field in dataclasses.fields(cls):
                field_meta = _get_field_meta(dc_field)
                field_name = field_meta.name or dc_field.name
                annotation = hints.get(dc_field.name, dc_field.type)
                if is_internal_field(dc_field.name, annotation):
                    # keep internal fields on the class but skip them in the schema
                    continue
                force_nullable = dc_field.default is None
                if type_kind == "subscription" and field_meta.resolver is not None:
                    annotation, iterator_optional = unwrap_async_iterable(annotation)
                    force_nullable = force_nullable or iterator_optional
                gql_type = _type_spec_from_annotation(
                    annotation, expect_input=False, force_nullable=force_nullable
                ).to_graphql()
                resolver_key = None
                args_def: list[dict[str, "Any"]] = []
                if field_meta.resolver is not None:
                    wrapper, args_def = _wrap_resolver(
                        field_meta.resolver, kind=type_kind, field_name=field_name
                    )
                    if not is_interface:
                        resolver_key = f"{meta.name}.{field_name}"
                        resolvers[resolver_key] = wrapper
                fields_def.append(
                    {
                        "name": field_name,
                        "source": dc_field.name,
                        "type": gql_type,
                        "args": args_def,
                        "resolver": resolver_key,
                        "description": field_meta.description,
                        "deprecation": field_meta.deprecation_reason,
                    }
                )
            types_def.append(
                {
                    "kind": type_kind,
                    "name": meta.name,
                    "fields": fields_def,
                    "description": meta.description,
                    "implements": [
                        _get_type_meta(iface).name for iface in meta.implements
                    ],
                }
            )
        elif meta.kind == "input":
            input_fields_def: list[dict[str, "Any"]] = []
            hints = _get_type_hints(cls)
            for dc_field in dataclasses.fields(cls):
                annotation = hints.get(dc_field.name, dc_field.type)
                if is_internal_field(dc_field.name, annotation):
                    # keep internal inputs for python use but skip graphql exposure
                    continue
                force_nullable = (
                    dc_field.default is not MISSING
                    or dc_field.default_factory is not MISSING
                )
                gql_type = _type_spec_from_annotation(
                    annotation, expect_input=True, force_nullable=force_nullable
                ).to_graphql()
                field_def: dict[str, "Any"] = {"name": dc_field.name, "type": gql_type}
                default_value = _input_field_default(dc_field, annotation)
                if default_value is not MISSING:
                    field_def["default"] = default_value
                input_fields_def.append(field_def)
            types_def.append(
                {
                    "kind": "input",
                    "name": meta.name,
                    "fields": input_fields_def,
                    "description": meta.description,
                }
            )
        else:
            raise unknown_type_kind(meta.kind)

    schema_def = {
        "schema": {
            "query": _get_type_meta(query).name,
            "mutation": _maybe_type_name(mutation),
            "subscription": _maybe_type_name(subscription),
        },
        "types": types_def,
        "scalars": [
            {
                "name": meta.name,
                "description": meta.description,
                "specified_by_url": meta.specified_by_url,
            }
            for meta in scalars.values()
        ],
        "enums": [
            {
                "name": meta.name,
                "description": meta.description,
                "values": list(getattr(enum_type, "__members__", {}).keys()),
            }
            for enum_type, meta in enums.items()
        ],
        "unions": [
            {
                "name": meta.name,
                "description": meta.description,
                "types": [_get_type_meta(tp).name for tp in meta.types],
            }
            for meta in unions.values()
        ],
    }
    scalar_bindings = [
        {
            "name": meta.name,
            "python_type": scalar_type,
            "serialize": meta.serialize,
            "parse_value": meta.parse_value,
        }
        for scalar_type, meta in scalars.items()
    ]
    return schema_def, resolvers, scalar_bindings
