from dataclasses import dataclass

import grommet as gm


@gm.type(name="Query")
@dataclass
class RootQuery:
    @gm.field
    async def value(self) -> str:
        return "ok"


async def test_field_resolver_returns_value() -> None:
    """Verifies field resolvers return values in responses."""
    schema = gm.Schema(query=RootQuery)
    result = await schema.execute("{ value }")

    assert result.data["value"] == "ok"
