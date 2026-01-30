from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Any


@dataclass(frozen=True)
class Info:
    """Holds GraphQL resolver metadata."""

    field_name: str
    context: "Any | None" = None
    root: "Any | None" = None
