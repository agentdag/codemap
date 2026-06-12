//! The language-plugin boundary. One implementation (Go) exists now; a second
//! language will plug in here later without touching the cross-file linker.

use core_ir::{CallSite, Edge, Node};

use crate::error::ExtractError;

/// A raw, **unresolved** call reference collected from a function/method body.
/// Resolution into a `calls` edge happens later in [`crate::extract_repo`] against
/// the full node set — `extract_file` never resolves these (see ADR 0001).
#[derive(Debug, Clone)]
pub struct CallRef {
    /// Node id of the enclosing function/method the call appears in.
    pub caller_node_id: String,
    /// The called name (`New`, `Hello`, ...) — the selector's final segment.
    pub callee_name: String,
    /// The leading identifier of a `qualifier.Name(...)` call, if any. May be a
    /// package name (resolves to a Module) or a receiver variable — the resolver
    /// decides using the file's imports.
    pub package_qualifier: Option<String>,
    /// True when the call is syntactically a method call on a non-identifier
    /// expression (e.g. `a.b.Name()`), so it can never be a package call.
    pub is_method_call: bool,
    /// Where the call occurs (0-based).
    pub site: CallSite,
}

/// Per-file syntactic facts produced by a [`LanguageExtractor`]. Cross-file
/// work (containment tree, import/call resolution) stays in the repo-level linker
/// ([`crate::extract_repo`]); the one exception is [`FileExtraction::edges`],
/// which an extractor uses for relationships it can resolve *within* one file.
#[derive(Debug, Clone)]
pub struct FileExtraction {
    /// The `File` node for this source file.
    pub file_node: Node,
    /// Function/Method/Class/Interface/TypeAlias nodes defined in the file.
    /// Method nodes may carry their receiver type in `metadata["receiver"]`.
    pub symbols: Vec<Node>,
    /// Resolved edges the extractor determined lexically within this file (e.g.
    /// TypeScript `Class`→`Method` containment, where methods nest in the class).
    /// A node appearing as a `contains` target here is *already parented*, so the
    /// linker won't re-parent it. Languages without lexical nesting leave this
    /// empty (Go links methods to receivers in the linker instead).
    pub edges: Vec<Edge>,
    /// Raw import path strings exactly as written in source (no resolution).
    pub imports: Vec<String>,
    /// Name used for the directory's `Module` node (Go: package name; TS: dir name).
    pub package_name: String,
    /// Raw, unresolved call references found in this file's bodies.
    pub calls: Vec<CallRef>,
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
