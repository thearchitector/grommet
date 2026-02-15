from dataclasses import dataclass
from typing import TYPE_CHECKING

from . import _core
from .metadata import MISSING, TypeKind

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any, Literal

    from .metadata import TypeMeta, TypeSpec

META_ATTR = "__grommet_meta__"
REFS_ATTR = "__grommet_refs__"
COMPILED_RESOLVER_ATTR = "__grommet_compiled_resolver__"
COMPILED_TYPE_ATTR = "__grommet_compiled_type__"


@dataclass(frozen=True, slots=True)
class CompiledArg:
    name: str
    type_spec: "TypeSpec"
    default: object = MISSING


@dataclass(frozen=True, slots=True)
class CompiledResolverField:
    kind: "Literal['field', 'subscription']"
    name: str
    func: "Callable[..., Any]"
    shape: str
    arg_names: tuple[str, ...]
    is_async: bool
    type_spec: "TypeSpec"
    description: str | None
    args: tuple[CompiledArg, ...]
    refs: frozenset["pytype"]


@dataclass(frozen=True, slots=True)
class CompiledDataField:
    name: str
    type_spec: "TypeSpec"
    description: str | None
    default: object
    resolver_func: "Callable[..., Any]"
    refs: frozenset["pytype"]


@dataclass(frozen=True, slots=True)
class CompiledInputField:
    name: str
    type_spec: "TypeSpec"
    description: str | None
    default: object
    refs: frozenset["pytype"]


@dataclass(frozen=True, slots=True)
class CompiledType:
    meta: "TypeMeta"
    object_fields: tuple["CompiledDataField | CompiledResolverField", ...] = ()
    subscription_fields: tuple[CompiledResolverField, ...] = ()
    input_fields: tuple[CompiledInputField, ...] = ()
    refs: frozenset["pytype"] = frozenset()


def _core_args(
    args: tuple[CompiledArg, ...],
) -> "list[tuple[str, TypeSpec, Any | None]] | None":
    values = [
        (arg.name, arg.type_spec, None if arg.default is MISSING else arg.default)
        for arg in args
    ]
    return values or None


def instantiate_core_type(compiled_type: CompiledType) -> "Any":
    """Instantiate a fresh Rust dynamic type from compiled Python metadata."""
    meta = compiled_type.meta

    if meta.kind is TypeKind.INPUT:
        input_fields = [
            _core.InputValue(
                field.name,
                field.type_spec,
                None if field.default is MISSING else field.default,
                field.description,
            )
            for field in compiled_type.input_fields
        ]
        return _core.InputObject(meta.name, meta.description, input_fields or None)

    if meta.kind is TypeKind.SUBSCRIPTION:
        subscription_fields = [
            _core.SubscriptionField(
                field.name,
                field.type_spec,
                field.func,
                field.shape,
                list(field.arg_names),
                field.description,
                _core_args(field.args),
            )
            for field in compiled_type.subscription_fields
        ]
        return _core.Subscription(
            meta.name, meta.description, subscription_fields or None
        )

    object_fields: list[_core.Field] = []
    for field in compiled_type.object_fields:
        if isinstance(field, CompiledDataField):
            object_fields.append(
                _core.Field(
                    field.name,
                    field.type_spec,
                    field.resolver_func,
                    "self_only",
                    [],
                    False,
                    field.description,
                )
            )
            continue

        object_fields.append(
            _core.Field(
                field.name,
                field.type_spec,
                field.func,
                field.shape,
                list(field.arg_names),
                field.is_async,
                field.description,
                _core_args(field.args),
            )
        )

    return _core.Object(meta.name, meta.description, object_fields or None)
