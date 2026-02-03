from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.input(name="UserInput")
@dataclass
class CoerceUserInput:
    id: gm.ID
    name: str


@gm.type(name="Query")
@dataclass
class CoerceQuery:
    @gm.field
    @staticmethod
    async def label(parent: "Any", info: "Any", user: CoerceUserInput) -> str:
        assert isinstance(user, CoerceUserInput)
        return f"{user.id}:{user.name}"


@pytest.mark.anyio
async def test_async_resolver_coerces_input_and_id() -> None:
    """
    Verifies async resolvers receive coerced input objects and ID scalars.
    """
    schema = gm.Schema(query=CoerceQuery)
    result = await schema.execute(
        "query ($user: UserInput!) { label(user: $user) }",
        variables={"user": {"id": 123, "name": "Ada"}},
    )

    assert result["data"]["label"] == "123:Ada"
