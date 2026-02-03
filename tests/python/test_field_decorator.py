from dataclasses import dataclass

import pytest

import grommet as gm


class Info:
    pass


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def hello(parent: "Query", info: Info) -> str:
        return "hi"


@pytest.mark.anyio
async def test_field_decorator_uses_resolver() -> None:
    """
    Verifies field resolvers execute and return values in responses.
    """
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ hello }")

    assert result["data"]["hello"] == "hi"
