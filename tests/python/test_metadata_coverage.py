import pytest

from grommet.errors import GrommetValueError
from grommet.metadata import TypeMeta, TypeSpec


def test_type_meta_unknown_kind_raises() -> None:
    """
    Ensures TypeMeta rejects unknown kind values.
    """
    with pytest.raises(GrommetValueError):
        TypeMeta(kind="mystery", name="Bad")


def test_typespec_list_requires_inner() -> None:
    """
    Ensures list TypeSpec entries require an inner type.
    """
    spec = TypeSpec(kind="list")
    with pytest.raises(GrommetValueError):
        spec.to_graphql()
