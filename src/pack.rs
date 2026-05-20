use crate::commands::query_document_with_budget;
use crate::{CompactMode, RbmemDocument, RbmemError};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackConfig {
    pub name: String,
    pub include: Vec<String>,
    pub query: Option<String>,
    pub graph_depth: usize,
    pub mode: Option<CompactMode>,
    pub max_tokens: Option<usize>,
}

pub fn default_pack_file(file: &Path) -> PathBuf {
    file.parent()
        .map(|parent| parent.join(".rbmempacks"))
        .unwrap_or_else(|| PathBuf::from(".rbmempacks"))
}

pub fn parse_pack_config(text: &str, name: &str) -> Result<PackConfig, RbmemError> {
    let mut current: Option<PackConfig> = None;
    let mut in_include = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(pack_name) = trimmed
            .strip_prefix("[pack:")
            .and_then(|value| value.strip_suffix(']'))
        {
            if current.as_ref().is_some_and(|pack| pack.name == name) {
                return Ok(current.unwrap());
            }
            current = Some(PackConfig {
                name: pack_name.trim().to_string(),
                ..PackConfig::default()
            });
            in_include = false;
            continue;
        }

        let Some(pack) = current.as_mut() else {
            continue;
        };
        if pack.name != name {
            continue;
        }

        if trimmed == "include:" {
            in_include = true;
            continue;
        }

        if in_include && trimmed.starts_with("- ") {
            pack.include.push(trimmed[2..].trim().to_string());
            continue;
        }
        in_include = false;

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key.trim() {
            "query" | "task" => pack.query = Some(value.to_string()),
            "graph_depth" => pack.graph_depth = value.parse::<usize>().unwrap_or(0),
            "mode" => pack.mode = Some(CompactMode::from_str(value)?),
            "max_tokens" => pack.max_tokens = value.parse::<usize>().ok(),
            _ => {}
        }
    }

    current
        .filter(|pack| pack.name == name)
        .ok_or_else(|| RbmemError::Parse(format!("pack '{name}' not found")))
}

pub fn pack_document(
    document: &RbmemDocument,
    pack: &PackConfig,
    include_parents: bool,
    max_tokens: Option<usize>,
) -> RbmemDocument {
    let mut selected = BTreeSet::new();

    for include in &pack.include {
        for section in &document.sections {
            if section.path == *include || section.path.starts_with(&format!("{include}.")) {
                selected.insert(section.path.clone());
            }
        }
    }

    if let Some(query) = &pack.query {
        selected.extend(
            query_document_with_budget(document, query, include_parents, pack.graph_depth, max_tokens)
                .sections
                .into_iter()
                .map(|section| section.path),
        );
    }

    if include_parents {
        include_parent_sections(document, &mut selected);
    }
    include_graph_neighbors(document, &mut selected, pack.graph_depth);

    if let Some(budget) = max_tokens {
        let mut ranked: Vec<_> = selected.iter().cloned().collect();
        ranked.sort();
        let mut result = BTreeSet::new();
        let mut used = 0usize;
        for path in ranked {
            let tokens = document
                .sections
                .iter()
                .find(|s| s.path == path)
                .map(|s| s.content.len() / 4)
                .unwrap_or(0)
                .max(1);
            if used + tokens > budget && !result.is_empty() {
                break;
            }
            used += tokens;
            result.insert(path);
        }
        selected = result;
    }

    subset_document(document, selected)
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
