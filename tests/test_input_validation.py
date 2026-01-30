from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.input
@dataclass
class RequiredInput:
    id: gm.ID
    name: str | None = None


@gm.type
@dataclass
class RequiredQuery:
    @gm.field
    @staticmethod
    async def lookup(parent: "Any", info: "Any", data: RequiredInput) -> str:
        return str(data.id)


@gm.input
@dataclass
class InnerInput:
    value: int


@gm.input
@dataclass
class OuterInput:
    inner: InnerInput


@gm.type
@dataclass
class NestedQuery:
    @gm.field
    @staticmethod
    async def nested(parent: "Any", info: "Any", payload: OuterInput) -> int:
        return payload.inner.value


@pytest.mark.anyio
async def test_missing_required_input_field_reports_error() -> None:
    schema = gm.Schema(query=RequiredQuery)
    result = await schema.execute(
        "query ($data: RequiredInput!) { lookup(data: $data) }",
        variables={"data": {"name": "Ada"}},
    )

    assert result["errors"], "Expected missing required field error"


@pytest.mark.anyio
async def test_missing_nested_required_input_field_reports_error() -> None:
    schema = gm.Schema(query=NestedQuery)
    result = await schema.execute(
        "query ($payload: OuterInput!) { nested(payload: $payload) }",
        variables={"payload": {"inner": {}}},
    )

    assert result["errors"], "Expected missing nested field error"
