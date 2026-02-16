"""Public contract tests for hidden fields and context examples."""

from dataclasses import dataclass
from typing import Annotated, ClassVar

import grommet


@grommet.type
@dataclass
class User:
    _foo: int
    bar: ClassVar[int]
    hidden: Annotated[int, grommet.Hidden]
    name: str

    @grommet.field
    async def greeting(self) -> str:
        return f"Hello {self.name}" + ("!" * self._foo * self.bar * self.hidden)


User.bar = 2


@grommet.type
@dataclass
class HiddenQuery:
    @grommet.field
    async def user(self, name: str) -> User:
        return User(_foo=2, hidden=2, name=name)


@grommet.type
@dataclass
class ContextQuery:
    @grommet.field
    async def greeting(
        self, context: Annotated[dict[str, str], grommet.Context]
    ) -> str:
        return f"Hello request {context['request_id']}!"


async def test_readme_hidden_fields_are_excluded_from_schema(
    assert_success, schema_sdl
):
    """Verifies private, ClassVar, and Hidden annotations are not exposed in SDL."""
    schema = grommet.Schema(query=HiddenQuery)
    sdl = schema_sdl(schema)

    assert "name" in sdl
    assert "greeting" in sdl
    assert "_foo" not in sdl
    assert "bar" not in sdl
    assert "hidden" not in sdl

    result = await schema.execute('{ user(name: "Gromit") { greeting } }')
    assert_success(result, {"user": {"greeting": "Hello Gromit!!!!!!!!"}})


async def test_readme_context_state_is_injected_into_resolver(assert_success):
    """Passes per-request context and verifies resolver-side context injection."""
    schema = grommet.Schema(query=ContextQuery)
    result = await schema.execute("{ greeting }", context={"request_id": "123"})
    assert_success(result, {"greeting": "Hello request 123!"})
