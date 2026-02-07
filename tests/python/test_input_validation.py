from dataclasses import dataclass

import grommet as gm


@gm.input
@dataclass
class RequiredInput:
    id: gm.ID
    name: str | None = None


@gm.type
@dataclass
class RequiredQuery:
    @gm.field
    async def lookup(self, data: RequiredInput) -> str:
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
    async def nested(self, payload: OuterInput) -> int:
        return payload.inner.value


async def test_missing_required_input_field_reports_error() -> None:
    """
    Ensures missing required input fields yield validation errors.
    """
    schema = gm.Schema(query=RequiredQuery)
    result = await schema.execute(
        "query ($data: RequiredInput!) { lookup(data: $data) }",
        variables={"data": {"name": "Ada"}},
    )

    assert result.errors, "Expected missing required field error"


async def test_missing_nested_required_input_field_reports_error() -> None:
    """
    Ensures missing nested required fields yield validation errors.
    """
    schema = gm.Schema(query=NestedQuery)
    result = await schema.execute(
        "query ($payload: OuterInput!) { nested(payload: $payload) }",
        variables={"payload": {"inner": {}}},
    )

    assert result.errors, "Expected missing nested field error"
