from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Query:
    names: list[str]
    maybe_names: list[str] | None
    maybe_items: list[str | None]
    maybe_items_optional: list[str | None] | None
    optional_scalar: str | None
    required_scalar: str


def test_nullability_sdl_shapes() -> None:
    """
    Verifies SDL nullability matches list and optional field annotations.
    """
    schema = gm.Schema(query=Query)
    sdl = schema.sdl()

    assert "names: [String!]!" in sdl
    assert "maybe_names: [String!]" in sdl
    assert "maybe_items: [String]!" in sdl
    assert "maybe_items_optional: [String]" in sdl
    assert "optional_scalar: String" in sdl
    assert "required_scalar: String!" in sdl
