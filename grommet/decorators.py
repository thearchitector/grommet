import dataclasses
import enum
import inspect
from builtins import type as pytype
from typing import TYPE_CHECKING, ParamSpec, TypeVar, overload

from .errors import (
    GrommetTypeError,
    dataclass_required,
    decorator_requires_callable,
    enum_requires_enum_subclass,
    field_default_conflict,
    input_field_resolver_not_allowed,
    resolver_missing_return_annotation,
    scalar_requires_callables,
    union_requires_name,
    union_requires_object_types,
    union_requires_types,
)
from .metadata import (
    MISSING,
    EnumMeta,
    FieldMeta,
    GrommetMetaType,
    ScalarMeta,
    TypeKind,
    TypeMeta,
    UnionMeta,
    _register_enum,
    _register_scalar,
    _register_type,
    _register_union,
)

P = ParamSpec("P")
R = TypeVar("R")

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable
    from typing import Any


class _FieldResolver:
    __slots__ = (
        "resolver",
        "description",
        "deprecation_reason",
        "name",
        "default",
        "default_factory",
        "init",
    )

    def __init__(
        self,
        resolver: "Callable[..., Any]",
        *,
        description: str | None,
        deprecation_reason: str | None,
        name: str | None,
        default: "Any",
        default_factory: "Callable[[], Any] | Any",
        init: bool | None,
    ) -> None:
        self.resolver = resolver
        self.description = description
        self.deprecation_reason = deprecation_reason
        self.name = name
        self.default = default
        self.default_factory = default_factory
        self.init = init


def _field_from_resolver(
    resolver: "Callable[..., Any]",
    *,
    description: str | None,
    deprecation_reason: str | None,
    name: str | None,
    default: "Any",
    default_factory: "Callable[[], Any] | Any",
    init: bool | None,
) -> "dataclasses.Field[Any]":
    if init is None:
        init = False
    meta = FieldMeta(
        resolver=resolver,
        description=description,
        deprecation_reason=deprecation_reason,
        name=name,
    )
    if default is not MISSING and default_factory is not MISSING:
        raise field_default_conflict()
    metadata = {"grommet": meta}
    field_def: "dataclasses.Field[Any]"
    if default is not MISSING:
        field_def = dataclasses.field(default=default, metadata=metadata, init=init)
    elif default_factory is not MISSING:
        field_def = dataclasses.field(
            default_factory=default_factory, metadata=metadata, init=init
        )
    else:
        field_def = dataclasses.field(metadata=metadata, init=init)
    return field_def


def _set_grommet_attr(target: pytype, name: str, value: "Any") -> None:
    setattr(target, name, value)


def _apply_field_resolvers(target: pytype) -> tuple[pytype, bool]:
    pending = {
        attr_name: value
        for attr_name, value in vars(target).items()
        if isinstance(value, _FieldResolver)
    }
    if not pending:
        return target, False

    annotations = getattr(target, "__annotations__", {})
    if not isinstance(annotations, dict):
        annotations = dict(annotations)

    for field_name, marker in pending.items():
        if field_name not in annotations:
            return_annotation = marker.resolver.__annotations__.get(
                "return", inspect._empty
            )
            if return_annotation is inspect._empty:
                raise resolver_missing_return_annotation(
                    marker.resolver.__name__, field_name
                )
            annotations[field_name] = return_annotation

        field_def = _field_from_resolver(
            marker.resolver,
            description=marker.description,
            deprecation_reason=marker.deprecation_reason,
            name=marker.name,
            default=marker.default,
            default_factory=marker.default_factory,
            init=marker.init,
        )
        setattr(target, field_name, field_def)

    target.__annotations__ = annotations
    return target, True


def _rebuild_dataclass(target: pytype) -> pytype:
    """Re-apply dataclasses.dataclass preserving existing params."""
    params = target.__dataclass_params__  # type: ignore[attr-defined]
    return dataclasses.dataclass(
        target,
        init=params.init,
        repr=params.repr,
        eq=params.eq,
        order=params.order,
        unsafe_hash=params.unsafe_hash,
        frozen=params.frozen,
        match_args=params.match_args,
        kw_only=params.kw_only,
        slots=params.slots,
        weakref_slot=params.weakref_slot,
    )


def _wrap_type_decorator(
    target: pytype,
    *,
    kind: TypeKind,
    name: str | None,
    description: str | None,
    implements: "tuple[pytype, ...]",
    allow_resolvers: bool,
) -> pytype:
    """Shared implementation for @type, @interface, and @input decorators."""
    if not allow_resolvers and any(
        isinstance(value, _FieldResolver) for value in vars(target).values()
    ):
        raise input_field_resolver_not_allowed()
    if not dataclasses.is_dataclass(target):
        raise dataclass_required(f"@grommet.{kind.value}")
    if allow_resolvers:
        target, applied = _apply_field_resolvers(target)
        if applied:
            target = _rebuild_dataclass(target)
    meta = TypeMeta(
        kind=kind,
        name=name or target.__name__,
        description=description,
        implements=implements,
    )
    _set_grommet_attr(target, "__grommet_meta__", meta)
    _register_type(target, meta)
    return target


@overload
def type(
    cls: pytype,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> pytype: ...


@overload
def type(
    cls: None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> "Callable[[pytype], pytype]": ...


def type(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Marks a dataclass as a GraphQL object type."""

    def wrap(target: pytype) -> pytype:
        return _wrap_type_decorator(
            target,
            kind=TypeKind.OBJECT,
            name=name,
            description=description,
            implements=tuple(implements or ()),
            allow_resolvers=True,
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def input(
    cls: pytype, *, name: str | None = None, description: str | None = None
) -> pytype: ...


@overload
def input(
    cls: None = None, *, name: str | None = None, description: str | None = None
) -> "Callable[[pytype], pytype]": ...


def input(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Marks a dataclass as a GraphQL input type."""

    def wrap(target: pytype) -> pytype:
        return _wrap_type_decorator(
            target,
            kind=TypeKind.INPUT,
            name=name,
            description=description,
            implements=(),
            allow_resolvers=False,
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def interface(
    cls: pytype,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> pytype: ...


@overload
def interface(
    cls: None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> "Callable[[pytype], pytype]": ...


def interface(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Marks a dataclass as a GraphQL interface type."""

    def wrap(target: pytype) -> pytype:
        return _wrap_type_decorator(
            target,
            kind=TypeKind.INTERFACE,
            name=name,
            description=description,
            implements=tuple(implements or ()),
            allow_resolvers=True,
        )

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def field(
    func: "Callable[P, R]",
    *,
    description: str | None = None,
    deprecation_reason: str | None = None,
    name: str | None = None,
    default: "Any" = MISSING,
    default_factory: "Callable[[], Any] | Any" = MISSING,
    init: bool | None = None,
) -> "_FieldResolver": ...


@overload
def field(
    func: None = None,
    *,
    description: str | None = None,
    deprecation_reason: str | None = None,
    name: str | None = None,
    default: "Any" = MISSING,
    default_factory: "Callable[[], Any] | Any" = MISSING,
    init: bool | None = None,
) -> "Callable[[Callable[P, R]], _FieldResolver]": ...


def field(
    func: "Callable[..., Any] | None" = None,
    *,
    description: str | None = None,
    deprecation_reason: str | None = None,
    name: str | None = None,
    default: "Any" = MISSING,
    default_factory: "Callable[[], Any] | Any" = MISSING,
    init: bool | None = None,
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
        return _FieldResolver(
            func,
            description=description,
            deprecation_reason=deprecation_reason,
            name=name,
            default=default,
            default_factory=default_factory,
            init=init,
        )

    if func is None:
        return wrap
    return wrap(func)


@overload
def scalar(
    cls: pytype,
    *,
    name: str | None = None,
    description: str | None = None,
    specified_by_url: str | None = None,
    serialize: "Callable[[Any], Any] | None" = None,
    parse_value: "Callable[[Any], Any] | None" = None,
) -> pytype: ...


@overload
def scalar(
    cls: None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    specified_by_url: str | None = None,
    serialize: "Callable[[Any], Any] | None" = None,
    parse_value: "Callable[[Any], Any] | None" = None,
) -> "Callable[[pytype], pytype]": ...


def scalar(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    specified_by_url: str | None = None,
    serialize: "Callable[[Any], Any] | None" = None,
    parse_value: "Callable[[Any], Any] | None" = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Registers a dataclass as a GraphQL scalar type."""

    if serialize is None or parse_value is None:
        raise scalar_requires_callables()

    def wrap(target: pytype) -> pytype:
        meta = ScalarMeta(
            name=name or target.__name__,
            serialize=serialize,
            parse_value=parse_value,
            description=description,
            specified_by_url=specified_by_url,
        )
        _set_grommet_attr(target, "__grommet_meta__", meta)
        _register_scalar(target)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


@overload
def enum_type(
    cls: pytype, *, name: str | None = None, description: str | None = None
) -> pytype: ...


@overload
def enum_type(
    cls: None = None, *, name: str | None = None, description: str | None = None
) -> "Callable[[pytype], pytype]": ...


def enum_type(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    """Registers an enum.Enum subclass as a GraphQL enum."""

    def wrap(target: pytype) -> pytype:
        if not issubclass(target, enum.Enum):
            raise enum_requires_enum_subclass()
        meta = EnumMeta(name=name or target.__name__, description=description)
        _set_grommet_attr(target, "__grommet_meta__", meta)
        _register_enum(target)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


def union(
    name: str, *, types: "Iterable[pytype]", description: str | None = None
) -> pytype:
    """Creates a GraphQL union type."""

    if not name:
        raise union_requires_name()
    type_list = tuple(types)
    if not type_list:
        raise union_requires_types()
    for tp in type_list:
        meta = getattr(tp, "__grommet_meta__", None)
        if meta is None or meta.type is not GrommetMetaType.TYPE:
            raise union_requires_object_types()
    meta = UnionMeta(name=name, types=type_list, description=description)
    target = pytype(name, (), {})
    _set_grommet_attr(target, "__grommet_meta__", meta)
    _register_union(target)
    return target
