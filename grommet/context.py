from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Protocol

    class Lookahead(Protocol):
        def exists(self) -> bool: ...
        def field(self, name: str) -> "Lookahead": ...


@dataclass(frozen=True, slots=True)
class Context[T = None]:
    _lookahead: "Lookahead" = field(repr=False)
    state: T | None = None

    def field(self, name: str) -> "Lookahead":
        return self._lookahead.field(name)

    def look_ahead(self) -> "Lookahead":
        return self._lookahead
