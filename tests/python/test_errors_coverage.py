import pytest

from grommet import errors
from grommet.errors import GrommetSchemaError, GrommetTypeError, GrommetValueError


@pytest.mark.parametrize(
    ("factory", "args", "exc_type", "message"),
    [
        (
            errors.schema_requires_query,
            (),
            GrommetSchemaError,
            "Schema requires a query type.",
        ),
        (
            errors.unknown_type_kind,
            ("mystery",),
            GrommetTypeError,
            "Unknown type kind: mystery",
        ),
        (
            errors.type_meta_unknown_kind,
            ("ghost",),
            GrommetValueError,
            "Unknown type meta kind: ghost",
        ),
        (
            errors.list_type_requires_parameter,
            (),
            GrommetTypeError,
            "List types must be parameterized.",
        ),
        (
            errors.list_type_requires_inner,
            (),
            GrommetValueError,
            "List types require an inner type.",
        ),
        (
            errors.async_iterable_requires_parameter,
            (),
            GrommetTypeError,
            "AsyncIterator/AsyncIterable must be parameterized.",
        ),
        (
            errors.resolver_missing_annotation,
            ("resolve", "arg"),
            GrommetTypeError,
            "Resolver resolve missing annotation for 'arg'.",
        ),
        (
            errors.resolver_missing_return_annotation,
            ("resolve", "field"),
            GrommetTypeError,
            "Resolver resolve missing return annotation for field 'field'.",
        ),
        (
            errors.resolver_requires_async,
            ("resolve", "field"),
            GrommetTypeError,
            "Resolver resolve for field 'field' must be async.",
        ),
        (
            errors.subscription_requires_async_iterator,
            ("resolve", "field"),
            GrommetTypeError,
            "Subscription resolver resolve for field 'field' must return an async iterator.",
        ),
        (
            errors.union_input_not_supported,
            (),
            GrommetTypeError,
            "Union types cannot be used as input",
        ),
        (
            errors.input_type_expected,
            ("Query",),
            GrommetTypeError,
            "Query is not an input type",
        ),
        (
            errors.output_type_expected,
            ("Input",),
            GrommetTypeError,
            "Input cannot be used as output",
        ),
        (
            errors.unsupported_annotation,
            ("weird",),
            GrommetTypeError,
            "Unsupported annotation: weird",
        ),
        (
            errors.not_grommet_type,
            ("Foo",),
            GrommetTypeError,
            "Foo is not decorated with @grommet.type, @grommet.interface, or @grommet.input",
        ),
        (
            errors.not_grommet_scalar,
            ("Foo",),
            GrommetTypeError,
            "Foo is not decorated with @grommet.scalar",
        ),
        (
            errors.not_grommet_enum,
            ("Foo",),
            GrommetTypeError,
            "Foo is not decorated with @grommet.enum",
        ),
        (
            errors.not_grommet_union,
            ("Foo",),
            GrommetTypeError,
            "Foo is not a grommet union",
        ),
        (
            errors.field_default_conflict,
            (),
            GrommetTypeError,
            "field() cannot specify both default and default_factory.",
        ),
        (
            errors.dataclass_required,
            ("@grommet.type",),
            GrommetTypeError,
            "@grommet.type requires an explicit dataclass.",
        ),
        (
            errors.input_field_resolver_not_allowed,
            (),
            GrommetTypeError,
            "Input types cannot declare field resolvers.",
        ),
        (
            errors.decorator_requires_callable,
            (),
            GrommetTypeError,
            "Decorator usage expects a callable resolver.",
        ),
        (
            errors.scalar_requires_callables,
            (),
            GrommetTypeError,
            "scalar() requires serialize and parse_value callables.",
        ),
        (
            errors.enum_requires_enum_subclass,
            (),
            GrommetTypeError,
            "@grommet.enum requires an enum.Enum subclass.",
        ),
        (errors.union_requires_name, (), GrommetTypeError, "union() requires a name."),
        (
            errors.union_requires_types,
            (),
            GrommetTypeError,
            "union() requires at least one possible type.",
        ),
        (
            errors.union_requires_object_types,
            (),
            GrommetTypeError,
            "union() types must be @grommet.type object types.",
        ),
        (
            errors.invalid_enum_value,
            ("NOPE", "Color"),
            GrommetValueError,
            "Invalid enum value 'NOPE' for Color",
        ),
        (
            errors.input_mapping_expected,
            ("Input",),
            GrommetTypeError,
            "Expected mapping for input type Input",
        ),
    ],
)
def test_error_factories(factory, args, exc_type, message) -> None:
    """
    Verifies error factory helpers return expected exception types and messages.
    """
    err = factory(*args)
    assert isinstance(err, exc_type)
    assert str(err) == message
