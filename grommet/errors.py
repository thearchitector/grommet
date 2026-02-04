from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Any


class GrommetError(Exception):
    """Base exception for grommet errors."""


class GrommetTypeError(TypeError, GrommetError):
    """Raised when grommet encounters an invalid type or annotation."""


class GrommetValueError(ValueError, GrommetError):
    """Raised when grommet encounters an invalid value."""


class GrommetSchemaError(GrommetValueError):
    """Raised when the schema definition is invalid."""


def schema_requires_query() -> GrommetSchemaError:
    return GrommetSchemaError("Schema requires a query type.")


def unknown_type_kind(kind: str) -> GrommetTypeError:
    return GrommetTypeError(f"Unknown type kind: {kind}")


def type_meta_unknown_kind(kind: str) -> GrommetValueError:
    return GrommetValueError(f"Unknown type meta kind: {kind}")


def list_type_requires_parameter() -> GrommetTypeError:
    return GrommetTypeError("List types must be parameterized.")


def list_type_requires_inner() -> GrommetValueError:
    return GrommetValueError("List types require an inner type.")


def async_iterable_requires_parameter() -> GrommetTypeError:
    return GrommetTypeError("AsyncIterator/AsyncIterable must be parameterized.")


def resolver_missing_annotation(
    resolver_name: str, param_name: str
) -> GrommetTypeError:
    return GrommetTypeError(
        f"Resolver {resolver_name} missing annotation for '{param_name}'."
    )


def resolver_missing_return_annotation(
    resolver_name: str, field_name: str
) -> GrommetTypeError:
    return GrommetTypeError(
        f"Resolver {resolver_name} missing return annotation for field '{field_name}'."
    )


def resolver_requires_async(resolver_name: str, field_name: str) -> GrommetTypeError:
    return GrommetTypeError(
        f"Resolver {resolver_name} for field '{field_name}' must be async."
    )


def subscription_requires_async_iterator(
    resolver_name: str, field_name: str
) -> GrommetTypeError:
    return GrommetTypeError(
        f"Subscription resolver {resolver_name} for field '{field_name}' must return an async iterator."
    )


def union_input_not_supported() -> GrommetTypeError:
    return GrommetTypeError("Union types cannot be used as input")


def input_type_expected(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} is not an input type")


def output_type_expected(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} cannot be used as output")


def unsupported_annotation(annotation: "Any") -> GrommetTypeError:
    return GrommetTypeError(f"Unsupported annotation: {annotation}")


def not_grommet_type(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(
        f"{type_name} is not decorated with @grommet.type, @grommet.interface, or @grommet.input"
    )


def not_grommet_scalar(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} is not decorated with @grommet.scalar")


def not_grommet_enum(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} is not decorated with @grommet.enum")


def not_grommet_union(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} is not a grommet union")


def field_default_conflict() -> GrommetTypeError:
    return GrommetTypeError("field() cannot specify both default and default_factory.")


def dataclass_required(decorator_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{decorator_name} requires an explicit dataclass.")


def input_field_resolver_not_allowed() -> GrommetTypeError:
    return GrommetTypeError("Input types cannot declare field resolvers.")


def decorator_requires_callable() -> GrommetTypeError:
    return GrommetTypeError("Decorator usage expects a callable resolver.")


def scalar_requires_callables() -> GrommetTypeError:
    return GrommetTypeError("scalar() requires serialize and parse_value callables.")


def enum_requires_enum_subclass() -> GrommetTypeError:
    return GrommetTypeError("@grommet.enum requires an enum.Enum subclass.")


def union_requires_name() -> GrommetTypeError:
    return GrommetTypeError("union() requires a name.")


def union_requires_types() -> GrommetTypeError:
    return GrommetTypeError("union() requires at least one possible type.")


def union_requires_object_types() -> GrommetTypeError:
    return GrommetTypeError("union() types must be @grommet.type object types.")


def invalid_enum_value(value: "Any", enum_name: str) -> GrommetValueError:
    return GrommetValueError(f"Invalid enum value '{value}' for {enum_name}")


def input_mapping_expected(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"Expected mapping for input type {type_name}")
