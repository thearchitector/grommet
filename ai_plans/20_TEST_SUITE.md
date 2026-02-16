# Alpha Test Overhaul: Pytest-First, 100% Branch Coverage, Rust-Only Cargo Tests

> i want to prep for an alpha release. all advertised functionality and usage patterns must be tested and validated, with each test having a docstring. pytest should be the entrypoint for testing, aiming for 100% cov _without being redunant_. tests should be eliminated from rust unless they cover rust-only isolated things. i expect a major overhaul of pytests, including by deleting irrelavent tests and adding new files and fixtures in conftest.

## Summary
Rebuild the test strategy around a single, explicit alpha contract: README examples + public Python API behavior are the required functional surface, covered by `pytest` with strict 100% line+branch coverage and non-redundant tests. Remove runtime-scope tests from alpha, enforce per-test docstrings, and reduce Rust tests to isolated internals that cannot be meaningfully validated from Python.

## Public API / Interface Impact
- No runtime library API changes are planned.
- Test interface changes:
  - Every collected pytest test function must have a docstring (enforced in `tests/python/conftest.py`).
  - `uv run pytest` will enforce 100% line+branch coverage for `grommet`.
- Runtime behavior tests currently in Python test suite are removed from alpha scope and deleted.

## Planned File Changes

### Remove existing Python test modules (full replacement to avoid redundancy)
- `tests/python/test_basic_query.py`
- `tests/python/test_context_state.py`
- `tests/python/test_descriptions.py`
- `tests/python/test_event_loop_runtime.py`
- `tests/python/test_free_threaded_import.py`
- `tests/python/test_hidden_fields.py`
- `tests/python/test_mutations.py`
- `tests/python/test_resolver_analysis.py`
- `tests/python/test_resolvers.py`
- `tests/python/test_subscriptions.py`
- `tests/python/test_unions_interfaces.py`

### Replace and expand pytest suite with explicit public/internal split
- `tests/python/conftest.py` (rewritten)
- `tests/python/public/test_readme_quickstart.py`
- `tests/python/public/test_readme_descriptions_and_fields.py`
- `tests/python/public/test_readme_hidden_and_context.py`
- `tests/python/public/test_readme_mutation_and_input.py`
- `tests/python/public/test_readme_subscriptions.py`
- `tests/python/public/test_readme_unions_and_interfaces.py`
- `tests/python/public/test_public_api_contracts.py`
- `tests/python/internal/test_annotations_branches.py`
- `tests/python/internal/test_coercion_branches.py`
- `tests/python/internal/test_compiler_branches.py`
- `tests/python/internal/test_plan_branches.py`
- `tests/python/internal/test_errors_module.py`

### Reduce Rust tests to Rust-only isolated behavior
- `tests/test_core.rs` will be pruned to keep only resolver/runtime bridge internals that are not directly reachable via public Python usage patterns.
- Rust tests for value conversion and Python-visible error behavior will be removed where equivalent behavior is validated in pytest public/internal tests.

### Coverage/config hardening
- `pyproject.toml` updates:
  - Enforce 100% coverage for pytest runs (`--cov-fail-under=100` and branch coverage).
  - Keep coverage scope as `grommet`.
  - Keep output reports (`term-missing`, `html`) unless they block deterministic CI.

## Implementation Plan

## 1) Rebuild `conftest.py` as the suite control center
- Add reusable fixtures for schema construction, async execution helpers, and subscription stream collection.
- Add a collection-time docstring guard that fails if any `test_*` function has no docstring.
- Add shared assertion helpers to reduce repeated result/error boilerplate and keep tests minimal but explicit.

## 2) Re-implement public contract tests from README + public API
- Create one canonical test path per advertised usage pattern, using public API only (`grommet.*`, `Schema.execute`, `Schema.sdl`).
- Validate:
  - quickstart query execution and SDL access via `Schema.sdl`
  - described types/fields
  - resolver args (required/optional)
  - hidden fields (`_`, `ClassVar`, `Hidden`)
  - mutation/input patterns with variables
  - subscription streaming and manual stream control (`__anext__`, `aclose`)
  - context injection usage
  - union naming/description/default naming
  - interface implementers and inherited resolver fields
  - public export surface in `grommet.__all__`
- Include function docstrings for every test.

## 3) Add targeted internal branch tests to reach non-redundant 100%
- Cover currently missed branches in:
  - `grommet/annotations.py`
  - `grommet/coercion.py`
  - `grommet/decorators.py`
  - `grommet/_resolver_compiler.py`
  - `grommet/_type_compiler.py`
  - `grommet/plan.py`
  - `grommet/errors.py`
- Use white-box tests only where public paths cannot hit a branch (hybrid-targeted policy), including controlled monkeypatches for defensive/exception branches.
- Ensure `Schema.sdl` property path is explicitly exercised to close `grommet/schema.py` gap.

## 4) Prune Rust tests to isolated Rust-only concerns
- Keep only tests in `tests/test_core.rs` that verify Rust-side async bridge/iterator internals not realistically testable through Python contracts.
- Remove Rust tests duplicating Python-observable behavior covered by pytest.
- Preserve cargo pass/fail health while reducing duplicated behavioral assertions.

## 5) Enforce and verify alpha quality gates
- Run and require success for:
  - `uv run pytest`
  - `uv run cargo test`
  - `uv run mypy .`
  - `prek run -a`
- Confirm pytest coverage report is exactly 100% line+branch for `grommet`.
- Confirm no collected test lacks a docstring.
- Confirm deleted runtime tests are absent.

## Test Cases and Scenarios (Decision-Complete Coverage Targets)
- README contract scenarios: each code-path family has one dedicated canonical test module.
- Public API contract scenarios: exports, schema execution result shapes, stream semantics, SDL accessor.
- Internal branch scenarios:
  - type-alias unwrapping (including cyclic alias guard path)
  - optional/list/async-iterable annotation edge handling
  - union input/member validation failures and metadata extraction
  - coercion for nested input defaults and optional/list coercers
  - decorator misuse guards (`staticmethod`, `classmethod`, non-callable)
  - resolver compiler missing-annotation and missing-return guards
  - type compiler interface/subscription invalid combinations
  - planner invalid compiled metadata guards and union registration conflicts
  - direct error-constructor message checks for all exported error helpers

## Assumptions and Defaults
- `README.md`, `grommet/schema.py`, and `grommet/_annotations.py` remain unchanged (`pragma: no ai`).
- “Advertised functionality” means README examples plus public API contracts from `grommet.__all__` and schema execution/SDL behavior.
- Runtime thread/GIL checks are intentionally removed from alpha scope and deleted.
- Pytest is the alpha testing entrypoint for Python behavior/coverage; cargo remains a separate rust-only verification command.
- Non-redundancy means each behavior/branch has one primary assertion location, with parameterization replacing duplicate tests.
