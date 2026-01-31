from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Node:
    name: str
    children: list["Node"] | None = None


def test_recursive_type_sdl() -> None:
    schema = gm.Schema(query=Node)
    sdl = schema.sdl()

    assert "type Node" in sdl
    assert "children: [Node!]" in sdl
