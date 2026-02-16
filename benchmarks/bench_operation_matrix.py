import argparse
import asyncio
import json
import time
from dataclasses import dataclass
from typing import Annotated

import uvloop

import grommet as gm


def _build_nested_sync_case(rows: int) -> tuple[str, gm.Schema, str]:
    @gm.type
    @dataclass
    class Cell:
        j: Annotated[int, gm.Hidden]

        @gm.field
        def value(self) -> str:
            return f"Cell {self.j}"

    @gm.type
    @dataclass
    class Row:
        a: int
        cells: list[Cell]

    @gm.type
    @dataclass
    class Query:
        @gm.field
        def rows(self) -> list[Row]:
            return [Row(a=i, cells=[Cell(j=j) for j in range(5)]) for i in range(rows)]

    return "nested_sync", gm.Schema(query=Query), "{ rows { a cells { value } } }"


def _build_nested_async_case(rows: int) -> tuple[str, gm.Schema, str]:
    @gm.type
    @dataclass
    class Cell:
        j: Annotated[int, gm.Hidden]

        @gm.field
        async def value(self) -> str:
            await asyncio.sleep(0)
            return f"Cell {self.j}"

    @gm.type
    @dataclass
    class Row:
        a: int
        cells: list[Cell]

    @gm.type
    @dataclass
    class Query:
        @gm.field
        def rows(self) -> list[Row]:
            return [Row(a=i, cells=[Cell(j=j) for j in range(5)]) for i in range(rows)]

    return "nested_async", gm.Schema(query=Query), "{ rows { a cells { value } } }"


def _build_args_async_input_case(items: int) -> tuple[str, gm.Schema, str]:
    @gm.input
    @dataclass
    class Factor:
        amount: int

    @gm.type
    @dataclass
    class Item:
        base: int

        @gm.field
        async def scaled(self, factor: Factor) -> int:
            await asyncio.sleep(0)
            return self.base * factor.amount

    @gm.type
    @dataclass
    class Query:
        @gm.field
        def items(self) -> list[Item]:
            return [Item(base=i) for i in range(items)]

    return (
        "args_async_input",
        gm.Schema(query=Query),
        "{ items { scaled(factor: { amount: 3 }) } }",
    )


async def _run_case(
    *, tag: str, schema: gm.Schema, query: str, rounds: int
) -> list[float]:
    elapsed: list[float] = []
    for _ in range(rounds):
        start = time.perf_counter()
        result = await schema.execute(query)
        elapsed.append(time.perf_counter() - start)
        assert not result.errors
    return elapsed


def _summarize(samples: list[float]) -> dict[str, float]:
    ordered = sorted(samples)
    middle = ordered[len(ordered) // 2]
    return {
        "median_seconds": middle,
        "min_seconds": ordered[0],
        "max_seconds": ordered[-1],
    }


async def _run_matrix(
    *, rows_large: int, rows_args: int, rounds: int
) -> dict[str, dict[str, float]]:
    cases = [
        _build_nested_sync_case(rows_large),
        _build_nested_async_case(rows_large),
        _build_args_async_input_case(rows_args),
    ]
    out: dict[str, dict[str, float]] = {}
    for tag, schema, query in cases:
        samples = await _run_case(tag=tag, schema=schema, query=query, rounds=rounds)
        out[tag] = _summarize(samples)
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rows-large", type=int, default=40_000)
    parser.add_argument("--rows-args", type=int, default=20_000)
    parser.add_argument("--rounds", type=int, default=1)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    results = uvloop.run(
        _run_matrix(
            rows_large=args.rows_large, rows_args=args.rows_args, rounds=args.rounds
        )
    )
    if args.json:
        print(json.dumps(results, sort_keys=True))
        return

    for tag, stats in results.items():
        print(
            f"{tag}: median={stats['median_seconds']:.4f}s "
            f"min={stats['min_seconds']:.4f}s max={stats['max_seconds']:.4f}s"
        )


if __name__ == "__main__":
    main()
