from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class ArgDefaultQuery:
    @gm.field
    async def greet(self, name: str = "Ada") -> str:
        return f"hi {name}"


@gm.input
@dataclass
class Options:
    enabled: bool = True


@gm.type
@dataclass
class InputDefaultQuery:
    @gm.field
    async def enabled(self, options: Options) -> bool:
        return options.enabled


@gm.type
@dataclass
class InputValidationQuery:
    @gm.field
    async def enabled(self, options: Options) -> bool:
        return options.enabled


async def test_argument_default_value_is_applied() -> None:
    """
    Verifies argument default values are applied when omitted.
    """
    schema = gm.Schema(query=ArgDefaultQuery)
    result = await schema.execute("{ greet }")

    assert result.data["greet"] == "hi Ada"


async def test_input_field_default_value_is_applied() -> None:
    """
    Verifies input field defaults are applied when input objects are empty.
    """
    schema = gm.Schema(query=InputDefaultQuery)
    result = await schema.execute(
        "query ($options: Options!) { enabled(options: $options) }",
        variables={"options": {}},
    )

    assert result.data["enabled"] is True


async def test_invalid_input_value_reports_error() -> None:
    """
    Ensures invalid input values return GraphQL errors.
    """
    schema = gm.Schema(query=InputValidationQuery)
    result = await schema.execute(
        "query ($options: Options!) { enabled(options: $options) }",
        variables={"options": "nope"},
    )

    assert result.errors, "Expected input validation error"
