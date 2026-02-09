# pragma: no ai
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Protocol

    class Graph(Protocol):
        def requests(self, name: str) -> bool: ...
        def peek(self, name: str | None = None) -> "Graph": ...


@dataclass(frozen=True, slots=True)
class Context[T = None]:
    _graph: "Graph" = field(repr=False)
    state: T | None = None

    def requests(self, name: str) -> bool:
        """
        Return True if the field named 'name' is requested at the current level of the
        execution graph.
        """
        ...

    def peek(self, name: str | None = None) -> "Graph":
        """
        Allows peeking into the execution graph of the field 'name' at the current
        graph level. If 'name' is None, current field's subgraph is peeked.

        If a field is peeked that is not requested in the current query, a sentinel
        `MISSING` Graph instance is returned, which always returns False for `requests`
        and itself for `peek`.
        """
        ...
