from annotationlib import Format
from annotationlib import get_annotations as _get_annotations
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Any


def get_annotations(obj: "Any") -> dict[str, "Any"]:
    """Resolve annotations for a class or function using annotationlib semantics."""
    return _get_annotations(obj, format=Format.VALUE)
