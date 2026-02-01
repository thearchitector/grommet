from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.scalar(
    name="Date",
    serialize=lambda value: value.value,
    parse_value=lambda value: Date(str(value)),
)
@dataclass(frozen=True)
class Date:
    value: str


class StrictInt(int):
    pass


def _serialize_strict_int(value: StrictInt) -> int:
    return int(value)


def _parse_strict_int(value: "Any") -> StrictInt:
    try:
        return StrictInt(int(value))
    except (TypeError, ValueError) as exc:
        raise ValueError("Invalid StrictInt value.") from exc


gm.scalar(
    name="StrictInt",
    serialize=_serialize_strict_int,
    parse_value=_parse_strict_int,
)(StrictInt)


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def echo(parent: "Any", info: "Any", when: Date) -> Date:
        assert isinstance(when, Date)
        return when


@gm.type
@dataclass
class StrictQuery:
    @gm.field
    @staticmethod
    async def accept(parent: "Any", info: "Any", value: StrictInt) -> StrictInt:
        return value


@pytest.mark.anyio
async def test_custom_scalar_parse_and_serialize() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute(
        "query ($when: Date!) { echo(when: $when) }",
        variables={"when": "2026-01-30"},
    )

    assert result["data"]["echo"] == "2026-01-30"


@pytest.mark.anyio
async def test_custom_scalar_serializes_python_value() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute(
        "query ($when: Date!) { echo(when: $when) }",
        variables={"when": Date("2026-01-30")},
    )

    assert result["data"]["echo"] == "2026-01-30"


@pytest.mark.anyio
async def test_custom_scalar_invalid_input_reports_error() -> None:
    schema = gm.Schema(query=StrictQuery)
    result = await schema.execute(
        "query ($value: StrictInt!) { accept(value: $value) }",
        variables={"value": "nope"},
    )

    assert result["errors"], "Expected strict scalar parse error"
