import asyncio
import time

import strawberry


@strawberry.type
class Cell:
    j: strawberry.Private[int]

    @strawberry.field
    @staticmethod
    async def value(parent: strawberry.Parent["Cell"]) -> str:
        await asyncio.sleep(0)
        return f"Cell {parent.j}"


@strawberry.type
class Row:
    a: int
    cells: list[Cell]


@strawberry.type
class Query:
    @strawberry.field
    async def rows(self) -> list[Row]:
        return [Row(a=i, cells=[Cell(j=j) for j in range(5)]) for i in range(100000)]


async def main() -> None:
    schema = strawberry.Schema(query=Query)
    start = time.perf_counter()
    result = await schema.execute("{ rows { a cells { value } } }")
    elapsed = time.perf_counter() - start
    assert not result.errors
    res = result.data
    assert res is not None
    size = len(res["rows"])
    print(f"Fetched {size * 5} cells ({size}x5) in {elapsed:.4f}s")


if __name__ == "__main__":
    asyncio.run(main())
