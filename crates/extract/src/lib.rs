//! Structural code extraction → core IR.
//!
//! Phase-0 scope: **structure + imports only** — no calls, no LSP, no heuristic
//! name matching. Every edge produced here is `Confidence::Resolved`.
//! See `docs/specs/ir.md` and `docs/adr/0001-go-structural-mapping.md`.

mod error;
mod extractor;
mod go;
mod repo;
mod time;

pub use error::ExtractError;
pub use extractor::{FileExtraction, LanguageExtractor};
pub use go::GoExtractor;
pub use repo::extract_repo;
