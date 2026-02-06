# grommet

![Made with AI](https://img.shields.io/badge/%E2%9C%A8-Made_with_AI-8A2BE2?style=for-the-badge)
![Licensed under BSD-3-Clause-Clear](https://img.shields.io/badge/license-BSD--3--Clause--Clear-yellow?style=for-the-badge)

High performance async Python GraphQL server library inspired by [Strawberry](https://strawberry.rocks/) and backed by [async-graphql](https://async-graphql.github.io/async-graphql/en/index.html).

This is an experiment in a nearly 100% AI-written project. I provide guidelines and design guidance through review of the generated code and curated revision plans, but AI does the heavy lifting. Features are developed as my token and usage counts reset.

The goal is to utilize AI to prove the concept, but do so while also laying solid technical foundations for future human-driven development and maintenance; my personal belief is that the latter is always necessary.

## Quick Start

### Installation

```bash
pip install grommet
# or
uv add grommet
```

### Examples

<!-- <AI_GENERATED> -->

Define your GraphQL types as decorated dataclasses, build a schema, and execute queries:

```python
import asyncio
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class Query:
    @grommet.field
    @staticmethod
    async def greeting() -> str:
        return "Hello, world!"

schema = grommet.Schema(query=Query)
result = asyncio.run(schema.execute("{ greeting }"))
print(result)  # {'data': {'greeting': 'Hello, world!'}}
```

Use `grommet.field` to define resolver-backed fields with arguments:

```python
@grommet.type
@dataclass
class Query:
    @grommet.field
    @staticmethod
    async def hello(name: str) -> str:
        return f"Hello, {name}!"

schema = grommet.Schema(query=Query)
result = asyncio.run(schema.execute('{ hello(name: "grommet") }'))
print(result)  # {'data': {'hello': 'Hello, grommet!'}}
```

Reference the resolution parent and info in resolvers through the `parent` and `info` arguments:

```python
@grommet.type
@dataclass
class Query:
    @grommet.field
    @staticmethod
    async def hello(parent: "Query", info: gm.Info) -> str:
        return f"Hello, {name}!"
```

Add mutations by defining a separate mutation type:

```python
@grommet.input
@dataclass
class AddUserInput:
    name: str
    email: str
    title: str | None = None

@grommet.type
@dataclass
class User:
    name: str
    email: str
    title: str | None

    @grommet.field
    @staticmethod
    async def full_name(parent: "User", info: gm.Info) -> str:
        return f"{parent.title} {parent.name}" if parent.title else parent.name

@grommet.type
@dataclass
class Mutation:
    @grommet.field
    @staticmethod
    async def add_user(input: AddUserInput) -> User:
        return User(name=input.name, email=input.email, title=input.title)

schema = grommet.Schema(query=Query, mutation=Mutation)
```

Stream real-time data with subscriptions:

```python
from collections.abc import AsyncIterator

@grommet.type(name="Subscription")
@dataclass
class Subscription:
    @grommet.field
    @staticmethod
    async def counter(limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i

schema = grommet.Schema(query=Query, subscription=Subscription)

async def main():
    stream = schema.subscribe("subscription { counter(limit: 3) }")
    async for event in stream:
        print(event)
        # {'data': {'counter': 0}}
        # {'data': {'counter': 1}}
        # {'data': {'counter': 2}}

asyncio.run(main())
```

<!-- </AI_GENERATED> -->

## Development

The public APIs for this project are defined by me (a human). Everything else is AI-written following `AGENTS.md` and plan guidelines. Implementation iterations take the form of plan documents in `ai_plans/`.

This project is configured for uv + maturin:

```bash
uv pip install -e .
# or
maturin develop
```

Run unit tests with:

```bash
pytest
cargo test
```
