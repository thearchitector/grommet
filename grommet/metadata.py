import dataclasses
import enum
from builtins import type as pytype
from typing import TYPE_CHECKING, Annotated, Generic, TypeVar

from .errors import list_type_requires_inner, type_meta_unknown_kind

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any

MISSING = dataclasses.MISSING
_INTERNAL_MARKER = object()
_GROMMET_TYPES: set[pytype] = set()
_GROMMET_SCALARS: set[pytype] = set()
_GROMMET_ENUMS: set[pytype] = set()
_GROMMET_UNIONS: set[pytype] = set()
_INTERFACE_IMPLEMENTERS: dict[pytype, set[pytype]] = {}


class ID(str):
    """GraphQL ID scalar."""


if TYPE_CHECKING:
    type Internal[T] = Annotated[T, _INTERNAL_MARKER]
    type Private[T] = Internal[T]
else:
    _T = TypeVar("_T")

    class Internal(Generic[_T]):
        """Marks a field as internal so it is excluded from the schema."""

        def __class_getitem__(cls, item: _T) -> Annotated[_T, _INTERNAL_MARKER]:
            return Annotated[item, _INTERNAL_MARKER]

    class Private(Generic[_T]):
        """Marks a field as private so it is excluded from the schema."""

        def __class_getitem__(cls, item: _T) -> Annotated[_T, _INTERNAL_MARKER]:
            return Annotated[item, _INTERNAL_MARKER]


class TypeKind(enum.Enum):
    OBJECT = "object"
    INTERFACE = "interface"
    INPUT = "input"
    SUBSCRIPTION = "subscription"


class GrommetMetaType(enum.Enum):
    TYPE = "type"
    INTERFACE = "interface"
    INPUT = "input"
    SCALAR = "scalar"
    ENUM = "enum"
    UNION = "union"
    FIELD = "field"


_TYPE_KIND_TO_META: dict[TypeKind, GrommetMetaType] = {
    TypeKind.OBJECT: GrommetMetaType.TYPE,
    TypeKind.INTERFACE: GrommetMetaType.INTERFACE,
    TypeKind.INPUT: GrommetMetaType.INPUT,
    TypeKind.SUBSCRIPTION: GrommetMetaType.TYPE,
}


class GrommetMeta:
    type: GrommetMetaType


@dataclasses.dataclass(frozen=True, slots=True)
class TypeMeta(GrommetMeta):
    kind: TypeKind
    name: str
    description: str | None = None
    implements: tuple[pytype, ...] = ()
    type: GrommetMetaType = dataclasses.field(init=False)

    def __post_init__(self) -> None:
        meta_type = _TYPE_KIND_TO_META.get(self.kind)
        if meta_type is None:
            raise type_meta_unknown_kind(self.kind.value)
        object.__setattr__(self, "type", meta_type)


def _register_type(cls: pytype, meta: TypeMeta) -> None:
    _GROMMET_TYPES.add(cls)
    if meta.kind is TypeKind.INTERFACE:
        _INTERFACE_IMPLEMENTERS.setdefault(cls, set())
        return
    if meta.kind is TypeKind.OBJECT:
        for iface in meta.implements:
            _INTERFACE_IMPLEMENTERS.setdefault(iface, set()).add(cls)


def _register_scalar(cls: pytype) -> None:
    _GROMMET_SCALARS.add(cls)


def _register_enum(cls: pytype) -> None:
    _GROMMET_ENUMS.add(cls)


def _register_union(cls: pytype) -> None:
    _GROMMET_UNIONS.add(cls)


def _interface_implementers(cls: pytype) -> tuple[pytype, ...]:
    return tuple(_INTERFACE_IMPLEMENTERS.get(cls, ()))


@dataclasses.dataclass(frozen=True, slots=True)
class FieldMeta(GrommetMeta):
    resolver: "Callable[..., Any] | None" = None
    description: str | None = None
    deprecation_reason: str | None = None
    name: str | None = None
    type: GrommetMetaType = dataclasses.field(init=False, default=GrommetMetaType.FIELD)


@dataclasses.dataclass(frozen=True, slots=True)
class ScalarMeta(GrommetMeta):
    name: str
    serialize: "Callable[[Any], Any]"
    parse_value: "Callable[[Any], Any]"
    description: str | None = None
    specified_by_url: str | None = None
    type: GrommetMetaType = dataclasses.field(
        init=False, default=GrommetMetaType.SCALAR
    )


@dataclasses.dataclass(frozen=True, slots=True)
class EnumMeta(GrommetMeta):
    name: str
    description: str | None = None
    type: GrommetMetaType = dataclasses.field(init=False, default=GrommetMetaType.ENUM)


@dataclasses.dataclass(frozen=True, slots=True)
class UnionMeta(GrommetMeta):
    name: str
    types: tuple[pytype, ...]
    description: str | None = None
    type: GrommetMetaType = dataclasses.field(init=False, default=GrommetMetaType.UNION)


@dataclasses.dataclass(frozen=True, slots=True)
class TypeSpec:
    kind: str
    name: str | None = None
    of_type: "TypeSpec | None" = None
    nullable: bool = False

    def to_graphql(self) -> str:
        if self.kind == "named":
            rendered = self.name or ""
        else:
            if self.of_type is None:
                raise list_type_requires_inner()
            rendered = f"[{self.of_type.to_graphql()}]"
        if not self.nullable:
            rendered += "!"
        return rendered


_SCALARS = {str: "String", int: "Int", float: "Float", bool: "Boolean", ID: "ID"}
