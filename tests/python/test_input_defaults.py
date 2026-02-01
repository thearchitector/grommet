from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class ArgDefaultQuery:
    @gm.field
    @staticmethod
    async def greet(parent: "Any", info: "Any", name: str = "Ada") -> str:
        return f"hi {name}"


@gm.input
@dataclass
class Options:
    enabled: bool = True


@gm.type
@dataclass
class InputDefaultQuery:
    @gm.field
    @staticmethod
    async def enabled(parent: "Any", info: "Any", options: Options) -> bool:
        return options.enabled


@gm.type
@dataclass
class InputValidationQuery:
    @gm.field
    @staticmethod
    async def enabled(parent: "Any", info: "Any", options: Options) -> bool:
        return options.enabled


@pytest.mark.anyio
async def test_argument_default_value_is_applied() -> None:
    schema = gm.Schema(query=ArgDefaultQuery)
    result = await schema.execute("{ greet }")

    assert result["data"]["greet"] == "hi Ada"


@pytest.mark.anyio
async def test_input_field_default_value_is_applied() -> None:
    schema = gm.Schema(query=InputDefaultQuery)
    result = await schema.execute(
        "query ($options: Options!) { enabled(options: $options) }",
        variables={"options": {}},
    )

    assert result["data"]["enabled"] is True


@pytest.mark.anyio
async def test_invalid_input_value_reports_error() -> None:
    schema = gm.Schema(query=InputValidationQuery)
    result = await schema.execute(
        "query ($options: Options!) { enabled(options: $options) }",
        variables={"options": "nope"},
    )

    assert result["errors"], "Expected input validation error"
