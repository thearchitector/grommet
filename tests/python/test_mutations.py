"""Tests for mutations with input types from README examples."""

import asyncio
from dataclasses import dataclass
from typing import Annotated

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
            if not self.title
            else f"Hello, {self.title} {self.name}."
        )


@grommet.type
@dataclass
class QueryForMutation:
    greeting: str = "Hello!"


@grommet.type
@dataclass
class Mutation:
    @grommet.field
    async def add_user(self, input: AddUserInput) -> User:
        return User(name=input.name, title=input.title)


SCHEMA = grommet.Schema(query=QueryForMutation, mutation=Mutation)


def test_mutation_without_optional():
    mutation = """
        mutation ($name: String!, $title: String) {
            add_user(input: { name: $name, title: $title }) { greeting }
        }
    """
    result = asyncio.run(SCHEMA.execute(mutation, variables={"name": "Gromit"}))
    assert result.data == {"add_user": {"greeting": "Hello Gromit!"}}


def test_mutation_with_optional():
    mutation = """
        mutation ($name: String!, $title: String) {
            add_user(input: { name: $name, title: $title }) { greeting }
        }
    """
    result = asyncio.run(
        SCHEMA.execute(mutation, variables={"name": "Gromit", "title": "Mr."})
    )
    assert result.data == {"add_user": {"greeting": "Hello, Mr. Gromit."}}


def test_input_descriptions_in_sdl():
    sdl = SCHEMA._schema.as_sdl()
    assert "User input." in sdl
    assert "The name of the user." in sdl
    assert "The title of the user, if any." in sdl
