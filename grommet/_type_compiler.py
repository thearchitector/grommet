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
    _get_type_meta,
    _is_grommet_type,
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


def _data_field_resolver(
    field_name: str, *, has_default: bool, default: object | None
) -> "Callable[[Any, Any, dict[str, Any]], Any]":
    getter = attrgetter(field_name)
    if not has_default:
        return lambda self, _context, _kwargs: getter(self)

    def _resolver(self: object, _context: object, _kwargs: dict[str, object]) -> object:
        if self is None:
            return default
        return getter(self)

    return _resolver


def _resolve_data_field_default(
    dc_field: "dataclasses.Field[Any]",
) -> tuple[bool, object | None]:
    if dc_field.default is not MISSING:
        return True, dc_field.default
    if dc_field.default_factory is not MISSING:
        return True, dc_field.default_factory()
    return False, None


def _iter_compiled_resolvers(cls: "pytype") -> list[CompiledResolverField]:
    resolver_fields: list[CompiledResolverField] = []
    seen_attrs: set[str] = set()
    for source_cls in cls.__mro__:
        if source_cls is object:
            continue
        for attr_name, attr_value in vars(source_cls).items():
            if attr_name in seen_attrs:
                continue
            seen_attrs.add(attr_name)
            compiled = getattr(attr_value, COMPILED_RESOLVER_ATTR, None)
            if isinstance(compiled, CompiledResolverField):
                resolver_fields.append(compiled)
    return resolver_fields


def _implemented_interfaces(
    cls: "pytype",
) -> tuple[tuple[str, ...], tuple["pytype", ...]]:
    names: list[str] = []
    refs: list[pytype] = []
    for base in cls.__mro__[1:]:
        if not _is_grommet_type(base):
            continue
        meta = _get_type_meta(base)
        if meta.kind is not TypeKind.INTERFACE:
            continue
        names.append(meta.name)
        refs.append(base)
    return tuple(dict.fromkeys(names)), tuple(dict.fromkeys(refs))


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


def _compile_subscription_fields(
    visible_fields: "tuple[tuple[dataclasses.Field[Any], Any, str | None, frozenset[pytype]], ...]",
    subscription_resolvers: list[CompiledResolverField],
) -> tuple[CompiledResolverField, ...]:
    if visible_fields:
        raise GrommetTypeError("Subscription types cannot declare data fields.")
    return tuple(subscription_resolvers)


def _compile_input_fields(
    visible_fields: "tuple[tuple[dataclasses.Field[Any], Any, str | None, frozenset[pytype]], ...]",
) -> tuple[CompiledInputField, ...]:
    fields: list[CompiledInputField] = []
    for dc_field, annotation, desc, field_refs in visible_fields:
        force_nullable = (
            dc_field.default is not MISSING or dc_field.default_factory is not MISSING
        )
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=force_nullable
        )
        default_value = _input_field_default(dc_field, annotation)
        has_default = default_value is not MISSING
        fields.append(
            CompiledInputField(
                name=dc_field.name,
                type_spec=type_spec,
                description=desc,
                has_default=has_default,
                default=default_value if has_default else None,
                refs=field_refs,
            )
        )
    return tuple(fields)


def _compile_object_fields(
    visible_fields: "tuple[tuple[dataclasses.Field[Any], Any, str | None, frozenset[pytype]], ...]",
    field_resolvers: list[CompiledResolverField],
) -> tuple[CompiledDataField | CompiledResolverField, ...]:
    fields: list[CompiledDataField | CompiledResolverField] = []
    for dc_field, annotation, desc, field_refs in visible_fields:
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=False, force_nullable=dc_field.default is None
        )
        has_default, default = _resolve_data_field_default(dc_field)
        fields.append(
            CompiledDataField(
                name=dc_field.name,
                type_spec=type_spec,
                description=desc,
                has_default=has_default,
                default=default,
                resolver_func=_data_field_resolver(
                    dc_field.name, has_default=has_default, default=default
                ),
                refs=field_refs,
            )
        )
    fields.extend(field_resolvers)
    return tuple(fields)


def compile_type_definition(
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
        if kind is TypeKind.INTERFACE and subscription_resolvers:
            raise GrommetTypeError(
                "Interface types cannot declare @subscription resolvers."
            )
        if subscription_resolvers:
            resolved_kind = TypeKind.SUBSCRIPTION

    visible_fields = tuple(_iter_visible_dataclass_fields(cls, hints))
    visible_field_names = {
        dc_field.name for dc_field, _ann, _desc, _refs in visible_fields
    }
    field_resolvers = [
        resolver
        for resolver in field_resolvers
        if resolver.name not in visible_field_names
    ]

    refs: list[pytype] = []
    for _dc_field, _annotation, _desc, field_refs in visible_fields:
        refs.extend(field_refs)

    implements: tuple[str, ...] = ()
    if resolved_kind in {TypeKind.OBJECT, TypeKind.INTERFACE}:
        implements, interface_refs = _implemented_interfaces(cls)
        refs.extend(interface_refs)

    object_fields: tuple[CompiledDataField | CompiledResolverField, ...] = ()
    input_fields: tuple[CompiledInputField, ...] = ()
    subscription_fields: tuple[CompiledResolverField, ...] = ()

    if resolved_kind is TypeKind.SUBSCRIPTION:
        subscription_fields = _compile_subscription_fields(
            visible_fields, subscription_resolvers
        )
    elif resolved_kind is TypeKind.INPUT:
        input_fields = _compile_input_fields(visible_fields)
    else:
        object_fields = _compile_object_fields(visible_fields, field_resolvers)

    resolver_ref_sources = (
        tuple(field_resolvers)
        if resolved_kind in {TypeKind.OBJECT, TypeKind.INTERFACE}
        else subscription_fields
    )
    for resolver in resolver_ref_sources:
        refs.extend(resolver.refs)

    meta = TypeMeta(kind=resolved_kind, name=type_name, description=description)
    compiled = CompiledType(
        meta=meta,
        object_fields=object_fields,
        subscription_fields=subscription_fields,
        input_fields=input_fields,
        implements=implements,
        refs=frozenset(refs),
    )

    setattr(cls, META_ATTR, meta)
    setattr(cls, REFS_ATTR, compiled.refs)
    setattr(cls, COMPILED_TYPE_ATTR, compiled)

    return compiled
