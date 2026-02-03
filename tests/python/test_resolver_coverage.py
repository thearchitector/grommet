import pytest

from grommet.errors import GrommetTypeError
from grommet.info import Info
from grommet.resolver import _normalize_info, _wrap_resolver


def test_normalize_info_variants() -> None:
    """
    Verifies info normalization accepts Info, mapping, and object variants.
    """
    info = Info(field_name="field", context="ctx", root="root")
    assert _normalize_info(info) is info

    info_dict = _normalize_info({"field_name": "name", "context": 1, "root": 2})
    assert info_dict.field_name == "name"
    assert info_dict.context == 1
    assert info_dict.root == 2

    class Dummy:
        field_name = "dummy"
        context = "ctx"
        root = "root"

    info_obj = _normalize_info(Dummy())
    assert info_obj.field_name == "dummy"
    assert info_obj.context == "ctx"
    assert info_obj.root == "root"


def test_wrap_resolver_missing_annotation_raises() -> None:
    """
    Ensures resolver wrapping fails when required annotations are missing.
    """

    def resolver(parent, info, value):
        return value

    with pytest.raises(GrommetTypeError):
        _wrap_resolver(resolver)


@pytest.mark.anyio
async def test_wrap_resolver_info_context_root() -> None:
    """
    Verifies wrapped resolvers receive coerced args and info/context/root.
    """

    def resolver(parent, info, context, root, value: int) -> tuple:
        return (parent, info.field_name, context, root, value)

    wrapper, arg_defs = _wrap_resolver(resolver)
    assert arg_defs[0]["name"] == "value"

    result = await wrapper(
        "parent", {"field_name": "f", "context": 1, "root": 2}, value="3"
    )
    assert result == ("parent", "f", 1, 2, 3)


@pytest.mark.anyio
async def test_wrap_resolver_without_info_and_missing_kwargs() -> None:
    """
    Verifies wrapped resolvers use defaults when info kwargs are omitted.
    """

    def resolver(parent, value: int = 5) -> tuple:
        return (parent, value)

    wrapper, _ = _wrap_resolver(resolver)
    result = await wrapper("parent", {"field_name": "ignored"})
    assert result == ("parent", 5)
