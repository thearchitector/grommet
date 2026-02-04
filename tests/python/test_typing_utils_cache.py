from grommet.typing_utils import _get_type_hints


class Example:
    foo: int


def test_get_type_hints_cached() -> None:
    first = _get_type_hints(Example)
    second = _get_type_hints(Example)
    assert first == second
    assert first["foo"] is int
