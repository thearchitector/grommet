from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@pytest.mark.anyio
async def test_info_context_root_are_available() -> None:
    root_obj = object()
    context_obj = object()

    @gm.type
    @dataclass
    class Query:
        @gm.field
        @staticmethod
        async def inspect(
            parent: "Any",
            info: gm.Info,
            context: "Any",
            root: "Any",
        ) -> str:
            assert info.field_name == "inspect"
            assert info.context is context_obj
            assert info.root is root_obj
            assert parent is root_obj
            assert context is context_obj
            assert root is root_obj
            return "ok"

    schema = gm.Schema(query=Query)
    result = await schema.execute("{ inspect }", root=root_obj, context=context_obj)

    assert result["data"]["inspect"] == "ok"
