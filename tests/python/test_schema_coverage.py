from dataclasses import dataclass

import pytest

import grommet as gm
from grommet.errors import GrommetSchemaError, GrommetTypeError
from grommet.registry import _traverse_schema
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
    traversal = _traverse_schema([Query, Payload])
    definition, _, _ = _build_schema_definition(
        query=Query,
        mutation=None,
        subscription=None,
        registry=traversal.types,
        scalars=traversal.scalars,
        enums=traversal.enums,
        unions=traversal.unions,
    )
    input_def = next(item for item in definition["types"] if item["kind"] == "input")
    field_names = {field["name"] for field in input_def["fields"]}
    assert "hidden" not in field_names
    assert "value" in field_names


def test_schema_unknown_kind_raises() -> None:
    """
    Ensures unknown type kinds raise a schema type error.
    """

    class Weird:
        pass

    fake_meta = type("FakeMeta", (), {"kind": "mystery"})()
    with pytest.raises(GrommetTypeError):
        _build_schema_definition(
            query=Query,
            mutation=None,
            subscription=None,
            registry={Weird: fake_meta},
            scalars={},
            enums={},
            unions={},
        )


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
    traversal = _traverse_schema([Node])
    _, resolvers, _ = _build_schema_definition(
        query=Query,
        mutation=None,
        subscription=None,
        registry=traversal.types,
        scalars=traversal.scalars,
        enums=traversal.enums,
        unions=traversal.unions,
    )
    assert resolvers == {}
