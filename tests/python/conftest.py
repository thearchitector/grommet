"""Shared fixtures and collection guards for the alpha test suite."""

from __future__ import annotations

import inspect
from collections.abc import Awaitable, Callable
from typing import Any

import pytest

import grommet


@pytest.fixture
def run_operation() -> Callable[
    [grommet.Schema, str, dict[str, Any] | None, Any], Awaitable[Any]
]:
    """Provides an async helper for executing schema operations."""

    async def _run(
        schema: grommet.Schema,
        query: str,
        variables: dict[str, Any] | None = None,
        context: Any = None,
    ) -> Any:
        return await schema.execute(query, variables=variables, context=context)

    return _run


@pytest.fixture
def assert_success() -> Callable[[Any, dict[str, Any]], None]:
    """Provides a shared assertion helper for successful operation results."""

    def _assert(result: Any, expected_data: dict[str, Any]) -> None:
        assert result.errors is None
        assert result.data == expected_data

    return _assert


@pytest.fixture
def collect_stream() -> Callable[[Any], Awaitable[list[dict[str, Any] | None]]]:
    """Collects all payloads from a subscription stream."""

    async def _collect(stream: Any) -> list[dict[str, Any] | None]:
        rows: list[dict[str, Any] | None] = []
        async for item in stream:
            rows.append(item.data)
        return rows

    return _collect


@pytest.fixture
def schema_sdl() -> Callable[[grommet.Schema], str]:
    """Provides SDL text by invoking Schema.sdl's underlying implementation."""

    def _sdl(schema: grommet.Schema) -> str:
        return type(schema).sdl.func(schema)

    return _sdl


def pytest_collection_modifyitems(
    config: pytest.Config, items: list[pytest.Item]
) -> None:
    """Fails collection when any test function omits a docstring."""
    del config

    missing_docstrings: set[str] = set()
    for item in items:
        if not item.name.startswith("test_"):
            continue
        obj = getattr(item, "obj", None)
        if obj is None:
            continue
        if inspect.getdoc(obj) is None:
            missing_docstrings.add(item.nodeid)

    if missing_docstrings:
        missing_lines = "\n".join(
            f"- {nodeid}" for nodeid in sorted(missing_docstrings)
        )
        raise pytest.UsageError(
            "Every collected test function must include a docstring.\n"
            f"Missing docstrings:\n{missing_lines}"
        )
