//! Source spans and locations.

use serde::{Deserialize, Serialize};

/// A half-open-agnostic source span. Lines and columns are extractor-defined
/// (Phase 0 does not mandate 0- vs 1-based; it only round-trips the values).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Where a node lives: the file it belongs to and its span within that file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub range: Range,
}
