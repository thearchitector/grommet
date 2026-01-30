import dataclasses
import enum
import inspect
from builtins import type as pytype
from typing import TYPE_CHECKING

from .metadata import MISSING, EnumMeta, FieldMeta, ScalarMeta, TypeMeta, UnionMeta

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
        raise TypeError("field() cannot specify both default and default_factory.")
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
                raise TypeError(
                    f"Resolver {marker.resolver.__name__} missing return annotation "
                    f"for field '{field_name}'."
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


def type(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> "Callable[[pytype], pytype] | pytype":
    def wrap(target: pytype) -> pytype:
        target, applied = _apply_field_resolvers(target)
        if not dataclasses.is_dataclass(target):
            target = dataclasses.dataclass(target)
        elif applied:
            params = target.__dataclass_params__  # type: ignore[attr-defined]
            target = dataclasses.dataclass(
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
        meta = TypeMeta(
            kind="object",
            name=name or target.__name__,
            description=description,
            implements=tuple(implements or ()),
        )
        setattr(target, "__grommet__", meta)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


def input(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    def wrap(target: pytype) -> pytype:
        if any(isinstance(value, _FieldResolver) for value in vars(target).values()):
            raise TypeError("Input types cannot declare field resolvers.")
        if not dataclasses.is_dataclass(target):
            target = dataclasses.dataclass(target)
        meta = TypeMeta(
            kind="input", name=name or target.__name__, description=description
        )
        setattr(target, "__grommet__", meta)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


def interface(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    implements: "Iterable[pytype] | None" = None,
) -> "Callable[[pytype], pytype] | pytype":
    def wrap(target: pytype) -> pytype:
        target, applied = _apply_field_resolvers(target)
        if not dataclasses.is_dataclass(target):
            target = dataclasses.dataclass(target)
        elif applied:
            params = target.__dataclass_params__  # type: ignore[attr-defined]
            target = dataclasses.dataclass(
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
        meta = TypeMeta(
            kind="interface",
            name=name or target.__name__,
            description=description,
            implements=tuple(implements or ()),
        )
        setattr(target, "__grommet__", meta)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


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
    def wrap(target: "Callable[..., Any]") -> _FieldResolver:
        func = target
        if isinstance(func, staticmethod | classmethod):
            func = func.__func__
        if not callable(func):
            raise TypeError("Decorator usage expects a callable resolver.")
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


def scalar(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
    specified_by_url: str | None = None,
    serialize: "Callable[[Any], Any] | None" = None,
    parse_value: "Callable[[Any], Any] | None" = None,
) -> "Callable[[pytype], pytype] | pytype":
    if serialize is None or parse_value is None:
        raise TypeError("scalar() requires serialize and parse_value callables.")

    def wrap(target: pytype) -> pytype:
        meta = ScalarMeta(
            name=name or target.__name__,
            serialize=serialize,
            parse_value=parse_value,
            description=description,
            specified_by_url=specified_by_url,
        )
        setattr(target, "__grommet_scalar__", meta)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


def enum_type(
    cls: pytype | None = None,
    *,
    name: str | None = None,
    description: str | None = None,
) -> "Callable[[pytype], pytype] | pytype":
    def wrap(target: pytype) -> pytype:
        if not issubclass(target, enum.Enum):
            raise TypeError("@grommet.enum requires an enum.Enum subclass.")
        meta = EnumMeta(name=name or target.__name__, description=description)
        setattr(target, "__grommet_enum__", meta)
        return target

    if cls is None:
        return wrap
    return wrap(cls)


def union(
    name: str,
    *,
    types: "Iterable[pytype]",
    description: str | None = None,
) -> pytype:
    if not name:
        raise TypeError("union() requires a name.")
    type_list = tuple(types)
    if not type_list:
        raise TypeError("union() requires at least one possible type.")
    for tp in type_list:
        meta = getattr(tp, "__grommet__", None)
        if meta is None or meta.kind != "object":
            raise TypeError("union() types must be @grommet.type object types.")
    meta = UnionMeta(name=name, types=type_list, description=description)
    target = pytype(name, (), {})
    setattr(target, "__grommet_union__", meta)
    return target
