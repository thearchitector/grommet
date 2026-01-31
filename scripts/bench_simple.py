import asyncio
import time
from dataclasses import dataclass
from typing import TYPE_CHECKING

import grommet as gm

if TYPE_CHECKING:
    from typing import Any


@gm.type
@dataclass
class Query:
    @gm.field
    @staticmethod
    async def ping(parent: "Any", info: "Any") -> str:
        return "pong"


async def main() -> None:
    schema = gm.Schema(query=Query)
    iterations = 500
    start = time.perf_counter()
    for _ in range(iterations):
        await schema.execute("{ ping }")
    elapsed = time.perf_counter() - start
    per_sec = iterations / elapsed if elapsed else 0
    print(f"{iterations} iterations in {elapsed:.4f}s ({per_sec:.1f} ops/s)")


if __name__ == "__main__":
    asyncio.run(main())
