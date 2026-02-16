# pragma: no ai
import asyncio
import time
from dataclasses import dataclass
from typing import Annotated

import strawberry
import uvloop

import grommet as gm


async def run_bench(tag, schema) -> None:
    start = time.perf_counter()
    result = await schema.execute("{ rows { a cells { value } } }")
    elapsed = time.perf_counter() - start
    assert not result.errors
    res = result.data
    assert res is not None
    size = len(res["rows"])
    print(f"{tag}: Fetched {size * 5} cells ({size}x5) in {elapsed:.4f}s")


@strawberry.type
class Cell:
    j: strawberry.Private[int]

    @strawberry.field
    async def value(self) -> str:
        await asyncio.sleep(0)
        return f"Cell {self.j}"


@strawberry.type
class Row:
    a: int
    cells: list[Cell]


@strawberry.type
class Query:
    @strawberry.field
    async def rows(self) -> list[Row]:
        return [Row(a=i, cells=[Cell(j=j) for j in range(5)]) for i in range(100000)]


uvloop.run(run_bench("strawberry", strawberry.Schema(query=Query)))


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
    async def rows(self) -> list[Row]:
        return [Row(a=i, cells=[Cell(j=j) for j in range(5)]) for i in range(100000)]


uvloop.run(run_bench("grommet", gm.Schema(query=Query)))
