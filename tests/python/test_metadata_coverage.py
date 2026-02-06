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
