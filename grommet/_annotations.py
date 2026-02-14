# pragma: no ai
import sys
from typing import TYPE_CHECKING

if sys.version_info >= (3, 14):
    from annotationlib import Format
    from annotationlib import get_annotations as _get_annotations
else:
    from typing_extensions import Format
    from typing_extensions import get_annotations as _get_annotations

if TYPE_CHECKING:
    from typing import Any


def get_annotations(obj: "Any") -> dict[str, "Any"]:
    """Resolve annotations for a class or function using annotationlib semantics."""
    return _get_annotations(obj, format=Format.VALUE)
