use crate::{RbmemDocument, RbmemError, Section, SectionType};
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum DiffFormat {
    Text,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    Ours,
    Theirs,
    Union,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SectionDiffKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectionDiff {
    pub path: String,
    pub kind: SectionDiffKind,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RbmemDiff {
    pub schema: String,
    pub changes: Vec<SectionDiff>,
}

pub fn diff_documents_typed(before: &RbmemDocument, after: &RbmemDocument) -> RbmemDiff {
    let before_by_path = section_map(before);
    let after_by_path = section_map(after);
    let paths = before_by_path
        .keys()
        .chain(after_by_path.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();

    for path in paths {
        match (before_by_path.get(path), after_by_path.get(path)) {
            (None, Some(_)) => changes.push(SectionDiff {
                path: path.to_string(),
                kind: SectionDiffKind::Added,
                fields: Vec::new(),
            }),
            (Some(_), None) => changes.push(SectionDiff {
                path: path.to_string(),
                kind: SectionDiffKind::Removed,
                fields: Vec::new(),
            }),
            (Some(before), Some(after)) => {
                let fields = changed_fields(before, after);
                if !fields.is_empty() {
                    changes.push(SectionDiff {
                        path: path.to_string(),
                        kind: SectionDiffKind::Changed,
                        fields,
                    });
                }
            }
            (None, None) => {}
        }
    }

    RbmemDiff {
        schema: "rbmem.diff.v1".to_string(),
        changes,
    }
}

pub fn diff_with_format(
    before: &RbmemDocument,
    after: &RbmemDocument,
    format: DiffFormat,
) -> Result<String, RbmemError> {
    let diff = diff_documents_typed(before, after);
    match format {
        DiffFormat::Text => Ok(render_diff_text(&diff)),
        DiffFormat::Json => Ok(serde_json::to_string_pretty(&diff)?),
        DiffFormat::Yaml => serde_yaml::to_string(&diff)
            .map_err(|error| RbmemError::Parse(format!("failed to render YAML diff: {error}"))),
    }
}

pub fn merge_documents(
    base: &RbmemDocument,
    local: &RbmemDocument,
    remote: &RbmemDocument,
    strategy: MergeStrategy,
    now: DateTime<Utc>,
) -> RbmemDocument {
    let base_by_path = section_map(base);
    let local_by_path = section_map(local);
    let remote_by_path = section_map(remote);
    let paths = base_by_path
        .keys()
        .chain(local_by_path.keys())
        .chain(remote_by_path.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    let mut merged = base.clone();
    merged.sections.clear();
    merged.meta.last_updated = now;

    for path in paths {
        let base_section = base_by_path.get(path).copied();
        let local_section = local_by_path.get(path).copied();
        let remote_section = remote_by_path.get(path).copied();

        if let Some(section) = merge_section(
            path,
            base_section,
            local_section,
            remote_section,
            strategy,
            now,
        ) {
            merged.sections.push(section);
        }
    }

    merged
        .sections
        .sort_by(|left, right| left.path.cmp(&right.path));
    merged
}

pub fn render_diff_text(diff: &RbmemDiff) -> String {
    let mut output = String::new();
    for change in &diff.changes {
        match change.kind {
            SectionDiffKind::Added => output.push_str(&format!("added: {}\n", change.path)),
            SectionDiffKind::Removed => output.push_str(&format!("removed: {}\n", change.path)),
            SectionDiffKind::Changed => {
                for field in &change.fields {
                    output.push_str(&format!("changed {field}: {}\n", change.path));
                }
            }
        }
    }

    if output.is_empty() {
        output.push_str("no RBMEM differences\n");
    }

    output
}

fn merge_section(
    path: &str,
    base: Option<&Section>,
    local: Option<&Section>,
    remote: Option<&Section>,
    strategy: MergeStrategy,
    now: DateTime<Utc>,
) -> Option<Section> {
    if sections_equal(local, remote) {
        return local.cloned();
    }
    if sections_equal(local, base) {
        return remote.cloned();
    }
    if sections_equal(remote, base) {
        return local.cloned();
    }

    match strategy {
        MergeStrategy::Ours => local.cloned(),
        MergeStrategy::Theirs => remote.cloned(),
        MergeStrategy::Union => union_sections(local, remote),
        MergeStrategy::Manual => Some(conflict_section(path, local, remote, now)),
    }
}

fn union_sections(local: Option<&Section>, remote: Option<&Section>) -> Option<Section> {
    match (local, remote) {
        (Some(local), Some(remote)) => {
            let mut section = local.clone();
            if local.content != remote.content && !remote.content.trim().is_empty() {
                if !section.content.trim().is_empty() {
                    section.content.push('\n');
                }
                section.content.push_str(remote.content.trim());
            }
            Some(section)
        }
        (Some(local), None) => Some(local.clone()),
        (None, Some(remote)) => Some(remote.clone()),
        (None, None) => None,
    }
}

fn conflict_section(
    path: &str,
    local: Option<&Section>,
    remote: Option<&Section>,
    now: DateTime<Utc>,
) -> Section {
    let mut section = Section::new(path, SectionType::Conflict, now);
    section.content = format!(
        "conflict_at: \"{}\"\nlocal_version: |\n{}\nremote_version: |\n{}",
        now.to_rfc3339(),
        indent_block(local.map(|section| section.content.as_str()).unwrap_or("")),
        indent_block(remote.map(|section| section.content.as_str()).unwrap_or(""))
    );
    section
}

fn indent_block(value: &str) -> String {
    value
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn changed_fields(before: &Section, after: &Section) -> Vec<String> {
    let mut fields = Vec::new();
    if before.section_type != after.section_type {
        fields.push("type".to_string());
    }
    if before.content != after.content {
        fields.push("content".to_string());
    }
    if before.temporal != after.temporal {
        fields.push("temporal".to_string());
    }
    if before.source != after.source {
        fields.push("source".to_string());
    }
    if before.graph != after.graph {
        fields.push("graph".to_string());
    }
    if before.encrypted != after.encrypted {
        fields.push("encrypted".to_string());
    }
    fields
}

fn sections_equal(left: Option<&Section>, right: Option<&Section>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => section_semantic_key(left) == section_semantic_key(right),
        (None, None) => true,
        _ => false,
    }
}

fn section_semantic_key(section: &Section) -> SectionSemanticKey<'_> {
    SectionSemanticKey {
        section_type: section.section_type,
        source: section.source.as_ref(),
        graph: section.graph.as_ref(),
        encrypted: section.encrypted.as_ref(),
        content: section.content.as_str(),
    }
}

#[derive(Debug, PartialEq)]
struct SectionSemanticKey<'a> {
    section_type: SectionType,
    source: Option<&'a crate::SourceInfo>,
    graph: Option<&'a crate::document::GraphInfo>,
    encrypted: Option<&'a crate::EncryptedPayload>,
    content: &'a str,
}

fn section_map(document: &RbmemDocument) -> BTreeMap<&str, &Section> {
    document
        .sections
        .iter()
        .map(|section| (section.path.as_str(), section))
        .collect()
}
