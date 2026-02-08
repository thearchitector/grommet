"""Tests for resolver-backed fields from README examples."""

import asyncio
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class Query:
    @grommet.field(description="A simple greeting")
    async def greeting(self, name: str, title: str | None = None) -> str:
        return f"Hello {name}!" if not title else f"Hello, {title} {name}."


def test_resolver_required_arg():
    schema = grommet.Schema(query=Query)
    result = asyncio.run(schema.execute('{ greeting(name: "Gromit") }'))
    assert result.data == {"greeting": "Hello Gromit!"}


def test_resolver_optional_arg():
    schema = grommet.Schema(query=Query)
    result = asyncio.run(schema.execute('{ greeting(name: "Gromit", title: "Mr.") }'))
    assert result.data == {"greeting": "Hello, Mr. Gromit."}


def test_resolver_description_in_sdl():
    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "A simple greeting" in sdl
