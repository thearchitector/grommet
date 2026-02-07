<!-- pragma: no ai -->
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

Define your GraphQL types as decorated dataclasses, build a schema, and execute queries:

```python
@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(self) -> str:
        return "Hello, world!"

schema = grommet.Schema(query=Query)
result = asyncio.run(schema.execute("{ greeting }"))
print(result)  # {'data': {'greeting': 'Hello, world!'}}
```

Use `grommet.field` to define fields from resolvers with required and optional arguments:

```python
@grommet.type
@dataclass
class Query:
    @grommet.field
    async def hello(self, name: str, title: str | None = None) -> str:
        return f"Hello, {name}!" if not title else f"Hello, {title} {name}."

schema = grommet.Schema(query=Query)
result = asyncio.run(schema.execute('{ hello(name: "grommet") }'))
print(result)  # {'data': {'hello': 'Hello, grommet!'}}

result = asyncio.run(schema.execute('{ hello(name: "Gromit", title: "Mr.") }'))
print(result)  # {'data': {'hello': 'Hello, Mr. Gromit!'}}
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
    async def full_name(self) -> str:
        return f"{self.title} {self.name}" if self.title else self.name

@grommet.type
@dataclass
class Mutation:
    @grommet.field
    async def add_user(self, input: AddUserInput) -> User:
        return User(name=input.name, email=input.email, title=input.title)

schema = grommet.Schema(query=Query, mutation=Mutation)
```

Stream real-time data with subscriptions:

```python
from collections.abc import AsyncIterator

@grommet.type
@dataclass
class Subscription:
    @grommet.field
    async def counter(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i

schema = grommet.Schema(query=Query, subscription=Subscription)

async def main():
    stream = await schema.execute("subscription { counter(limit: 3) }")
    async for result in stream:
        print(result.data)
        # {'counter': 0}
        # {'counter': 1}
        # {'counter': 2}

asyncio.run(main())
```

Store arbitrary operation state using custom context state:

```python
@dataclass
class MyState:
    request_id: str

@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(self, context: grommet.Context[MyState]) -> str:
        return f"Hello request {context.state.request_id}!"

schema = grommet.Schema(query=Query)
result = asyncio.run(schema.execute("{ greeting }", state=MyState(request_id="123")))
print(result)  # {'data': {'greeting': 'Hello request 123!'}}
```

Analyze the current operation by peeking into the execution context:

```python
@grommet.type
@dataclass
class SubObject:
    @grommet.field
    def b(self) -> str:
        return "foo"

@grommet.type
@dataclass
class Object:
    @grommet.field
    def a(self) -> int:
        return 1

    @grommet.field
    def sub(self) -> SubObject:
        return SubObject()

@grommet.type
@dataclass
class Query:
    @grommet.field
    async def obj(self, context: grommet.Context) -> Object:
        print("requests a:", context.field("a").exists())
        print("requests b:", context.look_ahead().field("sub").field("b").exists())
        return Object()

schema = grommet.Schema(query=Query)
result = asyncio.run(schema.execute("{ obj { a } }"))
# >>> requests a: True
# >>> requests b: False

result = asyncio.run(schema.execute("{ obj { sub { b } } }"))
# >>> requests a: False
# >>> requests b: True
```

## Development

The public APIs for this project are defined by me (a human). Everything else is AI-written following `AGENTS.md` and plan guidelines. Implementation iterations take the form of plan documents in `ai_plans/`.

This project is configured for uv + maturin.

Run unit tests with:

```bash
maturin develop --uv
uv run pytest
uv run cargo test  # you need to be in the venv!
```
