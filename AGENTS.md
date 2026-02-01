# AGENTS.md

## Development Guidelines
- Match existing patterns: Mirror the established architecture, type hints, and docstring style referenced in the pattern guides.
- Document intent: Update relevant docs or TODO items when behavior shifts or tasks complete.
- Prove with pytest: Cover modifications with pytest unit tests, and address any failures prior to finishing.
- Run tests with uv: Use `uv run pytest -sv tests/` as the default test command unless a task explicitly requires a different invocation.
- ALWAYS use uv run for Python: ALWAYS use `uv run` when running Python commands. Never run Python directly (e.g., python script.py). Always use `uv run python script.py` or `uv run python -c "..."`. This ensures the correct virtual environment and dependencies are used.
- Verify linting with mypy: Use `uv run mypy .` to verify typing. Address any failures prior to finishing.
- Lint with ruff: Use `uv run ruff check --fix --exit-non-zero-on-fix .` to lint. Address any failures prior to finishing.

## Common Patterns

### 1. Type Hints

ALWAYS assume you can type according to Python 3.13+ conventions. Use `type Foo = Bar` to create type aliases where necessasry, and import generic structure from `collections.abc`. ALWAYS try to import as few things as possible during runtime, ideally importing specific items when `TYPE_CHECKING` is true. NEVER import the entire `typing` module. ALWAYS import specific items from `typing`. ALWAYS use forward references when using a type that requires type-time importing only. NEVER use forward references for types that must be imported at runtime, or for any builtin types like `float`, `list`, `str`, etc.

ALWAYS follow this general pattern for importing and using types:

```python
from typing import TYPE_CHECKING

from some_module import SomeClass

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import Any

# `Callable` and `Any` are only used for typing, so their imports exist only in the TYPE_CHECKING block,
# and they are forward referenced in the function signature.
def foo(bar: "Callable[[Any], Any]") -> None:
    pass

# `list` is a builtin type, so no import is necessary and thus it can be used directly. `Any` is only used
# for typing, so it is forward referenced in the function signature.
def hello(tomato: list["Any"]) -> str | None:
    pass

# `SomeClass` is used during runtime, so it does not need to be forward referenced.
def world() -> SomeClass:
    return SomeClass()
```

NEVER follow this pattern in unit tests. When writing unit tests, ALWAYS import types directly and NEVER write `TYPE_CHECKING` blocks.

### 2. Type Usage

ALWAYS ensure that static typing is valid and passes a check with mypy, via `uv run mypy .`. When fixing typing errors, ALWAYS prefer to `cast()` types in instances ONLY where we can make assumptions about the type that the type checker is unaware of. NEVER use `# type: ignore` UNLESS we're circumventing a typing error from an external library.

For example:

**BAD**:
```python
import json

def get_user_email(payload: str) -> str:
    data = json.loads(payload)
    return data["user"]["email"] # type: ignore[no-return-any]
```

**GOOD**:
```python
import json
from typing import cast

def get_user_email(payload: str) -> str:
    data = json.loads(payload)
    return cast(str, data["user"]["email"])
```

### 3. Prefer to import specific items instead of entire modules

USUALLY prefer to import specific items instead of entire modules. For example:

**BAD**:
```python
import some_module

some_module.func()
```

**GOOD**:
```python
from some_module import func

func()
```

There are a select few exceptions to this rule, mainly semantic. For example, it would be preferable to `import orjson` and use `orjson.dumps` rather than `from orjson import dumps`, as the "dumps" is fairly nondescript. Similarly, `asyncio.run` instead of `from asyncio import run`. When this distinction is not clear, ALWAYS ask the user for clarification.

### 4. Docstrings

ALWAYS write docstrings for _public-facing_ functions and classes. If a function or class is not intended to be used outside of the module, docstrings are OPTIONAL. Docstrings should be concise, and only include a brief description of what the actual function does. Do not include implementation details, parameter descriptions, or examples in the docstring. ALWAYS place the triple quotes on their own line, EXCEPT for when the docstring can fit within a 85 characters limit. For example:

```python
def fibbonaci(n: int) -> int:
    """Recursively computes the n-th Fibonacci number."""
    pass

# This docstring is too long and will be formatted as a multi-line docstring.
def hello(tomato: list["Any"]) -> str | None:
    """
    This docstring is too long and must be formatted as a multi-line docstring, where the length of each line is
    capped at the maximum allowable by Ruff, and where the triple quotes are on their own lines.
    """
    pass
```
