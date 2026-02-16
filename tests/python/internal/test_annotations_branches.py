"""Targeted branch coverage tests for grommet.annotations."""

from dataclasses import dataclass
from typing import Annotated, AsyncIterator, ClassVar, List

import pytest

import grommet
import grommet.annotations as annotations_module
from grommet.annotations import (
    _annotation_name,
    _build_union_type_spec,
    _get_type_meta,
    _get_union_metadata,
    _is_input_type,
    _split_optional,
    _type_spec_from_annotation,
    _unwrap_type_alias,
    analyze_annotation,
    is_hidden_field,
    unwrap_async_iterable,
    walk_annotation,
)
from grommet.metadata import Context, Hidden


@grommet.type
@dataclass
class OutputType:
    value: int = 1


@grommet.type
@dataclass
class OutputTypeB:
    value: int = 2


@grommet.input
@dataclass
class InputType:
    value: int


@grommet.interface
@dataclass
class InterfaceType:
    value: int


def test_unwrap_type_alias_breaks_cycles_without_looping():
    """Stops alias unwrapping when a TypeAliasType cycle is detected."""
    namespace: dict[str, object] = {}
    exec("type A = B\ntype B = A", namespace, namespace)
    alias = namespace["A"]
    assert _unwrap_type_alias(alias) is alias


def test_annotation_name_falls_back_to_string_for_non_types():
    """Uses __name__ when available and string conversion otherwise."""
    assert _annotation_name(OutputType) == "OutputType"
    assert _annotation_name(123) == "123"


def test_get_union_metadata_returns_first_union_item_or_none():
    """Finds the first Union metadata item in Annotated metadata tuples."""
    assert _get_union_metadata(("x", 1)) is None
    found = _get_union_metadata((
        "x",
        grommet.Union(name="Named"),
        grommet.Union(name="Other"),
    ))
    assert found is not None
    assert found.name == "Named"


def test_analyze_annotation_detects_optional_list_hidden_and_context():
    """Extracts list, optional, hidden, context, and classvar flags from annotations."""
    info = analyze_annotation(Annotated[list[int] | None, Hidden])
    assert info.optional is True
    assert info.is_list is True
    assert info.list_item is int
    assert info.is_hidden is True

    async_info = analyze_annotation(Annotated[AsyncIterator[str], Context])
    assert async_info.is_async_iterable is True
    assert async_info.async_item is str
    assert async_info.is_context is True

    classvar_info = analyze_annotation(ClassVar[int])
    assert classvar_info.is_classvar is True


def test_unwrap_async_iterable_rejects_unparameterized_and_passthroughs_plain_annotations():
    """Rejects bare AsyncIterator annotations and passes through non-iterable annotations."""
    with pytest.raises(TypeError, match="parameterized"):
        unwrap_async_iterable(AsyncIterator)

    inner, optional = unwrap_async_iterable(AsyncIterator[int] | None)
    assert inner is int
    assert optional is True

    plain, plain_optional = unwrap_async_iterable(int)
    assert plain is int
    assert plain_optional is False


def test_split_optional_handles_multi_member_unions():
    """Normalizes optional unions to the non-None union payload."""
    annotation, optional = _split_optional(int | str | None)
    assert optional is True
    assert annotation == (int | str)

    plain, plain_optional = _split_optional(int)
    assert plain is int
    assert plain_optional is False


def test_is_hidden_field_supports_private_name_metadata_and_classvar():
    """Treats private names, Hidden metadata, and ClassVar fields as hidden."""
    assert is_hidden_field("_private", int) is True
    assert is_hidden_field("hidden", Annotated[int, Hidden]) is True
    assert is_hidden_field("shared", ClassVar[int]) is True
    assert is_hidden_field("visible", int) is False


def test_walk_annotation_skips_context_and_none_and_recurses_unions():
    """Skips context-only annotations, tolerates None annotations, and walks union members."""
    assert list(walk_annotation(Annotated[int, Context])) == []
    assert list(walk_annotation(None)) == []
    assert list(walk_annotation(OutputType | None | OutputType)) == [OutputType]


def test_walk_annotation_skips_context_inside_nested_inner_types():
    """Skips context-only members while recursing through nested inner annotations."""
    assert list(walk_annotation(list[Annotated[int, Context]])) == []


def test_walk_annotation_rejects_unparameterized_lists():
    """Raises for unparameterized typing.List annotations."""
    with pytest.raises(TypeError, match="List types must be parameterized"):
        list(walk_annotation(List))


def test_type_spec_from_annotation_handles_scalars_and_grommet_type_kinds():
    """Builds TypeSpec for scalars and validates input/output kind compatibility."""
    scalar = _type_spec_from_annotation(int, expect_input=False)
    assert scalar.kind == "named"
    assert scalar.name == "Int"
    assert scalar.nullable is False

    input_spec = _type_spec_from_annotation(InputType, expect_input=True)
    assert input_spec.name == "InputType"

    output_spec = _type_spec_from_annotation(OutputType, expect_input=False)
    assert output_spec.name == "OutputType"

    with pytest.raises(TypeError, match="is not an input type"):
        _type_spec_from_annotation(OutputType, expect_input=True)

    with pytest.raises(TypeError, match="cannot be used as output"):
        _type_spec_from_annotation(InputType, expect_input=False)


def test_type_spec_from_annotation_rejects_context_lists_and_unsupported_types():
    """Raises on unsupported context annotations, unparameterized lists, and unknown types."""
    with pytest.raises(TypeError, match="Unsupported annotation"):
        _type_spec_from_annotation(Annotated[int, Context], expect_input=False)

    with pytest.raises(TypeError, match="List types must be parameterized"):
        _type_spec_from_annotation(List, expect_input=False)

    with pytest.raises(TypeError, match="Unsupported annotation"):
        _type_spec_from_annotation(dict[str, int], expect_input=False)


def test_get_type_meta_rejects_undecorated_types():
    """Rejects classes that do not carry compiled grommet metadata."""

    class Plain:
        pass

    with pytest.raises(TypeError, match="not decorated"):
        _get_type_meta(Plain)


def test_build_union_type_spec_returns_none_for_non_unions():
    """Returns None when building TypeSpec for non-union annotations."""
    assert (
        _build_union_type_spec(
            annotation=int, inner=int, metadata=(), expect_input=False, nullable=False
        )
        is None
    )


def test_build_union_type_spec_rejects_input_positions_and_invalid_members():
    """Rejects union usage in inputs and non-object union members."""
    with pytest.raises(TypeError, match="not supported in input annotations"):
        _build_union_type_spec(
            annotation=OutputType | OutputTypeB,
            inner=OutputType | OutputTypeB,
            metadata=(),
            expect_input=True,
            nullable=False,
        )

    with pytest.raises(TypeError, match="Union member 'int'"):
        _build_union_type_spec(
            annotation=int | str,
            inner=int | str,
            metadata=(),
            expect_input=False,
            nullable=False,
        )

    with pytest.raises(TypeError, match="Union member 'InterfaceType'"):
        _build_union_type_spec(
            annotation=OutputType | InterfaceType,
            inner=OutputType | InterfaceType,
            metadata=(),
            expect_input=False,
            nullable=False,
        )


def test_build_union_type_spec_rejects_empty_member_sets(
    monkeypatch: pytest.MonkeyPatch,
):
    """Raises when union member discovery yields no object members."""
    monkeypatch.setattr(
        "grommet.annotations._iter_union_members", lambda _annotation: iter(())
    )
    with pytest.raises(TypeError, match="Unsupported annotation"):
        _build_union_type_spec(
            annotation=OutputType | OutputTypeB,
            inner=OutputType | OutputTypeB,
            metadata=(),
            expect_input=False,
            nullable=False,
        )


def test_build_union_type_spec_supports_custom_metadata_and_defaults():
    """Builds union specs with explicit metadata and automatic name fallback."""
    custom = _build_union_type_spec(
        annotation=OutputType | OutputTypeB,
        inner=OutputType | OutputTypeB,
        metadata=(grommet.Union(name="Custom", description="A custom union"),),
        expect_input=False,
        nullable=True,
    )
    assert custom is not None
    assert custom.name == "Custom"
    assert custom.union_description == "A custom union"
    assert custom.nullable is True

    automatic = _build_union_type_spec(
        annotation=OutputType | OutputTypeB,
        inner=OutputType | OutputTypeB,
        metadata=(),
        expect_input=False,
        nullable=False,
    )
    assert automatic is not None
    assert automatic.name == "OutputTypeOutputTypeB"


def test_build_union_type_spec_skips_none_union_members():
    """Skips None union members while preserving object member registrations."""
    spec = _build_union_type_spec(
        annotation=OutputType | None | OutputTypeB,
        inner=OutputType | None | OutputTypeB,
        metadata=(),
        expect_input=False,
        nullable=False,
    )
    assert spec is not None
    assert spec.union_members == ("OutputType", "OutputTypeB")


def test_analyze_annotation_handles_empty_annotated_args(
    monkeypatch: pytest.MonkeyPatch,
):
    """Covers the defensive Annotated-without-args fallback branch."""
    original_get_args = annotations_module.get_args

    def patched_get_args(annotation: object) -> tuple[object, ...]:
        if annotations_module.get_origin(annotation) is Annotated:
            return ()
        return original_get_args(annotation)

    monkeypatch.setattr(annotations_module, "get_args", patched_get_args)
    info = analyze_annotation(Annotated[int, Context])
    assert info.inner == Annotated[int, Context]


def test_is_input_type_detects_decorated_input_classes():
    """Returns True only for classes decorated as grommet input types."""
    assert _is_input_type(InputType) is True
    assert _is_input_type(OutputType) is False
