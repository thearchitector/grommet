"""Targeted branch coverage tests for grommet.plan."""

from dataclasses import dataclass

import pytest

import grommet
from grommet._compiled import (
    COMPILED_TYPE_ATTR,
    REFS_ATTR,
    CompiledDataField,
    CompiledType,
)
from grommet.metadata import TypeKind, TypeMeta, TypeSpec
from grommet.plan import (
    _class_sort_key,
    _collect_compiled_unions,
    _get_compiled_type,
    _iter_interface_implementers,
    _iter_union_type_specs,
    _validate_root_defaults,
    _walk_and_collect,
    build_schema_graph,
)


def _compiled_object_with_union(
    *,
    type_name: str,
    union_name: str,
    members: tuple[str, ...],
    description: str | None = None,
) -> CompiledType:
    union_spec = TypeSpec(
        kind="union",
        name=union_name,
        union_members=members,
        union_description=description,
        nullable=False,
    )
    field = CompiledDataField(
        name="value",
        type_spec=union_spec,
        description=None,
        has_default=True,
        default=1,
        resolver_func=lambda self, _context, _kwargs: self.value,
        refs=frozenset(),
    )
    return CompiledType(
        meta=TypeMeta(kind=TypeKind.OBJECT, name=type_name), object_fields=(field,)
    )


def test_get_compiled_type_rejects_classes_without_compiled_metadata():
    """Raises when classes do not provide compiled grommet metadata."""

    class Plain:
        pass

    with pytest.raises(TypeError, match="missing compiled"):
        _get_compiled_type(Plain)


def test_walk_and_collect_deduplicates_roots_and_skips_non_grommet_refs():
    """Skips duplicate roots and ignores non-grommet classes discovered via refs."""

    @grommet.type
    @dataclass
    class Query:
        greeting: str = "Hello"

    class Plain:
        pass

    setattr(Query, REFS_ATTR, frozenset({Query, Plain}))
    collected = _walk_and_collect(Query, Query, None)
    assert collected == [Query]


def test_walk_and_collect_skips_already_visited_interface_implementers():
    """Avoids re-queueing interface implementers that are already visited roots."""

    @grommet.interface
    @dataclass
    class InterfaceRoot:
        value: str = "iface"

    @grommet.type
    @dataclass
    class ObjectImpl(InterfaceRoot):
        pass

    collected = _walk_and_collect(InterfaceRoot, ObjectImpl, None)
    assert collected == [ObjectImpl, InterfaceRoot]


def test_iter_interface_implementers_only_yields_decorated_object_types():
    """Yields object implementers while ignoring undecorated and input subclasses."""

    @grommet.interface
    @dataclass
    class InterfaceRoot:
        name: str = "iface"

    class Left(InterfaceRoot):
        pass

    class Right(InterfaceRoot):
        pass

    class Undecorated(InterfaceRoot):
        __grommet_meta__ = None

    @grommet.type
    @dataclass
    class ObjectImpl(Left, Right):
        pass

    @grommet.input
    @dataclass
    class InputImpl(InterfaceRoot):
        pass

    implementers = list(_iter_interface_implementers(InterfaceRoot))
    assert implementers == [ObjectImpl]
    assert Undecorated not in implementers
    assert InputImpl not in implementers


def test_collect_compiled_unions_rejects_empty_names():
    """Rejects union registrations that do not provide a non-empty name."""
    compiled = _compiled_object_with_union(
        type_name="Query", union_name="", members=("A",)
    )
    with pytest.raises(TypeError, match="non-empty name"):
        _collect_compiled_unions([compiled])


def test_collect_compiled_unions_rejects_empty_member_lists():
    """Rejects union registrations that omit all possible object types."""
    compiled = _compiled_object_with_union(
        type_name="Query", union_name="Named", members=()
    )
    with pytest.raises(TypeError, match="at least one object type"):
        _collect_compiled_unions([compiled])


def test_collect_compiled_unions_rejects_conflicting_definitions():
    """Rejects conflicting union definitions sharing the same union name."""
    first = _compiled_object_with_union(
        type_name="QueryA", union_name="Named", members=("A", "B")
    )
    second = _compiled_object_with_union(
        type_name="QueryB", union_name="Named", members=("A", "C")
    )

    with pytest.raises(TypeError, match="conflicting definitions"):
        _collect_compiled_unions([first, second])


def test_collect_compiled_unions_merges_stable_union_definitions():
    """Collects stable union definitions into sorted CompiledUnion registrations."""
    first = _compiled_object_with_union(
        type_name="QueryA",
        union_name="Named",
        members=("A", "B"),
        description="A union",
    )
    second = _compiled_object_with_union(
        type_name="QueryB", union_name="Other", members=("C", "D")
    )

    unions = _collect_compiled_unions([second, first])
    assert [union.meta.name for union in unions] == ["Named", "Other"]
    assert unions[0].possible_types == ("A", "B")
    assert unions[0].meta.description == "A union"


def test_iter_union_type_specs_recurses_into_nested_type_specs():
    """Finds nested union specs under list wrappers."""
    union_spec = TypeSpec(kind="union", name="Named", union_members=("A", "B"))
    nested = TypeSpec(kind="list", of_type=union_spec)
    assert list(_iter_union_type_specs(nested)) == [union_spec]


def test_validate_root_defaults_rejects_missing_defaults():
    """Rejects root data fields lacking defaults in compiled schema metadata."""

    class Root:
        pass

    compiled = CompiledType(
        meta=TypeMeta(kind=TypeKind.OBJECT, name="Root"),
        object_fields=(
            CompiledDataField(
                name="greeting",
                type_spec=TypeSpec(kind="named", name="String"),
                description=None,
                has_default=False,
                default=None,
                resolver_func=lambda self, _context, _kwargs: self.greeting,
                refs=frozenset(),
            ),
        ),
    )
    setattr(Root, COMPILED_TYPE_ATTR, compiled)

    with pytest.raises(TypeError, match="must declare a default value"):
        _validate_root_defaults(Root)


def test_build_schema_graph_rejects_roots_missing_compiled_type_metadata():
    """Raises when build_schema_graph receives undecorated root classes."""

    class Plain:
        pass

    with pytest.raises(TypeError, match="missing compiled"):
        build_schema_graph(query=Plain)


def test_class_sort_key_uses_module_and_qualname():
    """Builds deterministic sort keys from module and qualname."""
    key = _class_sort_key(TypeMeta)
    assert key.endswith("grommet.metadata.TypeMeta")
