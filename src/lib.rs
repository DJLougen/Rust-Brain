//! RBMEM v1.4.0 library surface.
//!
//! The binary in `main.rs` is intentionally thin.  Most behavior lives here so
//! parser and document semantics can be tested without spawning a process.

pub mod commands;
pub mod crypto;
pub mod diff;
pub mod document;
pub mod export;
pub mod index;
pub mod parser;
pub mod server;
pub mod version;

pub use commands::{
    add_guard, context, context_json, create, decrypt_section, delete_section, diff, diff_documents,
    diff_file_with_format, encrypt_section, health_report, list_guards, list_snapshots, load, query,
    query_document, read, read_content_argument, render_context_document, render_context_output,
    remove_guard, review_commit, review_out, rollback_to_snapshot, save, update, ContextOptions,
    ContextOutputRequest, CreateOptions, GuardAction, GuardConstraint, HealthReport, OutputFormat,
    ReadOptions, SnapshotRecord, UpdateOptions,
};
pub use crypto::EncryptionKey;
pub use diff::{
    diff_documents_typed, diff_with_format, merge_documents, render_diff_text, DiffFormat,
    MergeStrategy, RbmemDiff, SectionDiff, SectionDiffKind,
};
pub use document::{
    CompactMode, EncryptedPayload, GraphEdge, GraphRelation, GraphView, InferenceStrategy, Meta,
    RbmemDocument, RbmemError, ResolvedSection, Section, SectionType, SourceInfo, Temporal,
    TimestampPolicy,
};
pub use export::{export_graph, ExportFormat};
pub use index::{CachedSectionIndex, SectionIndex};
pub use version::{
    is_supported_format_version, RBMEM_FORMAT_LABEL, RBMEM_FORMAT_VERSION,
    RBMEM_LEGACY_FORMAT_VERSION,
};
