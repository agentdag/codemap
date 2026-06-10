//! Edges: relationships between nodes (containment, imports, calls).

use serde::{Deserialize, Serialize};

use crate::location::Range;
use crate::node::Metadata;

/// What a relationship represents. Phase 0; extensible.
///
/// `contains` edges form a strict tree; `imports`/`calls` are the cross-cutting
/// graph on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
}

impl EdgeKind {
    /// Canonical wire string (matches the serde representation). Used to build
    /// edge ids of the form `<source>=[<kind>]=><target>`.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Imports => "imports",
            EdgeKind::Calls => "calls",
        }
    }
}

/// How much to trust an edge.
///
/// CARDINAL RULE: a `calls` edge produced by Phase-0 tree-sitter is
/// [`Confidence::Heuristic`]. Never label a guess `Resolved`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Syntactic/semantic fact (containment, imports; later LSP-resolved calls).
    Resolved,
    /// Inferred by name/scope matching (Phase-0 calls, pre-LSP).
    Heuristic,
    /// Human-authored (from the overlay; never produced by extractors).
    Asserted,
}

/// One concrete location backing an edge (e.g. a single call expression).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSite {
    pub file: String,
    pub range: Range,
}

/// A relationship between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub sites: Vec<CallSite>,
    pub metadata: Metadata,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;

    #[test]
    fn confidence_serializes_to_spec_strings() {
        assert_eq!(serde_json::to_string(&Confidence::Resolved).unwrap(), "\"resolved\"");
        assert_eq!(serde_json::to_string(&Confidence::Heuristic).unwrap(), "\"heuristic\"");
        assert_eq!(serde_json::to_string(&Confidence::Asserted).unwrap(), "\"asserted\"");

        assert_eq!(
            serde_json::from_str::<Confidence>("\"resolved\"").unwrap(),
            Confidence::Resolved
        );
        assert_eq!(
            serde_json::from_str::<Confidence>("\"heuristic\"").unwrap(),
            Confidence::Heuristic
        );
        assert_eq!(
            serde_json::from_str::<Confidence>("\"asserted\"").unwrap(),
            Confidence::Asserted
        );
    }

    #[test]
    fn edge_and_node_kinds_serialize_to_spec_strings() {
        assert_eq!(serde_json::to_string(&EdgeKind::Contains).unwrap(), "\"contains\"");
        assert_eq!(serde_json::to_string(&EdgeKind::Imports).unwrap(), "\"imports\"");
        assert_eq!(serde_json::to_string(&EdgeKind::Calls).unwrap(), "\"calls\"");
        // EdgeKind::as_str matches the serde wire form (id construction relies on it).
        assert_eq!(EdgeKind::Calls.as_str(), "calls");

        assert_eq!(serde_json::to_string(&NodeKind::File).unwrap(), "\"File\"");
        assert_eq!(serde_json::to_string(&NodeKind::TypeAlias).unwrap(), "\"TypeAlias\"");
        assert_eq!(
            serde_json::from_str::<NodeKind>("\"Interface\"").unwrap(),
            NodeKind::Interface
        );
    }
}
