from dataclasses import dataclass

import grommet as gm


async def test_context_state_is_available() -> None:
    """Verifies resolvers receive Context with state."""
    state_obj = {"key": "value"}

    @gm.type
    @dataclass
    class Query:
        @gm.field
        async def inspect(self, ctx: gm.Context[dict]) -> str:
            assert ctx.state is state_obj
            return "ok"

    schema = gm.Schema(query=Query)
    result = await schema.execute("{ inspect }", state=state_obj)

    assert result.data["inspect"] == "ok"


async def test_context_lookahead() -> None:
    """Verifies resolvers can use look_ahead and field on Context."""

    @gm.type
    @dataclass
    class Query:
        @gm.field
        async def check(self, ctx: gm.Context) -> str:
            la = ctx.look_ahead()
            assert la.exists()
            return "ok"

    schema = gm.Schema(query=Query)
    result = await schema.execute("{ check }")

    assert result.data["check"] == "ok"
