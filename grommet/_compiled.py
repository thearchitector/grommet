from dataclasses import dataclass
from typing import TYPE_CHECKING

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
    has_default: bool = False
    default: object | None = None


@dataclass(frozen=True, slots=True)
class CompiledResolverField:
    kind: "Literal['field', 'subscription']"
    name: str
    func: "Callable[..., Any]"
    needs_context: bool
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
    has_default: bool
    default: object | None
    resolver_func: "Callable[..., Any]"
    refs: frozenset["pytype"]


@dataclass(frozen=True, slots=True)
class CompiledInputField:
    name: str
    type_spec: "TypeSpec"
    description: str | None
    has_default: bool
    default: object | None
    refs: frozenset["pytype"]


@dataclass(frozen=True, slots=True)
class CompiledType:
    meta: "TypeMeta"
    object_fields: tuple["CompiledDataField | CompiledResolverField", ...] = ()
    subscription_fields: tuple[CompiledResolverField, ...] = ()
    input_fields: tuple[CompiledInputField, ...] = ()
    refs: frozenset["pytype"] = frozenset()
