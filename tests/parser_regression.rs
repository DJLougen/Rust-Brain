use chrono::{TimeZone, Utc};
use rbmem::parser::parse_document;
use rbmem::{SectionType, TimestampPolicy};

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 12, 9, 0, 0).unwrap()
}

#[test]
fn parser_survives_common_llm_output_variants() {
    let now = fixed_time();
    let cases = [
        "meta:\n  version: 1.3\n[SECTION: a]\ntype: text\ncontent: |\n  hello\n[END SECTION]\n",
        "meta:\n  version: 1.2\n[SECTION: a]\ntype: text\ncontent: hi\n[END SECTION]\n",
        "meta:\n[SECTION: missing.type]\ncontent: |\n  defaults\n[END SECTION]\n",
        "meta:\n  version: 1.3\ntext[SECTION: prefixed]\ntype: list\ncontent: |\n  - one\n[END SECTION]\n",
        "meta:\n  version: 1.3\n=== SECTION: human\ncontent: Human note.\n=== END SECTION\n",
        "meta:\n  version: 1.3\n[SECTION: json]\ntype: json\ncontent: |\n  {\"a\":1}\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: timeline]\ntype: timeline\ncontent: |\n  2026: event\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: template]\ntype: template\ncontent: |\n  Hello {{name}}\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: memory]\ntype: hermes:memory\ncontent: |\n  - fact\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: encrypted]\ntype: encrypted\nnonce: \"aaaaaaaaaaaaaaaa\"\nciphertext: \"bbbb\"\nencrypted_at: \"2026-05-12T09:00:00Z\"\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: conflict]\ntype: conflict\ncontent: |\n  conflict_at: now\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: source]\ntype: text\nsource:\n  kind: \"markdown\"\n  path: \"notes/a.md\"\n  actor: \"sync\"\n  hash: \"sha256:abc\"\ncontent: |\n  sourced\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: graph]\ntype: text\ngraph:\n  node_type: \"Rule\"\n  relations:\n    - to: \"other\"\n      type: \"depends_on\"\ncontent: |\n  linked\n[END SECTION]\n",
        "meta:\n  version: 1.3\n  compact_mode: minified\n[SECTION: compact]\ncontent: compact\n[END SECTION]\n",
        "meta:\n  version: 1.3\n  default_expiry_days: 7\n[SECTION: expiry]\ncontent: expires\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: markdown]\ntype: text\ncontent: |\n  # This is content\n  [not a section]\n[END SECTION]\n",
        "meta:\n  version: 1.3\r\n[SECTION: crlf]\r\ntype: text\r\ncontent: |\r\n  ok\r\n[END SECTION]\r\n",
        "meta:\n  version: 1.3\n\n\n[SECTION: blanks]\n\ncontent: |\n\n  ok\n\n[END SECTION]\n",
        "# preface\nmeta:\n  version: 1.3\n[SECTION: preface]\ncontent: ok\n[END SECTION]\n",
        "meta:\n  version: 1.3\n[SECTION: duplicate]\ncontent: one\n[END SECTION]\n[SECTION: duplicate]\ncontent: two\n[END SECTION]\n",
    ];

    for (index, input) in cases.iter().enumerate() {
        let parsed = parse_document(input, TimestampPolicy::Protect { now })
            .unwrap_or_else(|error| panic!("case {index} failed: {error}"));
        assert!(
            !parsed.document.sections.is_empty(),
            "case {index} produced no sections"
        );
    }
}

#[test]
fn parser_tracks_source_version_and_source_hash() {
    let parsed = parse_document(
        r#"meta:
  version: 1.2
[SECTION: synced]
type: text
source:
  kind: "markdown"
  path: "notes/a.md"
  actor: "sync"
  hash: "sha256:abc"
content: |
  hello
[END SECTION]
"#,
        TimestampPolicy::Preserve,
    )
    .unwrap();

    assert_eq!(parsed.document.meta.version, "1.4.0");
    assert_eq!(parsed.document.meta.source_version.as_deref(), Some("1.2"));
    assert_eq!(
        parsed.document.sections[0]
            .source
            .as_ref()
            .and_then(|source| source.hash.as_deref()),
        Some("sha256:abc")
    );
}

#[test]
fn parser_accepts_new_section_types() {
    assert_eq!(
        "encrypted".parse::<SectionType>().unwrap(),
        SectionType::Encrypted
    );
    assert_eq!(
        "conflict".parse::<SectionType>().unwrap(),
        SectionType::Conflict
    );
}
