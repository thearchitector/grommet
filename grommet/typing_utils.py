import sys
from typing import TYPE_CHECKING, get_type_hints

if TYPE_CHECKING:
    from typing import Any


def _get_type_hints(obj: "Any") -> dict[str, "Any"]:
    try:
        globalns = vars(sys.modules[obj.__module__])
        localns = dict(vars(obj))
        return get_type_hints(
            obj, globalns=globalns, localns=localns, include_extras=True
        )
    except Exception:
        return getattr(obj, "__annotations__", {})
