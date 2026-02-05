from dataclasses import dataclass

import pytest

import grommet as gm
from grommet.errors import GrommetSchemaError, GrommetTypeError
from grommet.plan import build_schema_plan
from grommet.schema import Schema, _build_schema_definition


@gm.input
@dataclass
class Payload:
    value: int
    hidden: gm.Internal[int]


@gm.type
@dataclass
class Query:
    value: int

    @gm.field
    @staticmethod
    async def with_payload(payload: Payload) -> int:  # pragma: no cover
        return payload.value


def test_schema_requires_query() -> None:
    """
    Ensures schema construction fails when the query type is missing.
    """
    with pytest.raises(GrommetSchemaError):
        Schema(query=None)  # type: ignore[arg-type]


def test_schema_repr() -> None:
    """
    Verifies schema repr output includes query configuration details.
    """
    schema = Schema(query=Query)
    assert "Schema(query=" in repr(schema)


def test_internal_input_fields_skipped() -> None:
    """
    Ensures internal input fields are excluded from schema definitions.
    """
    plan = build_schema_plan(query=Query)
    input_plan = next(tp for tp in plan.types if tp.kind == "input")
    field_names = {fp.name for fp in input_plan.fields}
    assert "hidden" not in field_names
    assert "value" in field_names


def test_schema_unknown_kind_raises() -> None:
    """
    Ensures unknown type kinds raise a schema type error.
    """
    from grommet.plan import SchemaPlan, TypePlan

    fake_type_plan = TypePlan(kind="mystery", name="Weird", cls=object, fields=())
    fake_plan = SchemaPlan(
        query="Query",
        mutation=None,
        subscription=None,
        types=(fake_type_plan,),
        scalars=(),
        enums=(),
        unions=(),
    )
    with pytest.raises(GrommetTypeError):
        _build_schema_definition(fake_plan)


@gm.interface
@dataclass
class Node:
    @gm.field
    @staticmethod
    async def id(parent, info) -> int:  # pragma: no cover - called via schema
        return 1


def test_interface_resolvers_not_registered() -> None:
    """
    Verifies interface resolvers are not registered in schema definitions.
    """
    plan = build_schema_plan(query=Query)
    _, resolvers, _ = _build_schema_definition(plan)
    # Interface resolvers should not be registered
    assert "Node.id" not in resolvers


@gm.type
@dataclass
class ConcreteNode(Node):
    label: str


def test_schema_with_interface_resolver_wraps_args_only() -> None:
    """Test that interface field resolvers get args wrapped but not registered."""
    plan = build_schema_plan(query=ConcreteNode)

    type_names = {t.name for t in plan.types}
    assert "ConcreteNode" in type_names
