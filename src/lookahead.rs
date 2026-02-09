use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::dynamic::ResolverContext;
use pyo3::prelude::*;

/// Lightweight owned snapshot of a selection set level, shared via `Arc` so
/// that `peek()` never deep-clones.
struct SelectionNode {
    children: HashMap<String, Arc<SelectionNode>>,
}

impl SelectionNode {
    fn empty() -> Arc<Self> {
        Arc::new(Self {
            children: HashMap::new(),
        })
    }
}

#[pyclass(module = "grommet._core", name = "Graph", frozen, from_py_object)]
#[derive(Clone)]
pub(crate) struct Graph {
    node: Arc<SelectionNode>,
}

#[pymethods]
impl Graph {
    fn requests(&self, name: &str) -> bool {
        self.node.children.contains_key(name)
    }

    fn peek(&self, name: &str) -> Graph {
        Graph {
            node: self
                .node
                .children
                .get(name)
                .cloned()
                .unwrap_or_else(SelectionNode::empty),
        }
    }
}

const MAX_DEPTH: u32 = 32;

pub(crate) fn extract_graph(ctx: &ResolverContext<'_>) -> Graph {
    let current = ctx.ctx.field();
    Graph {
        node: build_node(current.selection_set(), 0),
    }
}

fn build_node<'a>(
    fields: impl Iterator<Item = async_graphql::SelectionField<'a>>,
    depth: u32,
) -> Arc<SelectionNode> {
    if depth >= MAX_DEPTH {
        return SelectionNode::empty();
    }
    let mut children = HashMap::new();
    for field in fields {
        let name = field.name().to_string();
        let child = build_node(field.selection_set(), depth + 1);
        children.insert(name, child);
    }
    Arc::new(SelectionNode { children })
}
