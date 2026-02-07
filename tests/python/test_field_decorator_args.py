from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Query:
    @gm.field(
        name="greet",
        description="Greets the caller.",
        deprecation_reason="Use helloNew",
    )
    async def hello(self) -> str:
        return "hi"


async def test_field_decorator_args_apply() -> None:
    """Verifies field decorator arguments affect execution and SDL output."""
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ greet }")

    assert result.data["greet"] == "hi"

    sdl = schema._core.as_sdl()
    assert "greet" in sdl
    assert "Greets the caller." in sdl
    assert "@deprecated" in sdl
