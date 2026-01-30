import dataclasses
from builtins import type as pytype
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any

MISSING = dataclasses.MISSING


class ID(str):
    """GraphQL ID scalar."""


@dataclasses.dataclass(frozen=True)
class TypeMeta:
    kind: str
    name: str
    description: str | None = None
    implements: tuple[pytype, ...] = ()


@dataclasses.dataclass(frozen=True)
class FieldMeta:
    resolver: "Callable[..., Any] | None" = None
    description: str | None = None
    deprecation_reason: str | None = None
    name: str | None = None


@dataclasses.dataclass(frozen=True)
class ScalarMeta:
    name: str
    serialize: "Callable[[Any], Any]"
    parse_value: "Callable[[Any], Any]"
    description: str | None = None
    specified_by_url: str | None = None


@dataclasses.dataclass(frozen=True)
class EnumMeta:
    name: str
    description: str | None = None


@dataclasses.dataclass(frozen=True)
class UnionMeta:
    name: str
    types: tuple[pytype, ...]
    description: str | None = None


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
