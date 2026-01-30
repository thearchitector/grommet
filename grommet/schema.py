import dataclasses
import inspect
from builtins import type as pytype
from collections.abc import AsyncIterable, AsyncIterator
from typing import TYPE_CHECKING, List, get_args, get_origin

from . import _core
from .info import Info
from .metadata import (
    _SCALARS,
    ID,
    MISSING,
    EnumMeta,
    FieldMeta,
    ScalarMeta,
    TypeMeta,
    TypeSpec,
    UnionMeta,
)
from .typing_utils import _get_type_hints, _split_optional, _unwrap_annotated

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable
    from typing import Any, Protocol

    class SubscriptionStream(Protocol):
        def __aiter__(self) -> AsyncIterator[dict[str, Any]]: ...

        async def __anext__(self) -> dict[str, Any]: ...

        async def aclose(self) -> None: ...


class Schema:
    def __init__(
        self,
        *,
        query: pytype,
        mutation: pytype | None = None,
        subscription: pytype | None = None,
        types: "Iterable[pytype]" = (),
        scalars: "Iterable[pytype]" = (),
    ) -> None:
        if query is None:
            raise ValueError("Schema requires a query type.")
        entrypoints = [query, mutation, subscription]
        entrypoints.extend(types)
        registry = _collect_types(entrypoints)
        scalar_registry = _collect_scalars(entrypoints, scalars)
        enum_registry = _collect_enums(entrypoints)
        union_registry = _collect_unions(entrypoints)
        definition, resolvers, scalar_bindings = _build_schema_definition(
            query=query,
            mutation=mutation,
            subscription=subscription,
            registry=registry,
            scalars=scalar_registry,
            enums=enum_registry,
            unions=union_registry,
        )
        self._core = _core.Schema(definition, resolvers, scalar_bindings)

    async def execute(
        self,
        query: str,
        variables: dict[str, "Any"] | None = None,
        root: "Any | None" = None,
        context: "Any | None" = None,
    ) -> dict[str, "Any"]:
        return await self._core.execute(query, variables, root, context)

    def subscribe(
        self,
        query: str,
        variables: dict[str, "Any"] | None = None,
        root: "Any | None" = None,
        context: "Any | None" = None,
    ) -> "SubscriptionStream":
        return self._core.subscribe(query, variables, root, context)

    def sdl(self) -> str:
        return self._core.sdl()

    def __repr__(self) -> str:
        return f"Schema(query={pytype(self._core).__name__})"


def _build_schema_definition(
    *,
    query: pytype,
    mutation: pytype | None,
    subscription: pytype | None,
    registry: dict[pytype, TypeMeta],
    scalars: dict[pytype, ScalarMeta],
    enums: dict[pytype, EnumMeta],
    unions: dict[pytype, UnionMeta],
) -> "tuple[dict[str, Any], dict[str, Callable[..., Any]], list[dict[str, Any]]]":
    types_def: list[dict[str, "Any"]] = []
    resolvers: dict[str, "Callable[..., Any]"] = {}

    for cls, meta in registry.items():
        if meta.kind in ("object", "interface"):
            is_interface = meta.kind == "interface"
            type_kind = (
                "subscription"
                if subscription is not None and cls is subscription
                else meta.kind
            )
            fields_def: list[dict[str, "Any"]] = []
            hints = _get_type_hints(cls)
            for dc_field in dataclasses.fields(cls):
                field_meta = _get_field_meta(dc_field)
                field_name = field_meta.name or dc_field.name
                annotation = hints.get(dc_field.name, dc_field.type)
                force_nullable = dc_field.default is None
                if type_kind == "subscription" and field_meta.resolver is not None:
                    annotation, iterator_optional = _unwrap_async_iterable(annotation)
                    force_nullable = force_nullable or iterator_optional
                gql_type = _type_spec_from_annotation(
                    annotation, expect_input=False, force_nullable=force_nullable
                ).to_graphql()
                resolver_key = None
                args_def: list[dict[str, "Any"]] = []
                if field_meta.resolver is not None:
                    wrapper, args_def = _wrap_resolver(field_meta.resolver)
                    if not is_interface:
                        resolver_key = f"{meta.name}.{field_name}"
                        resolvers[resolver_key] = wrapper
                fields_def.append(
                    {
                        "name": field_name,
                        "source": dc_field.name,
                        "type": gql_type,
                        "args": args_def,
                        "resolver": resolver_key,
                        "description": field_meta.description,
                        "deprecation": field_meta.deprecation_reason,
                    }
                )
            types_def.append(
                {
                    "kind": type_kind,
                    "name": meta.name,
                    "fields": fields_def,
                    "description": meta.description,
                    "implements": [
                        _get_type_meta(iface).name for iface in meta.implements
                    ],
                }
            )
        elif meta.kind == "input":
            input_fields_def: list[dict[str, "Any"]] = []
            hints = _get_type_hints(cls)
            for dc_field in dataclasses.fields(cls):
                annotation = hints.get(dc_field.name, dc_field.type)
                force_nullable = (
                    dc_field.default is not MISSING
                    or dc_field.default_factory is not MISSING
                )
                gql_type = _type_spec_from_annotation(
                    annotation, expect_input=True, force_nullable=force_nullable
                ).to_graphql()
                field_def: dict[str, "Any"] = {"name": dc_field.name, "type": gql_type}
                default_value = _input_field_default(dc_field, annotation)
                if default_value is not MISSING:
                    field_def["default"] = default_value
                input_fields_def.append(field_def)
            types_def.append(
                {
                    "kind": "input",
                    "name": meta.name,
                    "fields": input_fields_def,
                    "description": meta.description,
                }
            )
        else:
            raise TypeError(f"Unknown type kind: {meta.kind}")

    schema_def = {
        "schema": {
            "query": _get_type_meta(query).name,
            "mutation": _maybe_type_name(mutation),
            "subscription": _maybe_type_name(subscription),
        },
        "types": types_def,
        "scalars": [
            {
                "name": meta.name,
                "description": meta.description,
                "specified_by_url": meta.specified_by_url,
            }
            for meta in scalars.values()
        ],
        "enums": [
            {
                "name": meta.name,
                "description": meta.description,
                "values": list(getattr(enum_type, "__members__", {}).keys()),
            }
            for enum_type, meta in enums.items()
        ],
        "unions": [
            {
                "name": meta.name,
                "description": meta.description,
                "types": [_get_type_meta(tp).name for tp in meta.types],
            }
            for meta in unions.values()
        ],
    }
    scalar_bindings = [
        {
            "name": meta.name,
            "python_type": scalar_type,
            "serialize": meta.serialize,
            "parse_value": meta.parse_value,
        }
        for scalar_type, meta in scalars.items()
    ]
    return schema_def, resolvers, scalar_bindings


def _collect_types(entrypoints: "Iterable[pytype | None]") -> dict[pytype, TypeMeta]:
    registry: dict[pytype, TypeMeta] = {}
    pending = [tp for tp in entrypoints if tp is not None]

    while pending:
        cls = pending.pop()
        if _is_enum_type(cls) or _is_scalar_type(cls):
            continue
        if _is_union_type(cls):
            union_meta = _get_union_meta(cls)
            pending.extend(union_meta.types)
            continue
        if cls in registry:
            continue
        meta = _get_type_meta(cls)
        registry[cls] = meta
        pending.extend(meta.implements)
        hints = _get_type_hints(cls)
        for dc_field in dataclasses.fields(cls):
            annotation = hints.get(dc_field.name, dc_field.type)
            pending.extend(_iter_type_refs(annotation))
            field_meta = _get_field_meta(dc_field)
            if field_meta.resolver is not None:
                arg_types = _resolver_arg_annotations(field_meta.resolver)
                for arg_ann in arg_types.values():
                    pending.extend(_iter_type_refs(arg_ann))
    return registry


def _collect_scalars(
    entrypoints: "Iterable[pytype | None]",
    explicit_scalars: "Iterable[pytype]",
) -> dict[pytype, ScalarMeta]:
    registry: dict[pytype, ScalarMeta] = {}
    pending = [tp for tp in entrypoints if tp is not None]

    for scalar in explicit_scalars:
        if not _is_scalar_type(scalar):
            raise TypeError(f"{scalar} is not decorated with @grommet.scalar")
        registry.setdefault(scalar, _get_scalar_meta(scalar))

    while pending:
        cls = pending.pop()
        if _is_union_type(cls):
            pending.extend(_get_union_meta(cls).types)
            continue
        if _is_scalar_type(cls):
            if cls not in registry:
                registry[cls] = _get_scalar_meta(cls)
            continue
        if not _is_grommet_type(cls):
            continue
        pending.extend(_get_type_meta(cls).implements)
        hints = _get_type_hints(cls)
        for dc_field in dataclasses.fields(cls):
            annotation = hints.get(dc_field.name, dc_field.type)
            pending.extend(_iter_scalar_refs(annotation))
            field_meta = _get_field_meta(dc_field)
            if field_meta.resolver is not None:
                arg_types = _resolver_arg_annotations(field_meta.resolver)
                for arg_ann in arg_types.values():
                    pending.extend(_iter_scalar_refs(arg_ann))
    return registry


def _collect_enums(entrypoints: "Iterable[pytype | None]") -> dict[pytype, EnumMeta]:
    registry: dict[pytype, EnumMeta] = {}
    pending = [tp for tp in entrypoints if tp is not None]

    while pending:
        cls = pending.pop()
        if _is_union_type(cls):
            pending.extend(_get_union_meta(cls).types)
            continue
        if _is_enum_type(cls):
            registry.setdefault(cls, _get_enum_meta(cls))
            continue
        if not _is_grommet_type(cls):
            continue
        pending.extend(_get_type_meta(cls).implements)
        hints = _get_type_hints(cls)
        for dc_field in dataclasses.fields(cls):
            annotation = hints.get(dc_field.name, dc_field.type)
            pending.extend(_iter_enum_refs(annotation))
            field_meta = _get_field_meta(dc_field)
            if field_meta.resolver is not None:
                arg_types = _resolver_arg_annotations(field_meta.resolver)
                for arg_ann in arg_types.values():
                    pending.extend(_iter_enum_refs(arg_ann))
    return registry


def _collect_unions(entrypoints: "Iterable[pytype | None]") -> dict[pytype, UnionMeta]:
    registry: dict[pytype, UnionMeta] = {}
    pending = [tp for tp in entrypoints if tp is not None]

    while pending:
        cls = pending.pop()
        if _is_union_type(cls):
            if cls not in registry:
                meta = _get_union_meta(cls)
                registry[cls] = meta
                pending.extend(meta.types)
            continue
        if not _is_grommet_type(cls):
            continue
        hints = _get_type_hints(cls)
        for dc_field in dataclasses.fields(cls):
            annotation = hints.get(dc_field.name, dc_field.type)
            pending.extend(_iter_union_refs(annotation))
            field_meta = _get_field_meta(dc_field)
            if field_meta.resolver is not None:
                arg_types = _resolver_arg_annotations(field_meta.resolver)
                for arg_ann in arg_types.values():
                    pending.extend(_iter_union_refs(arg_ann))
    return registry


_RESERVED_PARAM_NAMES = {"parent", "root", "self", "info", "context"}


def _resolver_params(resolver: "Callable[..., Any]") -> list[inspect.Parameter]:
    sig = inspect.signature(resolver)
    return [
        p
        for p in sig.parameters.values()
        if p.kind
        not in (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD)
    ]


def _resolver_arg_params(resolver: "Callable[..., Any]") -> list[inspect.Parameter]:
    return [p for p in _resolver_params(resolver) if p.name not in _RESERVED_PARAM_NAMES]


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


def _wrap_resolver(
    resolver: "Callable[..., Any]",
) -> "tuple[Callable[..., Any], list[dict[str, Any]]]":
    hints = _get_type_hints(resolver)
    params = _resolver_params(resolver)
    parent_param = _find_param(params, {"parent", "self"})
    root_param = _find_param(params, {"root"})
    info_param = _find_param(params, {"info"})
    context_param = _find_param(params, {"context"})
    arg_params = [p for p in params if p.name not in _RESERVED_PARAM_NAMES]

    arg_defs: list[dict[str, "Any"]] = []
    arg_annotations: dict[str, "Any"] = {}
    for param in arg_params:
        annotation = hints.get(param.name, param.annotation)
        if annotation is inspect._empty:
            raise TypeError(
                f"Resolver {resolver.__name__} missing annotation for '{param.name}'."
            )
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
        arg_annotations[param.name] = annotation

    async def wrapper(parent: "Any", info: "Any", **kwargs: "Any") -> "Any":
        call_kwargs: dict[str, "Any"] = {}
        info_obj = None
        if info_param is not None or context_param is not None or root_param is not None:
            info_obj = _normalize_info(info)
        if parent_param is not None:
            call_kwargs[parent_param.name] = parent
        if info_param is not None:
            call_kwargs[info_param.name] = info_obj or _normalize_info(info)
        if context_param is not None:
            call_kwargs[context_param.name] = info_obj.context if info_obj else None
        if root_param is not None:
            call_kwargs[root_param.name] = info_obj.root if info_obj else None
        for name, annotation in arg_annotations.items():
            if name in kwargs:
                call_kwargs[name] = _coerce_value(kwargs[name], annotation)
        result = resolver(**call_kwargs)
        if inspect.isawaitable(result):
            return await result
        return result

    return wrapper, arg_defs


def _default_value_for_annotation(annotation: "Any", default: "Any") -> "Any":
    if default is MISSING:
        return default
    annotation = _unwrap_annotated(annotation)
    inner, _ = _split_optional(annotation)
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        if isinstance(default, list | tuple):
            return [
                _default_value_for_annotation(item_type, item) for item in list(default)
            ]
        return default
    if _is_input_type(inner):
        if isinstance(default, inner):
            return dataclasses.asdict(default)
        if isinstance(default, dict):
            return default
    return default


def _input_field_default(
    dc_field: "dataclasses.Field[Any]", annotation: "Any"
) -> "Any":
    if dc_field.default is not MISSING:
        return _default_value_for_annotation(annotation, dc_field.default)
    if dc_field.default_factory is not MISSING:
        return _default_value_for_annotation(annotation, dc_field.default_factory())
    return MISSING


def _coerce_value(value: "Any", annotation: "Any") -> "Any":
    if value is None:
        return None
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    if is_optional:
        return _coerce_value(value, inner)
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        return [_coerce_value(item, item_type) for item in value]
    if inner is ID:
        return str(value)
    if _is_enum_type(inner):
        if isinstance(value, inner):
            return value
        if isinstance(value, str):
            try:
                return inner[value]
            except KeyError as exc:
                raise ValueError(
                    f"Invalid enum value '{value}' for {inner.__name__}"
                ) from exc
        return inner(value)
    if _is_scalar_type(inner):
        return _get_scalar_meta(inner).parse_value(value)
    if inner in (str, int, float):
        return inner(value)
    if inner is bool:
        return bool(value)
    if _is_input_type(inner):
        if isinstance(value, inner):
            return value
        if isinstance(value, dict):
            return inner(**value)
        raise TypeError(f"Expected mapping for input type {inner.__name__}")
    return value


def _unwrap_async_iterable(annotation: "Any") -> tuple["Any", bool]:
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    origin = get_origin(inner)
    if origin in (AsyncIterator, AsyncIterable):
        args = get_args(inner)
        if not args:
            raise TypeError("AsyncIterator/AsyncIterable must be parameterized.")
        return args[0], is_optional
    return annotation, False


def _unwrap_async_iterable_inner(annotation: "Any") -> "Any":
    origin = get_origin(annotation)
    if origin in (AsyncIterator, AsyncIterable):
        args = get_args(annotation)
        if not args:
            raise TypeError("AsyncIterator/AsyncIterable must be parameterized.")
        return args[0]
    return annotation


def _iter_type_refs(annotation: "Any") -> list[pytype]:
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    _ = is_optional
    inner = _unwrap_async_iterable_inner(inner)
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        return _iter_type_refs(item_type)
    if _is_grommet_type(inner):
        return [inner]
    if _is_union_type(inner):
        return [inner]
    return []


def _iter_scalar_refs(annotation: "Any") -> list[pytype]:
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    _ = is_optional
    inner = _unwrap_async_iterable_inner(inner)
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        return _iter_scalar_refs(item_type)
    if _is_scalar_type(inner):
        return [inner]
    return []


def _iter_enum_refs(annotation: "Any") -> list[pytype]:
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    _ = is_optional
    inner = _unwrap_async_iterable_inner(inner)
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        return _iter_enum_refs(item_type)
    if _is_enum_type(inner):
        return [inner]
    return []


def _iter_union_refs(annotation: "Any") -> list[pytype]:
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    _ = is_optional
    inner = _unwrap_async_iterable_inner(inner)
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        return _iter_union_refs(item_type)
    if _is_union_type(inner):
        return [inner]
    return []


def _type_spec_from_annotation(
    annotation: "Any",
    *,
    expect_input: bool,
    force_nullable: bool = False,
) -> TypeSpec:
    annotation = _unwrap_annotated(annotation)
    inner, is_optional = _split_optional(annotation)
    nullable = is_optional or force_nullable
    origin = get_origin(inner)
    if origin in (list, List):
        (item_type,) = get_args(inner)
        return TypeSpec(
            kind="list",
            of_type=_type_spec_from_annotation(item_type, expect_input=expect_input),
            nullable=nullable,
        )
    if _is_scalar_type(inner):
        scalar_meta = _get_scalar_meta(inner)
        return TypeSpec(kind="named", name=scalar_meta.name, nullable=nullable)
    if _is_enum_type(inner):
        enum_meta = _get_enum_meta(inner)
        return TypeSpec(kind="named", name=enum_meta.name, nullable=nullable)
    if _is_union_type(inner):
        if expect_input:
            raise TypeError("Union types cannot be used as input")
        union_meta = _get_union_meta(inner)
        return TypeSpec(kind="named", name=union_meta.name, nullable=nullable)
    if inner in _SCALARS:
        return TypeSpec(kind="named", name=_SCALARS[inner], nullable=nullable)
    if _is_grommet_type(inner):
        type_meta = _get_type_meta(inner)
        if expect_input and type_meta.kind != "input":
            raise TypeError(f"{type_meta.name} is not an input type")
        if not expect_input and type_meta.kind == "input":
            raise TypeError(f"{type_meta.name} cannot be used as output")
        return TypeSpec(kind="named", name=type_meta.name, nullable=nullable)
    raise TypeError(f"Unsupported annotation: {annotation}")


def _get_type_meta(cls: pytype) -> TypeMeta:
    meta: TypeMeta | None = getattr(cls, "__grommet__", None)
    if meta is None:
        raise TypeError(
            f"{cls.__name__} is not decorated with @grommet.type, @grommet.interface, or @grommet.input"
        )
    return meta


def _get_scalar_meta(cls: pytype) -> ScalarMeta:
    meta: ScalarMeta | None = getattr(cls, "__grommet_scalar__", None)
    if meta is None:
        raise TypeError(f"{cls.__name__} is not decorated with @grommet.scalar")
    return meta


def _get_enum_meta(cls: pytype) -> EnumMeta:
    meta: EnumMeta | None = getattr(cls, "__grommet_enum__", None)
    if meta is None:
        raise TypeError(f"{cls.__name__} is not decorated with @grommet.enum")
    return meta


def _get_union_meta(cls: pytype) -> UnionMeta:
    meta: UnionMeta | None = getattr(cls, "__grommet_union__", None)
    if meta is None:
        raise TypeError(f"{cls.__name__} is not a grommet union")
    return meta


def _maybe_type_name(cls: pytype | None) -> str | None:
    if cls is None:
        return None
    return _get_type_meta(cls).name


def _get_field_meta(dc_field: "dataclasses.Field[Any]") -> FieldMeta:
    meta = dc_field.metadata.get("grommet") if dc_field.metadata else None
    if isinstance(meta, FieldMeta):
        return meta
    return FieldMeta()


def _is_grommet_type(obj: "Any") -> bool:
    return hasattr(obj, "__grommet__")


def _is_scalar_type(obj: "Any") -> bool:
    return hasattr(obj, "__grommet_scalar__")


def _is_enum_type(obj: "Any") -> bool:
    return hasattr(obj, "__grommet_enum__")


def _is_union_type(obj: "Any") -> bool:
    return hasattr(obj, "__grommet_union__")


def _is_input_type(obj: "Any") -> bool:
    return _is_grommet_type(obj) and _get_type_meta(obj).kind == "input"
