import inspect
from typing import TYPE_CHECKING, get_origin

from ._annotations import get_annotations
from .coercion import _arg_coercer
from .context import Context
from .errors import resolver_missing_annotation, resolver_requires_async
from .metadata import TypeKind

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any

_MIN_CONTEXT_PARAMS = 2


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


def _wrap_resolver(
    resolver: "Callable[..., Any]", *, kind: TypeKind, field_name: str
) -> "Callable[..., Any]":
    """Wrap a resolver with arg coercion."""
    resolver_name = _resolver_name(resolver)
    is_subscription = kind is TypeKind.SUBSCRIPTION
    is_asyncgen = inspect.isasyncgenfunction(resolver)
    is_coroutine = inspect.iscoroutinefunction(resolver)
    if is_subscription:
        if not (is_asyncgen or is_coroutine):
            raise resolver_requires_async(resolver_name, field_name)
    elif not is_coroutine:
        raise resolver_requires_async(resolver_name, field_name)

    hints = get_annotations(resolver)
    params = _resolver_params(resolver)

    # params[0] is always self (the parent instance)
    # params[1] may be a Context[T] param — detected by type annotation
    context_param: inspect.Parameter | None = None
    if len(params) >= _MIN_CONTEXT_PARAMS:
        ann = hints.get(params[1].name, params[1].annotation)
        if ann is not inspect._empty and _is_context_annotation(ann):
            context_param = params[1]

    # GraphQL args start after self and optional context
    arg_start = _MIN_CONTEXT_PARAMS if context_param is not None else 1
    arg_params = params[arg_start:]

    arg_coercers: list[tuple[str, "Callable[[Any], Any] | None"]] = []
    for param in arg_params:
        annotation = hints.get(param.name, param.annotation)
        if annotation is inspect._empty:
            raise resolver_missing_annotation(resolver_name, param.name)
        arg_coercers.append((param.name, _arg_coercer(annotation)))

    async def wrapper(parent: "Any", context_obj: "Any", **kwargs: "Any") -> "Any":
        call_kwargs: dict[str, "Any"] = {"self": parent}
        if context_param is not None:
            call_kwargs[context_param.name] = context_obj
        for name, coercer in arg_coercers:
            if name in kwargs:
                value = kwargs[name]
                call_kwargs[name] = value if coercer is None else coercer(value)
        result = resolver(**call_kwargs)
        if is_subscription and is_asyncgen:
            return result
        return await result

    return wrapper
