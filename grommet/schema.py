from builtins import type as pytype
from typing import TYPE_CHECKING

from . import _core
from .errors import schema_requires_query, unknown_type_kind
from .plan import _NO_DEFAULT, SchemaPlan, build_schema_plan
from .resolver import _wrap_resolver

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
        plan = build_schema_plan(
            query=query, mutation=mutation, subscription=subscription
        )
        definition, resolvers, scalar_bindings = _build_schema_definition(plan)
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
    plan: SchemaPlan,
) -> "tuple[dict[str, Any], dict[str, Callable[..., Any]], list[dict[str, Any]]]":
    """Convert a SchemaPlan to the dict format expected by Rust."""
    types_def: list[dict[str, "Any"]] = []
    resolvers: dict[str, "Callable[..., Any]"] = {}

    for type_plan in plan.types:
        if type_plan.kind in ("object", "interface", "subscription"):
            is_interface = type_plan.kind == "interface"
            fields_def: list[dict[str, "Any"]] = []

            for field_plan in type_plan.fields:
                resolver_key = None
                args_def: list[dict[str, "Any"]] = []

                if field_plan.resolver is not None and not is_interface:
                    wrapper, args_def = _wrap_resolver(
                        field_plan.resolver,
                        kind=type_plan.kind,
                        field_name=field_plan.name,
                    )
                    resolver_key = f"{type_plan.name}.{field_plan.name}"
                    resolvers[resolver_key] = wrapper
                elif field_plan.resolver is not None:
                    _, args_def = _wrap_resolver(
                        field_plan.resolver,
                        kind=type_plan.kind,
                        field_name=field_plan.name,
                    )

                fields_def.append(
                    {
                        "name": field_plan.name,
                        "source": field_plan.source,
                        "type": field_plan.graphql_type,
                        "args": args_def,
                        "resolver": resolver_key,
                        "description": field_plan.description,
                        "deprecation": field_plan.deprecation,
                    }
                )

            types_def.append(
                {
                    "kind": type_plan.kind,
                    "name": type_plan.name,
                    "fields": fields_def,
                    "description": type_plan.description,
                    "implements": list(type_plan.implements),
                }
            )
        elif type_plan.kind == "input":
            input_fields_def: list[dict[str, "Any"]] = []

            for field_plan in type_plan.fields:
                field_def: dict[str, "Any"] = {
                    "name": field_plan.name,
                    "type": field_plan.graphql_type,
                }
                if field_plan.default is not _NO_DEFAULT:
                    field_def["default"] = field_plan.default
                input_fields_def.append(field_def)

            types_def.append(
                {
                    "kind": "input",
                    "name": type_plan.name,
                    "fields": input_fields_def,
                    "description": type_plan.description,
                }
            )
        else:
            raise unknown_type_kind(type_plan.kind)

    schema_def = {
        "schema": {
            "query": plan.query,
            "mutation": plan.mutation,
            "subscription": plan.subscription,
        },
        "types": types_def,
        "scalars": [
            {
                "name": scalar_plan.meta.name,
                "description": scalar_plan.meta.description,
                "specified_by_url": scalar_plan.meta.specified_by_url,
            }
            for scalar_plan in plan.scalars
        ],
        "enums": [
            {
                "name": enum_plan.meta.name,
                "description": enum_plan.meta.description,
                "values": list(getattr(enum_plan.cls, "__members__", {}).keys()),
            }
            for enum_plan in plan.enums
        ],
        "unions": [
            {
                "name": union_plan.meta.name,
                "description": union_plan.meta.description,
                "types": [t.__grommet_meta__.name for t in union_plan.meta.types],  # type: ignore[attr-defined]
            }
            for union_plan in plan.unions
        ],
    }
    scalar_bindings = [
        {
            "name": scalar_plan.meta.name,
            "python_type": scalar_plan.cls,
            "serialize": scalar_plan.meta.serialize,
            "parse_value": scalar_plan.meta.parse_value,
        }
        for scalar_plan in plan.scalars
    ]
    return schema_def, resolvers, scalar_bindings
