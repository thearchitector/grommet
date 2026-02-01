from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class Query:
    @gm.field(
        name="greet",
        description="Greets the caller.",
        deprecation_reason="Use helloNew",
    )
    @staticmethod
    async def hello(parent: "Any", info: "Any") -> str:
        return "hi"

    @gm.field
    @classmethod
    async def kind(cls, info: "Any") -> str:
        return cls.__name__


@pytest.mark.anyio
async def test_field_decorator_args_apply() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ greet kind }")

    assert result["data"]["greet"] == "hi"
    assert result["data"]["kind"] == "Query"

    sdl = schema.sdl()
    assert "greet" in sdl
    assert "Greets the caller." in sdl
    assert "@deprecated" in sdl
