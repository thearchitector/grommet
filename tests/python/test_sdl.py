from dataclasses import dataclass

import grommet as gm


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
    async def greet(self, user: SdlUserInput) -> str:
        return f"hi {user.name}"


async def test_schema_sdl_contains_types() -> None:
    """
    Verifies generated SDL includes referenced types and omits unused ones.
    """
    schema = gm.Schema(query=SdlQuery)
    sdl = schema._core.as_sdl()

    assert "type Query" in sdl
    assert "input UserInput" in sdl
    assert "greet" in sdl
    assert "SdlUser" not in sdl
