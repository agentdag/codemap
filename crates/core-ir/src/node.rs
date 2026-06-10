//! Nodes: the things in code (files, modules, symbols).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::location::Location;

/// Free-form metadata map. `BTreeMap` keeps keys sorted, which is part of the
/// determinism guarantee. `serde_json::Value` is itself key-sorted by default
/// (serde_json sorts object keys unless the `preserve_order` feature is on).
pub type Metadata = BTreeMap<String, serde_json::Value>;

/// What a node represents. Phase 0; extensible without breaking existing ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Module,
    Function,
    Method,
    Class,
    Interface,
    TypeAlias,
    Variable,
}

/// A thing in the code: a file, module, or symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Readable logical id (see [`Node::make_id`]).
    pub id: String,
    pub kind: NodeKind,
    /// Short display name.
    pub name: String,
    pub qualified_name: String,
    pub language: String,
    pub location: Location,
    /// blake3 of the node's source slice (file contents for `File` nodes).
    pub content_hash: String,
    pub metadata: Metadata,
}
