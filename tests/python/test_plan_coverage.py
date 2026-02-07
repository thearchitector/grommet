"""Tests for grommet/plan.py coverage."""

from dataclasses import dataclass
from enum import Enum

import pytest

import grommet as gm
from grommet.errors import GrommetTypeError
from grommet.metadata import TypeKind
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
    async def with_scalar(self, value: CustomScalar) -> str:
        return str(value)

    @gm.field
    async def with_enum(self, status: Status) -> str:
        return status.value

    @gm.field
    async def with_union(self) -> TestUnion:
        return TypeA(name="test")

    @gm.field
    async def with_interface(self) -> Node:
        return ConcreteNode(id=1, label="test")

    @gm.field
    async def with_input(self, filter: FilterInput) -> str:
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
        async def get_item(self) -> TestUnion:
            return TypeA(name="test")

    plan = build_schema_plan(query=QueryWithUnionField)
    union_names = {u.meta.name for u in plan.unions}
    assert "TestUnion" in union_names

    type_names = {t.name for t in plan.types}
    assert "TypeA" in type_names
    assert "TypeB" in type_names


def test_plan_skips_unannotated_resolver_arg() -> None:
    """Ensures _build_field_plans skips resolver args without annotations."""
    from grommet.plan import _build_field_plans

    @gm.type
    @dataclass
    class TypeWithUnannotated:
        @gm.field
        async def compute(self, annotated: int, unannotated) -> int:  # type: ignore[no-untyped-def]
            return annotated

    from grommet.annotations import _get_type_meta

    meta = _get_type_meta(TypeWithUnannotated)
    field_plans = _build_field_plans(
        TypeWithUnannotated, meta, TypeKind.OBJECT, is_interface=False
    )
    fp = next(fp for fp in field_plans if fp.name == "compute")
    arg_names = [a.name for a in fp.args]
    assert "annotated" in arg_names
    assert "unannotated" not in arg_names


def test_plan_interface_field_resolver_not_keyed() -> None:
    """Ensures interface fields with resolvers are kept but not assigned resolver keys."""
    from grommet.plan import FieldPlan, TypePlan, _wrap_plan_resolvers

    async def dummy(self: object) -> int:
        return 1  # pragma: no cover

    iface_plan = TypePlan(
        kind=TypeKind.INTERFACE,
        name="IFace",
        cls=object,
        fields=(
            FieldPlan(
                name="id",
                source="id",
                type_spec=gm.metadata.TypeSpec(kind="named", name="Int"),
                resolver=dummy,
            ),
        ),
    )
    resolvers: dict = {}
    result = _wrap_plan_resolvers([iface_plan], resolvers)
    assert len(resolvers) == 0
    assert result[0].fields[0].resolver_key is None


def test_plan_pending_discovers_union_enum_scalar() -> None:
    """Verifies the pending loop handles union/enum/scalar types from implements."""

    @gm.type
    @dataclass
    class Holder:
        value: str

    HolderUnion = gm.union("HolderUnion", types=[Holder])

    @gm.type
    @dataclass
    class QueryDiscovery:
        items: list[HolderUnion]

    plan = build_schema_plan(query=QueryDiscovery)
    union_names = {u.meta.name for u in plan.unions}
    assert "HolderUnion" in union_names
    type_names = {t.name for t in plan.types}
    assert "Holder" in type_names


def test_plan_pending_loop_union_via_implements() -> None:
    """Covers the union branch in the pending while-loop."""

    @gm.type
    @dataclass
    class MemberA:
        x: int

    ImplUnion = gm.union("ImplUnion", types=[MemberA])

    # Putting a union in implements forces it into the pending list.
    # Discovery succeeds (covering the union branch) but type plan building
    # fails because unions are not valid interface types.
    @gm.type(implements=[ImplUnion])  # type: ignore[arg-type]
    @dataclass
    class TypeWithUnionImpl:
        value: str

    @gm.type
    @dataclass
    class QueryPendingUnion:
        item: TypeWithUnionImpl

    with pytest.raises(GrommetTypeError):
        build_schema_plan(query=QueryPendingUnion)


def test_plan_pending_loop_enum_via_implements() -> None:
    """Covers the enum branch in the pending while-loop."""

    @gm.enum
    class Priority(Enum):
        HIGH = "high"
        LOW = "low"

    @gm.type(implements=[Priority])  # type: ignore[arg-type]
    @dataclass
    class TypeWithEnumImpl:
        value: str

    @gm.type
    @dataclass
    class QueryPendingEnum:
        item: TypeWithEnumImpl

    with pytest.raises(GrommetTypeError):
        build_schema_plan(query=QueryPendingEnum)


def test_plan_pending_loop_scalar_via_implements() -> None:
    """Covers the scalar branch in the pending while-loop."""

    @gm.scalar(serialize=str, parse_value=str)
    class Token:
        pass

    @gm.type(implements=[Token])  # type: ignore[arg-type]
    @dataclass
    class TypeWithScalarImpl:
        value: str

    @gm.type
    @dataclass
    class QueryPendingScalar:
        item: TypeWithScalarImpl

    with pytest.raises(GrommetTypeError):
        build_schema_plan(query=QueryPendingScalar)


def test_plan_pending_loop_skips_non_grommet_type() -> None:
    """Covers the non-grommet type skip in the pending while-loop."""

    class PlainClass:
        pass

    @gm.type(implements=[PlainClass])  # type: ignore[arg-type]
    @dataclass
    class TypeWithPlainImpl:
        value: str

    @gm.type
    @dataclass
    class QueryPendingPlain:
        item: TypeWithPlainImpl

    with pytest.raises(GrommetTypeError):
        build_schema_plan(query=QueryPendingPlain)
