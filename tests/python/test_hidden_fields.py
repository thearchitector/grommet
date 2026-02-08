"""Tests for hidden fields from README examples."""

import asyncio
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
class Query:
    @grommet.field
    async def user(self, name: str) -> User:
        return User(_foo=2, hidden=2, name=name)


def test_hidden_fields_excluded_from_sdl():
    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "name" in sdl
    assert "greeting" in sdl
    assert "_foo" not in sdl
    assert "hidden" not in sdl
    assert "bar" not in sdl


def test_hidden_fields_still_accessible_in_resolvers():
    schema = grommet.Schema(query=Query)
    result = asyncio.run(schema.execute('{ user(name: "Gromit") { greeting } }'))
    assert result.data == {"user": {"greeting": "Hello Gromit" + "!" * (2 * 2 * 2)}}
