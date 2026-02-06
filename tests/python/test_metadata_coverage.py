import pytest

from grommet.errors import GrommetValueError
from grommet.metadata import TypeKind, TypeMeta, TypeSpec


def test_type_meta_valid_kinds() -> None:
    """
    Ensures TypeMeta accepts valid TypeKind values.
    """
    meta = TypeMeta(kind=TypeKind.OBJECT, name="Foo")
    assert meta.kind is TypeKind.OBJECT
    assert meta.name == "Foo"


def test_typespec_list_requires_inner() -> None:
    """
    Ensures list TypeSpec entries require an inner type.
    """
    spec = TypeSpec(kind="list")
    with pytest.raises(GrommetValueError):
        spec.to_graphql()


def test_typespec_list_to_graphql() -> None:
    """Ensures list TypeSpec renders to correct GraphQL notation."""
    inner = TypeSpec(kind="named", name="String")
    spec = TypeSpec(kind="list", of_type=inner, nullable=True)
    assert spec.to_graphql() == "[String!]"

    spec_nn = TypeSpec(kind="list", of_type=inner)
    assert spec_nn.to_graphql() == "[String!]!"


def test_type_meta_unknown_kind_raises() -> None:
    """Ensures TypeMeta rejects kind values not in _TYPE_KIND_TO_META."""
    from unittest.mock import MagicMock

    from grommet.errors import GrommetValueError

    fake_kind = MagicMock()
    fake_kind.value = "bogus"
    with pytest.raises((GrommetValueError, KeyError)):
        TypeMeta(kind=fake_kind, name="Bad")  # type: ignore[arg-type]
