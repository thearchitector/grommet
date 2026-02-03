import pytest

import grommet as gm


def test_type_requires_dataclass() -> None:
    """
    Ensures @grommet.type requires an explicit dataclass wrapper.
    """
    with pytest.raises(TypeError, match="explicit dataclass"):

        @gm.type
        class Query:
            pass


def test_input_requires_dataclass() -> None:
    """
    Ensures @grommet.input requires an explicit dataclass wrapper.
    """
    with pytest.raises(TypeError, match="explicit dataclass"):

        @gm.input
        class Input:
            pass


def test_interface_requires_dataclass() -> None:
    """
    Ensures @grommet.interface requires an explicit dataclass wrapper.
    """
    with pytest.raises(TypeError, match="explicit dataclass"):

        @gm.interface
        class Node:
            pass
