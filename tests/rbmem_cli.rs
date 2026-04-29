use chrono::{TimeZone, Utc};
use rbmem::parser::parse_document;
use rbmem::{SectionType, TimestampPolicy};

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 27, 13, 10, 0).unwrap()
}

#[test]
fn round_trip_keeps_sections_and_content() {
    let now = fixed_time();
    let input = r#"meta:
  version: 1.3
  purpose: "personal-agent-memory"
  generated_at: "2026-04-27T13:10:00Z"
  last_updated: "2026-04-27T13:10:00Z"
  valid_until: null
  created_by: "me"
  default_expiry_days: null

[SECTION: agents.reader]
type: text
temporal:
  created_at: "2026-04-27T13:10:00Z"
  updated_at: "2026-04-27T13:10:00Z"
  expires_at: null
content: |
  Reads memory carefully.
[END SECTION]
"#;

    let parsed = parse_document(input, TimestampPolicy::Protect { now }).unwrap();
    let serialized = parsed.document.to_rbmem_string();
    let reparsed = parse_document(&serialized, TimestampPolicy::Preserve).unwrap();

    assert_eq!(reparsed.document.sections.len(), 1);
    assert_eq!(reparsed.document.sections[0].path, "agents.reader");
    assert_eq!(
        reparsed.document.sections[0].content,
        "Reads memory carefully."
    );
}

#[test]
fn hierarchy_resolution_uses_child_scalar_and_parent_list() {
    let now = fixed_time();
    let input = r#"meta:
  version: 1.3
[SECTION: tasks]
type: list
content: |
  - parent
[END SECTION]
[SECTION: tasks.today]
type: list
content: |
  - child
[END SECTION]
"#;

    let parsed = parse_document(input, TimestampPolicy::Protect { now }).unwrap();
    let resolved = parsed
        .document
        .resolved_sections()
        .into_iter()
        .find(|section| section.path == "tasks.today")
        .unwrap();

    assert_eq!(resolved.section_type, SectionType::List);
    assert_eq!(resolved.content, "- parent\n- child");
}

#[test]
fn graph_json_has_contains_edges_from_dotted_paths() {
    let now = fixed_time();
    let input = r#"meta:
  version: 1.3
[SECTION: a.b.c]
type: text
content: |
  node
[END SECTION]
"#;

    let parsed = parse_document(input, TimestampPolicy::Protect { now }).unwrap();
    let graph = parsed.document.graph_view();

    assert!(graph
        .edges
        .iter()
        .any(|edge| edge.from == "a.b" && edge.to == "a.b.c"));
}

#[test]
fn markdown_comparison_content_survives_as_plain_text() {
    let now = fixed_time();
    let input = r#"meta:
  version: 1.3
[SECTION: markdown.note]
type: text
content: |
  # Title
  This should remain markdown, not become RBMEM structure.
[END SECTION]
"#;

    let parsed = parse_document(input, TimestampPolicy::Protect { now }).unwrap();

    assert!(parsed.document.sections[0].content.contains("# Title"));
    assert!(parsed.document.sections[0]
        .content
        .contains("not become RBMEM structure"));
}

#[test]
fn hermes_self_evolution_example_is_valid_rbmem() {
    let now = fixed_time();
    let input = include_str!("../examples/hermes-self-evolution.rbmem");
    let parsed = parse_document(input, TimestampPolicy::Protect { now }).unwrap();
    let warnings = parsed.document.validate();

    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(parsed
        .document
        .sections
        .iter()
        .any(|section| section.path == "evolution.runs.demo-gepa-001.report"));
    assert!(parsed.document.sections.iter().any(|section| {
        section.path == "evolution.skills.github-code-review.candidates.demo-gepa-001.metadata"
    }));
}
