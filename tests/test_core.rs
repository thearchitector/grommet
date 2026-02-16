#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::needless_return)]
#![allow(clippy::redundant_clone)]

use pyo3::prelude::*;

fn with_py<F, R>(f: F) -> R
where
    F: for<'py> FnOnce(Python<'py>) -> R,
{
    use std::sync::Once;
    static INIT: Once = Once::new();

    Python::initialize();
    INIT.call_once(|| {
        Python::attach(|py| {
            py.run(
                pyo3::ffi::c_str!(
                    r#"
import os
import sys

def candidate_paths(base: str) -> list[str]:
    version = f"python{sys.version_info.major}.{sys.version_info.minor}"
    abi = getattr(sys, "abiflags", "")
    candidates = [f"{version}{abi}", f"{version}t", version]
    return [os.path.join(base, "lib", candidate, "site-packages") for candidate in candidates]

search_roots = []
venv = os.environ.get("VIRTUAL_ENV")
if venv:
    search_roots.append(venv)
search_roots.append(os.path.join(os.getcwd(), ".venv"))

for root in search_roots:
    for path in candidate_paths(root):
        if os.path.isdir(path) and path not in sys.path:
            sys.path.insert(0, path)
"#
                ),
                None,
                None,
            )
            .unwrap();
            py.import("asyncio").unwrap();
            py.import("grommet.plan").unwrap();
        });
    });

    Python::attach(f)
}

mod errors {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/errors.rs"));
}

mod types {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/types.rs"));
}

mod values {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/values.rs"));
}

mod resolver {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/resolver.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::task::{Context, Poll, Waker};

        use pyo3::types::PyDict;

        fn noop_waker() -> Waker {
            Waker::noop().clone()
        }

        /// Ensures custom awaitable bridge reports missing event loop errors.
        #[test]
        fn awaitable_bridge_requires_running_loop() {
            let mut future = crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
import asyncio
async def coro():
    return 7
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();
                let coro = locals.get_item("coro").unwrap().unwrap();
                let awaitable = coro.call0().unwrap();
                awaitable_into_future(awaitable)
            });

            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);
            assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));

            match future.as_mut().poll(&mut cx) {
                Poll::Ready(Err(err)) => {
                    let message = crate::with_py(|py| {
                        err.value(py).str().unwrap().to_str().unwrap().to_string()
                    });
                    assert!(
                        message.to_ascii_lowercase().contains("event loop"),
                        "unexpected error: {message}"
                    );
                }
                _ => panic!("expected missing running event loop error"),
            }
        }

        /// Ensures async-iterator detection accepts objects with __anext__.
        #[test]
        fn subscription_iterator_accepts_anext_only_object() {
            crate::with_py(|py| {
                let locals = PyDict::new(py);
                py.run(
                    pyo3::ffi::c_str!(
                        r#"
class IterOnly:
    async def __anext__(self):
        return 1
"#
                    ),
                    None,
                    Some(&locals),
                )
                .unwrap();

                let cls = locals.get_item("IterOnly").unwrap().unwrap();
                let value = cls.call0().unwrap();
                let _ = subscription_iterator(&value).unwrap();
            });
        }

        /// Ensures async-iterator detection rejects non-iterator objects.
        #[test]
        fn subscription_iterator_rejects_non_async_iterators() {
            crate::with_py(|py| {
                let none = py.None();
                let value = none.bind(py);
                let err = match subscription_iterator(&value) {
                    Ok(_) => panic!("expected iterator error"),
                    Err(err) => err,
                };
                let message = err.value(py).str().unwrap().to_str().unwrap().to_string();
                assert!(message.contains("async iterator"));
            });
        }
    }
}
