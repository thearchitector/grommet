import pytest

from grommet.errors import GrommetValueError
from grommet.metadata import TypeMeta, TypeSpec


def test_type_meta_unknown_kind_raises() -> None:
    with pytest.raises(GrommetValueError):
        TypeMeta(kind="mystery", name="Bad")


def test_typespec_list_requires_inner() -> None:
    spec = TypeSpec(kind="list")
    with pytest.raises(GrommetValueError):
        spec.to_graphql()
