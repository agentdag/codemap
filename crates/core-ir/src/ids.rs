//! Readable, logical id constructors for nodes and edges (see `docs/specs/ir.md`).
//!
//! These are inherent methods on [`Node`] and [`Edge`], so the public paths
//! `Node::make_id` / `Edge::make_id` are unchanged by the module split.

use crate::edge::{Edge, EdgeKind};
use crate::node::{Node, NodeKind};

impl Node {
    /// Build the canonical, readable node id per the spec scheme:
    /// - `File`   -> `file:<relpath>`
    /// - `Module` -> `module:<relpath-or-package>`
    /// - symbol   -> `<lang>:<relpath>#<qualifiedName>`
    ///
    /// `File`/`Module` ignore `qualified_name` (it is empty for them) and
    /// `language`. `language` is required for symbol ids because the spec's
    /// symbol id format embeds `<lang>`.
    pub fn make_id(
        kind: NodeKind,
        language: &str,
        relpath: &str,
        qualified_name: &str,
    ) -> String {
        match kind {
            NodeKind::File => format!("file:{relpath}"),
            NodeKind::Module => format!("module:{relpath}"),
            NodeKind::Function
            | NodeKind::Method
            | NodeKind::Class
            | NodeKind::Interface
            | NodeKind::TypeAlias
            | NodeKind::Variable => {
                format!("{language}:{relpath}#{qualified_name}")
            }
        }
    }
}

impl Edge {
    /// Build the canonical edge id: `<sourceId>=[<kind>]=><targetId>`.
    pub fn make_id(source: &str, kind: EdgeKind, target: &str) -> String {
        format!("{source}=[{}]=>{target}", kind.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_is_deterministic_and_matches_spec_scheme() {
        assert_eq!(
            Node::make_id(NodeKind::File, "ts", "src/a.ts", ""),
            "file:src/a.ts"
        );
        assert_eq!(
            Node::make_id(NodeKind::Module, "ts", "src/a.ts", ""),
            "module:src/a.ts"
        );
        assert_eq!(
            Node::make_id(NodeKind::Method, "ts", "src/users/user.service.ts", "UserService.create"),
            "ts:src/users/user.service.ts#UserService.create"
        );
        // Same inputs -> same id.
        assert_eq!(
            Node::make_id(NodeKind::Class, "py", "a/b.py", "Foo"),
            Node::make_id(NodeKind::Class, "py", "a/b.py", "Foo"),
        );
    }

    #[test]
    fn edge_id_is_deterministic_and_matches_spec_scheme() {
        let id = Edge::make_id("ts:src/a.ts#A.b", EdgeKind::Calls, "ts:src/a.ts#C");
        assert_eq!(id, "ts:src/a.ts#A.b=[calls]=>ts:src/a.ts#C");
        assert_eq!(
            id,
            Edge::make_id("ts:src/a.ts#A.b", EdgeKind::Calls, "ts:src/a.ts#C")
        );
    }
}
