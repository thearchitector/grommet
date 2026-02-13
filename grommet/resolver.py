import inspect
from dataclasses import dataclass
from typing import TYPE_CHECKING, get_origin

from noaio import can_syncify, syncify

from ._annotations import get_annotations
from .annotations import _type_spec_from_annotation
from .coercion import _arg_coercer, _default_value_for_annotation
from .context import Context
from .errors import resolver_missing_annotation, resolver_requires_async
from .metadata import MISSING, ArgPlan, TypeKind

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any

_MIN_CONTEXT_PARAMS = 2


@dataclass(frozen=True, slots=True)
class ResolverResult:
    """Result of resolver analysis, used to populate FieldPlan inline."""

    func: "Callable[..., Any]"
    shape: str
    arg_names: list[str]
    is_async: bool
    is_async_gen: bool
    args: tuple[ArgPlan, ...] = ()


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


def _resolver_arg_info(
    resolver: "Callable[..., Any]",
) -> list[tuple[inspect.Parameter, "Any"]]:
    """Return (param, annotation) pairs for GraphQL arguments (excluding self and context)."""
    params = _resolver_params(resolver)
    hints = get_annotations(resolver)
    start = 1
    if len(params) >= _MIN_CONTEXT_PARAMS:
        ann = hints.get(params[1].name, params[1].annotation)
        if ann is not inspect._empty and _is_context_annotation(ann):
            start = _MIN_CONTEXT_PARAMS
    return [(p, hints.get(p.name, p.annotation)) for p in params[start:]]


def _resolver_name(resolver: "Callable[..., Any]") -> str:
    return getattr(resolver, "__name__", type(resolver).__name__)


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
) -> tuple[list[str], list[tuple[str, "Callable[[Any], Any]"]], list[ArgPlan]]:
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


def _analyze_resolver(
    resolver: "Callable[..., Any]", *, kind: TypeKind, field_name: str
) -> ResolverResult:
    """Analyze a resolver and return metadata for Rust-side dispatch."""
    resolver_name = _resolver_name(resolver)
    is_subscription = kind is TypeKind.SUBSCRIPTION
    is_asyncgen = inspect.isasyncgenfunction(resolver)
    is_coroutine = inspect.iscoroutinefunction(resolver)
    if is_subscription and not (is_asyncgen or is_coroutine):
        raise resolver_requires_async(resolver_name, field_name)

    hints = get_annotations(resolver)
    params = _resolver_params(resolver)

    # params[0] is always self (the parent instance)
    # params[1] may be a Context[T] param — detected by type annotation
    has_context = False
    if len(params) >= _MIN_CONTEXT_PARAMS:
        ann = hints.get(params[1].name, params[1].annotation)
        if ann is not inspect._empty and _is_context_annotation(ann):
            has_context = True

    # GraphQL args start after self and optional context
    arg_start = _MIN_CONTEXT_PARAMS if has_context else 1
    arg_names, coercers, arg_plans = _build_arg_info(
        resolver_name, params[arg_start:], hints
    )

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
    is_async = is_coroutine or is_asyncgen
    func = resolver
    if is_coroutine and not is_asyncgen and can_syncify(resolver):
        func = syncify(resolver)
        is_async = False

    # Bake arg coercion into the func so Rust only needs to vectorcall
    if coercers:
        func = _wrap_with_coercion(func, coercers)

    return ResolverResult(
        func=func,
        shape=shape,
        arg_names=arg_names,
        is_async=is_async,
        is_async_gen=is_asyncgen,
        args=tuple(arg_plans),
    )
