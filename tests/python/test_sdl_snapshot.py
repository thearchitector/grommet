import enum
from dataclasses import dataclass, field
from pathlib import Path

import grommet as gm


@gm.scalar(
    name="Date",
    serialize=lambda value: value.value,
    parse_value=lambda value: Date(str(value)),
)
@dataclass(frozen=True)
class Date:
    value: str


@gm.enum
class Role(enum.Enum):
    ADMIN = "admin"
    USER = "user"


@gm.interface
@dataclass
class Node:
    id: gm.ID


@gm.type(implements=[Node])
@dataclass
class User:
    id: gm.ID
    name: str
    role: Role


@gm.type
@dataclass
class Book:
    title: str


SearchResult = gm.union("SearchResult", types=[User, Book])


@gm.input
@dataclass
class Filter:
    term: str
    tags: list[str] = field(default_factory=list)


@gm.type
@dataclass
class Query:
    @gm.field
    async def node(self, id: gm.ID) -> Node:
        raise AssertionError("resolver not called")

    @gm.field
    async def search(self, filter: Filter) -> SearchResult:  # type: ignore[valid-type]
        raise AssertionError("resolver not called")

    @gm.field
    async def today(self) -> Date:
        raise AssertionError("resolver not called")


def test_schema_sdl_snapshot() -> None:
    """
    Verifies generated SDL matches the stored schema snapshot.
    """
    schema = gm.Schema(query=Query)
    sdl = schema._core.as_sdl().strip()

    snapshot = Path(__file__).parent / "fixtures" / "schema_snapshot.graphql"
    expected = snapshot.read_text(encoding="utf-8").strip()

    assert sdl == expected
