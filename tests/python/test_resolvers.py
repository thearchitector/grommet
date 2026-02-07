from dataclasses import dataclass

import grommet as gm


@gm.input(name="UserInput")
@dataclass
class CoerceUserInput:
    id: gm.ID
    name: str


@gm.type(name="Query")
@dataclass
class CoerceQuery:
    @gm.field
    async def label(self, user: CoerceUserInput) -> str:
        assert isinstance(user, CoerceUserInput)
        return f"{user.id}:{user.name}"


async def test_async_resolver_coerces_input_and_id() -> None:
    """Verifies async resolvers receive coerced input objects and ID scalars."""
    schema = gm.Schema(query=CoerceQuery)
    result = await schema.execute(
        "query ($user: UserInput!) { label(user: $user) }",
        variables={"user": {"id": 123, "name": "Ada"}},
    )

    assert result.data["label"] == "123:Ada"
