use chrono::{TimeZone, Utc};
use rbmem::{
    diff_with_format, merge_documents, DiffFormat, MergeStrategy, RbmemDocument, SectionType,
};

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 7, 13, 0, 0).unwrap()
}

#[test]
fn typed_diff_renders_json_and_text() {
    let now = fixed_time();
    let mut before = RbmemDocument::new(now, "me");
    before.upsert_section("rules", SectionType::Text, "old".to_string(), now);

    let mut after = before.clone();
    after.upsert_section("rules", SectionType::Text, "new".to_string(), now);
    after.upsert_section("memory", SectionType::Text, "added".to_string(), now);

    let text = diff_with_format(&before, &after, DiffFormat::Text).unwrap();
    assert!(text.contains("added: memory"));
    assert!(text.contains("changed content: rules"));

    let json = diff_with_format(&before, &after, DiffFormat::Json).unwrap();
    assert!(json.contains("\"schema\": \"rbmem.diff.v1\""));
    assert!(json.contains("\"path\": \"memory\""));
}

#[test]
fn three_way_merge_auto_resolves_theirs_when_local_matches_base() {
    let now = fixed_time();
    let mut base = RbmemDocument::new(now, "me");
    base.upsert_section("rules", SectionType::Text, "base".to_string(), now);
    let local = base.clone();
    let mut remote = base.clone();
    remote.upsert_section("rules", SectionType::Text, "remote".to_string(), now);

    let merged = merge_documents(&base, &local, &remote, MergeStrategy::Manual, now);
    let section = merged
        .sections
        .iter()
        .find(|section| section.path == "rules")
        .unwrap();

    assert_eq!(section.content, "remote");
    assert_eq!(section.section_type, SectionType::Text);
}

#[test]
fn three_way_merge_manual_emits_conflict_section_for_true_conflict() {
    let now = fixed_time();
    let mut base = RbmemDocument::new(now, "me");
    base.upsert_section("rules", SectionType::Text, "base".to_string(), now);
    let mut local = base.clone();
    local.upsert_section("rules", SectionType::Text, "local".to_string(), now);
    let mut remote = base.clone();
    remote.upsert_section("rules", SectionType::Text, "remote".to_string(), now);

    let merged = merge_documents(&base, &local, &remote, MergeStrategy::Manual, now);
    let section = merged
        .sections
        .iter()
        .find(|section| section.path == "rules")
        .unwrap();

    assert_eq!(section.section_type, SectionType::Conflict);
    assert!(section.content.contains("local_version:"));
    assert!(section.content.contains("remote_version:"));
}
