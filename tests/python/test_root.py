from dataclasses import dataclass

import pytest

import grommet as gm


@gm.type(name="Query")
@dataclass
class RootQuery:
    value: str


@pytest.mark.anyio
async def test_field_without_resolver_uses_root_value() -> None:
    """
    Verifies fields without resolvers read values from the root object.
    """
    schema = gm.Schema(query=RootQuery)
    result = await schema.execute("{ value }", root={"value": "ok"})

    assert result["data"]["value"] == "ok"
