"""Tests for unions and interfaces described by the README API."""

import asyncio
from dataclasses import dataclass
from typing import Annotated

import pytest

import grommet


def test_public_exports_include_interface_and_union():
    assert hasattr(grommet, "interface")
    assert hasattr(grommet, "Union")
    assert "interface" in grommet.__all__
    assert "Union" in grommet.__all__


def test_named_union_with_type_alias_executes_and_is_emitted_in_sdl():
    @grommet.type
    @dataclass
    class A:
        a: int

    @grommet.type
    @dataclass
    class B:
        b: int

    type NamedAB = Annotated[A | B, grommet.Union(name="NamedAB", description="A or B")]

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def named(self, type: str) -> NamedAB:
            return A(a=1) if type == "A" else B(b=2)

    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "union NamedAB = A | B" in sdl
    assert "A or B" in sdl

    result = asyncio.run(
        schema.execute('{ named(type: "A") { ... on A { a } ... on B { b } } }')
    )
    assert result.data == {"named": {"a": 1}}

    result = asyncio.run(
        schema.execute('{ named(type: "B") { ... on A { a } ... on B { b } } }')
    )
    assert result.data == {"named": {"b": 2}}


def test_automatic_union_name_is_emitted_and_executes():
    @grommet.type
    @dataclass
    class A:
        a: int

    @grommet.type
    @dataclass
    class B:
        b: int

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def unnamed(self, type: str) -> A | B:
            return A(a=1) if type == "A" else B(b=2)

    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "union AB = A | B" in sdl

    result = asyncio.run(
        schema.execute('{ unnamed(type: "B") { ... on A { a } ... on B { b } } }')
    )
    assert result.data == {"unnamed": {"b": 2}}


def test_interface_types_auto_include_implementers_and_execute():
    @grommet.interface(description="A letter")
    @dataclass
    class Letter:
        letter: str

    @grommet.type
    @dataclass
    class A(Letter):
        pass

    @grommet.type
    @dataclass
    class B(Letter):
        some_subfield: list[int]

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def common(self, type: str) -> Letter:
            return A(letter="A") if type == "A" else B(letter="B", some_subfield=[42])

    schema = grommet.Schema(query=Query)
    sdl = schema._schema.as_sdl()
    assert "interface Letter" in sdl
    assert "type A implements Letter" in sdl
    assert "type B implements Letter" in sdl

    result = asyncio.run(
        schema.execute(
            '{ common(type: "B") { letter ... on B { some_subfield } ... on A { letter } } }'
        )
    )
    assert result.data == {"common": {"letter": "B", "some_subfield": [42]}}


def test_interface_resolver_methods_are_inherited_by_implementers():
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

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def current(self) -> Named:
            return User(name="gromit")

    schema = grommet.Schema(query=Query)
    result = asyncio.run(schema.execute("{ current { loud ... on User { name } } }"))
    assert result.data == {"current": {"loud": "GROMIT", "name": "gromit"}}


def test_union_name_conflict_fails_fast():
    @grommet.type
    @dataclass
    class A:
        a: int

    @grommet.type
    @dataclass
    class B:
        b: int

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


def test_union_is_rejected_for_input_positions():
    @grommet.type
    @dataclass
    class A:
        a: int

    @grommet.type
    @dataclass
    class B:
        b: int

    with pytest.raises(TypeError, match="not supported in input annotations"):

        @grommet.input
        @dataclass
        class BadInput:
            value: A | B
