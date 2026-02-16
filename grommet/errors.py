from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Any


class GrommetError(Exception):
    """Base exception for grommet errors."""


class GrommetTypeError(TypeError, GrommetError):
    """Raised when grommet encounters an invalid type or annotation."""


def list_type_requires_parameter() -> GrommetTypeError:
    return GrommetTypeError("List types must be parameterized.")


def async_iterable_requires_parameter() -> GrommetTypeError:
    return GrommetTypeError("AsyncIterator/AsyncIterable must be parameterized.")


def resolver_missing_annotation(
    resolver_name: str, param_name: str
) -> GrommetTypeError:
    return GrommetTypeError(
        f"Resolver {resolver_name} missing annotation for '{param_name}'."
    )


def resolver_context_annotation_requires_annotated(
    resolver_name: str, param_name: str
) -> GrommetTypeError:
    return GrommetTypeError(
        f"Resolver {resolver_name} parameter '{param_name}' must use "
        "Annotated[T, grommet.Context] for context injection."
    )


def resolver_requires_async(resolver_name: str, field_name: str) -> GrommetTypeError:
    return GrommetTypeError(
        f"Resolver {resolver_name} for field '{field_name}' must be async."
    )


def input_type_expected(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} is not an input type")


def output_type_expected(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{type_name} cannot be used as output")


def unsupported_annotation(annotation: "Any") -> GrommetTypeError:
    return GrommetTypeError(f"Unsupported annotation: {annotation}")


def not_grommet_type(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(
        f"{type_name} is not decorated with @grommet.type, "
        "@grommet.interface, or @grommet.input"
    )


def dataclass_required(decorator_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"{decorator_name} requires an explicit dataclass.")


def input_field_resolver_not_allowed() -> GrommetTypeError:
    return GrommetTypeError("Input types cannot declare field resolvers.")


def decorator_requires_callable() -> GrommetTypeError:
    return GrommetTypeError("Decorator usage expects a callable resolver.")


def input_mapping_expected(type_name: str) -> GrommetTypeError:
    return GrommetTypeError(f"Expected mapping for input type {type_name}")


def union_input_not_supported() -> GrommetTypeError:
    return GrommetTypeError("Union types are not supported in input annotations.")


def union_member_must_be_object(member_name: str) -> GrommetTypeError:
    return GrommetTypeError(
        f"Union member '{member_name}' must be a type decorated with @grommet.type."
    )


def union_definition_conflict(union_name: str) -> GrommetTypeError:
    return GrommetTypeError(
        f"Union '{union_name}' has conflicting definitions across the schema graph."
    )
