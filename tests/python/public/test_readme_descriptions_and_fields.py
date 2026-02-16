"""Public contract tests for README description and resolver examples."""

from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet


@grommet.type(description="All queries")
@dataclass
class DescribedQuery:
    greeting: Annotated[str, grommet.Field(description="A simple greeting")] = (
        "Hello world!"
    )


@grommet.type
@dataclass
class ResolverQuery:
    @grommet.field(description="A simple greeting")
    async def greeting(self, name: str, title: str | None = None) -> str:
        return f"Hello {name}!" if not title else f"Hello, {title} {name}."


async def test_readme_description_metadata_is_reflected_in_sdl(schema_sdl):
    """Verifies type and field descriptions from Annotated metadata appear in SDL."""
    schema = grommet.Schema(query=DescribedQuery)
    sdl = schema_sdl(schema)
    assert "All queries" in sdl
    assert "A simple greeting" in sdl


@pytest.mark.parametrize(
    ("query", "expected"),
    [
        ('{ greeting(name: "Gromit") }', "Hello Gromit!"),
        ('{ greeting(name: "Gromit", title: "Mr.") }', "Hello, Mr. Gromit."),
    ],
)
async def test_readme_resolver_arguments_cover_required_and_optional_paths(
    query: str, expected: str, assert_success
):
    """Executes README resolver examples with both required-only and optional arguments."""
    schema = grommet.Schema(query=ResolverQuery)
    result = await schema.execute(query)
    assert_success(result, {"greeting": expected})


def test_readme_field_decorator_description_is_reflected_in_sdl(schema_sdl):
    """Verifies @grommet.field(description=...) is emitted in SDL."""
    schema = grommet.Schema(query=ResolverQuery)
    assert "A simple greeting" in schema_sdl(schema)
