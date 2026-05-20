//! RBMEM v1.4.2 - Structured Memory for AI Agents
//!
//! RBMEM (Rust-Brain Memory) is a library for managing structured, temporal, graph-aware
//! memory for AI agents. It provides a file format and API for organizing agent memory with
//! stable section paths, protected timestamps, hierarchical organization, and graph relationships.
//!
//! # Core Concepts
//!
//! - **Sections**: Individual memory units with stable paths like `project.rules`
//! - **Graph Relations**: Explicit edges between sections (depends_on, related_to, etc.)
//! - **Temporal Metadata**: Protected created/updated timestamps
//! - **Compact Output**: Minified context optimized for LLM consumption
//!
//! # Quick Example
//!
//! ```rust,no_run
//! use rbmem::{create, update, query_document, CreateOptions, UpdateOptions, SectionType};
//! use chrono::Utc;
//!
//! // Create a new memory file
//! let doc = create("memory.rbmem", CreateOptions {
//!     now: Utc::now(),
//!     created_by: "example".to_string(),
//!     purpose: "Example memory".to_string(),
//!     default_expiry_days: None,
//!     human: true,
//! }).unwrap();
//!
//! // Add a section
//! update("memory.rbmem", UpdateOptions {
//!     section: "project.rules".to_string(),
//!     section_type: SectionType::List,
//!     content: "- Prefer small, tested changes".to_string(),
//!     actor: "example".to_string(),
//!     human: true,
//!     dry_run: false,
//!     now: Utc::now(),
//! }).unwrap();
//!
//! // Query for relevant context
//! let results = query_document(&doc, "project rules", true, 1);
//! println!("Found {} sections", results.sections.len());
//! ```

//!
//! # Module Overview
//!
//! The binary in `main.rs` is intentionally thin. Most behavior lives here so parser and
//! document semantics can be tested without spawning a process.
//!
//! - `commands`: High-level CLI operations (create, update, query, context, etc.)
//! - `document`: Core document model with sections, graph, and temporal metadata
//! - `parser`: RBMEM file format parser
//! - `planner`: SAT-based planning with Kissat/CaDiCaL support
//! - `server`: Axum-based HTTP server for agent runtimes
//! - `crypto`: AES-256-GCM encryption for sensitive sections
//! - `diff`: Three-way merge and conflict resolution
//! - `hermes`: Hermes agent workflow integration
//! - `markdown`: Import from Markdown with automatic graph inference
//! - `pack`: Reusable context configurations via `.rbmempacks`
//! - `index`: Fast section lookups with cached indexing
//!

pub mod commands;
pub mod crypto;
pub mod diff;
pub mod document;
pub mod export;
pub mod index;
pub mod parser;
pub mod planner;
pub mod server;
pub mod hermes;
pub mod markdown;
pub mod pack;
#[path = "sync.rs"]
pub mod md_sync;
pub mod version;

pub use commands::{
    add_guard, context, context_json, create, decrypt_section, delete_section, diff,
    diff_documents, diff_file_with_format, encrypt_section, health_report, list_guards,
    list_snapshots, load, query, query_document, query_document_with_budget,
    query_document_with_index, query_document_with_budget_and_index, read, read_content_argument,
    remove_guard, render_context_document, render_context_output, review_commit, review_out,
    rollback_to_snapshot, save, update, ContextOptions, ContextOutputRequest, CreateOptions,
    GuardAction, GuardConstraint, HealthReport, OutputFormat, ReadOptions, SnapshotRecord,
    UpdateOptions,
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
pub use planner::{
    discover_memory_files, plan_memory, PlanOptions, PlanReport, PlanStep, ProofReport, SatBackend,
    SatStatus,
};
pub use version::{
    is_supported_format_version, RBMEM_FORMAT_LABEL, RBMEM_FORMAT_VERSION,
    RBMEM_LEGACY_FORMAT_VERSION,
};
