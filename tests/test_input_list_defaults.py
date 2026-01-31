from dataclasses import dataclass, field
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


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
    @staticmethod
    async def total(parent: "Any", info: "Any", payload: Payload) -> int:
        return sum(item.value for item in payload.items)


@pytest.mark.anyio
async def test_input_list_defaults_apply() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute(
        "query ($payload: Payload!) { total(payload: $payload) }",
        variables={"payload": {}},
    )

    assert result["data"]["total"] == 1
