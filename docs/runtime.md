# Runtime Configuration

Grommet uses a Tokio runtime for async execution. You can configure this runtime before creating schemas.

## Default Runtime

By default, Grommet uses a multi-threaded Tokio runtime. This is suitable for most applications.

## Configuring the Runtime

Use `gm.configure_runtime()` to customize the runtime:

```python
import grommet as gm

# Configure before creating any schemas
gm.configure_runtime(use_current_thread=True)

# Now create your schema
schema = gm.Schema(query=Query)
```

!!! warning
    `configure_runtime()` must be called before creating any `Schema` instances. Calling it after schema creation has no effect.

## Options

### use_current_thread

Run the Tokio runtime on the current thread instead of spawning worker threads:

```python
gm.configure_runtime(use_current_thread=True)
```

This is useful for:

- Single-threaded environments
- Debugging
- Reducing resource usage in simple applications

### worker_threads

Set the number of worker threads for the multi-threaded runtime:

```python
gm.configure_runtime(worker_threads=4)
```

!!! note
    `worker_threads` cannot be used with `use_current_thread=True`. This combination raises an error.

## Examples

### Single-Threaded Runtime

```python
import grommet as gm

gm.configure_runtime(use_current_thread=True)
```

### Custom Thread Count

```python
import grommet as gm

# Use 8 worker threads
gm.configure_runtime(worker_threads=8)
```

### Default Multi-Threaded Runtime

```python
import grommet as gm

# Explicitly use defaults (multi-threaded with auto thread count)
gm.configure_runtime()
```

## When to Configure

Configure the runtime at application startup, before any GraphQL operations:

```python
# app.py
import grommet as gm
from myapp.schema import Query

# Configure runtime first
gm.configure_runtime(worker_threads=4)

# Then create schema
schema = gm.Schema(query=Query)
```

## Integration with Web Frameworks

### FastAPI

```python
from contextlib import asynccontextmanager

import grommet as gm
from fastapi import FastAPI


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Configure runtime at startup
    gm.configure_runtime(worker_threads=4)
    yield


app = FastAPI(lifespan=lifespan)
```

### Starlette

```python
import grommet as gm
from starlette.applications import Starlette

# Configure at module load
gm.configure_runtime(worker_threads=4)

app = Starlette()
```

## Return Value

`configure_runtime()` returns `True` on success:

```python
success = gm.configure_runtime(use_current_thread=True)
assert success is True
```

## Errors

### Invalid Configuration

```python
# Raises GrommetTypeError
gm.configure_runtime(use_current_thread=True, worker_threads=4)
# Error: worker_threads cannot be set for a current-thread runtime
```

## Performance Considerations

- **Multi-threaded (default)**: Best for production with concurrent requests
- **Current-thread**: Lower overhead, suitable for testing or single-request scenarios
- **Custom thread count**: Tune based on your workload and available cores
