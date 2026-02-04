import asyncio
import dataclasses
import time

import grommet as gm


@gm.type
@dataclasses.dataclass
class Query:
    @gm.field
    async def value(self) -> int:
        return 1


async def run_query(schema: gm.Schema) -> None:
    await schema.execute("{ value }")


def main() -> None:
    schema = gm.Schema(query=Query)
    runs = 1000

    async def runner() -> None:
        start = time.perf_counter()
        for _ in range(runs):
            await run_query(schema)
        elapsed = time.perf_counter() - start
        per = elapsed / runs
        print(f"query executes: {runs} total, {per:.6f}s per")

    asyncio.run(runner())


if __name__ == "__main__":
    main()
