import dataclasses
from typing import TYPE_CHECKING, ParamSpec, TypeVar, overload

from .errors import (
    GrommetTypeError,
    dataclass_required,
    decorator_requires_callable,
    input_field_resolver_not_allowed,
)
from .metadata import TypeKind, TypeMeta

P = ParamSpec("P")
R = TypeVar("R")

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any


_META_ATTR: str = "__grommet_meta__"


class _FieldResolver:
    __slots__ = ("resolver", "description", "name")

    def __init__(
        self,
        resolver: "Callable[..., Any]",
        *,
        description: str | None,
        name: str | None,
    ) -> None:
        self.resolver = resolver
        self.description = description
        self.name = name


def _set_meta(target: "pytype", meta: TypeMeta) -> None:
    setattr(target, _META_ATTR, meta)


def _wrap_type_decorator(
    target: "pytype",
    *,
    kind: TypeKind,
    name: str | None,
    description: str | None,
    allow_resolvers: bool,
) -> "pytype":
    """Shared implementation for @type and @input decorators."""
    if not allow_resolvers and any(
        isinstance(value, _FieldResolver) for value in vars(target).values()
    ):
        raise input_field_resolver_not_allowed()
    if not dataclasses.is_dataclass(target):
        raise dataclass_required(f"@grommet.{kind.value}")
    meta = TypeMeta(kind=kind, name=name or target.__name__, description=description)
    _set_meta(target, meta)
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
        return _wrap_type_decorator(
            target,
            kind=TypeKind.OBJECT,
            name=name,
            description=description,
            allow_resolvers=True,
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
        return _wrap_type_decorator(
            target,
            kind=TypeKind.INPUT,
            name=name,
            description=description,
            allow_resolvers=False,
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def field(
    func: "Callable[P, R]", *, description: str | None = None, name: str | None = None
) -> "_FieldResolver": ...


@overload
def field(
    func: None = None, *, description: str | None = None, name: str | None = None
) -> "Callable[[Callable[P, R]], _FieldResolver]": ...


def field(
    func: "Callable[..., Any] | None" = None,
    *,
    description: str | None = None,
    name: str | None = None,
) -> "_FieldResolver | Callable[[Callable[..., Any]], _FieldResolver]":
    """Declares a resolver-backed field on a GraphQL type."""

    def wrap(target: "Callable[..., Any]") -> _FieldResolver:
        func = target
        if isinstance(func, (staticmethod, classmethod)):
            raise GrommetTypeError(
                "Resolvers must be instance methods; "
                "@staticmethod and @classmethod are not supported."
            )
        if not callable(func):
            raise decorator_requires_callable()
        return _FieldResolver(func, description=description, name=name)

    if func is None:
        return wrap
    return wrap(func)
