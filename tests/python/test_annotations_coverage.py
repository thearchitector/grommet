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
    info = analyze_annotation(Annotated[int, _INTERNAL_MARKER])
    assert info.metadata == (_INTERNAL_MARKER,)
    assert info.is_internal is True


def test_unwrap_async_iterable_with_optional() -> None:
    annotation = AsyncIterator[int] | None
    inner, optional = unwrap_async_iterable(annotation)
    assert inner is int
    assert optional is True
    assert unwrap_async_iterable_inner(AsyncIterator[int]) is int


def test_unwrap_async_iterable_for_non_async_annotation() -> None:
    inner, optional = unwrap_async_iterable(int)
    assert inner is int
    assert optional is False
    assert unwrap_async_iterable_inner(int) is int


def test_unwrap_async_iterable_requires_parameter() -> None:
    with pytest.raises(GrommetTypeError):
        unwrap_async_iterable(TypingAsyncIterator)
    with pytest.raises(GrommetTypeError):
        unwrap_async_iterable_inner(TypingAsyncIterator)


def test_analyze_annotation_handles_empty_annotated_args(monkeypatch) -> None:
    from grommet import annotations as ann_module

    def fake_get_args(_value):
        return ()

    monkeypatch.setattr(ann_module, "get_args", fake_get_args)
    info = ann_module.analyze_annotation(Annotated[int, _INTERNAL_MARKER])
    assert info.metadata == ()


def test_is_internal_field_respects_prefix_and_classvar() -> None:
    assert is_internal_field("_hidden", int) is True
    assert is_internal_field("value", ClassVar[int]) is True
    assert is_internal_field("value", int) is False
