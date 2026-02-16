"""Targeted branch coverage tests for grommet.errors."""

import pytest

from grommet.errors import (
    GrommetError,
    GrommetTypeError,
    async_iterable_requires_parameter,
    dataclass_required,
    decorator_requires_callable,
    input_field_resolver_not_allowed,
    input_mapping_expected,
    input_type_expected,
    list_type_requires_parameter,
    not_grommet_type,
    output_type_expected,
    resolver_context_annotation_requires_annotated,
    resolver_missing_annotation,
    resolver_requires_async,
    union_definition_conflict,
    union_input_not_supported,
    union_member_must_be_object,
    unsupported_annotation,
)


@pytest.mark.parametrize(
    ("factory", "expected_message"),
    [
        (list_type_requires_parameter, "List types must be parameterized."),
        (
            async_iterable_requires_parameter,
            "AsyncIterator/AsyncIterable must be parameterized.",
        ),
        (
            lambda: resolver_missing_annotation("resolver", "value"),
            "missing annotation",
        ),
        (
            lambda: resolver_context_annotation_requires_annotated(
                "resolver", "context"
            ),
            "must use Annotated[T, grommet.Context]",
        ),
        (
            lambda: resolver_requires_async("resolver", "field"),
            "Resolver resolver for field 'field' must be async.",
        ),
        (lambda: input_type_expected("Thing"), "Thing is not an input type"),
        (lambda: output_type_expected("Thing"), "Thing cannot be used as output"),
        (lambda: unsupported_annotation("bad"), "Unsupported annotation: bad"),
        (
            lambda: not_grommet_type("Thing"),
            "Thing is not decorated with @grommet.type",
        ),
        (
            lambda: dataclass_required("@grommet.type"),
            "@grommet.type requires an explicit dataclass.",
        ),
        (
            input_field_resolver_not_allowed,
            "Input types cannot declare field resolvers.",
        ),
        (decorator_requires_callable, "Decorator usage expects a callable resolver."),
        (
            lambda: input_mapping_expected("Input"),
            "Expected mapping for input type Input",
        ),
        (
            union_input_not_supported,
            "Union types are not supported in input annotations.",
        ),
        (
            lambda: union_member_must_be_object("X"),
            "Union member 'X' must be a type decorated with @grommet.type.",
        ),
        (
            lambda: union_definition_conflict("Named"),
            "Union 'Named' has conflicting definitions across the schema graph.",
        ),
    ],
)
def test_error_factories_emit_expected_type_and_message(factory, expected_message: str):
    """Validates all error helpers return GrommetTypeError instances with stable messages."""
    err = factory()
    assert isinstance(err, GrommetTypeError)
    assert expected_message in str(err)


def test_grommet_type_error_inherits_from_base_error_and_type_error():
    """Confirms the custom error hierarchy matches the public contract."""
    err = GrommetTypeError("boom")
    assert isinstance(err, GrommetError)
    assert isinstance(err, TypeError)
