"""Public contract tests for README unions and interfaces examples."""

from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet


@grommet.type
@dataclass
class A:
    a: int


@grommet.type
@dataclass
class B:
    b: int


type NamedAB = Annotated[A | B, grommet.Union(name="NamedAB", description="A or B")]


@grommet.interface(description="A letter")
@dataclass
class Letter:
    letter: str


@grommet.type
@dataclass
class LetterA(Letter):
    pass


@grommet.type
@dataclass
class LetterB(Letter):
    some_subfield: list[int]


@grommet.interface
@dataclass
class Named:
    name: str

    @grommet.field
    def loud(self) -> str:
        return self.name.upper()


@grommet.type
@dataclass
class User(Named):
    pass


async def test_readme_named_union_is_emitted_and_executes(assert_success, schema_sdl):
    """Verifies named unions are emitted in SDL and resolve to both member types."""

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def named(self, kind: str) -> NamedAB:
            return A(a=1) if kind == "A" else B(b=2)

    schema = grommet.Schema(query=Query)
    sdl = schema_sdl(schema)
    assert "union NamedAB = A | B" in sdl
    assert "A or B" in sdl

    result_a = await schema.execute(
        '{ named(kind: "A") { ... on A { a } ... on B { b } } }'
    )
    assert_success(result_a, {"named": {"a": 1}})

    result_b = await schema.execute(
        '{ named(kind: "B") { ... on A { a } ... on B { b } } }'
    )
    assert_success(result_b, {"named": {"b": 2}})


async def test_readme_unnamed_union_uses_auto_generated_name(
    assert_success, schema_sdl
):
    """Verifies unnamed unions concatenate member names for SDL registration."""

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def unnamed(self, kind: str) -> A | B:
            return A(a=1) if kind == "A" else B(b=2)

    schema = grommet.Schema(query=Query)
    assert "union AB = A | B" in schema_sdl(schema)

    result = await schema.execute(
        '{ unnamed(kind: "B") { ... on A { a } ... on B { b } } }'
    )
    assert_success(result, {"unnamed": {"b": 2}})


async def test_readme_interface_implementers_are_registered(assert_success, schema_sdl):
    """Verifies interface implementers are included automatically and resolve correctly."""

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def common(self, kind: str) -> Letter:
            return (
                LetterA(letter="A")
                if kind == "A"
                else LetterB(letter="B", some_subfield=[42])
            )

    schema = grommet.Schema(query=Query)
    sdl = schema_sdl(schema)
    assert "interface Letter" in sdl
    assert "type LetterA implements Letter" in sdl
    assert "type LetterB implements Letter" in sdl

    result = await schema.execute(
        '{ common(kind: "B") { letter ... on LetterB { some_subfield } } }'
    )
    assert_success(result, {"common": {"letter": "B", "some_subfield": [42]}})


async def test_readme_interface_resolver_methods_are_inherited(assert_success):
    """Verifies resolver methods declared on interfaces are inherited by implementers."""

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def current(self) -> Named:
            return User(name="gromit")

    schema = grommet.Schema(query=Query)
    result = await schema.execute("{ current { loud ... on User { name } } }")
    assert_success(result, {"current": {"loud": "GROMIT", "name": "gromit"}})


def test_union_name_conflicts_fail_schema_build():
    """Fails schema construction when the same union name has conflicting members."""

    @grommet.type
    @dataclass
    class C:
        c: int

    type ConflictAB = Annotated[A | B, grommet.Union(name="Conflict")]
    type ConflictAC = Annotated[A | C, grommet.Union(name="Conflict")]

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def first(self) -> ConflictAB:
            return A(a=1)

        @grommet.field
        async def second(self) -> ConflictAC:
            return C(c=2)

    with pytest.raises(TypeError, match="conflicting definitions"):
        grommet.Schema(query=Query)
