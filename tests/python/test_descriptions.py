"""Tests for type and field descriptions from README examples."""

import asyncio
from dataclasses import dataclass
from typing import Annotated

import grommet


@grommet.type(description="All queries")
@dataclass
class Query:
    greeting: Annotated[str, grommet.Field(description="A simple greeting")] = (
        "Hello world!"
    )


def test_type_and_field_descriptions_in_sdl():
    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "All queries" in sdl
    assert "A simple greeting" in sdl


def test_described_query_still_executes():
    schema = grommet.Schema(query=Query)
    result = asyncio.run(schema.execute("{ greeting }"))
    assert result.data == {"greeting": "Hello world!"}
