"""Public API contract tests beyond direct README snippets."""

from dataclasses import dataclass

import pytest

import grommet


@grommet.type
@dataclass
class Query:
    greeting: str = "Hello world!"


@grommet.type
@dataclass
class RootWithoutDefault:
    greeting: str


def test_public_exports_match_the_supported_surface():
    """Ensures __all__ exposes the documented public entry points."""
    expected = {
        "Context",
        "Field",
        "Hidden",
        "Schema",
        "Union",
        "field",
        "input",
        "interface",
        "subscription",
        "type",
    }
    assert set(grommet.__all__) == expected


async def test_operation_result_supports_mapping_style_access(assert_success):
    """Validates OperationResult mapping-style fields and unknown-key behavior."""
    schema = grommet.Schema(query=Query)
    result = await schema.execute("{ greeting }")

    assert_success(result, {"greeting": "Hello world!"})
    assert result["data"] == {"greeting": "Hello world!"}
    assert result["errors"] is None
    assert result["extensions"] is None

    with pytest.raises(KeyError):
        _ = result["missing"]


def test_execute_rejects_legacy_state_kwarg():
    """Confirms Schema.execute rejects unsupported keyword arguments."""
    schema = grommet.Schema(query=Query)
    with pytest.raises(TypeError, match="state"):
        schema.execute("{ greeting }", state={"request_id": "123"})


def test_root_type_fields_require_defaults_or_resolvers():
    """Rejects root data fields without defaults during schema construction."""
    with pytest.raises(TypeError, match="must declare a default value"):
        grommet.Schema(query=RootWithoutDefault)
