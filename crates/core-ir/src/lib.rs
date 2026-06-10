//! IR data model
//!
//! A language-neutral, directed, typed property graph describing a codebase.
//! This crate is *pure data*: no file I/O, no parsing, no analysis. It is the
//! single source of truth every other crate reads and writes.
//!
//! Determinism is mandatory: the same logical document always serializes to
//! byte-identical JSON via [`IrDocument::to_canonical_json`], regardless of the
//! order nodes/edges were inserted. See `docs/specs/ir.md`.

mod document;
mod edge;
mod hash;
mod ids;
mod location;
mod node;

pub use document::{IrDocument, RootInfo, SCHEMA_VERSION};
pub use edge::{CallSite, Confidence, Edge, EdgeKind};
pub use hash::content_hash;
pub use location::{Location, Range};
pub use node::{Metadata, Node, NodeKind};
