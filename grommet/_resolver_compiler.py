import inspect
from typing import TYPE_CHECKING, get_origin

from noaio import can_syncify, syncify

from ._annotations import get_annotations
from ._compiled import CompiledArg, CompiledResolverField
from .annotations import (
    _type_spec_from_annotation,
    unwrap_async_iterable,
    walk_annotation,
)
from .coercion import _arg_coercer, _default_value_for_annotation
from .context import Context
from .errors import resolver_missing_annotation, resolver_requires_async
from .metadata import MISSING

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any, Literal

_MIN_CONTEXT_PARAMS = 2
_SHAPE_BY_RESOLVER_SIGNATURE = {
    (False, False): "self_only",
    (False, True): "self_and_args",
    (True, False): "self_and_context",
    (True, True): "self_context_and_args",
}


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
) -> tuple[list[str], list[tuple[str, "Callable[[Any], Any]"]], list[CompiledArg]]:
    arg_names: list[str] = []
    coercers: list[tuple[str, "Callable[[Any], Any]"]] = []
    args: list[CompiledArg] = []

    for param in params:
        annotation = hints.get(param.name, param.annotation)
        if annotation is inspect._empty:
            raise resolver_missing_annotation(resolver_name, param.name)

        arg_names.append(param.name)

        coercer = _arg_coercer(annotation)
        if coercer is not None:
            coercers.append((param.name, coercer))

        force_nullable = param.default is not inspect._empty
        type_spec = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=force_nullable
        )

        default: object = MISSING
        if param.default is not inspect._empty:
            default = _default_value_for_annotation(annotation, param.default)

        args.append(CompiledArg(name=param.name, type_spec=type_spec, default=default))

    return arg_names, coercers, args


def _collect_refs(
    return_ann: "Any",
    arg_params: list[inspect.Parameter],
    hints: dict[str, "Any"],
    arg_start: int,
    arg_count: int,
) -> "frozenset[pytype]":
    refs: list[pytype] = list(walk_annotation(return_ann))
    for i in range(arg_count):
        param = arg_params[arg_start + i]
        param_ann = hints.get(param.name, param.annotation)
        if param_ann is not inspect._empty:
            refs.extend(walk_annotation(param_ann))
    return frozenset(refs)


def compile_resolver_field(
    resolver: "Callable[..., Any]",
    *,
    field_name: str,
    description: str | None,
    kind: "Literal['field', 'subscription']",
) -> CompiledResolverField:
    """Compile a resolver into an immutable blueprint used for schema registration."""
    resolver_name = _resolver_name(resolver)

    if kind == "subscription" and not inspect.isasyncgenfunction(resolver):
        raise resolver_requires_async(resolver_name, field_name)

    hints = get_annotations(resolver)
    params = _resolver_params(resolver)
    context_ann = (
        hints.get(params[1].name, params[1].annotation)
        if len(params) >= _MIN_CONTEXT_PARAMS
        else inspect._empty
    )
    has_context = context_ann is not inspect._empty and _is_context_annotation(
        context_ann
    )

    arg_start = _MIN_CONTEXT_PARAMS if has_context else 1
    arg_names, coercers, args = _build_arg_info(
        resolver_name, params[arg_start:], hints
    )
    shape = _SHAPE_BY_RESOLVER_SIGNATURE[(has_context, bool(arg_names))]

    is_coroutine = inspect.iscoroutinefunction(resolver)
    is_async = kind == "subscription" or is_coroutine
    func = resolver

    if kind == "field" and is_coroutine and can_syncify(resolver):
        func = syncify(resolver)
        is_async = False

    if coercers:
        func = _wrap_with_coercion(func, coercers)

    return_ann = hints.get("return", inspect._empty)
    if return_ann is inspect._empty:
        raise resolver_missing_annotation(resolver_name, "return")

    output_ann = (
        unwrap_async_iterable(return_ann)[0] if kind == "subscription" else return_ann
    )
    type_spec = _type_spec_from_annotation(output_ann, expect_input=False)

    refs = _collect_refs(return_ann, params, hints, arg_start, len(args))

    return CompiledResolverField(
        kind=kind,
        name=field_name,
        func=func,
        shape=shape,
        arg_names=tuple(arg_names),
        is_async=is_async,
        type_spec=type_spec,
        description=description,
        args=tuple(args),
        refs=refs,
    )
