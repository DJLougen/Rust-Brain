use crate::document::graph_view_to_json;
use crate::parser::parse_document;
use crate::{crypto, diff as diff_engine, DiffFormat, EncryptionKey};
use crate::{
    CompactMode, RbmemDocument, RbmemError, Section, SectionType, SourceInfo, TimestampPolicy,
};
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
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
            actor: Some("me".to_string()),
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
    let context = query_document(&document, text, options.resolve, options.graph_depth);
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
    let context = query_document(&document, task, options.resolve, options.graph_depth);
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
        .ok_or_else(|| RbmemError::Parse(format!("section '{section_path}' not found")))?;

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
        .ok_or_else(|| RbmemError::Parse(format!("section '{section_path}' not found")))?;

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
    let query_terms = query_terms(query);
    let phrase = normalize_for_query(query);
    let mut selected = query_matches(document, &query_terms, &phrase);

    if include_parents {
        include_parent_sections(document, &mut selected);
    }

    include_graph_neighbors(document, &mut selected, graph_depth);
    subset_document(document, selected)
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

fn query_matches(document: &RbmemDocument, terms: &[String], phrase: &str) -> BTreeSet<String> {
    let mut scored = document
        .sections
        .iter()
        .filter_map(|section| {
            let score = query_score(section, terms, phrase);
            (score > 0).then_some((section.path.clone(), score))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.into_iter().map(|(path, _)| path).collect()
}

fn query_score(section: &Section, terms: &[String], phrase: &str) -> usize {
    let path = normalize_for_query(&section.path);
    let content = normalize_for_query(&section.content);
    let mut score = 0;

    if !phrase.is_empty() {
        if path.contains(phrase) {
            score += 20;
        }
        if content.contains(phrase) {
            score += 12;
        }
    }

    for term in terms {
        if path.contains(term) {
            score += 5;
        }
        if content.contains(term) {
            score += 1;
        }
    }

    score
}

fn include_parent_sections(document: &RbmemDocument, selected: &mut BTreeSet<String>) {
    let known_paths = document
        .sections
        .iter()
        .map(|section| section.path.as_str())
        .collect::<BTreeSet<_>>();
    let matched = selected.iter().cloned().collect::<Vec<_>>();

    for path in matched {
        let parts = path.split('.').collect::<Vec<_>>();
        for depth in 1..parts.len() {
            let parent = parts[..depth].join(".");
            if known_paths.contains(parent.as_str()) {
                selected.insert(parent);
            }
        }
    }
}

fn include_graph_neighbors(
    document: &RbmemDocument,
    selected: &mut BTreeSet<String>,
    graph_depth: usize,
) {
    if graph_depth == 0 || selected.is_empty() {
        return;
    }

    let known_paths = document
        .sections
        .iter()
        .map(|section| section.path.clone())
        .collect::<BTreeSet<_>>();
    let graph = document.graph_view();
    let mut frontier = selected.clone();

    for _ in 0..graph_depth {
        let mut next = BTreeSet::new();
        for edge in &graph.edges {
            if frontier.contains(&edge.from) && known_paths.contains(&edge.to) {
                next.insert(edge.to.clone());
            }
            if frontier.contains(&edge.to) && known_paths.contains(&edge.from) {
                next.insert(edge.from.clone());
            }
        }

        let before = selected.len();
        selected.extend(next.iter().cloned());
        if selected.len() == before {
            break;
        }
        frontier = next;
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
}
