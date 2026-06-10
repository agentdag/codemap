//! The language-plugin boundary. One implementation (Go) exists now; a second
//! language will plug in here later without touching the cross-file linker.

use core_ir::Node;

use crate::error::ExtractError;

/// Per-file syntactic facts produced by a [`LanguageExtractor`]. This is
/// deliberately *local*: no cross-file linking, no edges. The repo-level linker
/// ([`crate::extract_repo`]) turns these into a connected [`core_ir::IrDocument`].
#[derive(Debug, Clone)]
pub struct FileExtraction {
    /// The `File` node for this source file.
    pub file_node: Node,
    /// Function/Method/Class/Interface/TypeAlias nodes defined in the file.
    /// Method nodes carry their receiver type in `metadata["receiver"]`.
    pub symbols: Vec<Node>,
    /// Raw import path strings exactly as written in source (no resolution).
    pub imports: Vec<String>,
    /// The declared package name (used to name the directory's `Module` node).
    pub package_name: String,
}

/// A pluggable, per-language structural extractor.
pub trait LanguageExtractor {
    /// IR `language` tag emitted on nodes (e.g. `"go"`).
    fn language(&self) -> &'static str;

    /// File extensions (without the dot) this extractor handles (e.g. `["go"]`).
    fn extensions(&self) -> &'static [&'static str];

    /// Extract per-file syntactic facts. Returns a `Result` (deviating from the
    /// task's bare `FileExtraction`) because parsing is fallible and the project
    /// rules forbid `unwrap()` / require surfacing errors — see ADR 0001.
    fn extract_file(&self, relpath: &str, source: &str) -> Result<FileExtraction, ExtractError>;
}
