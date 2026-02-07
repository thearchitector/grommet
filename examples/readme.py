# pragma: no ai
import asyncio
from collections.abc import AsyncIterator
from dataclasses import dataclass

import grommet


@grommet.type
@dataclass
class Query:
    @grommet.field
    async def greeting(self) -> str:
        return "Hello, world!"


async def test_basic():

    schema = grommet.Schema(query=Query)
    result = await schema.execute("{ greeting }")
    assert result.data == {"greeting": "Hello, world!"}


async def test_args():
    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def hello(self, name: str, title: str | None = None) -> str:
            return f"Hello, {name}!" if not title else f"Hello, {title} {name}."

    schema = grommet.Schema(query=Query)
    result = await schema.execute('{ hello(name: "grommet") }')
    assert result.data == {"hello": "Hello, grommet!"}

    result = await schema.execute('{ hello(name: "Gromit", title: "Mr.") }')
    assert result.data == {"hello": "Hello, Mr. Gromit."}


async def test_mutation():
    @grommet.input
    @dataclass
    class AddUserInput:
        name: str
        title: str | None = None

    @grommet.type
    @dataclass
    class User:
        name: str
        title: str | None

        @grommet.field
        async def full_name(self) -> str:
            return f"{self.title} {self.name}" if self.title else self.name

    @grommet.type
    @dataclass
    class Mutation:
        @grommet.field
        async def add_user(self, input: AddUserInput) -> User:
            return User(name=input.name, title=input.title)

    schema = grommet.Schema(query=Query, mutation=Mutation)
    result = await schema.execute(
        """
        mutation ($name: String!) {
            add_user(input: { name: $name, title: "Mr." }) {
                full_name
            }
        }
        """,
        variables={"name": "Gromit"},
    )
    assert result.data == {"add_user": {"full_name": "Mr. Gromit"}}, result.errors


async def test_subscription():

    @grommet.type
    @dataclass
    class Subscription:
        @grommet.field
        async def counter(self, limit: int) -> AsyncIterator[int]:
            for i in range(limit):
                yield i

    schema = grommet.Schema(query=Query, subscription=Subscription)
    stream = await schema.execute("subscription { counter(limit: 3) }")
    expected = iter(range(3))
    async for result in stream:
        assert result.data == {"counter": next(expected)}


async def test_state():
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
    result = await schema.execute("{ greeting }", state=MyState(request_id="123"))
    assert result.data == {"greeting": "Hello request 123!"}


async def test_peek():

    @grommet.type
    @dataclass
    class SubObject:
        @grommet.field
        async def b(self) -> str:
            return "foo"

    @grommet.type
    @dataclass
    class Object:
        @grommet.field
        async def a(self) -> int:
            return 1

        @grommet.field
        async def sub(self) -> SubObject:
            return SubObject()

    @grommet.type
    @dataclass
    class Query:
        @grommet.field
        async def obj(self, context: grommet.Context) -> Object:
            nonlocal requests_a
            nonlocal requests_b
            requests_a = context.field("a").exists()
            requests_b = context.look_ahead().field("sub").field("b").exists()
            return Object()

    requests_a = None
    requests_b = None
    schema = grommet.Schema(query=Query)
    await schema.execute("{ obj { a } }")
    assert requests_a
    assert not requests_b

    requests_a = None
    requests_b = None
    await schema.execute("{ obj { sub { b } } }")
    assert not requests_a
    assert requests_b


async def main():
    await test_basic()
    await test_args()
    await test_mutation()
    await test_subscription()
    await test_state()
    await test_peek()


asyncio.run(main())
