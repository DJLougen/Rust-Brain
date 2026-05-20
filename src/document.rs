use crate::version::{is_supported_format_version, RBMEM_FORMAT_LABEL, RBMEM_FORMAT_VERSION};
use chrono::{DateTime, Duration, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::{self, Display};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RbmemError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("cryptography error: {0}")]
    Crypto(String),

    #[error("invalid section type: {0}")]
    InvalidSectionType(String),

    #[error("document validation failed")]
    InvalidDocument,

    #[error("not found: {0}")]
    NotFound(String),
}

/// Controls how parsed timestamps are treated.
///
/// `Preserve` is useful when simply reading a trusted file. `Protect` is used
/// when importing or updating LLM-produced content: incoming timestamps are
/// ignored and replaced by the tool's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampPolicy {
    Preserve,
    Protect { now: DateTime<Utc> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
    pub purpose: String,
    pub generated_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub created_by: String,
    pub default_expiry_days: Option<i64>,
    pub compact_mode: CompactMode,
}

impl Meta {
    pub fn new(now: DateTime<Utc>, created_by: impl Into<String>) -> Self {
        Self {
            version: RBMEM_FORMAT_VERSION.to_string(),
            source_version: None,
            purpose: "personal-agent-memory".to_string(),
            generated_at: now,
            last_updated: now,
            valid_until: None,
            created_by: created_by.into(),
            default_expiry_days: None,
            compact_mode: CompactMode::Full,
        }
    }

    pub fn enforce_v13(&mut self, warnings: &mut Vec<String>) {
        if self.version != RBMEM_FORMAT_VERSION {
            let original = self.version.clone();
            self.source_version = Some(self.version.clone());
            if is_supported_format_version(&original) {
                warnings.push(format!(
                    "legacy document version '{original}' was normalized to {RBMEM_FORMAT_LABEL}"
                ));
            } else {
                warnings.push(format!(
                    "document version '{original}' was normalized to {RBMEM_FORMAT_LABEL}"
                ));
            }
            self.version = RBMEM_FORMAT_VERSION.to_string();
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompactMode {
    #[default]
    Full,
    Compact,
    Minified,
}

impl Display for CompactMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompactMode::Full => f.write_str("full"),
            CompactMode::Compact => f.write_str("compact"),
            CompactMode::Minified => f.write_str("minified"),
        }
    }
}

impl FromStr for CompactMode {
    type Err = RbmemError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" => Ok(Self::Full),
            "compact" => Ok(Self::Compact),
            "minified" | "tiny" => Ok(Self::Minified),
            other => Err(RbmemError::Parse(format!("invalid compact mode: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum InferenceStrategy {
    Off,
    Explicit,
    #[default]
    Balanced,
    Aggressive,
}

impl InferenceStrategy {
    fn effective_min_confidence(self, min_confidence: f64) -> f64 {
        let min_confidence = min_confidence.clamp(0.0, 1.0);
        match self {
            Self::Aggressive => (min_confidence - 0.10).clamp(0.0, 1.0),
            _ => min_confidence,
        }
    }

    fn allows_candidate_kind(self, kind: InferredRelationKind) -> bool {
        match self {
            Self::Off => false,
            Self::Explicit => kind == InferredRelationKind::Explicit,
            Self::Balanced | Self::Aggressive => true,
        }
    }
}

impl Display for InferenceStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => f.write_str("off"),
            Self::Explicit => f.write_str("explicit"),
            Self::Balanced => f.write_str("balanced"),
            Self::Aggressive => f.write_str("aggressive"),
        }
    }
}

impl FromStr for InferenceStrategy {
    type Err = RbmemError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "disabled" => Ok(Self::Off),
            "explicit" | "strict" | "conservative" => Ok(Self::Explicit),
            "balanced" | "default" => Ok(Self::Balanced),
            "aggressive" => Ok(Self::Aggressive),
            other => Err(RbmemError::Parse(format!(
                "invalid inference strategy: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SectionType {
    #[default]
    Text,
    List,
    Json,
    Timeline,
    Template,
    HermesMemory,
    Encrypted,
    Conflict,
    Guards,
    Review,
}

impl SectionType {
    pub fn as_str(self) -> &'static str {
        match self {
            SectionType::Text => "text",
            SectionType::List => "list",
            SectionType::Json => "json",
            SectionType::Timeline => "timeline",
            SectionType::Template => "template",
            SectionType::HermesMemory => "hermes:memory",
            SectionType::Encrypted => "encrypted",
            SectionType::Conflict => "conflict",
            SectionType::Guards => "guards",
            SectionType::Review => "review",
        }
    }
}

impl FromStr for SectionType {
    type Err = RbmemError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "list" => Ok(Self::List),
            "json" => Ok(Self::Json),
            "timeline" => Ok(Self::Timeline),
            "template" => Ok(Self::Template),
            "hermes:memory" | "hermes_memory" | "hermes-memory" => Ok(Self::HermesMemory),
            "encrypted" => Ok(Self::Encrypted),
            "conflict" => Ok(Self::Conflict),
            "guards" => Ok(Self::Guards),
            "review" => Ok(Self::Review),
            other => Err(RbmemError::InvalidSectionType(other.to_string())),
        }
    }
}

impl Display for SectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedPayload {
    pub nonce: String,
    pub ciphertext: String,
    pub encrypted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Temporal {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Temporal {
    pub fn new(now: DateTime<Utc>, default_expiry_days: Option<i64>) -> Self {
        Self {
            created_at: now,
            updated_at: now,
            expires_at: default_expiry_days.map(|days| now + Duration::days(days)),
        }
    }

    pub fn protected(now: DateTime<Utc>, default_expiry_days: Option<i64>) -> Self {
        Self::new(now, default_expiry_days)
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|expiry| expiry < now)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphRelation {
    pub to: String,
    #[serde(rename = "type")]
    pub relation_type: String,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub inferred: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GraphInfo {
    pub node_type: Option<String>,
    pub relations: Vec<GraphRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceInfo {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Section {
    pub path: String,
    pub section_type: SectionType,
    pub temporal: Temporal,
    pub source: Option<SourceInfo>,
    pub graph: Option<GraphInfo>,
    pub encrypted: Option<EncryptedPayload>,
    pub content: String,
}

impl Section {
    pub fn new(path: impl Into<String>, section_type: SectionType, now: DateTime<Utc>) -> Self {
        Self {
            path: path.into(),
            section_type,
            temporal: Temporal::new(now, None),
            source: None,
            graph: None,
            encrypted: None,
            content: String::new(),
        }
    }

    pub fn path_parts(&self) -> Vec<&str> {
        self.path
            .split('.')
            .filter(|part| !part.is_empty())
            .collect()
    }

    pub fn parent_paths(&self) -> Vec<String> {
        let parts = self.path_parts();
        (1..parts.len()).map(|i| parts[..i].join(".")).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedSection {
    pub path: String,
    pub section_type: SectionType,
    pub temporal: Temporal,
    pub source: Option<SourceInfo>,
    pub graph: Option<GraphInfo>,
    pub encrypted: Option<EncryptedPayload>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_until: Option<DateTime<Utc>>,
    pub inferred: bool,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphView {
    pub nodes: Vec<String>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RbmemDocument {
    pub meta: Meta,
    pub sections: Vec<Section>,
    #[serde(skip)]
    pub warnings: Vec<String>,
}

impl RbmemDocument {
    pub fn new(now: DateTime<Utc>, created_by: impl Into<String>) -> Self {
        Self {
            meta: Meta::new(now, created_by),
            sections: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if !is_supported_format_version(&self.meta.version) {
            warnings.push(format!(
                "expected RBMEM version {RBMEM_FORMAT_VERSION}, found {}",
                self.meta.version
            ));
        }

        let mut seen = BTreeSet::new();
        for section in &self.sections {
            if section.path.trim().is_empty() {
                warnings.push("section with empty path".to_string());
            }
            if !seen.insert(section.path.clone()) {
                warnings.push(format!("duplicate section path '{}'", section.path));
            }
            if section.section_type == SectionType::Json {
                if let Err(error) = serde_json::from_str::<Value>(&section.content) {
                    warnings.push(format!(
                        "section '{}' is type json but content is not valid JSON: {}",
                        section.path, error
                    ));
                }
            }
        }

        warnings
    }

    pub fn enforce_protected_timestamps(&mut self, now: DateTime<Utc>) {
        self.meta.last_updated = now;

        for section in &mut self.sections {
            section.temporal.updated_at = now;
            if section.temporal.created_at > now {
                section.temporal.created_at = now;
            }
        }
    }

    pub fn infer_relations(&mut self, now: DateTime<Utc>, min_confidence: f64) -> usize {
        self.infer_relations_with_strategy(now, min_confidence, InferenceStrategy::Balanced)
    }

    pub fn infer_relations_with_strategy(
        &mut self,
        now: DateTime<Utc>,
        min_confidence: f64,
        strategy: InferenceStrategy,
    ) -> usize {
        if strategy == InferenceStrategy::Off {
            return 0;
        }

        let min_confidence = strategy.effective_min_confidence(min_confidence);
        let known_paths: Vec<String> = self
            .sections
            .iter()
            .map(|section| section.path.clone())
            .collect();
        let mut added = 0;

        for section in &mut self.sections {
            let inferred =
                infer_section_relations(section, &known_paths, now, min_confidence, strategy);
            if inferred.is_empty() {
                continue;
            }

            let graph = section.graph.get_or_insert_with(GraphInfo::default);
            for relation in inferred {
                let already_present = graph.relations.iter().any(|existing| {
                    // A hand-written relation to the same target is more
                    // specific than our prose heuristic, so inference leaves it
                    // alone instead of adding a second edge.
                    existing.to == relation.to
                });
                if !already_present {
                    graph.relations.push(relation);
                    added += 1;
                }
            }
        }

        if added > 0 {
            self.meta.last_updated = now;
        }

        added
    }

    pub fn upsert_section(
        &mut self,
        path: &str,
        section_type: SectionType,
        content: String,
        now: DateTime<Utc>,
    ) {
        self.meta.last_updated = now;

        if let Some(section) = self
            .sections
            .iter_mut()
            .find(|section| section.path == path)
        {
            let merge_type = if section.section_type == SectionType::HermesMemory {
                SectionType::HermesMemory
            } else {
                section_type
            };
            section.section_type = section_type;
            section.encrypted = None;
            section.content = merge_update_content(merge_type, &section.content, &content);
            section.temporal.updated_at = now;
            return;
        }

        let mut section = Section::new(path, section_type, now);
        section.temporal.expires_at = self
            .meta
            .default_expiry_days
            .map(|days| now + Duration::days(days));
        section.content = content;
        self.sections.push(section);
        self.sections
            .sort_by(|left, right| left.path.cmp(&right.path));
    }

    pub fn prune_expired(&mut self, now: DateTime<Utc>) -> usize {
        let before = self.sections.len();
        self.sections
            .retain(|section| !section.temporal.is_expired(now));
        let removed = before - self.sections.len();
        if removed > 0 {
            self.meta.last_updated = now;
        }
        removed
    }

    pub fn resolved_sections(&self) -> Vec<ResolvedSection> {
        let by_path: HashMap<&str, &Section> = self
            .sections
            .iter()
            .map(|section| (section.path.as_str(), section))
            .collect();

        self.sections
            .iter()
            .map(|section| {
                let mut chain = Vec::new();
                for parent in section.parent_paths() {
                    if let Some(parent_section) = by_path.get(parent.as_str()) {
                        chain.push(*parent_section);
                    }
                }
                chain.push(section);
                merge_section_chain(&chain)
            })
            .collect()
    }

    pub fn graph_view(&self) -> GraphView {
        let mut nodes = BTreeSet::new();
        let mut edges = Vec::new();

        for section in &self.sections {
            nodes.insert(section.path.clone());

            let parts = section.path_parts();
            for depth in 1..parts.len() {
                let parent = parts[..depth].join(".");
                let child = parts[..=depth].join(".");
                nodes.insert(parent.clone());
                nodes.insert(child.clone());
                edges.push(GraphEdge {
                    from: parent,
                    to: child,
                    edge_type: "contains".to_string(),
                    valid_from: Some(section.temporal.created_at),
                    valid_until: section.temporal.expires_at,
                    inferred: false,
                    confidence: None,
                });
            }

            if let Some(graph) = &section.graph {
                for relation in &graph.relations {
                    nodes.insert(relation.to.clone());
                    edges.push(GraphEdge {
                        from: section.path.clone(),
                        to: relation.to.clone(),
                        edge_type: relation.relation_type.clone(),
                        valid_from: relation.valid_from,
                        valid_until: relation.valid_until,
                        inferred: relation.inferred,
                        confidence: relation.confidence,
                    });
                }
            }
        }

        GraphView {
            nodes: nodes.into_iter().collect(),
            edges,
        }
    }

    #[cfg(feature = "graph")]
    pub fn petgraph(&self) -> petgraph::Graph<String, String> {
        let view = self.graph_view();
        let mut graph = petgraph::Graph::<String, String>::new();
        let mut indices = BTreeMap::new();

        for node in view.nodes {
            let index = graph.add_node(node.clone());
            indices.insert(node, index);
        }

        for edge in view.edges {
            if let (Some(from), Some(to)) = (indices.get(&edge.from), indices.get(&edge.to)) {
                graph.add_edge(*from, *to, edge.edge_type);
            }
        }

        graph
    }

    pub fn graph_as_dot(&self) -> String {
        let view = self.graph_view();
        let mut output = String::from("digraph rbmem {\n");

        for node in view.nodes {
            output.push_str(&format!("  \"{}\";\n", escape_dot(&node)));
        }

        for edge in view.edges {
            output.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                escape_dot(&edge.from),
                escape_dot(&edge.to),
                escape_dot(&edge_label(&edge))
            ));
        }

        output.push_str("}\n");
        output
    }

    pub fn tree(&self) -> String {
        let mut root = TreeNode::default();
        for section in &self.sections {
            root.insert(&section.path_parts());
        }

        let mut output = String::new();
        root.render("", &mut output);
        output
    }

    pub fn timeline(&self) -> Vec<&Section> {
        let mut sections: Vec<&Section> = self
            .sections
            .iter()
            .filter(|section| section.section_type == SectionType::Timeline)
            .collect();
        sections.sort_by_key(|section| section.temporal.created_at);
        sections
    }

    pub fn to_rbmem_string(&self) -> String {
        self.to_rbmem_string_with_options(RbmemWriteOptions::canonical())
    }

    pub fn to_human_rbmem_string(&self) -> String {
        self.to_rbmem_string_with_options(RbmemWriteOptions::human())
    }

    pub fn to_compact_string(&self, resolve: bool, now: DateTime<Utc>) -> String {
        let mut output = String::new();
        output.push_str("meta:\n");
        output.push_str(&format!("  version: {}\n", self.meta.version));
        if let Some(source_version) = &self.meta.source_version {
            output.push_str(&format!(
                "  _source_version: \"{}\"\n",
                escape_string(source_version)
            ));
        }
        output.push_str(&format!(
            "  purpose: \"{}\"\n\n",
            escape_string(&self.meta.purpose)
        ));

        if resolve {
            for section in self.resolved_sections() {
                output.push_str(&format!("[SECTION: {}]\n", section.path));
                output.push_str(&format!("type: {}\n", section.section_type));
                if section.temporal.is_expired(now) {
                    output.push_str("temporal:\n");
                    output.push_str(&format!(
                        "  expires_at: {}\n",
                        format_optional_time(section.temporal.expires_at)
                    ));
                }
                if section.section_type == SectionType::Encrypted {
                    write_encrypted_fields(&mut output, section.encrypted.as_ref());
                } else {
                    write_content_block(&mut output, &section.content, RbmemWriteStyle::Canonical);
                }
                output.push_str("[END SECTION]\n\n");
            }
        } else {
            for section in &self.sections {
                output.push_str(&format!("[SECTION: {}]\n", section.path));
                output.push_str(&format!("type: {}\n", section.section_type));
                if section.temporal.is_expired(now) {
                    output.push_str("temporal:\n");
                    output.push_str(&format!(
                        "  expires_at: {}\n",
                        format_optional_time(section.temporal.expires_at)
                    ));
                }
                if section.section_type == SectionType::Encrypted {
                    write_encrypted_fields(&mut output, section.encrypted.as_ref());
                } else {
                    write_content_block(&mut output, &section.content, RbmemWriteStyle::Canonical);
                }
                output.push_str("[END SECTION]\n\n");
            }
        }

        output
    }

    pub fn to_minified_string(&self, resolve: bool) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "@ v={} purpose=\"{}\"\n",
            self.meta.version,
            escape_string(&self.meta.purpose)
        ));

        if resolve {
            for section in self.resolved_sections() {
                write_minified_section(
                    &mut output,
                    &section.path,
                    section.section_type,
                    &section.content,
                );
            }
        } else {
            for section in &self.sections {
                write_minified_section(
                    &mut output,
                    &section.path,
                    section.section_type,
                    &section.content,
                );
            }
        }

        output
    }

    pub fn to_rbmem_string_hiding_empty_temporal(&self) -> String {
        self.to_rbmem_string_with_options(RbmemWriteOptions {
            style: RbmemWriteStyle::Canonical,
            hide_empty_temporal: true,
        })
    }

    fn to_rbmem_string_with_options(&self, options: RbmemWriteOptions) -> String {
        let mut output = String::new();
        match options.style {
            RbmemWriteStyle::Canonical => {
                output.push_str(&format!(
                    "rbmem# {RBMEM_FORMAT_LABEL} - Rust-Brain Memory Format\n\n"
                ));
            }
            RbmemWriteStyle::Human => {
                output.push_str(&format!("# {RBMEM_FORMAT_LABEL} human-editable file\n"));
                output.push_str("# The parser accepts these short section delimiters.\n\n");
            }
        }
        output.push_str("meta:\n");
        output.push_str(&format!("  version: {}\n", self.meta.version));
        output.push_str(&format!(
            "  purpose: \"{}\"\n",
            escape_string(&self.meta.purpose)
        ));
        output.push_str(&format!(
            "  generated_at: \"{}\"\n",
            self.meta.generated_at.to_rfc3339()
        ));
        output.push_str(&format!(
            "  last_updated: \"{}\"\n",
            self.meta.last_updated.to_rfc3339()
        ));
        output.push_str(&format!(
            "  valid_until: {}\n",
            format_optional_time(self.meta.valid_until)
        ));
        output.push_str(&format!(
            "  created_by: \"{}\"\n",
            escape_string(&self.meta.created_by)
        ));
        output.push_str(&format!(
            "  default_expiry_days: {}\n",
            self.meta
                .default_expiry_days
                .map(|days| days.to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
        output.push_str(&format!("  compact_mode: {}\n\n", self.meta.compact_mode));

        for section in &self.sections {
            match options.style {
                RbmemWriteStyle::Canonical => {
                    output.push_str(&format!("[SECTION: {}]\n", section.path));
                }
                RbmemWriteStyle::Human => {
                    output.push_str(&format!("=== SECTION: {}\n", section.path));
                    output
                        .push_str("# Edit type/content freely; timestamps remain tool-managed.\n");
                }
            }
            output.push_str(&format!("type: {}\n", section.section_type));
            if !(options.hide_empty_temporal && section_has_default_temporal(section)) {
                output.push_str("temporal:\n");
                output.push_str(&format!(
                    "  created_at: \"{}\"\n",
                    section.temporal.created_at.to_rfc3339()
                ));
                output.push_str(&format!(
                    "  updated_at: \"{}\"\n",
                    section.temporal.updated_at.to_rfc3339()
                ));
                output.push_str(&format!(
                    "  expires_at: {}\n",
                    format_optional_time(section.temporal.expires_at)
                ));
            }

            if let Some(source) = &section.source {
                output.push_str("source:\n");
                output.push_str(&format!("  kind: \"{}\"\n", escape_string(&source.kind)));
                if let Some(path) = &source.path {
                    output.push_str(&format!("  path: \"{}\"\n", escape_string(path)));
                }
                if let Some(actor) = &source.actor {
                    output.push_str(&format!("  actor: \"{}\"\n", escape_string(actor)));
                }
                if let Some(hash) = &source.hash {
                    output.push_str(&format!("  hash: \"{}\"\n", escape_string(hash)));
                }
            }

            if let Some(graph) = &section.graph {
                output.push_str("graph:\n");
                if let Some(node_type) = &graph.node_type {
                    output.push_str(&format!("  node_type: \"{}\"\n", escape_string(node_type)));
                }
                if !graph.relations.is_empty() {
                    output.push_str("  relations:\n");
                    for relation in &graph.relations {
                        output
                            .push_str(&format!("    - to: \"{}\"\n", escape_string(&relation.to)));
                        output.push_str(&format!(
                            "      type: \"{}\"\n",
                            escape_string(&relation.relation_type)
                        ));
                        output.push_str(&format!(
                            "      valid_from: {}\n",
                            format_optional_time(relation.valid_from)
                        ));
                        output.push_str(&format!(
                            "      valid_until: {}\n",
                            format_optional_time(relation.valid_until)
                        ));
                        if relation.inferred {
                            output.push_str("      inferred: true\n");
                        }
                        if let Some(confidence) = relation.confidence {
                            output.push_str(&format!(
                                "      confidence: {:.2}\n",
                                confidence.clamp(0.0, 1.0)
                            ));
                        }
                    }
                }
            }

            if section.section_type == SectionType::Encrypted {
                write_encrypted_fields(&mut output, section.encrypted.as_ref());
            } else {
                write_content_block(&mut output, &section.content, options.style);
            }
            match options.style {
                RbmemWriteStyle::Canonical => output.push_str("[END SECTION]\n\n"),
                RbmemWriteStyle::Human => output.push_str("=== END SECTION\n\n"),
            }
        }

        output
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RbmemWriteStyle {
    Canonical,
    Human,
}

#[derive(Debug, Clone, Copy)]
struct RbmemWriteOptions {
    style: RbmemWriteStyle,
    hide_empty_temporal: bool,
}

impl RbmemWriteOptions {
    fn canonical() -> Self {
        Self {
            style: RbmemWriteStyle::Canonical,
            hide_empty_temporal: false,
        }
    }

    fn human() -> Self {
        Self {
            style: RbmemWriteStyle::Human,
            hide_empty_temporal: false,
        }
    }
}

fn merge_section_chain(chain: &[&Section]) -> ResolvedSection {
    let mut resolved = chain[0].clone();

    for child in chain.iter().skip(1) {
        let parent_type = resolved.section_type;
        let parent_content = resolved.content.clone();
        resolved.path = child.path.clone();
        resolved.section_type = child.section_type;
        resolved.temporal = child.temporal.clone();
        if child.source.is_some() {
            resolved.source = child.source.clone();
        }
        resolved.graph = merge_graph(resolved.graph.as_ref(), child.graph.as_ref());
        resolved.content = merge_content(
            parent_type,
            &parent_content,
            child.section_type,
            &child.content,
        );
    }

    ResolvedSection {
        path: resolved.path,
        section_type: resolved.section_type,
        temporal: resolved.temporal,
        source: resolved.source,
        graph: resolved.graph,
        encrypted: resolved.encrypted,
        content: resolved.content,
    }
}

fn merge_graph(parent: Option<&GraphInfo>, child: Option<&GraphInfo>) -> Option<GraphInfo> {
    match (parent, child) {
        (None, None) => None,
        (Some(parent), None) => Some(parent.clone()),
        (None, Some(child)) => Some(child.clone()),
        (Some(parent), Some(child)) => {
            let mut merged = parent.clone();
            if child.node_type.is_some() {
                merged.node_type = child.node_type.clone();
            }
            merged.relations.extend(child.relations.clone());
            Some(merged)
        }
    }
}

fn merge_content(
    parent_type: SectionType,
    parent_content: &str,
    child_type: SectionType,
    child_content: &str,
) -> String {
    if child_content.trim().is_empty() {
        return parent_content.to_string();
    }

    match (parent_type, child_type) {
        (SectionType::List, SectionType::List) => append_lists(parent_content, child_content),
        (SectionType::HermesMemory, SectionType::HermesMemory) => {
            append_memory_entries(parent_content, child_content)
        }
        (SectionType::Json, SectionType::Json) => {
            deep_merge_json_text(parent_content, child_content)
        }
        _ => child_content.to_string(),
    }
}

fn merge_update_content(section_type: SectionType, existing: &str, incoming: &str) -> String {
    match section_type {
        SectionType::HermesMemory | SectionType::Timeline | SectionType::List => {
            append_memory_entries(existing, incoming)
        }
        _ => incoming.to_string(),
    }
}

fn infer_section_relations(
    section: &Section,
    known_paths: &[String],
    now: DateTime<Utc>,
    min_confidence: f64,
    strategy: InferenceStrategy,
) -> Vec<GraphRelation> {
    let text = normalize_for_matching(&section.content);
    let mut relations = Vec::new();

    for target in known_paths {
        if target == &section.path {
            continue;
        }

        if let Some(candidate) = infer_relation_candidate(&text, target, strategy) {
            if candidate.confidence < min_confidence {
                continue;
            }

            relations.push(GraphRelation {
                to: target.clone(),
                relation_type: candidate.relation_type,
                valid_from: Some(now),
                valid_until: None,
                inferred: true,
                confidence: Some(candidate.confidence),
            });
        }
    }

    relations
}

#[derive(Debug, Clone)]
struct InferredRelationCandidate {
    relation_type: String,
    confidence: f64,
    kind: InferredRelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferredRelationKind {
    Explicit,
    VerbWindow,
    Mention,
}

fn infer_relation_candidate(
    text: &str,
    target_path: &str,
    strategy: InferenceStrategy,
) -> Option<InferredRelationCandidate> {
    let aliases = target_aliases(target_path);
    let mut best: Option<InferredRelationCandidate> = None;

    for alias in aliases {
        if alias.is_empty() || !contains_phrase(text, &alias) {
            continue;
        }

        for pattern in RELATION_PATTERNS {
            if relation_pattern_matches(text, pattern.phrase, &alias) {
                push_best(
                    &mut best,
                    InferredRelationCandidate {
                        relation_type: pattern.relation_type.to_string(),
                        confidence: pattern.confidence,
                        kind: InferredRelationKind::Explicit,
                    },
                    strategy,
                );
            }
        }

        if verb_near_alias(text, &alias) {
            push_best(
                &mut best,
                InferredRelationCandidate {
                    relation_type: "references".to_string(),
                    confidence: 0.72,
                    kind: InferredRelationKind::VerbWindow,
                },
                strategy,
            );
        }

        push_best(
            &mut best,
            InferredRelationCandidate {
                relation_type: "mentions".to_string(),
                confidence: if alias == normalize_for_matching(target_path) {
                    0.68
                } else {
                    0.60
                },
                kind: InferredRelationKind::Mention,
            },
            strategy,
        );
    }

    best
}

#[derive(Debug, Clone, Copy)]
struct RelationPattern {
    phrase: &'static str,
    relation_type: &'static str,
    confidence: f64,
}

const RELATION_PATTERNS: &[RelationPattern] = &[
    RelationPattern {
        phrase: "depends on",
        relation_type: "depends_on",
        confidence: 0.95,
    },
    RelationPattern {
        phrase: "requires",
        relation_type: "requires",
        confidence: 0.94,
    },
    RelationPattern {
        phrase: "uses",
        relation_type: "uses",
        confidence: 0.92,
    },
    RelationPattern {
        phrase: "inherits from",
        relation_type: "inherits",
        confidence: 0.95,
    },
    RelationPattern {
        phrase: "extends",
        relation_type: "extends",
        confidence: 0.90,
    },
    RelationPattern {
        phrase: "collaborates with",
        relation_type: "collaborates_with",
        confidence: 0.93,
    },
    RelationPattern {
        phrase: "calls",
        relation_type: "calls",
        confidence: 0.90,
    },
    RelationPattern {
        phrase: "imports",
        relation_type: "imports",
        confidence: 0.91,
    },
    RelationPattern {
        phrase: "references",
        relation_type: "references",
        confidence: 0.88,
    },
];

fn push_best(
    best: &mut Option<InferredRelationCandidate>,
    candidate: InferredRelationCandidate,
    strategy: InferenceStrategy,
) {
    if !strategy.allows_candidate_kind(candidate.kind) {
        return;
    }

    if best
        .as_ref()
        .is_none_or(|current| candidate.confidence > current.confidence)
    {
        *best = Some(candidate);
    }
}

fn relation_pattern_matches(text: &str, phrase: &str, alias: &str) -> bool {
    let direct = format!("{phrase} {alias}");
    let passive = format!("{alias} {phrase}");
    let found = text.contains(&direct) || text.contains(&passive);
    if !found {
        return false;
    }
    // Check for negation in the surrounding context
    let match_pos = text.find(&direct).or_else(|| text.find(&passive));
    if let Some(pos) = match_pos {
        let window_start = pos.saturating_sub(40);
        let window_end = (pos + direct.len().max(passive.len())).min(text.len());
        let window = &text[window_start..window_end];
        if is_negated(window) {
            return false;
        }
    }
    true
}

const NEGATION_WORDS: &[&str] = &[
    "not",
    "no",
    "don't",
    "doesn't",
    "didn't",
    "won't",
    "can't",
    "cannot",
    "never",
    "avoid",
    "avoiding",
    "without",
    "instead of",
    "rather than",
    "neither",
    "nor",
    "deny",
    "denies",
    "refuse",
    "refuses",
];

fn is_negated(window: &str) -> bool {
    let lower = window.to_ascii_lowercase();
    NEGATION_WORDS.iter().any(|word| {
        let needle = format!(" {word} ");
        let padded = format!(" {lower} ");
        padded.contains(&needle) || lower.starts_with(&format!("{word} "))
    })
}

fn verb_near_alias(text: &str, alias: &str) -> bool {
    let words = text.split_whitespace().collect::<Vec<_>>();
    let alias_words = alias.split_whitespace().collect::<Vec<_>>();
    if alias_words.is_empty() {
        return false;
    }

    for index in 0..words.len() {
        if !window_matches(&words[index..], &alias_words) {
            continue;
        }

        let start = index.saturating_sub(6);
        let end = (index + alias_words.len() + 6).min(words.len());
        let local_window = &words[start..end];
        if RELATION_PATTERNS
            .iter()
            .any(|pattern| contains_word_phrase(local_window, pattern.phrase))
        {
            if local_window.iter().any(|word| {
                NEGATION_WORDS
                    .iter()
                    .any(|neg| word.trim_matches(|c: char| !c.is_ascii_alphanumeric()) == *neg)
            }) {
                continue;
            }
            return true;
        }
    }

    false
}

fn contains_word_phrase(words: &[&str], phrase: &str) -> bool {
    let phrase_words = phrase.split_whitespace().collect::<Vec<_>>();
    words
        .windows(phrase_words.len())
        .any(|window| window_matches(window, &phrase_words))
}

fn window_matches(words: &[&str], pattern: &[&str]) -> bool {
    words.len() >= pattern.len()
        && words
            .iter()
            .zip(pattern.iter())
            .take(pattern.len())
            .all(|(word, expected)| word == expected)
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    let needle = format!(" {phrase} ");
    let haystack = format!(" {text} ");
    haystack.contains(&needle)
}

fn target_aliases(path: &str) -> Vec<String> {
    let normalized_path = normalize_for_matching(path);
    let leaf = path.rsplit('.').next().unwrap_or(path);
    let normalized_leaf = normalize_for_matching(leaf);
    let leaf_without_common_suffix = normalized_leaf
        .strip_suffix(" section")
        .unwrap_or(&normalized_leaf)
        .to_string();

    let mut aliases = vec![normalized_path, normalized_leaf, leaf_without_common_suffix];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn normalize_for_matching(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_space = true;

    for ch in value.chars() {
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

fn append_lists(parent: &str, child: &str) -> String {
    parent
        .lines()
        .chain(child.lines())
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn append_memory_entries(existing: &str, incoming: &str) -> String {
    if existing.trim().is_empty() {
        return incoming.trim().to_string();
    }
    if incoming.trim().is_empty() || existing.contains(incoming.trim()) {
        return existing.to_string();
    }

    let mut output = existing.trim_end().to_string();
    output.push('\n');
    output.push_str(incoming.trim());
    output
}

fn deep_merge_json_text(parent: &str, child: &str) -> String {
    let parent_json = serde_json::from_str::<Value>(parent).unwrap_or_else(|_| json!({}));
    let child_json = serde_json::from_str::<Value>(child).unwrap_or_else(|_| json!({}));
    let merged = deep_merge_json(parent_json, child_json);
    serde_json::to_string_pretty(&merged).unwrap_or_else(|_| child.to_string())
}

fn deep_merge_json(parent: Value, child: Value) -> Value {
    match (parent, child) {
        (Value::Object(mut parent), Value::Object(child)) => {
            for (key, child_value) in child {
                let merged_value = match parent.remove(&key) {
                    Some(parent_value) => deep_merge_json(parent_value, child_value),
                    None => child_value,
                };
                parent.insert(key, merged_value);
            }
            Value::Object(parent)
        }
        (Value::Array(mut parent), Value::Array(child)) => {
            parent.extend(child);
            Value::Array(parent)
        }
        (_, child) => child,
    }
}

#[derive(Default)]
struct TreeNode {
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn insert(&mut self, parts: &[&str]) {
        if let Some((first, rest)) = parts.split_first() {
            self.children
                .entry((*first).to_string())
                .or_default()
                .insert(rest);
        }
    }

    fn render(&self, prefix: &str, output: &mut String) {
        for (name, child) in &self.children {
            output.push_str(prefix);
            output.push_str(name);
            output.push('\n');
            let next_prefix = format!("{}  ", prefix);
            child.render(&next_prefix, output);
        }
    }
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn edge_label(edge: &GraphEdge) -> String {
    match (edge.inferred, edge.confidence) {
        (true, Some(confidence)) => format!("{} inferred {:.2}", edge.edge_type, confidence),
        (true, None) => format!("{} inferred", edge.edge_type),
        _ => edge.edge_type.clone(),
    }
}

fn format_optional_time(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|time| format!("\"{}\"", time.to_rfc3339()))
        .unwrap_or_else(|| "null".to_string())
}

fn section_has_default_temporal(section: &Section) -> bool {
    section.temporal.created_at == section.temporal.updated_at
        && section.temporal.expires_at.is_none()
}

fn write_minified_section(
    output: &mut String,
    path: &str,
    section_type: SectionType,
    content: &str,
) {
    let lines = minified_content_lines(content);

    if lines.is_empty() {
        if section_type == SectionType::Text {
            output.push_str(&format!("[{}]\n", path));
        } else {
            output.push_str(&format!("[{}] type={}\n", path, section_type));
        }
        return;
    }

    if lines.len() == 1 {
        if section_type == SectionType::Text {
            output.push_str(&format!("[{}] {}\n", path, lines[0]));
        } else {
            output.push_str(&format!("[{}] type={} {}\n", path, section_type, lines[0]));
        }
        return;
    }

    if section_type == SectionType::Text {
        output.push_str(&format!("[{}]\n", path));
    } else {
        output.push_str(&format!("[{}] type={}\n", path, section_type));
    }
    for line in lines {
        output.push_str(&line);
        output.push('\n');
    }
}

fn minified_content_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('#'))
        .map(collapse_inline_whitespace)
        .collect()
}

fn collapse_inline_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn write_content_block(output: &mut String, content: &str, style: RbmemWriteStyle) {
    let can_write_single_line =
        style == RbmemWriteStyle::Human && !content.contains('\n') && !content.trim().is_empty();

    if can_write_single_line {
        output.push_str("content: ");
        output.push_str(content.trim());
        output.push('\n');
        return;
    }

    output.push_str("content: |\n");
    if content.is_empty() {
        output.push('\n');
    } else {
        for line in content.lines() {
            output.push_str("  ");
            output.push_str(line);
            output.push('\n');
        }
    }
}

fn write_encrypted_fields(output: &mut String, encrypted: Option<&EncryptedPayload>) {
    if let Some(payload) = encrypted {
        output.push_str(&format!("nonce: \"{}\"\n", escape_string(&payload.nonce)));
        output.push_str(&format!(
            "ciphertext: \"{}\"\n",
            escape_string(&payload.ciphertext)
        ));
        output.push_str(&format!(
            "encrypted_at: \"{}\"\n",
            payload.encrypted_at.to_rfc3339()
        ));
    }
}

pub fn graph_view_to_json(view: &GraphView) -> Value {
    let nodes = view.nodes.iter().map(|id| json!({ "id": id })).collect();
    let edges = view
        .edges
        .iter()
        .map(|edge| {
            json!({
                "from": edge.from,
                "to": edge.to,
                "type": edge.edge_type,
                "valid_from": edge.valid_from,
                "valid_until": edge.valid_until,
                "inferred": edge.inferred,
                "confidence": edge.confidence,
                "source": graph_edge_source(edge),
            })
        })
        .collect();

    let mut object = Map::new();
    object.insert("nodes".to_string(), Value::Array(nodes));
    object.insert("edges".to_string(), Value::Array(edges));
    Value::Object(object)
}

fn graph_edge_source(edge: &GraphEdge) -> &'static str {
    if edge.edge_type == "contains" {
        "implicit"
    } else if edge.inferred {
        "inferred"
    } else {
        "manual"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-27T13:10:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn list_children_append_parent_items() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section("prefs", SectionType::List, "- coffee".to_string(), now);
        doc.upsert_section("prefs.morning", SectionType::List, "- tea".to_string(), now);

        let resolved = doc
            .resolved_sections()
            .into_iter()
            .find(|section| section.path == "prefs.morning")
            .unwrap();

        assert_eq!(resolved.content, "- coffee\n- tea");
    }

    #[test]
    fn json_children_deep_merge_objects_and_append_arrays() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "agent",
            SectionType::Json,
            r#"{"settings":{"voice":"quiet"},"tags":["base"]}"#.to_string(),
            now,
        );
        doc.upsert_section(
            "agent.home",
            SectionType::Json,
            r#"{"settings":{"speed":"fast"},"tags":["home"]}"#.to_string(),
            now,
        );

        let resolved = doc
            .resolved_sections()
            .into_iter()
            .find(|section| section.path == "agent.home")
            .unwrap();
        let json: Value = serde_json::from_str(&resolved.content).unwrap();

        assert_eq!(json["settings"]["voice"], "quiet");
        assert_eq!(json["settings"]["speed"], "fast");
        assert_eq!(json["tags"], json!(["base", "home"]));
    }

    #[test]
    fn graph_view_contains_implicit_and_explicit_edges() {
        let now = fixed_time();
        let mut section = Section::new("agent.memory.core", SectionType::Text, now);
        section.graph = Some(GraphInfo {
            node_type: Some("Module".to_string()),
            relations: vec![GraphRelation {
                to: "agent.tools".to_string(),
                relation_type: "uses".to_string(),
                valid_from: Some(now),
                valid_until: None,
                inferred: false,
                confidence: None,
            }],
        });

        let mut doc = RbmemDocument::new(now, "me");
        doc.sections.push(section);

        let graph = doc.graph_view();
        assert!(graph.edges.iter().any(|edge| {
            edge.from == "agent.memory"
                && edge.to == "agent.memory.core"
                && edge.edge_type == "contains"
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.from == "agent.memory.core" && edge.to == "agent.tools" && edge.edge_type == "uses"
        }));
    }

    #[test]
    fn prune_removes_expired_sections() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        let mut expired = Section::new("old", SectionType::Text, now);
        expired.temporal.expires_at = Some(now - Duration::days(1));
        doc.sections.push(expired);

        assert_eq!(doc.prune_expired(now), 1);
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn relation_inference_adds_non_destructive_inferred_edges() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "agents.reader",
            SectionType::Text,
            "The reader uses writer.".to_string(),
            now,
        );
        doc.upsert_section(
            "agents.writer",
            SectionType::Text,
            "The writer emits canonical RBMEM.".to_string(),
            now,
        );

        assert_eq!(doc.infer_relations(now, 0.6), 1);

        let relation = &doc.sections[0].graph.as_ref().unwrap().relations[0];
        assert_eq!(relation.to, "agents.writer");
        assert_eq!(relation.relation_type, "uses");
        assert!(relation.inferred);
        assert_eq!(relation.confidence, Some(0.92));
    }

    #[test]
    fn relation_inference_detects_multiple_verbs_with_confidence() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "runtime.loader",
            SectionType::Text,
            "The loader imports parser, calls executor, and references cache.".to_string(),
            now,
        );
        doc.upsert_section(
            "runtime.parser",
            SectionType::Text,
            "Parser.".to_string(),
            now,
        );
        doc.upsert_section(
            "runtime.executor",
            SectionType::Text,
            "Executor.".to_string(),
            now,
        );
        doc.upsert_section(
            "runtime.cache",
            SectionType::Text,
            "Cache.".to_string(),
            now,
        );

        assert_eq!(doc.infer_relations(now, 0.8), 3);

        let loader = doc
            .sections
            .iter()
            .find(|section| section.path == "runtime.loader")
            .unwrap();
        let relations = &loader.graph.as_ref().unwrap().relations;
        assert!(relations.iter().any(|relation| {
            relation.to == "runtime.parser"
                && relation.relation_type == "imports"
                && relation.confidence == Some(0.91)
        }));
        assert!(relations.iter().any(|relation| {
            relation.to == "runtime.executor"
                && relation.relation_type == "calls"
                && relation.confidence == Some(0.90)
        }));
        assert!(relations.iter().any(|relation| {
            relation.to == "runtime.cache"
                && relation.relation_type == "references"
                && relation.confidence == Some(0.88)
        }));
    }

    #[test]
    fn relation_inference_threshold_filters_weak_mentions() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "notes.alpha",
            SectionType::Text,
            "Alpha mentions beta without an action verb.".to_string(),
            now,
        );
        doc.upsert_section("notes.beta", SectionType::Text, "Beta.".to_string(), now);

        assert_eq!(doc.infer_relations(now, 0.7), 0);
        let alpha_index = doc
            .sections
            .iter()
            .position(|section| section.path == "notes.alpha")
            .unwrap();
        assert!(doc.sections[alpha_index].graph.is_none());
        assert_eq!(doc.infer_relations(now, 0.6), 1);
    }

    #[test]
    fn relation_inference_strategy_off_disables_edges() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "agents.reader",
            SectionType::Text,
            "The reader uses writer.".to_string(),
            now,
        );
        doc.upsert_section(
            "agents.writer",
            SectionType::Text,
            "Writer.".to_string(),
            now,
        );

        assert_eq!(
            doc.infer_relations_with_strategy(now, 0.6, InferenceStrategy::Off),
            0
        );
        assert!(doc.sections[0].graph.is_none());
    }

    #[test]
    fn relation_inference_explicit_strategy_ignores_mentions() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "notes.alpha",
            SectionType::Text,
            "Alpha mentions beta without an action verb.".to_string(),
            now,
        );
        doc.upsert_section("notes.beta", SectionType::Text, "Beta.".to_string(), now);

        assert_eq!(
            doc.infer_relations_with_strategy(now, 0.6, InferenceStrategy::Explicit),
            0
        );
    }

    #[test]
    fn relation_inference_aggressive_strategy_lowers_threshold() {
        let now = fixed_time();
        let mut balanced = RbmemDocument::new(now, "me");
        balanced.upsert_section(
            "notes.alpha",
            SectionType::Text,
            "Alpha mentions beta without an action verb.".to_string(),
            now,
        );
        balanced.upsert_section("notes.beta", SectionType::Text, "Beta.".to_string(), now);
        assert_eq!(
            balanced.infer_relations_with_strategy(now, 0.69, InferenceStrategy::Balanced),
            0
        );

        let mut aggressive = RbmemDocument::new(now, "me");
        aggressive.upsert_section(
            "notes.alpha",
            SectionType::Text,
            "Alpha mentions beta without an action verb.".to_string(),
            now,
        );
        aggressive.upsert_section("notes.beta", SectionType::Text, "Beta.".to_string(), now);
        assert_eq!(
            aggressive.infer_relations_with_strategy(now, 0.69, InferenceStrategy::Aggressive),
            1
        );
    }

    #[test]
    fn relation_inference_never_overwrites_manual_relations() {
        let now = fixed_time();
        let mut section = Section::new("agent.reader", SectionType::Text, now);
        section.content = "The reader uses writer.".to_string();
        section.graph = Some(GraphInfo {
            node_type: None,
            relations: vec![GraphRelation {
                to: "agent.writer".to_string(),
                relation_type: "manual_link".to_string(),
                valid_from: Some(now),
                valid_until: None,
                inferred: false,
                confidence: None,
            }],
        });

        let mut doc = RbmemDocument::new(now, "me");
        doc.sections.push(section);
        doc.upsert_section(
            "agent.writer",
            SectionType::Text,
            "Writer.".to_string(),
            now,
        );

        assert_eq!(doc.infer_relations(now, 0.6), 0);
        let relations = &doc.sections[0].graph.as_ref().unwrap().relations;
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].relation_type, "manual_link");
        assert!(!relations[0].inferred);
    }

    #[test]
    fn graph_json_marks_inferred_manual_and_implicit_edges() {
        let now = fixed_time();
        let mut section = Section::new("agent.reader", SectionType::Text, now);
        section.graph = Some(GraphInfo {
            node_type: None,
            relations: vec![
                GraphRelation {
                    to: "agent.writer".to_string(),
                    relation_type: "uses".to_string(),
                    valid_from: Some(now),
                    valid_until: None,
                    inferred: true,
                    confidence: Some(0.92),
                },
                GraphRelation {
                    to: "agent.policy".to_string(),
                    relation_type: "depends_on".to_string(),
                    valid_from: Some(now),
                    valid_until: None,
                    inferred: false,
                    confidence: None,
                },
            ],
        });

        let mut doc = RbmemDocument::new(now, "me");
        doc.sections.push(section);
        let json = graph_view_to_json(&doc.graph_view());
        let edges = json["edges"].as_array().unwrap();

        assert!(edges
            .iter()
            .any(|edge| { edge["type"] == "contains" && edge["source"] == "implicit" }));
        assert!(edges.iter().any(|edge| {
            edge["type"] == "uses"
                && edge["source"] == "inferred"
                && edge["inferred"] == true
                && edge["confidence"] == 0.92
        }));
        assert!(edges.iter().any(|edge| {
            edge["type"] == "depends_on" && edge["source"] == "manual" && edge["inferred"] == false
        }));
    }

    #[test]
    fn minified_output_is_substantially_smaller_than_compact_output() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section(
            "agent.reader",
            SectionType::Text,
            "The reader validates memory.\n\n# comment removed\nIt uses writer.".to_string(),
            now,
        );
        doc.upsert_section(
            "agent.writer",
            SectionType::Text,
            "The writer emits canonical RBMEM.".to_string(),
            now,
        );

        let compact = doc.to_compact_string(true, now);
        let minified = doc.to_minified_string(true);

        assert!(!minified.contains("type: text"));
        assert!(!minified.contains("content:"));
        assert!(!minified.contains("# comment removed"));
        assert!(minified.len() <= compact.len() * 7 / 10);
    }

    #[test]
    fn hide_empty_temporal_omits_default_temporal_blocks() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "me");
        doc.upsert_section("note", SectionType::Text, "hello".to_string(), now);

        let hidden = doc.to_rbmem_string_hiding_empty_temporal();
        assert!(!hidden.contains("temporal:"));
        assert!(hidden.contains("compact_mode: full"));
    }

    #[test]
    fn section_type_accepts_hermes_memory() {
        assert_eq!(
            <SectionType as std::str::FromStr>::from_str("hermes:memory").unwrap(),
            SectionType::HermesMemory
        );
        assert_eq!(SectionType::HermesMemory.to_string(), "hermes:memory");
    }
}
