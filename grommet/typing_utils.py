import sys
from functools import lru_cache
from typing import TYPE_CHECKING, get_type_hints

if TYPE_CHECKING:
    from typing import Any


def _resolve_type_hints(obj: "Any") -> dict[str, "Any"]:
    try:
        globalns = vars(sys.modules[obj.__module__])
        localns = dict(vars(obj))
        return get_type_hints(
            obj, globalns=globalns, localns=localns, include_extras=True
        )
    except Exception:
        return getattr(obj, "__annotations__", {})


@lru_cache(maxsize=512)
def _cached_type_hints(obj: "Any") -> dict[str, "Any"]:
    return _resolve_type_hints(obj)


def _get_type_hints(obj: "Any") -> dict[str, "Any"]:
    try:
        return _cached_type_hints(obj)
    except TypeError:
        return _resolve_type_hints(obj)
