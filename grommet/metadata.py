import dataclasses
import enum
from builtins import type as pytype
from typing import TYPE_CHECKING, Annotated

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any

MISSING = dataclasses.MISSING
_INTERNAL_MARKER = object()


class ID(str):
    """GraphQL ID scalar."""


type Internal[T] = Annotated[T, _INTERNAL_MARKER]
type Private[T] = Internal[T]


class GrommetMetaType(enum.Enum):
    TYPE = "type"
    INTERFACE = "interface"
    INPUT = "input"
    SCALAR = "scalar"
    ENUM = "enum"
    UNION = "union"
    FIELD = "field"


class GrommetMeta:
    type: GrommetMetaType


@dataclasses.dataclass(frozen=True)
class TypeMeta(GrommetMeta):
    kind: str
    name: str
    description: str | None = None
    implements: tuple[pytype, ...] = ()
    type: GrommetMetaType = dataclasses.field(init=False)

    def __post_init__(self) -> None:
        meta_type = {
            "object": GrommetMetaType.TYPE,
            "interface": GrommetMetaType.INTERFACE,
            "input": GrommetMetaType.INPUT,
        }.get(self.kind)
        if meta_type is None:
            raise ValueError(f"Unknown type meta kind: {self.kind}")
        object.__setattr__(self, "type", meta_type)


@dataclasses.dataclass(frozen=True)
class FieldMeta(GrommetMeta):
    resolver: "Callable[..., Any] | None" = None
    description: str | None = None
    deprecation_reason: str | None = None
    name: str | None = None
    type: GrommetMetaType = dataclasses.field(
        init=False, default=GrommetMetaType.FIELD
    )


@dataclasses.dataclass(frozen=True)
class ScalarMeta(GrommetMeta):
    name: str
    serialize: "Callable[[Any], Any]"
    parse_value: "Callable[[Any], Any]"
    description: str | None = None
    specified_by_url: str | None = None
    type: GrommetMetaType = dataclasses.field(
        init=False, default=GrommetMetaType.SCALAR
    )


@dataclasses.dataclass(frozen=True)
class EnumMeta(GrommetMeta):
    name: str
    description: str | None = None
    type: GrommetMetaType = dataclasses.field(
        init=False, default=GrommetMetaType.ENUM
    )


@dataclasses.dataclass(frozen=True)
class UnionMeta(GrommetMeta):
    name: str
    types: tuple[pytype, ...]
    description: str | None = None
    type: GrommetMetaType = dataclasses.field(
        init=False, default=GrommetMetaType.UNION
    )


@dataclasses.dataclass(frozen=True)
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
                raise ValueError("List types require an inner type.")
            rendered = f"[{self.of_type.to_graphql()}]"
        if not self.nullable:
            rendered += "!"
        return rendered


_SCALARS = {
    str: "String",
    int: "Int",
    float: "Float",
    bool: "Boolean",
    ID: "ID",
}
