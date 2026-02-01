from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class SdlUser:
    name: str
    age: int | None = None


@gm.input(name="UserInput")
@dataclass
class SdlUserInput:
    name: str
    age: int | None = None


@gm.type(name="Query")
@dataclass
class SdlQuery:
    @gm.field
    @staticmethod
    async def greet(parent: "Any", info: "Any", user: SdlUserInput) -> str:
        return f"hi {user.name}"


@pytest.mark.anyio
async def test_schema_sdl_contains_types() -> None:
    schema = gm.Schema(query=SdlQuery)
    sdl = schema.sdl()

    assert "type Query" in sdl
    assert "input UserInput" in sdl
    assert "greet" in sdl
    assert "SdlUser" not in sdl
