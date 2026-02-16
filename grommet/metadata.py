import dataclasses
import enum

MISSING = dataclasses.MISSING

NO_DEFAULT: object = object()


Context = object()
Hidden = object()


@dataclasses.dataclass(frozen=True, slots=True)
class Field:
    """Annotated metadata providing field-level GraphQL configuration."""

    description: str | None = None


@dataclasses.dataclass(frozen=True, slots=True)
class Union:
    """Annotated metadata providing union-level GraphQL configuration."""

    name: str | None = None
    description: str | None = None


class TypeKind(enum.Enum):
    OBJECT = "object"
    INPUT = "input"
    INTERFACE = "interface"
    SUBSCRIPTION = "subscription"
    UNION = "union"


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
    union_members: tuple[str, ...] = ()
    union_description: str | None = None
    nullable: bool = False


@dataclasses.dataclass(frozen=True, slots=True)
class ArgPlan:
    """Planned argument for a resolver field."""

    name: str
    type_spec: TypeSpec
    default: object = NO_DEFAULT


_SCALARS = {str: "String", int: "Int", float: "Float", bool: "Boolean"}
