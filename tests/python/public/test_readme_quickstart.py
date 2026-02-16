"""Public contract tests for README quickstart examples."""

from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class Query:
    greeting: str = "Hello world!"


async def test_readme_quickstart_query_executes(assert_success):
    """Executes the quickstart query against a schema with default-backed fields."""
    schema = grommet.Schema(query=Query)
    result = await schema.execute("{ greeting }")
    assert_success(result, {"greeting": "Hello world!"})


def test_readme_quickstart_schema_sdl_property_is_public(schema_sdl):
    """Reads SDL through the public Schema.sdl property."""
    schema = grommet.Schema(query=Query)
    sdl = schema_sdl(schema)
    assert "type Query" in sdl
    assert "greeting: String!" in sdl
