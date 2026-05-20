use crate::document::graph_view_to_json;
use crate::parser::parse_document;
use crate::{crypto, diff as diff_engine, DiffFormat, EncryptionKey};
use crate::index::SectionIndex;
use crate::{
    CompactMode, RbmemDocument, RbmemError, Section, SectionType, SourceInfo, TimestampPolicy,
};
use md5;

/// Record of a memory snapshot for rollback purposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub label: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub file_hash: String,
    pub section_count: usize,
}

/// Health scoring report for RBMEM memory files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthReport {
    pub total_sections: usize,
    pub stale_sections: usize,
    pub orphaned_edges: usize,
    pub conflicts: usize,
    pub score: f64,
}

/// Guard constraint types for validation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GuardConstraint {
    pub max_tokens: Option<u64>,
    pub max_iterations: Option<u64>,
    pub max_retries: Option<u64>,
    pub output_validation: Option<String>,
}

/// Guard operation actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardAction {
    Set,
    Remove,
    Check,
}

use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::hash::Hasher;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateOptions {
    pub created_by: String,
    pub purpose: String,
    pub default_expiry_days: Option<i64>,
    pub human: bool,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadOptions {
    pub resolve: bool,
    pub compact: bool,
    pub minified: bool,
    pub hide_empty_temporal: bool,
    pub decrypt: bool,
    pub key: Option<EncryptionKey>,
    pub policy: TimestampPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateOptions {
    pub section: String,
    pub section_type: SectionType,
    pub content: String,
    pub actor: String,
    pub human: bool,
    pub dry_run: bool,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextOptions {
    pub resolve: bool,
    pub compact: bool,
    pub minified: bool,
    pub graph_depth: usize,
    pub decrypt: bool,
    pub key: Option<EncryptionKey>,
    pub format: OutputFormat,
    pub policy: TimestampPolicy,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextOutputRequest {
    pub operation: String,
    pub file: String,
    pub selector_name: String,
    pub selector_value: String,
    pub resolve: bool,
    pub compact: bool,
    pub minified: bool,
    pub graph_depth: usize,
    pub format: OutputFormat,
}

pub fn load(path: impl AsRef<Path>, policy: TimestampPolicy) -> Result<RbmemDocument, RbmemError> {
    let input = fs::read_to_string(path)?;
    Ok(parse_document(&input, policy)?.document)
}

pub fn save(
    path: impl AsRef<Path>,
    document: &RbmemDocument,
    human: bool,
) -> Result<(), RbmemError> {
    let text = if human {
        document.to_human_rbmem_string()
    } else {
        document.to_rbmem_string()
    };
    fs::write(path, text)?;
    Ok(())
}

pub fn create(path: impl AsRef<Path>, options: CreateOptions) -> Result<RbmemDocument, RbmemError> {
    let mut document = RbmemDocument::new(options.now, options.created_by);
    document.meta.purpose = options.purpose;
    document.meta.default_expiry_days = options.default_expiry_days;
    save(path, &document, options.human)?;
    Ok(document)
}

pub fn delete_section(
    path: impl AsRef<Path>,
    section_path: &str,
    dry_run: bool,
    now: DateTime<Utc>,
) -> Result<RbmemDocument, RbmemError> {
    let path = path.as_ref();
    let mut document = load(path, TimestampPolicy::Preserve)?;
    let before = document.sections.len();
    document
        .sections
        .retain(|section| section.path != section_path);
    if document.sections.len() == before {
        return Err(RbmemError::Parse(format!(
            "section '{section_path}' not found"
        )));
    }
    // Clean orphaned graph relations referencing the deleted section
    let deleted_path = section_path.to_string();
    for section in &mut document.sections {
        if let Some(ref mut graph) = section.graph {
            graph.relations.retain(|rel| rel.to != deleted_path);
        }
    }

    document.meta.last_updated = now;
    if !dry_run {
        save(path, &document, false)?;
    }
    Ok(document)
}

pub fn read(path: impl AsRef<Path>, options: ReadOptions) -> Result<String, RbmemError> {
    let document = load(path, options.policy)?;
    let document = prepare_encrypted_sections(document, options.decrypt, options.key.as_ref())?;
    Ok(render_read_document(&document, &options))
}

pub fn update(path: impl AsRef<Path>, options: UpdateOptions) -> Result<RbmemDocument, RbmemError> {
    let path = path.as_ref();
    let mut document = if path.exists() {
        load(path, TimestampPolicy::Preserve)?
    } else {
        RbmemDocument::new(options.now, "me")
    };

    document.upsert_section(
        &options.section,
        options.section_type,
        options.content,
        options.now,
    );
    set_section_source(
        &mut document,
        &options.section,
        SourceInfo {
            kind: "cli".to_string(),
            path: None,
            actor: Some(options.actor.clone()),
            hash: None,
        },
    );
    document.enforce_protected_timestamps(options.now);
    if !options.dry_run {
        save(path, &document, options.human)?;
    }
    Ok(document)
}

pub fn query(
    path: impl AsRef<Path>,
    text: &str,
    options: ContextOptions,
) -> Result<String, RbmemError> {
    let path = path.as_ref();
    let document = load(path, options.policy)?;
    let document = prepare_encrypted_sections(document, options.decrypt, options.key.as_ref())?;
    let context = query_document_with_budget(
        &document,
        text,
        options.resolve,
        options.graph_depth,
        options.max_tokens,
    );
    render_context_output(
        ContextOutputRequest {
            operation: "query".to_string(),
            file: path.display().to_string(),
            selector_name: "text".to_string(),
            selector_value: text.to_string(),
            resolve: options.resolve,
            compact: options.compact,
            minified: options.minified,
            graph_depth: options.graph_depth,
            format: options.format,
        },
        &document,
        &context,
    )
}

pub fn context(
    path: impl AsRef<Path>,
    task: &str,
    options: ContextOptions,
) -> Result<String, RbmemError> {
    let path = path.as_ref();
    let document = load(path, options.policy)?;
    let document = prepare_encrypted_sections(document, options.decrypt, options.key.as_ref())?;
    let context = query_document_with_budget(
        &document,
        task,
        options.resolve,
        options.graph_depth,
        options.max_tokens,
    );
    render_context_output(
        ContextOutputRequest {
            operation: "context".to_string(),
            file: path.display().to_string(),
            selector_name: "task".to_string(),
            selector_value: task.to_string(),
            resolve: options.resolve,
            compact: options.compact,
            minified: options.minified,
            graph_depth: options.graph_depth,
            format: options.format,
        },
        &document,
        &context,
    )
}

pub fn diff(before: impl AsRef<Path>, after: impl AsRef<Path>) -> Result<String, RbmemError> {
    diff_file_with_format(before, after, DiffFormat::Text)
}

pub fn diff_file_with_format(
    before: impl AsRef<Path>,
    after: impl AsRef<Path>,
    format: DiffFormat,
) -> Result<String, RbmemError> {
    let before = load(before, TimestampPolicy::Preserve)?;
    let after = load(after, TimestampPolicy::Preserve)?;
    diff_engine::diff_with_format(&before, &after, format)
}

pub fn encrypt_section(
    path: impl AsRef<Path>,
    section_path: &str,
    key: &EncryptionKey,
    now: DateTime<Utc>,
) -> Result<RbmemDocument, RbmemError> {
    let path = path.as_ref();
    let mut document = load(path, TimestampPolicy::Preserve)?;
    let section = document
        .sections
        .iter_mut()
        .find(|section| section.path == section_path)
        .ok_or_else(|| RbmemError::NotFound(format!("section '{section_path}' not found")))?;

    if section.section_type != SectionType::Encrypted {
        let payload = crypto::encrypt_content(&section.content, key, now)?;
        section.section_type = SectionType::Encrypted;
        section.encrypted = Some(payload);
        section.content.clear();
        section.temporal.updated_at = now;
        document.meta.last_updated = now;
        save(path, &document, false)?;
    }

    Ok(document)
}

pub fn decrypt_section(
    path: impl AsRef<Path>,
    section_path: &str,
    key: &EncryptionKey,
    now: DateTime<Utc>,
) -> Result<RbmemDocument, RbmemError> {
    let path = path.as_ref();
    let mut document = load(path, TimestampPolicy::Preserve)?;
    let section = document
        .sections
        .iter_mut()
        .find(|section| section.path == section_path)
        .ok_or_else(|| RbmemError::NotFound(format!("section '{section_path}' not found")))?;

    let payload = section
        .encrypted
        .as_ref()
        .ok_or_else(|| RbmemError::Parse(format!("section '{section_path}' is not encrypted")))?;
    section.content = crypto::decrypt_content(payload, key)?;
    section.section_type = SectionType::Text;
    section.encrypted = None;
    section.temporal.updated_at = now;
    document.meta.last_updated = now;
    save(path, &document, false)?;

    Ok(document)
}

pub fn query_document(
    document: &RbmemDocument,
    query: &str,
    include_parents: bool,
    graph_depth: usize,
) -> RbmemDocument {
    query_document_with_budget(document, query, include_parents, graph_depth, None)
}

pub fn query_document_with_index(
    document: &RbmemDocument,
    query: &str,
    include_parents: bool,
    graph_depth: usize,
    index: &SectionIndex,
) -> RbmemDocument {
    query_document_with_budget_and_index(document, query, include_parents, graph_depth, None, index)
}

pub fn query_document_with_budget(
    document: &RbmemDocument,
    query: &str,
    include_parents: bool,
    graph_depth: usize,
    max_tokens: Option<usize>,
) -> RbmemDocument {
    let index = SectionIndex::build(document);
    query_document_with_budget_and_index(document, query, include_parents, graph_depth, max_tokens, &index)
}

pub fn query_document_with_budget_and_index(
    document: &RbmemDocument,
    query: &str,
    include_parents: bool,
    graph_depth: usize,
    max_tokens: Option<usize>,
    index: &SectionIndex,
) -> RbmemDocument {
    let query_terms = query_terms(query);
    let phrase = normalize_for_query(query);
    let now = Utc::now();
    let scored_matches = query_matches_indexed(document, index, &query_terms, &phrase, now);
    let mut selected: BTreeSet<String> = scored_matches.iter().map(|(p, _)| p.clone()).collect();

    if include_parents {
        include_parent_sections_indexed(index, &mut selected);
    }

    if graph_depth > 0 && !selected.is_empty() {
        for path in selected.clone() {
            for neighbor in index.related(&path, graph_depth) {
                selected.insert(neighbor);
            }
        }
    }

    if let Some(budget) = max_tokens {
        selected = truncate_to_token_budget(document, &scored_matches, &selected, budget);
    }

    subset_document(document, selected)
}

fn truncate_to_token_budget(
    document: &RbmemDocument,
    scored: &[(String, f64)],
    selected: &BTreeSet<String>,
    max_tokens: usize,
) -> BTreeSet<String> {
    // Build score and token lookups in O(n) instead of O(k*n)
    let score_map: std::collections::HashMap<&str, f64> = scored
        .iter()
        .map(|(p, s)| (p.as_str(), *s))
        .collect();

    let token_map: std::collections::HashMap<&str, usize> = document
        .sections
        .iter()
        .map(|s| (s.path.as_str(), (s.content.len() / 4).max(1)))
        .collect();

    // Sort selected by score descending (unscored sections like parents get 0)
    let mut ranked: Vec<(&str, f64, usize)> = selected
        .iter()
        .map(|path| {
            let score = score_map.get(path.as_str()).copied().unwrap_or(0.0);
            let tokens = token_map.get(path.as_str()).copied().unwrap_or(1);
            (path.as_str(), score, tokens)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut result = BTreeSet::new();
    let mut used = 0usize;
    for (path, _score, tokens) in ranked {
        if used + tokens > max_tokens && !result.is_empty() {
            break;
        }
        used += tokens;
        result.insert(path.to_string());
    }
    result
}

pub fn render_context_document(
    document: &RbmemDocument,
    resolve: bool,
    compact: bool,
    minified: bool,
) -> String {
    if minified || (!compact && document.meta.compact_mode == CompactMode::Minified) {
        document.to_minified_string(resolve)
    } else if compact || document.meta.compact_mode == CompactMode::Compact {
        document.to_compact_string(resolve, Utc::now())
    } else {
        document.to_rbmem_string_hiding_empty_temporal()
    }
}

pub fn render_context_output(
    request: ContextOutputRequest,
    source: &RbmemDocument,
    context: &RbmemDocument,
) -> Result<String, RbmemError> {
    match request.format {
        OutputFormat::Text => Ok(render_context_document(
            context,
            request.resolve,
            request.compact,
            request.minified,
        )),
        OutputFormat::Json => Ok(serde_json::to_string_pretty(&context_json(
            request, source, context,
        ))?),
    }
}

pub fn context_json(
    request: ContextOutputRequest,
    source: &RbmemDocument,
    context: &RbmemDocument,
) -> Value {
    json!({
        "schema": "rbmem.context.v1",
        "operation": request.operation,
        "file": request.file,
        "selector": {
            "kind": request.selector_name,
            "value": request.selector_value,
        },
        "options": {
            "resolve": request.resolve,
            "compact": request.compact,
            "minified": request.minified,
            "graph_depth": request.graph_depth,
        },
        "source": {
            "meta": document_meta_json(source),
            "section_count": source.sections.len(),
        },
        "context": render_context_document(
            context,
            request.resolve,
            request.compact,
            request.minified,
        ),
        "context_meta": document_meta_json(context),
        "sections": sections_json(context, request.resolve),
        "graph": graph_view_to_json(&context.graph_view()),
    })
}

pub fn diff_documents(before: &RbmemDocument, after: &RbmemDocument) -> String {
    diff_engine::diff_with_format(before, after, DiffFormat::Text)
        .unwrap_or_else(|error| format!("error rendering diff: {error}\n"))
}

pub fn read_content_argument(
    content: Option<String>,
    content_file: Option<impl AsRef<Path>>,
) -> Result<String, RbmemError> {
    match (content, content_file) {
        (Some(content), None) => Ok(content),
        (None, Some(path)) => Ok(fs::read_to_string(path)?),
        (Some(_), Some(_)) => Err(RbmemError::Parse(
            "use either --content or --content-file, not both".to_string(),
        )),
        (None, None) => Err(RbmemError::Parse(
            "missing --content or --content-file".to_string(),
        )),
    }
}

fn render_read_document(document: &RbmemDocument, options: &ReadOptions) -> String {
    if options.minified
        || (options.resolve
            && !options.compact
            && document.meta.compact_mode == CompactMode::Minified)
    {
        document.to_minified_string(options.resolve)
    } else if options.compact
        || (options.resolve && document.meta.compact_mode == CompactMode::Compact)
    {
        document.to_compact_string(options.resolve, Utc::now())
    } else if options.resolve {
        render_resolved_sections(document)
    } else if options.hide_empty_temporal {
        document.to_rbmem_string_hiding_empty_temporal()
    } else {
        document.to_rbmem_string()
    }
}

fn prepare_encrypted_sections(
    mut document: RbmemDocument,
    decrypt: bool,
    key: Option<&EncryptionKey>,
) -> Result<RbmemDocument, RbmemError> {
    if decrypt {
        let resolved_key;
        let key = match key {
            Some(key) => key,
            None => {
                resolved_key = EncryptionKey::resolve()?;
                &resolved_key
            }
        };
        decrypt_document_sections(&mut document, key)?;
    } else {
        document
            .sections
            .retain(|section| section.section_type != SectionType::Encrypted);
    }

    Ok(document)
}

fn decrypt_document_sections(
    document: &mut RbmemDocument,
    key: &EncryptionKey,
) -> Result<(), RbmemError> {
    for section in &mut document.sections {
        if section.section_type != SectionType::Encrypted {
            continue;
        }
        let payload = section.encrypted.as_ref().ok_or_else(|| {
            RbmemError::Crypto(format!(
                "encrypted section '{}' is missing encrypted payload fields",
                section.path
            ))
        })?;
        section.content = crypto::decrypt_content(payload, key)?;
        section.section_type = SectionType::Text;
    }
    Ok(())
}

fn render_resolved_sections(document: &RbmemDocument) -> String {
    let mut output = String::new();
    for section in document.resolved_sections() {
        output.push_str(&format!(
            "[{}] type={}\n",
            section.path, section.section_type
        ));
        output.push_str(&section.content);
        output.push_str("\n\n");
    }
    output
}

fn query_matches_indexed(
    document: &RbmemDocument,
    index: &SectionIndex,
    terms: &[String],
    phrase: &str,
    now: chrono::DateTime<Utc>,
) -> Vec<(String, f64)> {
    let candidate_paths: BTreeSet<String> = terms
        .iter()
        .flat_map(|term| index.keyword(term))
        .chain(
            phrase
                .split_whitespace()
                .flat_map(|word| index.keyword(word)),
        )
        .collect();

    let sections_to_score: Vec<&Section> = if candidate_paths.is_empty() {
        document.sections.iter().collect()
    } else {
        document
            .sections
            .iter()
            .filter(|s| candidate_paths.contains(&s.path))
            .collect()
    };

    let mut scored = sections_to_score
        .into_iter()
        .filter_map(|section| {
            let score = query_score(section, terms, phrase, now);
            (score > 0.0).then_some((section.path.clone(), score))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    scored
}

fn query_score(section: &Section, terms: &[String], phrase: &str, now: chrono::DateTime<Utc>) -> f64 {
    let path = normalize_for_query(&section.path);
    let content = normalize_for_query(&section.content);
    let mut score: f64 = 0.0;

    let word_count = content.split_whitespace().count().max(1) as f64;

    if !phrase.is_empty() {
        if path.contains(phrase) {
            score += 20.0;
        }
        if content.contains(phrase) {
            score += 12.0 / word_count.sqrt();
        }
    }

    for term in terms {
        if path.contains(term.as_str()) {
            score += 5.0;
        }
        let term_count = content.matches(term.as_str()).count() as f64;
        if term_count > 0.0 {
            score += (term_count / word_count.sqrt()) * 3.0;
        }
    }

    // Recency bonus: up to 5 points for recently updated sections
    let days_since_update = (now - section.temporal.updated_at).num_days().max(0) as f64;
    if days_since_update < 7.0 {
        score += 5.0;
    } else if days_since_update < 30.0 {
        score += 3.0;
    } else if days_since_update < 90.0 {
        score += 1.0;
    }

    // Path depth: prefer shallower sections (fewer dots)
    let depth = section.path.chars().filter(|&c| c == '.').count();
    if depth > 2 {
        score -= (depth - 2) as f64;
    }

    score.max(0.0)
}

fn include_parent_sections_indexed(index: &SectionIndex, selected: &mut BTreeSet<String>) {
    let matched = selected.iter().cloned().collect::<Vec<_>>();

    for path in matched {
        let parts = path.split('.').collect::<Vec<_>>();
        for depth in 1..parts.len() {
            let parent = parts[..depth].join(".");
            if index.contains_path(&parent) {
                selected.insert(parent);
            }
        }
    }
}

fn subset_document(document: &RbmemDocument, selected: BTreeSet<String>) -> RbmemDocument {
    let mut subset = document.clone();
    subset
        .sections
        .retain(|section| selected.contains(&section.path));
    subset
}

fn query_terms(text: &str) -> Vec<String> {
    normalize_for_query(text)
        .split_whitespace()
        .filter(|term| term.len() > 1)
        .map(ToString::to_string)
        .collect()
}

fn normalize_for_query(text: &str) -> String {
    let mut output = String::new();
    let mut last_was_space = true;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            output.push(' ');
            last_was_space = true;
        }
    }

    output.trim().to_string()
}

fn set_section_source(document: &mut RbmemDocument, path: &str, source: SourceInfo) {
    if let Some(section) = document
        .sections
        .iter_mut()
        .find(|section| section.path == path)
    {
        section.source = Some(source);
    }
}

fn document_meta_json(document: &RbmemDocument) -> Value {
    json!({
        "version": document.meta.version,
        "source_version": document.meta.source_version,
        "purpose": document.meta.purpose,
        "generated_at": document.meta.generated_at,
        "last_updated": document.meta.last_updated,
        "valid_until": document.meta.valid_until,
        "created_by": document.meta.created_by,
        "default_expiry_days": document.meta.default_expiry_days,
        "compact_mode": document.meta.compact_mode.to_string(),
    })
}

fn sections_json(document: &RbmemDocument, resolve: bool) -> Vec<Value> {
    if resolve {
        document
            .resolved_sections()
            .into_iter()
            .map(|section| {
                json!({
                    "path": section.path,
                    "type": section.section_type.to_string(),
                    "temporal": {
                        "created_at": section.temporal.created_at,
                        "updated_at": section.temporal.updated_at,
                        "expires_at": section.temporal.expires_at,
                    },
                    "source": section.source,
                    "graph": section.graph,
                    "content": section.content,
                })
            })
            .collect()
    } else {
        document
            .sections
            .iter()
            .map(|section| {
                json!({
                    "path": section.path,
                    "type": section.section_type.to_string(),
                    "temporal": {
                        "created_at": section.temporal.created_at,
                        "updated_at": section.temporal.updated_at,
                        "expires_at": section.temporal.expires_at,
                    },
                    "source": section.source,
                    "graph": section.graph,
                    "content": section.content,
                })
            })
            .collect()
    }
}

pub fn create_snapshot(path: impl AsRef<Path>, label: &str) -> Result<SnapshotRecord, RbmemError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)?;
    let hash = format!("{:x}", md5::compute(&raw));
    let now = Utc::now();

    let doc = parse_document(&raw, TimestampPolicy::Preserve)?.document;

    let record = SnapshotRecord {
        label: label.to_string(),
        timestamp: now,
        file_hash: hash,
        section_count: doc.sections.len(),
    };

    let snapshot_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".rbmem");
    fs::create_dir_all(&snapshot_dir).ok();

    // Store metadata as JSON (robust against labels with colons/quotes)
    // Generate unique filename to avoid collisions: label + 4-char hash suffix
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&label, &mut hasher);
    let label_hash = format!("{:04x}", hasher.finish() & 0xFFFF);
    let snapshot_file = snapshot_dir.join(format!("{}-{}.snap", label, label_hash));
    let snapshot_content = serde_json::to_string_pretty(&record)?;
    fs::write(&snapshot_file, snapshot_content)?;

    // Store actual file content for rollback
    let content_file = snapshot_dir.join(format!("{}-{}.rbmem", label, label_hash));
    fs::write(&content_file, &raw)?;

    Ok(record)
}

pub fn list_snapshots(path: impl AsRef<Path>) -> Result<Vec<SnapshotRecord>, RbmemError> {
    let path = path.as_ref();
    let snapshot_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".rbmem");

    if !snapshot_dir.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    for entry in fs::read_dir(&snapshot_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.extension().and_then(|e| e.to_str()) == Some("snap") {
            let content = fs::read_to_string(&entry_path)?;
            let record: SnapshotRecord = serde_json::from_str(&content)?;
            snapshots.push(record);
        }
    }
    Ok(snapshots)
}

pub fn rollback_to_snapshot(path: impl AsRef<Path>, label: &str) -> Result<(), RbmemError> {
    let path = path.as_ref();
    let snapshot_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".rbmem");

    // Find snapshot file matching label + hash suffix pattern
    let mut snapshot_file = None;
    let mut content_file = None;
    if snapshot_dir.exists() {
        for entry in fs::read_dir(&snapshot_dir)? {
            let entry_path = entry?.path();
            let file_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if file_name.starts_with(&format!("{}-", label)) {
                if file_name.ends_with(".snap") {
                    snapshot_file = Some(entry_path.clone());
                } else if file_name.ends_with(".rbmem") {
                    content_file = Some(entry_path.clone());
                }
            }
        }
    }

    let _snapshot_file = snapshot_file
        .ok_or_else(|| RbmemError::Parse(format!("snapshot '{}' not found", label)))?;

    let content_file = content_file.ok_or_else(|| {
        RbmemError::Parse(format!(
            "snapshot content file not found for label '{}'",
            label
        ))
    })?;

    // Auto-backup before rollback
    let auto_label = format!("pre-rollback-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    if let Err(e) = create_snapshot(path, &auto_label) {
        eprintln!(
            "Warning: failed to create auto-backup before rollback: {}",
            e
        );
    }


    let backup = fs::read_to_string(&content_file)?;
    fs::write(path, &backup)?;
    println!("rolled back '{}' to {}", label, path.display());
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub total_sections: usize,
    pub stale_sections: usize,
    pub orphaned_edges: usize,
    pub conflicts: usize,
    pub score: f64,
}

pub fn health_report(path: impl AsRef<Path>, stale_days: u64) -> Result<HealthScore, RbmemError> {
    let document = load(path, TimestampPolicy::Preserve)?;
    let now = Utc::now();
    let stale_cutoff = now - chrono::Duration::days(stale_days as i64);

    let stale_sections = document
        .sections
        .iter()
        .filter(|s| s.temporal.updated_at < stale_cutoff)
        .count();

    let orphaned_edges = count_orphaned_edges(&document);
    let conflicts = count_conflicts(&document);

    let total = document.sections.len() as f64;
    let score = if total == 0.0 {
        100.0
    } else {
        let penalty = (stale_sections as f64 / total) * 30.0
            + (orphaned_edges as f64 / total.max(1.0)) * 20.0
            + (conflicts as f64 / total.max(1.0)) * 25.0;
        (100.0 - penalty).clamp(0.0, 100.0)
    };

    Ok(HealthScore {
        total_sections: document.sections.len(),
        stale_sections,
        orphaned_edges,
        conflicts,
        score,
    })
}

fn count_orphaned_edges(document: &RbmemDocument) -> usize {
    let all_section_paths: std::collections::HashSet<&str> =
        document.sections.iter().map(|s| s.path.as_str()).collect();
    let mut count = 0;
    for section in &document.sections {
        if let Some(graph) = &section.graph {
            for edge in &graph.relations {
                if !all_section_paths.contains(edge.to.as_str()) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn count_conflicts(document: &RbmemDocument) -> usize {
    use std::collections::HashMap;
    let mut by_path: HashMap<&str, Vec<&str>> = HashMap::new();
    for section in &document.sections {
        by_path
            .entry(section.path.as_str())
            .or_default()
            .push(section.content.as_str());
    }

    let mut conflicts = 0;
    for contents in by_path.values() {
        let n = contents.len();
        if n < 2 {
            continue;
        }
        let total_pairs = n * (n - 1) / 2;
        let mut same_content_counts: HashMap<&str, usize> = HashMap::new();
        for content in contents {
            *same_content_counts.entry(content).or_default() += 1;
        }
        let same_pairs: usize = same_content_counts
            .values()
            .map(|&count| count * (count - 1) / 2)
            .sum();
        conflicts += total_pairs - same_pairs;
    }
    conflicts
}

pub fn add_guard(
    path: impl AsRef<Path>,
    guard_type: &str,
    value: &str,
    now: DateTime<Utc>,
) -> Result<RbmemDocument, RbmemError> {
    let path = path.as_ref();
    let mut document = load(path, TimestampPolicy::Preserve)?;

    let guard_section = document
        .sections
        .iter_mut()
        .find(|s| s.section_type == SectionType::Guards)
        .ok_or_else(|| RbmemError::Parse("no guards section found".to_string()))?;

    let mut existing_guards: GuardConstraint =
        serde_json::from_str(&guard_section.content).unwrap_or_else(|_| GuardConstraint::default());

    match guard_type {
        "max-tokens" => {
            existing_guards.max_tokens = Some(
                value
                    .parse::<u64>()
                    .map_err(|e| RbmemError::Parse(e.to_string()))?,
            )
        }
        "max-iterations" => {
            existing_guards.max_iterations = Some(
                value
                    .parse::<u64>()
                    .map_err(|e| RbmemError::Parse(e.to_string()))?,
            )
        }
        "max-retries" => {
            existing_guards.max_retries = Some(
                value
                    .parse::<u64>()
                    .map_err(|e| RbmemError::Parse(e.to_string()))?,
            )
        }
        "output-validation" => existing_guards.output_validation = Some(value.to_string()),
        other => return Err(RbmemError::Parse(format!("unknown guard type: {other}"))),
    }

    guard_section.content = serde_json::to_string(&existing_guards)?;
    guard_section.temporal.updated_at = now;
    document.meta.last_updated = now;

    if guard_section.source.is_none() {
        guard_section.source = Some(SourceInfo {
            kind: "cli".to_string(),
            path: None,
            actor: Some("user".to_string()),
            hash: None,
        });
    }

    save(path, &document, false)?;
    Ok(document)
}

pub fn remove_guard(
    path: impl AsRef<Path>,
    guard_type: &str,
    now: DateTime<Utc>,
) -> Result<RbmemDocument, RbmemError> {
    let path = path.as_ref();
    let mut document = load(path, TimestampPolicy::Preserve)?;

    let guard_section = document
        .sections
        .iter_mut()
        .find(|s| s.section_type == SectionType::Guards)
        .ok_or_else(|| RbmemError::Parse("no guards section found".to_string()))?;

    let mut updated_guards: GuardConstraint =
        serde_json::from_str(&guard_section.content).unwrap_or_else(|_| GuardConstraint::default());

    match guard_type {
        "max-tokens" => updated_guards.max_tokens = None,
        "max-iterations" => updated_guards.max_iterations = None,
        "max-retries" => updated_guards.max_retries = None,
        "output-validation" => updated_guards.output_validation = None,
        _ => {
            return Err(RbmemError::Parse(format!(
                "unknown guard type: {guard_type}"
            )))
        }
    }
    guard_section.content = serde_json::to_string(&updated_guards)?;
    guard_section.temporal.updated_at = now;
    document.meta.last_updated = now;

    save(path, &document, false)?;
    Ok(document)
}

pub fn review_out(
    path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<Vec<Section>, RbmemError> {
    let document = load(path, TimestampPolicy::Preserve)?;
    let mut pending = Vec::new();

    for section in &document.sections {
        let should_flag = match section.source.as_ref().map(|s| s.kind.as_str()) {
            Some("agent") | Some("llm") | Some("auto") => true,
            Some("sync") => false,
            None => false,
            Some(other) if other.contains("review") => true,
            _ => false,
        };

        if should_flag {
            pending.push(section.clone());
        }
    }

    let mut review_doc = RbmemDocument::new(Utc::now(), "review");
    for section in &pending {
        let mut flagged = section.clone();
        if let Some(ref mut source) = flagged.source {
            source.kind = format!("{}-pending-review", source.kind);
        } else {
            flagged.source = Some(SourceInfo {
                kind: "pending-review".to_string(),
                path: None,
                actor: Some("reviewer".to_string()),
                hash: None,
            });
        }
        review_doc.upsert_section(
            &flagged.path,
            SectionType::Review,
            format!(
                "[REVIEW] type={}\n{}\n",
                flagged.section_type, flagged.content
            ),
            Utc::now(),
        );
    }

    save(output_path, &review_doc, false)?;
    Ok(pending)
}

pub fn review_commit(
    pending_path: impl AsRef<Path>,
    target_path: impl AsRef<Path>,
) -> Result<usize, RbmemError> {
    let pending = load(pending_path, TimestampPolicy::Preserve)?;
    let mut target = if target_path.as_ref().exists() {
        load(&target_path, TimestampPolicy::Preserve)?
    } else {
        RbmemDocument::new(Utc::now(), "me")
    };

    let mut applied = 0;
    for section in &pending.sections {
        if section.section_type == SectionType::Review {
            let original_type = section
                .content
                .lines()
                .next()
                .and_then(|l| l.split_once("type="))
                .and_then(|(_, t)| t.split_whitespace().next())
                .unwrap_or("text")
                .to_string();

            let content_start = section
                .content
                .find("]\n")
                .map(|i| &section.content[i + 2..])
                .unwrap_or("");

            target.upsert_section(
                &section.path,
                original_type.parse().unwrap_or(SectionType::Text),
                content_start.to_string(),
                Utc::now(),
            );
            applied += 1;
        }
    }

    if applied > 0 {
        save(&target_path, &target, false)?;
    }

    Ok(applied)
}

pub fn list_guards(path: impl AsRef<Path>) -> Result<GuardConstraint, RbmemError> {
    let document = load(path, TimestampPolicy::Preserve)?;

    let guard_section = document
        .sections
        .iter()
        .find(|s| s.section_type == SectionType::Guards)
        .ok_or_else(|| RbmemError::Parse("no guards section found".to_string()))?;

    let guards: GuardConstraint =
        serde_json::from_str(&guard_section.content).unwrap_or_else(|_| GuardConstraint::default());

    Ok(guards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 7, 12, 0, 0).unwrap()
    }

    #[test]
    fn query_context_includes_matches_parents_and_graph_neighbors() {
        let now = fixed_time();
        let mut document = RbmemDocument::new(now, "me");
        document.upsert_section("rules", SectionType::List, "- Base".to_string(), now);
        document.upsert_section(
            "rules.review",
            SectionType::Text,
            "Review pull requests.".to_string(),
            now,
        );
        document.upsert_section(
            "memory.testing",
            SectionType::Text,
            "Run Rust tests.".to_string(),
            now,
        );
        document.sections[1].graph = Some(crate::document::GraphInfo {
            node_type: None,
            relations: vec![crate::GraphRelation {
                to: "memory.testing".to_string(),
                relation_type: "depends_on".to_string(),
                valid_from: Some(now),
                valid_until: None,
                inferred: false,
                confidence: None,
            }],
        });

        let context = query_document(&document, "pull requests", true, 1);
        let paths = context
            .sections
            .iter()
            .map(|section| section.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["memory.testing", "rules", "rules.review"]);
    }

    #[test]
    fn diff_reports_added_and_changed_sections() {
        let now = fixed_time();
        let mut before = RbmemDocument::new(now, "me");
        before.upsert_section("rules", SectionType::Text, "old".to_string(), now);

        let mut after = before.clone();
        after.upsert_section("rules", SectionType::Text, "new".to_string(), now);
        after.upsert_section("memory", SectionType::Text, "added".to_string(), now);

        let diff = diff_documents(&before, &after);

        assert!(diff.contains("added: memory"));
        assert!(diff.contains("changed content: rules"));
    }

    #[test]
    fn test_create_and_list_snapshot() {
        let content = r#"meta:
  purpose: test document
  created_by: test

[SECTION: memory.user]
type: text

Daniel, PhD CogNeuro UofT
[END SECTION]
"#;
        let dir = std::env::temp_dir().join(format!(
            "rbmem_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("memory.rbmem");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(&path, content).ok();

        let record = create_snapshot(&path, "test-snapshot").unwrap();
        assert_eq!(record.label, "test-snapshot");
        assert_eq!(record.section_count, 1);
        assert!(!record.file_hash.is_empty());

        let snapshots = list_snapshots(&path).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].label, "test-snapshot");

        // Cleanup
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_rollback_restores_content() {
        let original = r#"meta:
  purpose: test document
  created_by: test

[SECTION: memory.user]
type: text

original content here
[END SECTION]
"#;
        let dir = std::env::temp_dir().join(format!(
            "rbmem_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("memory.rbmem");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(&path, original).ok();

        // Create snapshot
        create_snapshot(&path, "before-modification").unwrap();

        // Modify the file
        let modified = r#"meta:
  purpose: test document
  created_by: test

[SECTION: memory.user]
type: text

modified content after rollback
[END SECTION]
"#;
        std::fs::write(&path, modified).unwrap();
        assert_ne!(std::fs::read_to_string(&path).unwrap(), original);

        // Rollback
        rollback_to_snapshot(&path, "before-modification").unwrap();

        // Verify restoration
        let restored = std::fs::read_to_string(&path).unwrap();
        assert_eq!(restored, original);

        // Cleanup
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_rollback_fails_on_missing_snapshot() {
        let dir = std::env::temp_dir().join(format!(
            "rbmem_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("memory.rbmem");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(
            &path,
            "memory:
  test: text
  content: hello
",
        )
        .ok();

        let result = rollback_to_snapshot(&path, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));

        // Cleanup
        let _ = std::fs::remove_dir_all(dir);
    }
}
