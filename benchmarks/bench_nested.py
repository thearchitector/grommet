import asyncio
import time
from dataclasses import dataclass

import grommet as gm


@gm.type
@dataclass
class Cell:
    j: gm.Internal[int]

    @gm.field
    @staticmethod
    async def value(parent: "Cell") -> str:
        await asyncio.sleep(0)
        return f"Cell {parent.j}"


@gm.type
@dataclass
class Row:
    a: int
    cells: list[Cell]


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def rows(parent: "Query") -> list[Row]:
        return [Row(a=i, cells=[Cell(j=j) for j in range(5)]) for i in range(100000)]


async def main() -> None:
    schema = gm.Schema(query=Query)
    start = time.perf_counter()
    result = await schema.execute("{ rows { a cells { value } } }")
    elapsed = time.perf_counter() - start
    res = result["data"]
    size = len(res["rows"])
    print(f"Fetched {size * 5} cells ({size}x5) in {elapsed:.4f}s")


if __name__ == "__main__":
    asyncio.run(main())
