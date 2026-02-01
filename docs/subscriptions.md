# Subscriptions

Subscriptions enable real-time updates by streaming data to clients. They're ideal for notifications, live feeds, and collaborative features.

## Defining Subscriptions

Create a subscription type with async generator resolvers:

```python
from dataclasses import dataclass
from typing import TYPE_CHECKING

import grommet as gm

if TYPE_CHECKING:
    from collections.abc import AsyncIterator


@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def countdown(parent, info, start: int) -> "AsyncIterator[int]":
        for i in range(start, 0, -1):
            yield i
            await asyncio.sleep(1)
```

## Creating a Schema with Subscriptions

```python
schema = gm.Schema(
    query=Query,
    subscription=Subscription,
)
```

## Subscribing

Use `schema.subscribe()` to get a stream:

```python
stream = schema.subscribe(
    "subscription { countdown(start: 5) }"
)

async for payload in stream:
    print(payload["data"]["countdown"])
    # Prints: 5, 4, 3, 2, 1
```

## Subscription Stream

The stream returned by `subscribe()` is an async iterator with these methods:

```python
stream = schema.subscribe("subscription { countdown(start: 3) }")

# Async iteration
async for payload in stream:
    print(payload)

# Manual iteration
payload = await stream.__anext__()

# Close the stream early
await stream.aclose()
```

## Subscription Arguments

Like queries, subscriptions can have arguments:

```python
@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def messages(
        parent, info, channel: str
    ) -> "AsyncIterator[Message]":
        async for message in message_queue.subscribe(channel):
            yield message
```

```graphql
subscription {
    messages(channel: "general") {
        id
        text
        author
    }
}
```

## Variables

Pass variables to subscriptions:

```python
stream = schema.subscribe(
    """
    subscription ($channel: String!) {
        messages(channel: $channel) {
            text
        }
    }
    """,
    variables={"channel": "general"},
)
```

## Context and Root

Like queries, subscriptions can receive context and root values:

```python
stream = schema.subscribe(
    "subscription { notifications }",
    context={"user_id": "123"},
    root={"tenant": "acme"},
)
```

Access them in resolvers:

```python
@gm.field
@staticmethod
async def notifications(parent, info) -> "AsyncIterator[Notification]":
    user_id = info.context["user_id"]
    async for notification in get_user_notifications(user_id):
        yield notification
```

## Real-World Examples

### Chat Messages

```python
from asyncio import Queue


message_queues: dict[str, Queue] = {}


@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def chat_messages(
        parent, info, room: str
    ) -> "AsyncIterator[ChatMessage]":
        queue = message_queues.setdefault(room, Queue())
        while True:
            message = await queue.get()
            yield message
```

### Live Updates

```python
@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def order_status(
        parent, info, order_id: gm.ID
    ) -> "AsyncIterator[OrderStatus]":
        async for status in watch_order_status(order_id):
            yield status
            if status.is_final:
                break
```

### Periodic Updates

```python
import asyncio


@gm.type
@dataclass
class Subscription:
    @gm.field
    @staticmethod
    async def server_time(parent, info) -> "AsyncIterator[str]":
        while True:
            yield datetime.now().isoformat()
            await asyncio.sleep(1)
```

## Handling Errors

Errors in subscription resolvers are yielded as error payloads:

```python
@gm.field
@staticmethod
async def risky_stream(parent, info) -> "AsyncIterator[int]":
    for i in range(5):
        if i == 3:
            raise ValueError("Something went wrong!")
        yield i
```

Client receives:

```python
# First three payloads
{"data": {"riskyStream": 0}}
{"data": {"riskyStream": 1}}
{"data": {"riskyStream": 2}}
# Error payload
{"data": {"riskyStream": null}, "errors": [...]}
```

## Closing Subscriptions

Always close subscriptions when done to release resources:

```python
stream = schema.subscribe("subscription { updates }")

try:
    async for payload in stream:
        if should_stop(payload):
            break
finally:
    await stream.aclose()
```

Or use async context managers:

```python
stream = schema.subscribe("subscription { updates }")
async for payload in stream:
    process(payload)
    if done:
        await stream.aclose()
        break
```

## Return Type Annotations

Subscription resolvers must have an `AsyncIterator` return type:

```python
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

@gm.field
@staticmethod
async def events(parent, info) -> "AsyncIterator[Event]":
    ...
```

!!! note
    Use `TYPE_CHECKING` to avoid runtime import of `AsyncIterator`.

## Nullable Subscriptions

For subscriptions that might yield `None`:

```python
@gm.field
@staticmethod
async def maybe_events(parent, info) -> "AsyncIterator[Event | None]":
    async for event in event_stream():
        yield event if event.is_valid else None
```
