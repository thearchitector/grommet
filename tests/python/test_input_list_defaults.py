from dataclasses import dataclass, field

import grommet as gm


@gm.input
@dataclass
class Item:
    value: int = 1


@gm.input
@dataclass
class Payload:
    items: list[Item] = field(default_factory=lambda: [Item()])


@gm.type
@dataclass
class Query:
    @gm.field
    async def total(self, payload: Payload) -> int:
        return sum(item.value for item in payload.items)


async def test_input_list_defaults_apply() -> None:
    """
    Verifies list input defaults are applied when omitted in variables.
    """
    schema = gm.Schema(query=Query)
    result = await schema.execute(
        "query ($payload: Payload!) { total(payload: $payload) }",
        variables={"payload": {}},
    )

    assert result.data["total"] == 1
