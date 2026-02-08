"""Tests for basic query functionality from README examples."""

import asyncio
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class Query:
    greeting: str = "Hello world!"


def test_basic_query():
    schema = grommet.Schema(query=Query)
    result = asyncio.run(schema.execute("{ greeting }"))
    assert result.data == {"greeting": "Hello world!"}


def test_sdl_output():
    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "greeting" in sdl
    assert "String!" in sdl
