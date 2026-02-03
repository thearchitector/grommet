import re
from dataclasses import dataclass
from typing import Any, ClassVar

import pytest

import grommet as gm


@gm.type
@dataclass
class Foo:
    num_count: ClassVar[int] = 2
    _foo: int
    bar: gm.Internal[str]
    baz: gm.Private[str]
    public: str

    @gm.field
    @staticmethod
    async def foobar(parent: "Foo", info: Any) -> str:
        return parent.bar * parent._foo * Foo.num_count


@pytest.mark.anyio
async def test_internal_fields_ignored_in_schema_and_resolvers_use_them() -> None:
    """
    Verifies internal fields are omitted from SDL but usable in resolvers.
    """
    schema = gm.Schema(query=Foo)
    sdl = schema.sdl()

    def has_field(name: str) -> bool:
        return re.search(rf"^\s*{re.escape(name)}\s*:", sdl, re.MULTILINE) is not None

    assert not has_field("_foo")
    assert not has_field("bar")
    assert not has_field("baz")
    assert not has_field("num_count")
    assert has_field("public")

    root = Foo(_foo=2, bar="x", baz="y", public="ok")
    result = await schema.execute("{ foobar }", root=root)

    assert result["data"]["foobar"] == "xxxx"
