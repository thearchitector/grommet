from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Protocol, TypeVar

    class Lookahead(Protocol):
        def exists(self) -> bool: ...
        def field(self, name: str) -> "Lookahead": ...

    StateType = TypeVar("StateType", default=None)


@dataclass(frozen=True, slots=True)
class Context[T: StateType]:
    state: T = None

    def field(self, name: str) -> "Lookahead": ...
    def look_ahead(self) -> "Lookahead": ...
