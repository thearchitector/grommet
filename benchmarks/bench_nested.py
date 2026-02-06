import asyncio
import time
from dataclasses import dataclass
from typing import TYPE_CHECKING

import grommet as gm

if TYPE_CHECKING:
    pass


@gm.type
@dataclass
class Cell:
    _value: str

    @gm.field
    async def value(self) -> str:
        await asyncio.sleep(0)
        return self._value


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
        return [
            Row(a=i, cells=[Cell(_value=f"Cell {j}") for j in range(5)])
            for i in range(100000)
        ]


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
