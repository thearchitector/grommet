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
async def test_error_includes_path_location_and_omits_traceback() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ boom }")

    if result["data"] is not None:
        assert result["data"]["boom"] is None
    assert result["errors"], "Expected GraphQL error"

    err = result["errors"][0]
    if "path" in err:
        assert err["path"] == ["boom"]
    assert err["locations"], "Expected locations on error"
    if "extensions" in err:
        assert "traceback" not in err["extensions"]


@pytest.mark.anyio
async def test_parse_error_sets_null_data() -> None:
    schema = gm.Schema(query=Query)
    result = await schema.execute("{")

    assert result["data"] is None
    assert result["errors"]
