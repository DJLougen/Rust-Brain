//! RBMEM v1.3 library surface.
//!
//! The binary in `main.rs` is intentionally thin.  Most behavior lives here so
//! parser and document semantics can be tested without spawning a process.

pub mod document;
pub mod parser;

pub use document::{
    CompactMode, GraphEdge, GraphRelation, GraphView, Meta, RbmemDocument, RbmemError,
    ResolvedSection, Section, SectionType, SourceInfo, Temporal, TimestampPolicy,
};
