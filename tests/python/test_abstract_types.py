import enum
from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.enum
class Color(enum.Enum):
    RED = "red"
    BLUE = "blue"


@gm.type
@dataclass
class ColorQuery:
    @gm.field
    @staticmethod
    async def paint(parent: "Any", info: "Any", color: Color) -> Color:
        assert isinstance(color, Color)
        return color


@gm.interface
@dataclass
class Node:
    id: gm.ID


@gm.type(implements=[Node])
@dataclass
class User(Node):
    name: str


@gm.type
@dataclass
class NodeQuery:
    @gm.field
    @staticmethod
    async def node(parent: "Any", info: "Any") -> Node:
        return User(id=gm.ID("1"), name="Ada")


@gm.type
@dataclass
class Book:
    title: str


@gm.type
@dataclass
class Movie:
    title: str


SearchResult = gm.union("SearchResult", types=[Book, Movie])


@gm.type
@dataclass
class SearchQuery:
    @gm.field
    @staticmethod
    async def result(parent: "Any", info: "Any", kind: str) -> SearchResult:  # type: ignore[valid-type]
        if kind == "book":
            return Book(title="Dune")
        return Movie(title="Alien")


@pytest.mark.anyio
async def test_enum_input_output() -> None:
    """
    Verifies enum inputs are accepted and serialized in responses.
    """
    schema = gm.Schema(query=ColorQuery)
    result = await schema.execute(
        "query ($color: Color!) { paint(color: $color) }", variables={"color": "RED"}
    )

    assert result["data"]["paint"] == "RED"


@pytest.mark.anyio
async def test_interface_resolution() -> None:
    """
    Verifies interface results resolve to implementing types in responses.
    """
    schema = gm.Schema(query=NodeQuery)
    result = await schema.execute("{ node { id ... on User { name } } }")

    assert result["data"]["node"] == {"id": "1", "name": "Ada"}


@pytest.mark.anyio
async def test_union_resolution() -> None:
    """
    Verifies union selections return the matched object fields.
    """
    schema = gm.Schema(query=SearchQuery)
    result = await schema.execute(
        "query ($kind: String!) { result(kind: $kind) { ... on Book { title } ... on Movie { title } } }",
        variables={"kind": "book"},
    )

    assert result["data"]["result"] == {"title": "Dune"}
