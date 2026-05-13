use chrono::{TimeZone, Utc};
use rbmem::{RbmemDocument, SectionIndex, SectionType};

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 7, 13, 30, 0).unwrap()
}

#[test]
fn index_finds_keywords_prefixes_and_related_sections() {
    let now = fixed_time();
    let mut document = RbmemDocument::new(now, "me");
    document.upsert_section(
        "agents.reader",
        SectionType::Text,
        "Performs careful github review.".to_string(),
        now,
    );
    document.upsert_section(
        "agents.writer",
        SectionType::Text,
        "Writes summaries.".to_string(),
        now,
    );
    document.upsert_section(
        "memory.testing",
        SectionType::Text,
        "Run tests.".to_string(),
        now,
    );
    document.sections[0].graph = Some(rbmem::document::GraphInfo {
        node_type: None,
        relations: vec![rbmem::GraphRelation {
            to: "memory.testing".to_string(),
            relation_type: "depends_on".to_string(),
            valid_from: Some(now),
            valid_until: None,
            inferred: false,
            confidence: None,
        }],
    });

    let index = SectionIndex::build(&document);

    assert_eq!(index.keyword("github"), vec!["agents.reader"]);
    assert_eq!(
        index.prefix("agents.*"),
        vec!["agents.reader", "agents.writer"]
    );
    assert_eq!(index.related("agents.reader", 1), vec!["memory.testing"]);
}
