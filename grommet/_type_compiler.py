import dataclasses
from operator import attrgetter
from typing import TYPE_CHECKING

from ._annotations import get_annotations
from ._compiled import (
    COMPILED_RESOLVER_ATTR,
    COMPILED_TYPE_ATTR,
    META_ATTR,
    REFS_ATTR,
    CompiledDataField,
    CompiledInputField,
    CompiledResolverField,
    CompiledType,
)
from .annotations import (
    _type_spec_from_annotation,
    analyze_annotation,
    is_hidden_field,
    walk_annotation,
)
from .coercion import _input_field_default
from .errors import GrommetTypeError, input_field_resolver_not_allowed
from .metadata import MISSING, Field, TypeKind, TypeMeta

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable, Iterator
    from typing import Any


def _get_annotated_field_meta(annotation: "Any") -> Field | None:
    info = analyze_annotation(annotation)
    for item in info.metadata:
        if isinstance(item, Field):
            return item
    return None


def _data_field_resolver(field_name: str, default: object) -> "Callable[[Any], Any]":
    getter = attrgetter(field_name)
    if default is MISSING:
        return getter

    def _resolver(self: object) -> object:
        if self is None:
            return default
        return getter(self)

    return _resolver


def _resolve_data_field_default(dc_field: "dataclasses.Field[Any]") -> object:
    if dc_field.default is not MISSING:
        return dc_field.default
    if dc_field.default_factory is not MISSING:
        return dc_field.default_factory()
    return MISSING


def _iter_compiled_resolvers(cls: "pytype") -> list[CompiledResolverField]:
    resolver_fields: list[CompiledResolverField] = []
    for attr_value in vars(cls).values():
        compiled = getattr(attr_value, COMPILED_RESOLVER_ATTR, None)
        if isinstance(compiled, CompiledResolverField):
            resolver_fields.append(compiled)
    return resolver_fields


def _iter_visible_dataclass_fields(
    cls: "pytype", hints: dict[str, "Any"]
) -> "Iterator[tuple[dataclasses.Field[Any], Any, str | None, frozenset[pytype]]]":
    """Yield visible dataclass fields with normalized metadata used by all compile modes."""
    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue

        refs = frozenset(walk_annotation(annotation))
        field_meta = _get_annotated_field_meta(annotation)
        description = field_meta.description if field_meta else None
        yield dc_field, annotation, description, refs


def compile_type_definition(  # noqa: PLR0912 - orchestrates three compile modes in one pass
    cls: "pytype", *, kind: TypeKind, name: str | None, description: str | None
) -> CompiledType:
    """Compile a decorated class into immutable metadata used at schema build time."""
    type_name = name or cls.__name__
    hints = get_annotations(cls)
    resolvers = _iter_compiled_resolvers(cls)

    field_resolvers = [resolver for resolver in resolvers if resolver.kind == "field"]
    subscription_resolvers = [
        resolver for resolver in resolvers if resolver.kind == "subscription"
    ]

    resolved_kind = kind
    if kind is TypeKind.INPUT:
        if resolvers:
            raise input_field_resolver_not_allowed()
    else:
        if field_resolvers and subscription_resolvers:
            raise GrommetTypeError(
                "A type cannot mix @field and @subscription decorators."
            )
        if subscription_resolvers:
            resolved_kind = TypeKind.SUBSCRIPTION

    visible_fields = tuple(_iter_visible_dataclass_fields(cls, hints))
    refs: list[pytype] = []
    for _dc_field, _annotation, _desc, field_refs in visible_fields:
        refs.extend(field_refs)

    object_fields: list[CompiledDataField | CompiledResolverField] = []
    input_fields: list[CompiledInputField] = []
    subscription_fields: tuple[CompiledResolverField, ...] = ()

    if resolved_kind is TypeKind.SUBSCRIPTION:
        if visible_fields:
            raise GrommetTypeError("Subscription types cannot declare data fields.")
        subscription_fields = tuple(subscription_resolvers)
    elif resolved_kind is TypeKind.INPUT:
        for dc_field, annotation, desc, field_refs in visible_fields:
            force_nullable = (
                dc_field.default is not MISSING
                or dc_field.default_factory is not MISSING
            )
            type_spec = _type_spec_from_annotation(
                annotation, expect_input=True, force_nullable=force_nullable
            )
            default_value = _input_field_default(dc_field, annotation)
            input_fields.append(
                CompiledInputField(
                    name=dc_field.name,
                    type_spec=type_spec,
                    description=desc,
                    default=default_value,
                    refs=field_refs,
                )
            )
    else:
        for dc_field, annotation, desc, field_refs in visible_fields:
            type_spec = _type_spec_from_annotation(
                annotation, expect_input=False, force_nullable=dc_field.default is None
            )
            default_value = _resolve_data_field_default(dc_field)
            object_fields.append(
                CompiledDataField(
                    name=dc_field.name,
                    type_spec=type_spec,
                    description=desc,
                    default=default_value,
                    resolver_func=_data_field_resolver(dc_field.name, default_value),
                    refs=field_refs,
                )
            )
        object_fields.extend(field_resolvers)

    resolver_ref_sources = (
        tuple(field_resolvers)
        if resolved_kind is TypeKind.OBJECT
        else subscription_fields
    )
    for resolver in resolver_ref_sources:
        refs.extend(resolver.refs)

    meta = TypeMeta(kind=resolved_kind, name=type_name, description=description)
    compiled = CompiledType(
        meta=meta,
        object_fields=tuple(object_fields),
        subscription_fields=subscription_fields,
        input_fields=tuple(input_fields),
        refs=frozenset(refs),
    )

    setattr(cls, META_ATTR, meta)
    setattr(cls, REFS_ATTR, compiled.refs)
    setattr(cls, COMPILED_TYPE_ATTR, compiled)

    return compiled
