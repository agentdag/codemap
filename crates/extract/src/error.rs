//! Error type for the extractor. Hand-written (the task's allowed-deps list does
//! not include `thiserror`); the CLI boundary maps these to its own reporting.

use std::fmt;
use std::path::PathBuf;

/// Anything that can go wrong while extracting a repository's IR.
#[derive(Debug)]
pub enum ExtractError {
    /// Filesystem read failure, with the offending path for context.
    Io { path: PathBuf, source: std::io::Error },
    /// `go.mod` was not found at the repo root.
    GoModMissing(PathBuf),
    /// `go.mod` exists but has no `module <path>` directive.
    GoModNoModule(PathBuf),
    /// The directory walk yielded an error entry.
    Walk(String),
    /// tree-sitter could not produce a parse tree for a file.
    Parse { relpath: String },
    /// tree-sitter language/grammar setup failed (build/ABI problem).
    Language(String),
}

impl fmt::Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractError::Io { path, source } => {
                write!(f, "reading {}: {source}", path.display())
            }
            ExtractError::GoModMissing(p) => write!(f, "no go.mod found at {}", p.display()),
            ExtractError::GoModNoModule(p) => {
                write!(f, "go.mod at {} has no `module` directive", p.display())
            }
            ExtractError::Walk(msg) => write!(f, "walking repository: {msg}"),
            ExtractError::Parse { relpath } => write!(f, "failed to parse {relpath}"),
            ExtractError::Language(msg) => write!(f, "tree-sitter language setup failed: {msg}"),
        }
    }
}

impl std::error::Error for ExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExtractError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
