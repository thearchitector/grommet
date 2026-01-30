import sys
from builtins import type as pytype
from typing import TYPE_CHECKING, Annotated, get_args, get_origin, get_type_hints

if TYPE_CHECKING:
    from typing import Any

_NONE_TYPE = pytype(None)


def _unwrap_annotated(annotation: "Any") -> "Any":
    origin = get_origin(annotation)
    if origin is Annotated:
        return get_args(annotation)[0]
    return annotation


def _split_optional(annotation: "Any") -> tuple["Any", bool]:
    args = get_args(annotation)
    if args:
        non_none = [arg for arg in args if arg is not _NONE_TYPE]
        if len(non_none) == 1 and len(non_none) != len(args):
            return non_none[0], True
    return annotation, False


def _get_type_hints(obj: "Any") -> dict[str, "Any"]:
    try:
        globalns = vars(sys.modules[obj.__module__])
        localns = dict(vars(obj))
        return get_type_hints(
            obj, globalns=globalns, localns=localns, include_extras=True
        )
    except Exception:
        return getattr(obj, "__annotations__", {})
