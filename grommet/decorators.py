import dataclasses
from typing import TYPE_CHECKING, ParamSpec, TypeVar, overload

from ._compiled import COMPILED_RESOLVER_ATTR, REFS_ATTR
from ._resolver_compiler import compile_resolver_field
from ._type_compiler import compile_type_definition
from .errors import GrommetTypeError, dataclass_required, decorator_requires_callable
from .metadata import TypeKind

P = ParamSpec("P")
R = TypeVar("R")

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any


def _compile_decorated_type(
    target: "pytype", *, kind: TypeKind, name: str | None, description: str | None
) -> "pytype":
    if not dataclasses.is_dataclass(target):
        raise dataclass_required(f"@grommet.{kind.value}")
    compile_type_definition(target, kind=kind, name=name, description=description)
    return target


@overload
def type(
    cls: "pytype", *, name: str | None = None, description: str | None = None
) -> "pytype": ...


@overload
def type(
    cls: None = None, *, name: str | None = None, description: str | None = None
) -> "Callable[[pytype], pytype]": ...


def type(
    cls: "pytype | None" = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Marks a dataclass as a GraphQL object type."""

    def wrap(target: "pytype") -> "pytype":
        return _compile_decorated_type(
            target, kind=TypeKind.OBJECT, name=name, description=description
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def input(
    cls: "pytype", *, name: str | None = None, description: str | None = None
) -> "pytype": ...


@overload
def input(
    cls: None = None, *, name: str | None = None, description: str | None = None
) -> "Callable[[pytype], pytype]": ...


def input(
    cls: "pytype | None" = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Marks a dataclass as a GraphQL input type."""

    def wrap(target: "pytype") -> "pytype":
        return _compile_decorated_type(
            target, kind=TypeKind.INPUT, name=name, description=description
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def interface(
    cls: "pytype", *, name: str | None = None, description: str | None = None
) -> "pytype": ...


@overload
def interface(
    cls: None = None, *, name: str | None = None, description: str | None = None
) -> "Callable[[pytype], pytype]": ...


def interface(
    cls: "pytype | None" = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Marks a dataclass as a GraphQL interface type."""

    def wrap(target: "pytype") -> "pytype":
        return _compile_decorated_type(
            target, kind=TypeKind.INTERFACE, name=name, description=description
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def field(
    func: "Callable[P, R]", *, description: str | None = None, name: str | None = None
) -> "Callable[P, R]": ...


@overload
def field(
    func: None = None, *, description: str | None = None, name: str | None = None
) -> "Callable[[Callable[P, R]], Callable[P, R]]": ...


def field(
    func: "Callable[..., Any] | None" = None,
    *,
    description: str | None = None,
    name: str | None = None,
) -> "Callable[..., Any]":
    """Declares a resolver-backed field on a GraphQL type."""

    def wrap(target: "Callable[..., Any]") -> "Callable[..., Any]":
        if isinstance(target, (staticmethod, classmethod)):
            raise GrommetTypeError(
                "Resolvers must be instance methods; "
                "@staticmethod and @classmethod are not supported."
            )
        if not callable(target):
            raise decorator_requires_callable()

        field_name = name or target.__name__
        compiled = compile_resolver_field(
            target, field_name=field_name, description=description, kind="field"
        )
        setattr(target, COMPILED_RESOLVER_ATTR, compiled)
        setattr(target, REFS_ATTR, compiled.refs)
        return target

    if func is None:
        return wrap
    return wrap(func)


@overload
def subscription(
    func: "Callable[P, R]", *, description: str | None = None, name: str | None = None
) -> "Callable[P, R]": ...


@overload
def subscription(
    func: None = None, *, description: str | None = None, name: str | None = None
) -> "Callable[[Callable[P, R]], Callable[P, R]]": ...


def subscription(
    func: "Callable[..., Any] | None" = None,
    *,
    description: str | None = None,
    name: str | None = None,
) -> "Callable[..., Any]":
    """Declares a subscription resolver field on a GraphQL type."""

    def wrap(target: "Callable[..., Any]") -> "Callable[..., Any]":
        if not callable(target):
            raise decorator_requires_callable()

        field_name = name or target.__name__
        compiled = compile_resolver_field(
            target, field_name=field_name, description=description, kind="subscription"
        )
        setattr(target, COMPILED_RESOLVER_ATTR, compiled)
        setattr(target, REFS_ATTR, compiled.refs)
        return target

    if func is None:
        return wrap
    return wrap(func)
