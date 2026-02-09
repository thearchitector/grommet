import dataclasses
import enum

MISSING = dataclasses.MISSING

NO_DEFAULT: object = object()


class _HiddenType:
    """Marker to exclude a field from the GraphQL schema."""

    def __repr__(self) -> str:
        return "Hidden"


Hidden = _HiddenType()


@dataclasses.dataclass(frozen=True, slots=True)
class Field:
    """Annotated metadata providing field-level GraphQL configuration."""

    description: str | None = None


class TypeKind(enum.Enum):
    OBJECT = "object"
    INPUT = "input"
    SUBSCRIPTION = "subscription"


@dataclasses.dataclass(frozen=True, slots=True)
class TypeMeta:
    kind: TypeKind
    name: str
    description: str | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class TypeSpec:
    kind: str
    name: str | None = None
    of_type: "TypeSpec | None" = None
    nullable: bool = False


@dataclasses.dataclass(frozen=True, slots=True)
class ArgPlan:
    """Planned argument for a resolver field."""

    name: str
    type_spec: TypeSpec
    default: object = NO_DEFAULT


_SCALARS = {str: "String", int: "Int", float: "Float", bool: "Boolean"}
