from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Query:
    @gm.field
    async def hello(self) -> str:
        return "hi"


async def test_field_decorator_uses_resolver() -> None:
    """Verifies field resolvers execute and return values in responses."""
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ hello }")

    assert result.data["hello"] == "hi"
