# pragma: no ai
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Protocol

    class Graph(Protocol):
        def requests(self, name: str) -> bool:
            """
            Return True if the field 'name' is requested at the current level of the
            execution graph.
            """

        def peek(self, name: str) -> "Graph":
            """
            Allows peeking into the execution graph of the field 'name' at the current
            graph level.

            If a field is peeked that is not requested in the current query, a sentinel
            `MISSING` Graph instance is returned, which always returns False for
            `requests` and itself for `peek`.
            """


@dataclass(frozen=True, slots=True)
class Context[T = None]:
    graph: "Graph"
    state: T | None = None
