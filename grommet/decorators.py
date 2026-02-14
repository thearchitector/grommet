import dataclasses
import inspect
from operator import attrgetter
from typing import TYPE_CHECKING, ParamSpec, TypeVar, get_origin, overload

from noaio import can_syncify, syncify

from . import _core
from ._annotations import get_annotations
from .annotations import (
    _type_spec_from_annotation,
    analyze_annotation,
    is_hidden_field,
    unwrap_async_iterable,
    walk_annotation,
)
from .coercion import _arg_coercer, _default_value_for_annotation, _input_field_default
from .context import Context
from .errors import (
    GrommetTypeError,
    dataclass_required,
    decorator_requires_callable,
    input_field_resolver_not_allowed,
    resolver_missing_annotation,
    resolver_requires_async,
)
from .metadata import MISSING, ArgPlan, Field, TypeKind, TypeMeta

P = ParamSpec("P")
R = TypeVar("R")

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any

_META_ATTR: str = "__grommet_meta__"
_REFS_ATTR: str = "__grommet_refs__"
_FIELD_DATA_ATTR: str = "__grommet_field_data__"
_SUB_FIELD_DATA_ATTR: str = "__grommet_sub_field_data__"
_MIN_CONTEXT_PARAMS = 2


# ---------------------------------------------------------------------------
# Resolver analysis helpers
# ---------------------------------------------------------------------------


def _resolver_params(resolver: "Callable[..., Any]") -> list[inspect.Parameter]:
    sig = inspect.signature(resolver)
    return [
        p
        for p in sig.parameters.values()
        if p.kind
        not in (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD)
    ]


def _is_context_annotation(annotation: "Any") -> bool:
    return annotation is Context or get_origin(annotation) is Context


def _resolver_name(resolver: "Callable[..., Any]") -> str:
    return getattr(resolver, "__name__", resolver.__class__.__name__)


def _wrap_with_coercion(
    func: "Callable[..., Any]", coercers: list[tuple[str, "Callable[[Any], Any]"]]
) -> "Callable[..., Any]":
    """Wrap a resolver so that input-type args are coerced before dispatch."""

    def _wrapper(*args: "Any", **kwargs: "Any") -> "Any":
        for name, coercer in coercers:
            if name in kwargs:
                kwargs[name] = coercer(kwargs[name])
        return func(*args, **kwargs)

    _wrapper.__name__ = getattr(func, "__name__", "wrapped")
    _wrapper.__qualname__ = getattr(func, "__qualname__", "wrapped")
    return _wrapper


def _build_arg_info(
    resolver_name: str, params: list[inspect.Parameter], hints: dict[str, "Any"]
) -> tuple[list[str], list[tuple[str, "Callable[[Any], Any]"]], list["ArgPlan"]]:
    """Extract arg names, coercers, and arg plans from resolver parameters."""
    arg_names: list[str] = []
    coercers: list[tuple[str, "Callable[[Any], Any]"]] = []
    arg_plans: list[ArgPlan] = []
    for param in params:
        annotation = hints.get(param.name, param.annotation)
        if annotation is inspect._empty:
            raise resolver_missing_annotation(resolver_name, param.name)
        arg_names.append(param.name)
        coercer = _arg_coercer(annotation)
        if coercer is not None:
            coercers.append((param.name, coercer))
        arg_force_nullable = param.default is not inspect._empty
        arg_spec = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=arg_force_nullable
        )
        arg_default: object = MISSING
        if param.default is not inspect._empty:
            arg_default = _default_value_for_annotation(annotation, param.default)
        arg_plans.append(
            ArgPlan(name=param.name, type_spec=arg_spec, default=arg_default)
        )
    return arg_names, coercers, arg_plans


def _collect_refs(
    return_ann: "Any",
    arg_params: list[inspect.Parameter],
    hints: dict[str, "Any"],
    arg_start: int,
    arg_count: int,
) -> "list[pytype]":
    """Collect grommet types referenced in return + arg annotations."""
    ref_types: list[pytype] = list(walk_annotation(return_ann))
    for i in range(arg_count):
        param_ann = hints.get(arg_params[arg_start + i].name)
        if param_ann is not None:
            ref_types.extend(walk_annotation(param_ann))
    return ref_types


def _analyze_resolver(
    resolver: "Callable[..., Any]",
) -> tuple[
    "Callable[..., Any]",
    str,
    list[str],
    bool,
    list["ArgPlan"],
    "Any",
    "list[pytype]",
    list[inspect.Parameter],
    int,
]:
    """Analyze a resolver, returning (func, shape, arg_names, is_async, arg_plans, return_ann, refs, params, arg_start)."""
    rname = _resolver_name(resolver)
    hints = get_annotations(resolver)
    params = _resolver_params(resolver)

    # Detect context param
    has_context = False
    if len(params) >= _MIN_CONTEXT_PARAMS:
        ann = hints.get(params[1].name, params[1].annotation)
        if ann is not inspect._empty and _is_context_annotation(ann):
            has_context = True

    arg_start = _MIN_CONTEXT_PARAMS if has_context else 1
    arg_names, coercers, arg_plans = _build_arg_info(rname, params[arg_start:], hints)

    has_args = len(arg_names) > 0
    if has_context and has_args:
        shape = "self_context_and_args"
    elif has_context:
        shape = "self_and_context"
    elif has_args:
        shape = "self_and_args"
    else:
        shape = "self_only"

    # Determine async status; demote await-free coroutines to sync
    is_coroutine = inspect.iscoroutinefunction(resolver)
    is_async = is_coroutine
    func = resolver
    if is_coroutine and can_syncify(resolver):
        func = syncify(resolver)
        is_async = False

    # Bake arg coercion into the func
    if coercers:
        func = _wrap_with_coercion(func, coercers)

    # Return annotation
    return_ann = hints.get("return", inspect._empty)
    if return_ann is inspect._empty:
        raise resolver_missing_annotation(rname, "return")

    # Collect referenced types
    ref_types = _collect_refs(return_ann, params, hints, arg_start, len(arg_plans))

    return (
        func,
        shape,
        arg_names,
        is_async,
        arg_plans,
        return_ann,
        ref_types,
        params,
        arg_start,
    )


def _make_rust_field(
    field_name: str,
    func: "Callable[..., Any]",
    shape: str,
    arg_names: list[str],
    is_async: bool,
    type_spec: "Any",
    description: str | None,
    arg_plans: list[ArgPlan],
) -> _core.Field:
    """Build a fresh _core.Field Rust object from pre-analyzed data."""
    args = [
        (ap.name, ap.type_spec, None if ap.default is MISSING else ap.default)
        for ap in arg_plans
    ]
    return _core.Field(
        field_name,
        type_spec,
        func,
        shape,
        arg_names,
        is_async,
        description,
        args or None,
    )


def _make_rust_subscription_field(
    field_name: str,
    func: "Callable[..., Any]",
    shape: str,
    arg_names: list[str],
    type_spec: "Any",
    description: str | None,
    arg_plans: list[ArgPlan],
) -> _core.SubscriptionField:
    """Build a fresh _core.SubscriptionField Rust object from pre-analyzed data."""
    args = [
        (ap.name, ap.type_spec, None if ap.default is MISSING else ap.default)
        for ap in arg_plans
    ]
    return _core.SubscriptionField(
        field_name, type_spec, func, shape, arg_names, description, args or None
    )


def _analyze_and_build_field(
    resolver: "Callable[..., Any]", *, field_name: str, description: str | None
) -> "Callable[..., Any]":
    """Analyze a resolver and cache analysis results on the function."""
    (
        func,
        shape,
        arg_names,
        is_async,
        arg_plans,
        return_ann,
        ref_types,
        _params,
        _arg_start,
    ) = _analyze_resolver(resolver)
    return_spec = _type_spec_from_annotation(return_ann, expect_input=False)

    # Store pre-analyzed data directly on the original function.
    # No sentinel class — just attributes containing the analysis results.
    resolver.__grommet_field_data__ = (  # type: ignore[attr-defined]
        field_name,
        func,
        shape,
        arg_names,
        is_async,
        return_spec,
        description,
        arg_plans,
    )
    resolver.__grommet_refs__ = ref_types  # type: ignore[attr-defined]
    return resolver


def _analyze_and_build_subscription_field(
    resolver: "Callable[..., Any]", *, field_name: str, description: str | None
) -> "Callable[..., Any]":
    """Analyze a subscription resolver and cache analysis results on the function."""
    rname = _resolver_name(resolver)
    if not inspect.isasyncgenfunction(resolver):
        raise resolver_requires_async(rname, field_name)

    hints = get_annotations(resolver)
    params = _resolver_params(resolver)

    # Detect context param
    has_context = False
    if len(params) >= _MIN_CONTEXT_PARAMS:
        ann = hints.get(params[1].name, params[1].annotation)
        if ann is not inspect._empty and _is_context_annotation(ann):
            has_context = True

    arg_start = _MIN_CONTEXT_PARAMS if has_context else 1
    arg_names, coercers, arg_plans = _build_arg_info(rname, params[arg_start:], hints)

    has_args = len(arg_names) > 0
    if has_context and has_args:
        shape = "self_context_and_args"
    elif has_context:
        shape = "self_and_context"
    elif has_args:
        shape = "self_and_args"
    else:
        shape = "self_only"

    func: Callable[..., Any] = resolver
    if coercers:
        func = _wrap_with_coercion(func, coercers)

    # Build return TypeSpec — unwrap AsyncIterator[T] to get T
    return_ann = hints.get("return", inspect._empty)
    if return_ann is inspect._empty:
        raise resolver_missing_annotation(rname, "return")
    inner_type, _optional = unwrap_async_iterable(return_ann)
    return_spec = _type_spec_from_annotation(inner_type, expect_input=False)

    # Collect referenced types
    ref_types = _collect_refs(return_ann, params, hints, arg_start, len(arg_plans))

    # Store pre-analyzed data directly on the function.
    resolver.__grommet_sub_field_data__ = (  # type: ignore[attr-defined]
        field_name,
        func,
        shape,
        arg_names,
        return_spec,
        description,
        arg_plans,
    )
    resolver.__grommet_refs__ = ref_types  # type: ignore[attr-defined]
    return resolver


# ---------------------------------------------------------------------------
# Helpers for detecting decorated functions
# ---------------------------------------------------------------------------


def _has_field(obj: object) -> bool:
    return hasattr(obj, _FIELD_DATA_ATTR)


def _has_subscription_field(obj: object) -> bool:
    return hasattr(obj, _SUB_FIELD_DATA_ATTR)


def _has_any_resolver(obj: object) -> bool:
    return _has_field(obj) or _has_subscription_field(obj)


def _get_annotated_field_meta(annotation: "Any") -> Field | None:
    """Extract Field metadata from Annotated type if present."""
    info = analyze_annotation(annotation)
    for item in info.metadata:
        if isinstance(item, Field):
            return item
    return None


# ---------------------------------------------------------------------------
# Data field resolver builder (handles both root and non-root)
# ---------------------------------------------------------------------------


def _data_field_resolver(field_name: str, default: object) -> "Callable[[Any], Any]":
    """Build a resolver for a dataclass data field.

    For non-root types, self is an instance and attrgetter works.
    For root types, self is None and the default value is returned.
    """
    getter = attrgetter(field_name)
    if default is MISSING:
        return getter

    def _resolver(self: object) -> object:
        if self is None:
            return default
        return getter(self)

    return _resolver


def _resolve_data_field_default(dc_field: "dataclasses.Field[Any]") -> object:
    """Extract default for a dataclass field, or return MISSING."""
    if dc_field.default is not MISSING:
        return dc_field.default
    if dc_field.default_factory is not MISSING:
        return dc_field.default_factory()
    return MISSING


# ---------------------------------------------------------------------------
# @type and @input: build Rust Object/InputObject/Subscription at decoration time
# ---------------------------------------------------------------------------


def _set_meta(target: "pytype", meta: TypeMeta) -> None:
    setattr(target, _META_ATTR, meta)


def _build_object_type(
    cls: "pytype", *, type_name: str, description: str | None
) -> tuple[_core.Object, "frozenset[pytype]"]:
    """Build a _core.Object with all fields (data + resolver) registered."""
    all_refs: list[pytype] = []
    fields: list[_core.Field] = []
    hints = get_annotations(cls)

    # Data fields from dataclass
    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue
        force_nullable = dc_field.default is None
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=False, force_nullable=force_nullable
        )
        annotated_field = _get_annotated_field_meta(annotation)
        desc = annotated_field.description if annotated_field else None

        default = _resolve_data_field_default(dc_field)
        func = _data_field_resolver(dc_field.name, default)
        fields.append(
            _core.Field(dc_field.name, type_spec, func, "self_only", [], False, desc)
        )
        all_refs.extend(walk_annotation(annotation))

    # Resolver fields from @field-decorated methods
    for _attr_name, attr_value in vars(cls).items():
        if _has_field(attr_value):
            fields.append(_make_rust_field(*attr_value.__grommet_field_data__))
            all_refs.extend(attr_value.__grommet_refs__)

    obj = _core.Object(type_name, description, fields or None)
    return obj, frozenset(all_refs)


def _build_subscription_type(
    cls: "pytype", *, type_name: str, description: str | None
) -> tuple[_core.Subscription, "frozenset[pytype]"]:
    """Build a _core.Subscription with all subscription fields registered."""
    all_refs: list[pytype] = []
    fields: list[_core.SubscriptionField] = []

    for _attr_name, attr_value in vars(cls).items():
        if _has_subscription_field(attr_value):
            fields.append(
                _make_rust_subscription_field(*attr_value.__grommet_sub_field_data__)
            )
            all_refs.extend(attr_value.__grommet_refs__)

    sub = _core.Subscription(type_name, description, fields or None)
    return sub, frozenset(all_refs)


def _build_input_type(
    cls: "pytype", *, type_name: str, description: str | None
) -> tuple[_core.InputObject, "frozenset[pytype]"]:
    """Build a _core.InputObject with all input fields registered."""
    all_refs: list[pytype] = []
    fields: list[_core.InputValue] = []
    hints = get_annotations(cls)

    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if is_hidden_field(dc_field.name, annotation):
            continue
        force_nullable = (
            dc_field.default is not MISSING or dc_field.default_factory is not MISSING
        )
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=force_nullable
        )
        annotated_field = _get_annotated_field_meta(annotation)
        desc = annotated_field.description if annotated_field else None
        default_value = _input_field_default(dc_field, annotation)
        dv = None if default_value is MISSING else default_value
        fields.append(_core.InputValue(dc_field.name, type_spec, dv, desc))
        all_refs.extend(walk_annotation(annotation))

    inp = _core.InputObject(type_name, description, fields or None)
    return inp, frozenset(all_refs)


def _wrap_type_decorator(
    target: "pytype", *, kind: TypeKind, name: str | None, description: str | None
) -> "pytype":
    """Shared implementation for @type and @input decorators.

    Validates the decorated class and stores metadata + refs. Rust types are
    built later by plan.py from the stored metadata (async-graphql types cannot
    be cloned, so fresh objects are constructed per schema).
    """
    if not dataclasses.is_dataclass(target):
        raise dataclass_required(f"@grommet.{kind.value}")

    type_name = name or target.__name__
    resolved_kind = kind

    if kind is TypeKind.INPUT:
        if any(_has_any_resolver(v) for v in vars(target).values()):
            raise input_field_resolver_not_allowed()
        # Validate eagerly; result is discarded
        _build_input_type(target, type_name=type_name, description=description)

    else:
        has_sub_fields = any(_has_subscription_field(v) for v in vars(target).values())
        has_regular_fields = any(_has_field(v) for v in vars(target).values())

        if has_sub_fields and has_regular_fields:
            raise GrommetTypeError(
                "A type cannot mix @field and @subscription decorators."
            )

        if has_sub_fields:
            resolved_kind = TypeKind.SUBSCRIPTION
            _build_subscription_type(
                target, type_name=type_name, description=description
            )
        else:
            _build_object_type(target, type_name=type_name, description=description)

    meta = TypeMeta(kind=resolved_kind, name=type_name, description=description)
    _set_meta(target, meta)
    setattr(target, _REFS_ATTR, _collect_class_refs(target))
    return target


def _collect_class_refs(cls: "pytype") -> "frozenset[pytype]":
    """Collect all referenced types from a decorated class's data + resolver fields."""
    all_refs: list[pytype] = []
    hints = get_annotations(cls)

    for dc_field in dataclasses.fields(cls):
        annotation = hints.get(dc_field.name, dc_field.type)
        if not is_hidden_field(dc_field.name, annotation):
            all_refs.extend(walk_annotation(annotation))

    for _attr_name, attr_value in vars(cls).items():
        if hasattr(attr_value, _REFS_ATTR):
            all_refs.extend(getattr(attr_value, _REFS_ATTR))

    return frozenset(all_refs)


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
        return _wrap_type_decorator(
            target, kind=TypeKind.INPUT, name=name, description=description
        )

    if cls is None:
        return wrap
    return wrap(cls)


# ---------------------------------------------------------------------------
# @field decorator
# ---------------------------------------------------------------------------


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
        return _analyze_and_build_field(
            target, field_name=field_name, description=description
        )

    if func is None:
        return wrap
    return wrap(func)


# ---------------------------------------------------------------------------
# @subscription decorator
# ---------------------------------------------------------------------------


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
        return _analyze_and_build_subscription_field(
            target, field_name=field_name, description=description
        )

    if func is None:
        return wrap
    return wrap(func)
