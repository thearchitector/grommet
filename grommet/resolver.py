import dataclasses
import inspect
from functools import lru_cache
from typing import TYPE_CHECKING

from .coercion import _arg_coercer, _default_value_for_annotation
from .errors import (
    resolver_missing_annotation,
    resolver_requires_async,
    subscription_requires_async_iterator,
)
from .info import Info
from .typespec import _type_spec_from_annotation
from .typing_utils import _get_type_hints

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any


@dataclasses.dataclass(frozen=True, slots=True)
class ResolverSpec:
    """Precomputed resolver metadata."""

    wrapper: "Callable[..., Any]"
    arg_defs: tuple[dict[str, "Any"], ...]
    is_subscription: bool
    is_asyncgen: bool


_RESOLVER_CACHE: dict[tuple["Callable[..., Any]", str], ResolverSpec] = {}


_RESERVED_PARAM_NAMES = {"parent", "root", "self", "info", "context"}


@lru_cache(maxsize=512)
def _resolver_signature(resolver: "Callable[..., Any]") -> inspect.Signature:
    return inspect.signature(resolver)


def _resolver_params(resolver: "Callable[..., Any]") -> list[inspect.Parameter]:
    try:
        sig = _resolver_signature(resolver)
    except TypeError:
        sig = inspect.signature(resolver)
    return [
        p
        for p in sig.parameters.values()
        if p.kind
        not in (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD)
    ]


def _resolver_arg_params(resolver: "Callable[..., Any]") -> list[inspect.Parameter]:
    return [
        p for p in _resolver_params(resolver) if p.name not in _RESERVED_PARAM_NAMES
    ]


def _find_param(
    params: list[inspect.Parameter], names: set[str]
) -> inspect.Parameter | None:
    for param in params:
        if param.name in names:
            return param
    return None


def _normalize_info(info: "Any") -> Info:
    if isinstance(info, Info):
        return info
    if isinstance(info, dict):
        return Info(
            field_name=str(info.get("field_name", "")),
            context=info.get("context"),
            root=info.get("root"),
        )
    field_name = getattr(info, "field_name", "")
    context = getattr(info, "context", None)
    root = getattr(info, "root", None)
    return Info(field_name=str(field_name), context=context, root=root)


def _resolver_arg_annotations(resolver: "Callable[..., Any]") -> dict[str, "Any"]:
    hints = _get_type_hints(resolver)
    arg_params = _resolver_arg_params(resolver)
    return {p.name: hints.get(p.name, p.annotation) for p in arg_params}


def _resolver_name(resolver: "Callable[..., Any]") -> str:
    name = getattr(resolver, "__name__", None)
    if name:
        return str(name)
    func = getattr(resolver, "func", None)
    if func is not None:
        name = getattr(func, "__name__", None)
        if name:
            return str(name)
    return type(resolver).__name__


def _is_coroutine_callable(resolver: "Callable[..., Any]") -> bool:
    if inspect.iscoroutinefunction(resolver):
        return True
    func = getattr(resolver, "func", None)
    return func is not None and inspect.iscoroutinefunction(func)


def _is_asyncgen_callable(resolver: "Callable[..., Any]") -> bool:
    if inspect.isasyncgenfunction(resolver):
        return True
    func = getattr(resolver, "func", None)
    return func is not None and inspect.isasyncgenfunction(func)


def _is_async_iterator(value: "Any") -> bool:
    return hasattr(value, "__aiter__") or hasattr(value, "__anext__")


def _wrap_resolver(
    resolver: "Callable[..., Any]", *, kind: str, field_name: str
) -> "tuple[Callable[..., Any], list[dict[str, Any]]]":
    """Wrap a resolver with coercion and return (wrapper, arg_defs)."""
    cache_key = (resolver, kind)
    if cache_key in _RESOLVER_CACHE:
        spec = _RESOLVER_CACHE[cache_key]
        return spec.wrapper, list(spec.arg_defs)

    spec = _build_resolver_spec(resolver, kind=kind, field_name=field_name)
    _RESOLVER_CACHE[cache_key] = spec
    return spec.wrapper, list(spec.arg_defs)


def _build_resolver_spec(
    resolver: "Callable[..., Any]", *, kind: str, field_name: str
) -> ResolverSpec:
    """Build a ResolverSpec with precomputed metadata."""
    resolver_name = _resolver_name(resolver)
    is_subscription = kind == "subscription"
    is_asyncgen = _is_asyncgen_callable(resolver)
    is_coroutine = _is_coroutine_callable(resolver)
    if is_subscription:
        if not (is_asyncgen or is_coroutine):
            raise resolver_requires_async(resolver_name, field_name)
    elif not is_coroutine:
        raise resolver_requires_async(resolver_name, field_name)

    hints = _get_type_hints(resolver)
    params = _resolver_params(resolver)
    parent_param = _find_param(params, {"parent", "self"})
    root_param = _find_param(params, {"root"})
    info_param = _find_param(params, {"info"})
    context_param = _find_param(params, {"context"})
    arg_params = [p for p in params if p.name not in _RESERVED_PARAM_NAMES]

    arg_defs: list[dict[str, "Any"]] = []
    arg_coercers: list[tuple[str, "Callable[[Any], Any] | None"]] = []
    for param in arg_params:
        annotation = hints.get(param.name, param.annotation)
        if annotation is inspect._empty:
            raise resolver_missing_annotation(resolver_name, param.name)
        force_nullable = param.default is not inspect._empty
        arg_spec = _type_spec_from_annotation(
            annotation, expect_input=True, force_nullable=force_nullable
        )
        arg_def: dict[str, "Any"] = {"name": param.name, "type": arg_spec.to_graphql()}
        if param.default is not inspect._empty:
            arg_def["default"] = _default_value_for_annotation(
                annotation, param.default
            )
        arg_defs.append(arg_def)
        arg_coercers.append((param.name, _arg_coercer(annotation)))

    async def wrapper(parent: "Any", info: "Any", **kwargs: "Any") -> "Any":
        call_kwargs: dict[str, "Any"] = {}
        info_obj = None
        if (
            info_param is not None
            or context_param is not None
            or root_param is not None
        ):
            info_obj = _normalize_info(info)
        if parent_param is not None:
            call_kwargs[parent_param.name] = parent
        if info_param is not None:
            call_kwargs[info_param.name] = info_obj or _normalize_info(info)
        if context_param is not None:
            call_kwargs[context_param.name] = info_obj.context if info_obj else None
        if root_param is not None:
            call_kwargs[root_param.name] = info_obj.root if info_obj else None
        for name, coercer in arg_coercers:
            if name in kwargs:
                value = kwargs[name]
                call_kwargs[name] = value if coercer is None else coercer(value)
        result = resolver(**call_kwargs)
        if is_subscription:
            if is_asyncgen:
                return result
            awaited = await result
            if not _is_async_iterator(awaited):
                raise subscription_requires_async_iterator(resolver_name, field_name)
            return awaited
        return await result

    return ResolverSpec(
        wrapper=wrapper,
        arg_defs=tuple(arg_defs),
        is_subscription=is_subscription,
        is_asyncgen=is_asyncgen,
    )
