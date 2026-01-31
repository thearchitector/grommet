from dataclasses import dataclass
from typing import TYPE_CHECKING

import pytest

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def boom(parent: "Any", info: "Any") -> str:
        raise ValueError("boom")


@pytest.mark.anyio
async def test_error_includes_path_location_and_traceback_when_debug() -> None:
    schema = gm.Schema(query=Query, debug=True)
    result = await schema.execute("{ boom }")

    if result["data"] is not None:
        assert result["data"]["boom"] is None
    assert result["errors"], "Expected GraphQL error"
    assert "extensions" in result

    err = result["errors"][0]
    if "path" in err:
        assert err["path"] == ["boom"]
    assert err["locations"], "Expected locations on error"
    assert "extensions" in err
    assert "traceback" in err["extensions"]


@pytest.mark.anyio
async def test_error_omits_traceback_when_debug_false() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ boom }")

    err = result["errors"][0]
    assert "extensions" not in err


@pytest.mark.anyio
async def test_parse_error_sets_null_data() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute("{")

    assert result["data"] is None
    assert result["errors"]
