import dataclasses
import time

import grommet as gm


@gm.type
@dataclasses.dataclass
class Query:
    value: int = 1


def main() -> None:
    runs = 1000
    start = time.perf_counter()
    for _ in range(runs):
        gm.Schema(query=Query)
    elapsed = time.perf_counter() - start
    per = elapsed / runs
    print(f"schema builds: {runs} total, {per:.6f}s per")


if __name__ == "__main__":
    main()
