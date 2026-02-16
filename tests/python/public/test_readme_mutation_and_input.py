"""Public contract tests for README mutation and input examples."""

from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet


@grommet.input(description="User input.")
@dataclass
class AddUserInput:
    name: Annotated[str, grommet.Field(description="The name of the user.")]
    title: Annotated[
        str | None, grommet.Field(description="The title of the user, if any.")
    ]


@grommet.type
@dataclass
class User:
    name: str
    title: str | None

    @grommet.field
    async def greeting(self) -> str:
        return (
            f"Hello {self.name}!"
            if self.title is None
            else f"Hello, {self.title} {self.name}."
        )


@grommet.type
@dataclass
class Query:
    greeting: str = "Hello!"


@grommet.type
@dataclass
class Mutation:
    @grommet.field
    async def add_user(self, input: AddUserInput) -> User:
        return User(name=input.name, title=input.title)


MUTATION = """
mutation ($name: String!, $title: String) {
  add_user(input: { name: $name, title: $title }) { greeting }
}
"""


@pytest.mark.parametrize(
    ("variables", "expected"),
    [
        ({"name": "Gromit"}, "Hello Gromit!"),
        ({"name": "Gromit", "title": "Mr."}, "Hello, Mr. Gromit."),
    ],
)
async def test_readme_mutation_executes_with_and_without_optional_values(
    variables: dict[str, str], expected: str, assert_success
):
    """Executes README mutation examples for both optional-input variants."""
    schema = grommet.Schema(query=Query, mutation=Mutation)
    result = await schema.execute(MUTATION, variables=variables)
    assert_success(result, {"add_user": {"greeting": expected}})


def test_readme_input_descriptions_are_reflected_in_sdl(schema_sdl):
    """Verifies input object and input field descriptions appear in SDL."""
    schema = grommet.Schema(query=Query, mutation=Mutation)
    sdl = schema_sdl(schema)
    assert "User input." in sdl
    assert "The name of the user." in sdl
    assert "The title of the user, if any." in sdl
