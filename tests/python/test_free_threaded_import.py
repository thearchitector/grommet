"""Verifies importing grommet does not re-enable the GIL."""

from __future__ import annotations

import json
import subprocess
import sys

import pytest

_CHECK_SCRIPT = """
import json
import sys
import warnings

if not hasattr(sys, "_is_gil_enabled"):
    print(json.dumps({"supported": False}))
    raise SystemExit(0)

before = sys._is_gil_enabled()
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    import grommet  # noqa: F401
after = sys._is_gil_enabled()

runtime_warnings = [
    str(item.message) for item in caught if issubclass(item.category, RuntimeWarning)
]
print(
    json.dumps(
        {
            "supported": True,
            "before": before,
            "after": after,
            "runtime_warnings": runtime_warnings,
        }
    )
)
"""


def test_import_keeps_gil_disabled() -> None:
    result = subprocess.run(
        [sys.executable, "-c", _CHECK_SCRIPT],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr or result.stdout

    payload = json.loads(result.stdout)
    if not payload["supported"]:
        pytest.skip("Current Python does not expose sys._is_gil_enabled().")
    if payload["before"]:
        pytest.skip("Current Python process started with GIL enabled.")

    assert payload["after"] is False
    assert all(
        "global interpreter lock" not in warning.lower()
        for warning in payload["runtime_warnings"]
    )
