//! Forgiving Nom parser for AIF v1.3.
//!
//! This parser is deliberately hand-written and small-function oriented. AIF is
//! meant to survive LLM output, so the parser accepts minor formatting drift,
//! records warnings, and lets `document.rs` re-emit canonical AIF.

use crate::document::{
    AIFDocument, AifError, CompactMode, GraphInfo, GraphRelation, Meta, Section, SectionType,
    Temporal, TimestampPolicy,
};
use chrono::{DateTime, Utc};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while1};
use nom::character::complete::{char, line_ending, multispace0, not_line_ending, space0};
use nom::combinator::opt;
use nom::multi::many0;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::IResult;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub document: AIFDocument,
    pub warnings: Vec<String>,
}

pub fn parse_document(input: &str, policy: TimestampPolicy) -> Result<ParsedDocument, AifError> {
    let normalized = input.replace("\r\n", "\n");
    let now = match policy {
        TimestampPolicy::Preserve => Utc::now(),
        TimestampPolicy::Protect { now } => now,
    };

    let (meta_text, section_text) = split_meta_and_sections(&normalized);
    let mut warnings = Vec::new();
    let mut meta = parse_meta(meta_text, policy, &mut warnings)?;
    meta.enforce_v13(&mut warnings);

    let mut sections = Vec::new();
    let mut remaining = section_text.trim_start();

    while !remaining.trim().is_empty() {
        match parse_section(remaining) {
            Ok((rest, raw)) => {
                sections.push(raw.into_section(&meta, policy, now, &mut warnings));
                remaining = rest.trim_start();
            }
            Err(_) => {
                warnings
                    .push("stopped parsing sections after unrepairable section syntax".to_string());
                break;
            }
        }
    }

    if sections.is_empty() {
        warnings.push("document contains no sections".to_string());
    }

    let document = AIFDocument {
        meta,
        sections,
        warnings: warnings.clone(),
    };

    Ok(ParsedDocument { document, warnings })
}

fn split_meta_and_sections(input: &str) -> (&str, &str) {
    if let Some(index) = input.find("[SECTION:") {
        return input.split_at(index);
    }

    if let Some(index) = input.find("=== SECTION:") {
        return input.split_at(index);
    }

    if let Some(index) = input.find("SECTION:") {
        return input.split_at(index.saturating_sub(1));
    }

    (input, "")
}

fn parse_meta(
    input: &str,
    policy: TimestampPolicy,
    warnings: &mut Vec<String>,
) -> Result<Meta, AifError> {
    let now = match policy {
        TimestampPolicy::Preserve => Utc::now(),
        TimestampPolicy::Protect { now } => now,
    };

    let mut values = HashMap::new();
    let mut in_meta = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed == "meta:" {
            in_meta = true;
            continue;
        }
        if !in_meta || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            values.insert(key.trim().to_string(), clean_scalar(value));
        }
    }

    let mut meta = Meta::new(
        now,
        values
            .get("created_by")
            .cloned()
            .unwrap_or_else(|| "me".to_string()),
    );

    if let Some(version) = values.get("version") {
        meta.version = version.clone();
    } else {
        warnings.push("missing meta.version; defaulted to 1.3".to_string());
    }

    if let Some(purpose) = values.get("purpose") {
        meta.purpose = purpose.clone();
    }

    if let Some(days) = values.get("default_expiry_days") {
        if days != "null" {
            match days.parse::<i64>() {
                Ok(days) => meta.default_expiry_days = Some(days),
                Err(_) => warnings.push("invalid default_expiry_days; treated as null".to_string()),
            }
        }
    }

    if let Some(mode) = values.get("compact_mode") {
        match CompactMode::from_str(mode) {
            Ok(mode) => meta.compact_mode = mode,
            Err(_) => warnings.push("invalid compact_mode; defaulted to full".to_string()),
        }
    }

    if matches!(policy, TimestampPolicy::Preserve) {
        meta.generated_at = parse_time(
            values.get("generated_at"),
            now,
            "meta.generated_at",
            warnings,
        );
        meta.last_updated = parse_time(
            values.get("last_updated"),
            now,
            "meta.last_updated",
            warnings,
        );
        meta.valid_until =
            parse_optional_time(values.get("valid_until"), "meta.valid_until", warnings);
    } else {
        warnings.push("protected meta timestamps replaced with tool time".to_string());
    }

    Ok(meta)
}

#[derive(Debug, Clone)]
struct RawSection {
    path: String,
    section_type: Option<SectionType>,
    temporal_values: HashMap<String, String>,
    graph: Option<GraphInfo>,
    content: String,
}

impl RawSection {
    fn into_section(
        self,
        meta: &Meta,
        policy: TimestampPolicy,
        now: DateTime<Utc>,
        warnings: &mut Vec<String>,
    ) -> Section {
        let section_type = self.section_type.unwrap_or_else(|| {
            warnings.push(format!(
                "section '{}' missing type; defaulted to text",
                self.path
            ));
            SectionType::Text
        });

        let temporal = match policy {
            TimestampPolicy::Protect { .. } => {
                warnings.push(format!(
                    "protected timestamps for section '{}' replaced with tool time",
                    self.path
                ));
                Temporal::protected(now, meta.default_expiry_days)
            }
            TimestampPolicy::Preserve => {
                let created = parse_time(
                    self.temporal_values.get("created_at"),
                    now,
                    "section.created_at",
                    warnings,
                );
                let updated = parse_time(
                    self.temporal_values.get("updated_at"),
                    now,
                    "section.updated_at",
                    warnings,
                );
                let expires = parse_optional_time(
                    self.temporal_values.get("expires_at"),
                    "section.expires_at",
                    warnings,
                );

                Temporal {
                    created_at: created,
                    updated_at: updated,
                    expires_at: expires,
                }
            }
        };

        Section {
            path: self.path,
            section_type,
            temporal,
            graph: self.graph,
            content: self.content,
        }
    }
}

fn parse_section(input: &str) -> IResult<&str, RawSection> {
    // `multispace0` makes the parser forgiving about blank lines before a
    // section. It consumes comments too broadly for a strict language, but AIF
    // sections are delimiter-based, so this is a practical recovery choice.
    let (input, _) = multispace0(input)?;

    if input.starts_with("=== SECTION:") {
        parse_human_section(input)
    } else {
        parse_canonical_section(input)
    }
}

fn parse_canonical_section(input: &str) -> IResult<&str, RawSection> {
    // Some LLMs echo `text[SECTION: path]`; others emit `[SECTION: path]`.
    // `opt(section_prefix)` accepts the first style without requiring it.
    let (input, _) = opt(section_prefix)(input)?;

    // The path is everything after `SECTION:` up to `]`. We trim it later so
    // extra spaces do not matter.
    let (input, path) = delimited(tag("[SECTION:"), take_until("]"), char(']'))(input)?;
    let (input, _) = many0(line_ending)(input)?;

    // The body is bounded by the explicit end marker. This keeps content free
    // to contain almost anything except the marker itself.
    let (input, body) = take_until("[END SECTION]")(input)?;
    let (input, _) = tag("[END SECTION]")(input)?;
    let (input, _) = opt(line_ending)(input)?;

    Ok((input, parse_section_body(path.trim(), body)))
}

fn parse_human_section(input: &str) -> IResult<&str, RawSection> {
    // Human mode keeps the same body grammar but uses a delimiter that is less
    // visually noisy in hand-authored files.
    let (input, _) = tag("=== SECTION:")(input)?;
    let (input, path) = not_line_ending(input)?;
    let (input, _) = many0(line_ending)(input)?;
    let (input, body) = take_until("=== END SECTION")(input)?;
    let (input, _) = tag("=== END SECTION")(input)?;
    let (input, _) = opt(not_line_ending)(input)?;
    let (input, _) = opt(line_ending)(input)?;

    Ok((input, parse_section_body(path.trim(), body)))
}

fn section_prefix(input: &str) -> IResult<&str, &str> {
    // A valid prefix is a small word such as `text` directly before `[SECTION`.
    // If no prefix is present, `opt(section_prefix)` leaves the input alone.
    take_while1(|ch: char| ch.is_ascii_alphabetic())(input)
}

fn parse_section_body(path: &str, body: &str) -> RawSection {
    let mut section_type = None;
    let mut temporal_values = HashMap::new();
    let mut graph = GraphInfo::default();
    let mut saw_graph = false;
    let mut content_lines = Vec::new();
    let mut mode = BodyMode::Top;
    let mut current_relation: Option<GraphRelation> = None;

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed == "temporal:" {
            mode = BodyMode::Temporal;
            continue;
        }
        if trimmed == "graph:" {
            mode = BodyMode::Graph;
            saw_graph = true;
            continue;
        }
        if let Some(content) = trimmed.strip_prefix("content:") {
            if let Some(relation) = current_relation.take() {
                graph.relations.push(relation);
            }
            mode = BodyMode::Content;
            let inline = content.trim();
            if !inline.is_empty() && inline != "|" {
                content_lines.push(clean_inline_content(inline));
            }
            continue;
        }

        match mode {
            BodyMode::Top => {
                if let Some(value) = parse_field_line(trimmed, "type") {
                    section_type = SectionType::from_str(&value).ok();
                }
            }
            BodyMode::Temporal => {
                if let Some((key, value)) = trimmed.split_once(':') {
                    temporal_values.insert(key.trim().to_string(), clean_scalar(value));
                }
            }
            BodyMode::Graph => {
                if let Some(value) = parse_field_line(trimmed, "node_type") {
                    graph.node_type = Some(value);
                    continue;
                }

                if trimmed == "relations:" {
                    continue;
                }

                if let Some(value) = parse_dash_field(trimmed, "to") {
                    if let Some(relation) = current_relation.take() {
                        graph.relations.push(relation);
                    }
                    current_relation = Some(GraphRelation {
                        to: value,
                        relation_type: "related".to_string(),
                        valid_from: None,
                        valid_until: None,
                        inferred: false,
                        confidence: None,
                    });
                    continue;
                }

                if let Some(relation) = current_relation.as_mut() {
                    if let Some(value) = parse_field_line(trimmed, "type") {
                        relation.relation_type = value;
                    } else if let Some(value) = parse_field_line(trimmed, "valid_from") {
                        relation.valid_from = parse_optional_time(
                            Some(&value),
                            "relation.valid_from",
                            &mut Vec::new(),
                        );
                    } else if let Some(value) = parse_field_line(trimmed, "valid_until") {
                        relation.valid_until = parse_optional_time(
                            Some(&value),
                            "relation.valid_until",
                            &mut Vec::new(),
                        );
                    } else if let Some(value) = parse_field_line(trimmed, "inferred") {
                        relation.inferred = value.eq_ignore_ascii_case("true");
                    } else if let Some(value) = parse_field_line(trimmed, "confidence") {
                        relation.confidence = parse_confidence(&value);
                    }
                }
            }
            BodyMode::Content => {
                content_lines.push(strip_content_indent(line).to_string());
            }
        }
    }

    if let Some(relation) = current_relation.take() {
        graph.relations.push(relation);
    }

    RawSection {
        path: path.to_string(),
        section_type,
        temporal_values,
        graph: saw_graph.then_some(graph),
        content: trim_trailing_blank_lines(content_lines).join("\n"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyMode {
    Top,
    Temporal,
    Graph,
    Content,
}

fn parse_field_line(line: &str, key: &str) -> Option<String> {
    let (_, (_, _, _, value)) = tuple((
        tag::<_, _, nom::error::Error<&str>>(key),
        space0,
        char(':'),
        preceded(space0, not_line_ending),
    ))(line)
    .ok()?;
    Some(clean_scalar(value))
}

fn parse_dash_field(line: &str, key: &str) -> Option<String> {
    // `nom::error::Error<&str>` is spelled out here because this helper is used
    // inside `Option` recovery code. Without the concrete error type, Rust has
    // several valid Nom error implementations to choose from.
    let (_, value) = preceded::<_, _, _, nom::error::Error<&str>, _, _>(
        tuple((char('-'), space0, tag(key), space0, char(':'), space0)),
        not_line_ending,
    )(line)
    .ok()?;
    Some(clean_scalar(value))
}

fn strip_content_indent(line: &str) -> &str {
    line.strip_prefix("  ")
        .or_else(|| line.strip_prefix('\t'))
        .unwrap_or(line)
}

fn trim_trailing_blank_lines(mut lines: Vec<String>) -> Vec<String> {
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}

fn clean_scalar(value: &str) -> String {
    let value = value.trim();
    if value.eq_ignore_ascii_case("null") {
        return "null".to_string();
    }
    value
        .trim_matches('"')
        .trim_matches('\'')
        .split('#')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn clean_inline_content(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

fn parse_confidence(value: &str) -> Option<f64> {
    let parsed = value.parse::<f64>().ok()?;
    let normalized = if parsed > 1.0 { parsed / 100.0 } else { parsed };
    Some(normalized.clamp(0.0, 1.0))
}

fn parse_time(
    value: Option<&String>,
    fallback: DateTime<Utc>,
    label: &str,
    warnings: &mut Vec<String>,
) -> DateTime<Utc> {
    match value.and_then(|value| DateTime::parse_from_rfc3339(value).ok()) {
        Some(time) => time.with_timezone(&Utc),
        None => {
            warnings.push(format!("missing or invalid {}; used tool time", label));
            fallback
        }
    }
}

fn parse_optional_time(
    value: Option<&String>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<DateTime<Utc>> {
    match value {
        None => None,
        Some(value) if value == "null" || value.is_empty() => None,
        Some(value) => match DateTime::parse_from_rfc3339(value) {
            Ok(time) => Some(time.with_timezone(&Utc)),
            Err(_) => {
                warnings.push(format!("invalid {}; treated as null", label));
                None
            }
        },
    }
}

// Tiny parser used by tests and as an example of field parsing. Keeping this
// here documents the grammar shape without complicating the forgiving scanner.
#[allow(dead_code)]
fn quoted_or_bare_value(input: &str) -> IResult<&str, &str> {
    alt((
        delimited(char('"'), take_until("\""), char('"')),
        terminated(not_line_ending, opt(line_ending)),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 27, 13, 10, 0).unwrap()
    }

    #[test]
    fn parses_canonical_document_round_trip_shape() {
        let parsed = parse_document(
            r#"aif# AIF v1.3

meta:
  version: 1.3
  purpose: "personal-agent-memory"
  generated_at: "2020-01-01T00:00:00Z"
  last_updated: "2020-01-01T00:00:00Z"
  valid_until: null
  created_by: "me"
  default_expiry_days: null

text[SECTION: memory.core]
type: text
temporal:
  created_at: "2020-01-01T00:00:00Z"
  updated_at: "2020-01-01T00:00:00Z"
  expires_at: null
graph:
  node_type: "Module"
  relations:
    - to: "memory.tools"
      type: "uses"
      valid_from: "2020-01-01T00:00:00Z"
      valid_until: null
content: |
  hello
[END SECTION]
"#,
            TimestampPolicy::Preserve,
        )
        .unwrap();

        assert_eq!(parsed.document.sections.len(), 1);
        assert_eq!(parsed.document.sections[0].path, "memory.core");
        assert_eq!(parsed.document.sections[0].content, "hello");
        assert_eq!(
            parsed.document.sections[0]
                .graph
                .as_ref()
                .unwrap()
                .relations[0]
                .relation_type,
            "uses"
        );
    }

    #[test]
    fn protected_policy_replaces_llm_timestamps() {
        let now = fixed_time();
        let parsed = parse_document(
            r#"meta:
  version: 1.3
  generated_at: "1900-01-01T00:00:00Z"
  last_updated: "1900-01-01T00:00:00Z"
[SECTION: x]
type: text
temporal:
  created_at: "1900-01-01T00:00:00Z"
  updated_at: "1900-01-01T00:00:00Z"
content: |
  repaired
[END SECTION]
"#,
            TimestampPolicy::Protect { now },
        )
        .unwrap();

        assert_eq!(parsed.document.meta.generated_at, now);
        assert_eq!(parsed.document.sections[0].temporal.updated_at, now);
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("protected")));
    }

    #[test]
    fn forgiving_parser_repairs_missing_temporal_and_type() {
        let parsed = parse_document(
            r#"meta:
  version: 1.3
[SECTION: loose]
content: |
  missing fields are okay
[END SECTION]
"#,
            TimestampPolicy::Preserve,
        )
        .unwrap();

        assert_eq!(parsed.document.sections[0].section_type, SectionType::Text);
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("missing type")));
    }

    #[test]
    fn markdown_like_text_inside_content_does_not_confuse_section_parser() {
        let parsed = parse_document(
            r#"meta:
  version: 1.3
[SECTION: note]
type: text
content: |
  # Heading
  - item
  ```rust
  fn main() {}
  ```
[END SECTION]
"#,
            TimestampPolicy::Preserve,
        )
        .unwrap();

        assert!(parsed.document.sections[0].content.contains("fn main"));
    }

    #[test]
    fn parses_human_mode_delimiters_and_inline_content() {
        let parsed = parse_document(
            r#"meta:
  version: 1.3
=== SECTION: notes.today
type: text
content: Single-line note.
=== END SECTION
"#,
            TimestampPolicy::Preserve,
        )
        .unwrap();

        assert_eq!(parsed.document.sections[0].path, "notes.today");
        assert_eq!(parsed.document.sections[0].content, "Single-line note.");
    }

    #[test]
    fn parses_float_and_legacy_integer_confidence_scores() {
        let parsed = parse_document(
            r#"meta:
  version: 1.3
[SECTION: agent.reader]
type: text
graph:
  relations:
    - to: "agent.writer"
      type: "uses"
      inferred: true
      confidence: 0.87
    - to: "agent.cache"
      type: "references"
      inferred: true
      confidence: 60
content: |
  reader
[END SECTION]
"#,
            TimestampPolicy::Preserve,
        )
        .unwrap();

        let relations = &parsed.document.sections[0]
            .graph
            .as_ref()
            .unwrap()
            .relations;
        assert_eq!(relations[0].confidence, Some(0.87));
        assert_eq!(relations[1].confidence, Some(0.60));
    }

    #[test]
    fn parses_meta_compact_mode() {
        let parsed = parse_document(
            r#"meta:
  version: 1.3
  compact_mode: minified
[SECTION: note]
type: text
content: |
  hello
[END SECTION]
"#,
            TimestampPolicy::Preserve,
        )
        .unwrap();

        assert_eq!(parsed.document.meta.compact_mode, CompactMode::Minified);
    }
}
