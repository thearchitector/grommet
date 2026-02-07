use std::collections::HashMap;

use async_graphql::dynamic::ResolverContext;
use pyo3::prelude::*;

#[pyclass(module = "grommet._core", name = "Lookahead", from_py_object)]
#[derive(Clone)]
pub(crate) struct Lookahead {
    exists: bool,
    children: HashMap<String, Lookahead>,
}

#[pymethods]
impl Lookahead {
    fn exists(&self) -> bool {
        self.exists
    }

    fn field(&self, name: &str) -> Lookahead {
        self.children
            .get(name)
            .cloned()
            .unwrap_or_else(|| Lookahead {
                exists: false,
                children: HashMap::new(),
            })
    }
}

impl Lookahead {
    fn empty() -> Self {
        Self {
            exists: false,
            children: HashMap::new(),
        }
    }
}

const MAX_DEPTH: u32 = 32;

pub(crate) fn extract_lookahead(ctx: &ResolverContext<'_>) -> Lookahead {
    let current = ctx.ctx.field();
    build_from_selection(current.selection_set(), 0)
}

fn build_from_selection<'a>(
    fields: impl Iterator<Item = async_graphql::SelectionField<'a>>,
    depth: u32,
) -> Lookahead {
    if depth >= MAX_DEPTH {
        return Lookahead::empty();
    }
    let mut children = HashMap::new();
    for field in fields {
        let name = field.name().to_string();
        let child = build_from_selection(field.selection_set(), depth + 1);
        children.insert(
            name,
            Lookahead {
                exists: true,
                children: child.children,
            },
        );
    }
    Lookahead {
        exists: true,
        children,
    }
}
