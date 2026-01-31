import asyncio
import time
from typing import TYPE_CHECKING

import strawberry

if TYPE_CHECKING:
    pass


@strawberry.type
class Cell:
    _value: strawberry.Private[str]

    @strawberry.field
    async def value(self) -> str:
        await asyncio.sleep(0)
        return self._value


@strawberry.type
class Row:
    a: int
    cells: list[Cell]


@strawberry.type
class Query:
    @strawberry.field
    async def rows(self) -> list[Row]:
        return [
            Row(a=i, cells=[Cell(_value=f"Cell {j}") for j in range(5)])
            for i in range(100000)
        ]


async def main() -> None:
    schema = strawberry.Schema(query=Query)
    start = time.perf_counter()
    result = await schema.execute("{ rows { a cells { value } } }")
    elapsed = time.perf_counter() - start
    assert not result.errors
    res = result.data
    assert res is not None
    size = len(res["rows"])
    print(f"Fetched {size} rows in {elapsed:.4f}s")


if __name__ == "__main__":
    asyncio.run(main())
