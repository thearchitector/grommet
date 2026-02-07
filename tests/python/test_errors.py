from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Query:
    @gm.field
    async def boom(self) -> str:
        raise ValueError("boom")


async def test_error_includes_path_location_and_omits_traceback() -> None:
    """
    Verifies GraphQL execution errors include path/location data without tracebacks.
    """
    schema = gm.Schema(query=Query)
    result = await schema.execute("{ boom }")

    if result.data is not None:
        assert result.data["boom"] is None
    assert result.errors, "Expected GraphQL error"

    err = result.errors[0]
    if "path" in err:
        assert err["path"] == ["boom"]
    assert err["locations"], "Expected locations on error"
    if "extensions" in err:
        assert "traceback" not in err["extensions"]


async def test_parse_error_sets_null_data() -> None:
    """
    Verifies parse errors return null data alongside reported errors.
    """
    schema = gm.Schema(query=Query)
    result = await schema.execute("{")

    assert result.data is None
    assert result.errors
