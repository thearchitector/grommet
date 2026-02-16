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
    greeting: str = "Hello world!"


schema = grommet.Schema(query=Query)
result = await schema.execute("{ greeting }")
print(result.data)  # {'greeting': 'Hello world!'}
```

Add descriptions to types and fields for better SDL:

```python
@grommet.type(description="All queries")
@dataclass
class Query:
    greeting: Annotated[str, grommet.Field(description="A simple greeting") = "Hello world!"

sdl = grommet.Schema(query=Query).sdl
print(sdl)
# """
# All queries
# """
# query Query {
#   "A simple greeting"
#   greeting: String!
# }
```

Root types (`Query`, `Mutation`, `Subscription`) cannot have fields without defaults. Use `grommet.field` to define
fields using resolvers to dynamically return values, possibly with required and optional arguments:

```python
@grommet.type
@dataclass
class Query:
    @grommet.field(description="A simple greeting")
    async def greeting(self, name: str, title: str | None = None) -> str:
        return f"Hello {name}!" if not title else f"Hello, {title} {name}."


schema = grommet.Schema(query=Query)
result = await schema.execute('{ greeting(name: "Gromit") }')
print(result.data)  # {'greeting': 'Hello Gromit!'}

result = await schema.execute('{ greeting(name: "Gromit", title: "Mr.") }')
print(result.data)  # {'greeting': 'Hello Mr. Gromit.'}
```

Limit what fields are exposed to the schema via `grommet.Hidden`, `ClassVar`, or the standard `_private_var` syntax:

```python
@grommet.type
@dataclass
class User:
    _foo: int
    bar: ClassVar[int]
    hidden: Annotated[int, grommet.Hidden]

    name: str

    def _message(self) -> str:
        return f"Hello {self.name}" + ("!" * self._foo * self.bar * self.hidden)

    @grommet.field
    async def greeting(self) -> str:
        return self._message()


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def user(self, name: str) -> User:
        return User(_foo=2, bar=2, hidden=2, name=name)


schema = grommet.Schema(query=Query)
result = await schema.execute('{ user(name: "Gromit") { greeting } }')
print(result.data)  # {'user': {'greeting': 'Hello Gromit!!!!!!'}}
```

Add mutations by defining a separate mutation root type, passing `variables`:

```python
@grommet.input(description="User input.")
@dataclass
class AddUserInput:
    name: Annotated[str, grommet.Field(description="The name of the user.")]
    title: Annotated[
        str | None, grommet.Field(description="The title of the user, if any.")
    ]


@grommet.type
@dataclass
class User:
    name: str
    title: str | None

    @grommet.field
    async def greeting(self) -> str:
        return (
            f"Hello {self.name}!"
            if not self.title
            else f"Hello, {self.title} {self.name}."
        )


@grommet.type
@dataclass
class Mutation:
    @grommet.field
    async def add_user(self, input: AddUserInput) -> User:
        return User(name=input.name, title=input.title)


schema = grommet.Schema(query=Query, mutation=Mutation)
mutation = """
    mutation ($name: String!, $title: String) {
        add_user(input: { name: $name, title: $title }) { greeting }
    }
"""
result = await schema.execute(mutation, variables={"name": "Gromit"})
print(result.data)  # {'add_user': {'greeting': 'Hello Gromit!'}}

result = await schema.execute(mutation, variables={"name": "Gromit", "title": "Mr."})
print(result.data)  # {'add_user': {'greeting': 'Hello Mr. Gromit.'}}
```

Stream real-time data with subscriptions:

```python
from collections.abc import AsyncIterator


@grommet.type
@dataclass
class Subscription:
    @grommet.subscription
    async def counter(self, limit: int) -> AsyncIterator[int]:
        for i in range(limit):
            yield i


schema = grommet.Schema(query=Query, subscription=Subscription)
stream = await schema.execute("subscription { counter(limit: 3) }")
async for result in stream:
    print(result.data)
    # {'counter': 0}
    # {'counter': 1}
    # {'counter': 2}
```

Store and access arbitrary information using the operation state:

```python
@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(
        self, context: Annotated[dict[str, str], grommet.Context]
    ) -> str:
        return f"Hello request {context['request_id']}!"


schema = grommet.Schema(query=Query)
result = await schema.execute("{ greeting }", context={"request_id": "123"})
print(result.data)  # {'greeting': 'Hello request 123!'}
```

Define unions, optionally providing a name or description:

```python
@grommet.type
@dataclass
class A:
    a: int


@grommet.type
@dataclass
class B:
    b: int


type NamedAB = Annotated[A | B, grommet.Union(name="NamedAB", description="A or B")]


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def named(self, type: str) -> NamedAB:
        return A(a=1) if type == "A" else B(b=2)

    @grommet.field
    async def unnamed(self, type: str) -> A | B:
        return A(a=1) if type == "A" else B(b=2)


schema = grommet.Schema(query=Query)
print("union NamedAB" in schema.sdl)  # True
## if a name is not explicitly set, grommet will concatenate all the member names
print("union AB" in schema.sdl)  # True

result = await schema.execute('{ named(type: "A") { ... on A { a } ... on B { b } } }')
print(result.data)  # {'named': {'a': 1}}

result = await schema.execute(
    '{ unnamed(type: "B") { ... on A { a } ... on B { b } } }'
)
print(result.data)  # {'unnamed': {'b': 2}}
```

Simplify unions through common interfaces:

```python
@grommet.interface(description="A letter")
@dataclass
class Letter:
    letter: str


@grommet.type
@dataclass
class A(Letter):
    pass


@grommet.type
@dataclass
class B(Letter):
    some_subfield: list[int]


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def common(self, type: str) -> Letter:
        return A(letter="A") if type == "A" else B(letter="B", some_subfield=[42])


schema = grommet.Schema(query=Query)
print(schema.sdl)
# """
# A letter
# """
# interface Letter {
#   letter: String!
# }
#
# type A implements Letter {
#   letter: String!
# }
#
# type B implements Letter {
#   letter: String!
#   some_subfield: [Int!]!
# }
#
# type Query {
#   common(type: String!): Letter!
# }
```

## Development

The public APIs for this project are defined by me (a human). Everything else is AI-written following `AGENTS.md` and plan guidelines. Implementation iterations take the form of plan documents in `ai_plans/`.

This project is configured for uv + maturin.

Install `prek` for quality control:
```bash
prek install
prek run -a
```

Run unit tests with:

```bash
maturin develop --uv
uv run pytest
uv run cargo test  # you need to be in the venv!
```

Run benchmarks with:

```bash
maturin develop --uv -r
uv run python benchmarks/bench_large.py
```
