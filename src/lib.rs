//! AIF v1.3 library surface.
//!
//! The binary in `main.rs` is intentionally thin.  Most behavior lives here so
//! parser and document semantics can be tested without spawning a process.

pub mod document;
pub mod parser;

pub use document::{
    AIFDocument, AifError, CompactMode, GraphEdge, GraphRelation, GraphView, Meta, ResolvedSection,
    Section, SectionType, Temporal, TimestampPolicy,
};
