"""Tests for grommet/plan.py coverage."""

from dataclasses import dataclass
from enum import Enum

import grommet as gm
from grommet.plan import build_schema_plan


@gm.scalar(serialize=str, parse_value=str)
class CustomScalar:
    pass


@gm.enum
class Status(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"


@gm.type
@dataclass
class TypeA:
    name: str


@gm.type
@dataclass
class TypeB:
    value: int


TestUnion = gm.union("TestUnion", types=[TypeA, TypeB])


@gm.interface
@dataclass
class Node:
    id: int


@gm.type
@dataclass
class ConcreteNode(Node):
    label: str


@gm.input
@dataclass
class FilterInput:
    status: Status
    custom: CustomScalar | None = None


@gm.type
@dataclass
class QueryWithAllTypes:
    simple: str

    @gm.field
    @staticmethod
    async def with_scalar(value: CustomScalar) -> str:
        return str(value)

    @gm.field
    @staticmethod
    async def with_enum(status: Status) -> str:
        return status.value

    @gm.field
    @staticmethod
    async def with_union() -> TestUnion:
        return TypeA(name="test")

    @gm.field
    @staticmethod
    async def with_interface() -> Node:
        return ConcreteNode(id=1, label="test")

    @gm.field
    @staticmethod
    async def with_input(filter: FilterInput) -> str:
        return filter.status.value


def test_plan_discovers_scalars_enums_unions_via_annotations() -> None:
    """Test that build_schema_plan discovers scalars, enums, unions through field annotations."""
    plan = build_schema_plan(query=QueryWithAllTypes)

    scalar_names = {s.meta.name for s in plan.scalars}
    enum_names = {e.meta.name for e in plan.enums}
    union_names = {u.meta.name for u in plan.unions}

    assert "CustomScalar" in scalar_names
    assert "Status" in enum_names
    assert "TestUnion" in union_names


def test_plan_discovers_interface_implementers() -> None:
    """Test that interface and its implementers are discovered via build_schema_plan."""

    @gm.type(implements=[Node])
    @dataclass
    class ImplNode:
        id: int
        label: str

    @gm.type
    @dataclass
    class QueryWithNode:
        node: Node

    plan = build_schema_plan(query=QueryWithNode)
    type_names = {t.name for t in plan.types}
    assert "Node" in type_names
    assert "ImplNode" in type_names


def test_plan_handles_union_as_entrypoint() -> None:
    """Test that unions can be discovered when passed as entrypoint types."""

    @gm.type
    @dataclass
    class QueryWithUnionField:
        @gm.field
        @staticmethod
        async def get_item() -> TestUnion:
            return TypeA(name="test")

    plan = build_schema_plan(query=QueryWithUnionField)
    union_names = {u.meta.name for u in plan.unions}
    assert "TestUnion" in union_names

    type_names = {t.name for t in plan.types}
    assert "TypeA" in type_names
    assert "TypeB" in type_names
