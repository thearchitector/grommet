import inspect
from typing import TYPE_CHECKING

from noaio import can_syncify, syncify

from ._annotations import get_annotations
from ._compiled import CompiledArg, CompiledResolverField
from .annotations import (
    _type_spec_from_annotation,
    analyze_annotation,
    unwrap_async_iterable,
    walk_annotation,
)
from .coercion import _arg_coercer, _default_value_for_annotation
from .errors import (
    resolver_context_annotation_requires_annotated,
    resolver_missing_annotation,
    resolver_requires_async,
)
from .metadata import Context

if TYPE_CHECKING:
    from builtins import type as pytype
    from collections.abc import Callable
    from typing import Any, Literal


def _resolver_params(resolver: "Callable[..., Any]") -> list[inspect.Parameter]:
    sig = inspect.signature(resolver)
    return [
        p
        for p in sig.parameters.values()
        if p.kind
        not in (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD)
    ]


def _is_context_annotation(annotation: "Any") -> bool:
    return analyze_annotation(annotation).is_context


def _is_bare_context_annotation(annotation: "Any") -> bool:
    return annotation is Context


def _resolver_name(resolver: "Callable[..., Any]") -> str:
    return getattr(resolver, "__name__", resolver.__class__.__name__)


def _resolver_adapter(
    func: "Callable[..., Any]",
    *,
    context_param_names: tuple[str, ...],
    arg_names: tuple[str, ...],
    coercers: list[tuple[str, "Callable[[Any], Any]"]],
) -> "Callable[..., Any]":
    """Adapt a resolver to a stable runtime call shape used by Rust."""
    coercer_map = dict(coercers)

    def _adapter(parent: "Any", context: "Any", kwargs: dict[str, "Any"]) -> "Any":
        call_kwargs: dict[str, "Any"] = {}

        for name in context_param_names:
            call_kwargs[name] = context

        for name in arg_names:
            if name not in kwargs:
                continue
            value = kwargs[name]
            coercer = coercer_map.get(name)
            if coercer is not None:
                value = coercer(value)
            call_kwargs[name] = value

        return func(parent, **call_kwargs)

    _adapter.__name__ = getattr(func, "__name__", "wrapped")
    _adapter.__qualname__ = getattr(func, "__qualname__", "wrapped")
    return _adapter


def _partition_context_params(
    resolver_name: str, params: list[inspect.Parameter], hints: dict[str, "Any"]
) -> tuple[list[str], list[inspect.Parameter]]:
    context_param_names: list[str] = []
    graphql_arg_params: list[inspect.Parameter] = []

    for param in params:
        annotation = hints.get(param.name, param.annotation)
        if annotation is inspect._empty:
            raise resolver_missing_annotation(resolver_name, param.name)

        if _is_bare_context_annotation(annotation):
            raise resolver_context_annotation_requires_annotated(
                resolver_name, param.name
            )

        if _is_context_annotation(annotation):
            context_param_names.append(param.name)
            continue

        graphql_arg_params.append(param)

    return context_param_names, graphql_arg_params


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

        has_default = param.default is not inspect._empty
        default: object | None = None
        if has_default:
            default = _default_value_for_annotation(annotation, param.default)

        args.append(
            CompiledArg(
                name=param.name,
                type_spec=type_spec,
                has_default=has_default,
                default=default,
            )
        )

    return arg_names, coercers, args


def _collect_refs(
    return_ann: "Any", arg_params: list[inspect.Parameter], hints: dict[str, "Any"]
) -> "frozenset[pytype]":
    refs: list[pytype] = list(walk_annotation(return_ann))
    for param in arg_params:
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
    context_param_names, graphql_arg_params = _partition_context_params(
        resolver_name, params[1:], hints
    )

    arg_names, coercers, args = _build_arg_info(
        resolver_name, graphql_arg_params, hints
    )
    is_coroutine = inspect.iscoroutinefunction(resolver)
    is_async = kind == "subscription" or is_coroutine
    func = resolver

    if kind == "field" and is_coroutine and can_syncify(resolver):
        func = syncify(resolver)
        is_async = False

    arg_names_tuple = tuple(arg_names)
    func = _resolver_adapter(
        func,
        context_param_names=tuple(context_param_names),
        arg_names=arg_names_tuple,
        coercers=coercers,
    )

    return_ann = hints.get("return", inspect._empty)
    if return_ann is inspect._empty:
        raise resolver_missing_annotation(resolver_name, "return")

    output_ann = (
        unwrap_async_iterable(return_ann)[0] if kind == "subscription" else return_ann
    )
    type_spec = _type_spec_from_annotation(output_ann, expect_input=False)

    refs = _collect_refs(return_ann, graphql_arg_params, hints)

    return CompiledResolverField(
        kind=kind,
        name=field_name,
        func=func,
        needs_context=bool(context_param_names),
        is_async=is_async,
        type_spec=type_spec,
        description=description,
        args=tuple(args),
        refs=refs,
    )
