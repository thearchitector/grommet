import re
from dataclasses import dataclass
from typing import ClassVar

import grommet as gm


@gm.type
@dataclass
class Foo:
    num_count: ClassVar[int] = 2
    _foo: int = 2
    bar: gm.Internal[str] = "x"
    baz: gm.Private[str] = "y"
    public: str = "ok"

    @gm.field
    async def foobar(self) -> str:
        return "xxxx"


async def test_internal_fields_ignored_in_schema() -> None:
    """Verifies internal fields are omitted from SDL."""
    schema = gm.Schema(query=Foo)
    sdl = schema._core.as_sdl()

    def has_field(name: str) -> bool:
        return re.search(rf"^\s*{re.escape(name)}\s*:", sdl, re.MULTILINE) is not None

    assert not has_field("_foo")
    assert not has_field("bar")
    assert not has_field("baz")
    assert not has_field("num_count")
    assert has_field("public")
    assert has_field("foobar")


async def test_resolver_executes_on_type_with_internal_fields() -> None:
    """Verifies resolvers execute on types with internal fields."""
    schema = gm.Schema(query=Foo)
    result = await schema.execute("{ foobar }")

    assert result.data["foobar"] == "xxxx"
