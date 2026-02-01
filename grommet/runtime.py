from . import _core


def configure_runtime(
    *, use_current_thread: bool = False, worker_threads: int | None = None
) -> bool:
    """Configures the Tokio runtime used for async execution."""

    return _core.configure_runtime(use_current_thread, worker_threads)
