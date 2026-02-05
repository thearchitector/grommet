from collections.abc import AsyncIterator
from typing import Annotated, ClassVar
from typing import AsyncIterator as TypingAsyncIterator

import pytest

from grommet.annotations import (
    analyze_annotation,
    is_internal_field,
    unwrap_async_iterable,
    unwrap_async_iterable_inner,
)
from grommet.errors import GrommetTypeError
from grommet.metadata import _INTERNAL_MARKER


def test_analyze_annotation_with_metadata_marks_internal() -> None:
    """
    Verifies annotated metadata marks fields as internal.
    """
    info = analyze_annotation(Annotated[int, _INTERNAL_MARKER])
    assert info.metadata == (_INTERNAL_MARKER,)
    assert info.is_internal is True


def test_unwrap_async_iterable_with_optional() -> None:
    """
    Verifies async iterable annotations are unwrapped with optionality.
    """
    annotation = AsyncIterator[int] | None
    inner, optional = unwrap_async_iterable(annotation)
    assert inner is int
    assert optional is True
    assert unwrap_async_iterable_inner(AsyncIterator[int]) is int


def test_unwrap_async_iterable_for_non_async_annotation() -> None:
    """
    Verifies non-async annotations pass through unwrap helpers.
    """
    inner, optional = unwrap_async_iterable(int)
    assert inner is int
    assert optional is False
    assert unwrap_async_iterable_inner(int) is int


def test_unwrap_async_iterable_requires_parameter() -> None:
    """
    Ensures async iterable helpers error on unparameterized types.
    """
    with pytest.raises(GrommetTypeError):
        unwrap_async_iterable(TypingAsyncIterator)
    with pytest.raises(GrommetTypeError):
        unwrap_async_iterable_inner(TypingAsyncIterator)


def test_analyze_annotation_handles_empty_annotated_args(monkeypatch) -> None:
    """
    Ensures analyze_annotation handles empty Annotated arguments gracefully.
    """
    from grommet import annotations as ann_module

    def fake_get_args(_value):
        return ()

    monkeypatch.setattr(ann_module, "get_args", fake_get_args)
    info = ann_module.analyze_annotation(Annotated[int, _INTERNAL_MARKER])
    assert info.metadata == ()


def test_is_internal_field_respects_prefix_and_classvar() -> None:
    """
    Verifies internal field detection respects prefixes and ClassVar markers.
    """
    assert is_internal_field("_hidden", int) is True
    assert is_internal_field("value", ClassVar[int]) is True
    assert is_internal_field("value", int) is False


def test_walk_annotation_exits_cleanly_on_simple_types() -> None:
    """Test walk_annotation handles simple non-grommet types."""
    from grommet.annotations import walk_annotation

    results = list(walk_annotation(int))
    assert results == []

    results = list(walk_annotation(str))
    assert results == []


def test_walk_annotation_nested_list_of_unions() -> None:
    """Test walk_annotation handles nested list of unions."""
    from dataclasses import dataclass

    import grommet as gm
    from grommet.annotations import walk_annotation

    @gm.type
    @dataclass
    class TypeA:
        name: str

    @gm.type
    @dataclass
    class TypeB:
        value: int

    TestUnion = gm.union("TestUnion", types=[TypeA, TypeB])

    results = list(walk_annotation(list[TestUnion]))
    kinds = [r[0] for r in results]
    assert "union" in kinds
