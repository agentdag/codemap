//! The IR document: ground-truth nodes + edges plus provenance, and its
//! canonical (deterministic) serialization.

use serde::{Deserialize, Serialize};

use crate::edge::Edge;
use crate::node::Node;

/// Schema version this crate implements (see `docs/specs/ir.md`).
pub const SCHEMA_VERSION: &str = "0.1.0";

/// Provenance for a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub repo_path: String,
    /// ISO8601 timestamp of analysis.
    pub analyzed_at: String,
    pub tool_version: String,
}

/// The whole IR document: ground-truth nodes + edges plus provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IrDocument {
    pub schema_version: String,
    pub root: RootInfo,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl IrDocument {
    /// Serialize to canonical JSON: nodes sorted by id, edges sorted by id, and
    /// (via `BTreeMap`/serde_json) all map keys sorted. The same logical
    /// document always produces byte-identical output, independent of the order
    /// nodes/edges were supplied.
    ///
    /// Serialization of this fully-owned, string-keyed model is infallible, so
    /// this returns `String` directly. Use [`IrDocument::try_to_canonical_json`]
    /// if you want the error surfaced instead.
    pub fn to_canonical_json(&self) -> String {
        self.try_to_canonical_json()
            .expect("IrDocument serialization is infallible for this data model")
    }

    /// Fallible form of [`IrDocument::to_canonical_json`].
    pub fn try_to_canonical_json(&self) -> Result<String, serde_json::Error> {
        let mut canonical = self.clone();
        canonical.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        canonical.edges.sort_by(|a, b| a.id.cmp(&b.id));
        serde_json::to_string(&canonical)
    }

    /// Parse a document from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
