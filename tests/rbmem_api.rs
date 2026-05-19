use chrono::{TimeZone, Utc};
use rbmem::{
    context, create, delete_section, query, read, update, ContextOptions, CreateOptions,
    OutputFormat, ReadOptions, SectionType, TimestampPolicy, UpdateOptions,
};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

fn temp_test_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rbmem-api-{name}-{suffix}"))
}

#[test]
fn public_api_create_update_read_query_and_context() {
    let root = temp_test_dir("milestone-1");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    let now = Utc.with_ymd_and_hms(2026, 5, 7, 12, 0, 0).unwrap();

    let created = create(
        &file,
        CreateOptions {
            created_by: "api-test".to_string(),
            purpose: "library smoke test".to_string(),
            default_expiry_days: None,
            human: false,
            now,
        },
    )
    .unwrap();
    assert_eq!(created.meta.created_by, "api-test");

    let updated = update(
        &file,
        UpdateOptions {
            actor: "test".to_string(),
            section: "agents.reader".to_string(),
            section_type: SectionType::Text,
            content: "Reads memory carefully for github code review.".to_string(),
            human: false,
            dry_run: false,
            now,
        },
    )
    .unwrap();
    assert_eq!(updated.sections.len(), 1);

    let full = read(
        &file,
        ReadOptions {
            resolve: false,
            compact: false,
            minified: false,
            hide_empty_temporal: false,
            decrypt: false,
            key: None,
            policy: TimestampPolicy::Preserve,
        },
    )
    .unwrap();
    assert!(full.contains("[SECTION: agents.reader]"));

    let query_output = query(
        &file,
        "github code review",
        ContextOptions {
            resolve: true,
            compact: false,
            minified: true,
            graph_depth: 0,
            decrypt: false,
            key: None,
            format: OutputFormat::Text,
            policy: TimestampPolicy::Preserve,
        },
    )
    .unwrap();
    assert!(query_output.contains("[agents.reader]"));

    let context_output = context(
        &file,
        "review this PR",
        ContextOptions {
            resolve: true,
            compact: false,
            minified: true,
            graph_depth: 0,
            decrypt: false,
            key: None,
            format: OutputFormat::Json,
            policy: TimestampPolicy::Preserve,
        },
    )
    .unwrap();
    assert!(context_output.contains("\"schema\": \"rbmem.context.v1\""));
    assert!(context_output.contains("\"operation\": \"context\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn public_api_update_dry_run_and_delete_section() {
    let root = temp_test_dir("dry-run-delete");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    let now = Utc.with_ymd_and_hms(2026, 5, 7, 12, 15, 0).unwrap();

    create(
        &file,
        CreateOptions {
            created_by: "api-test".to_string(),
            purpose: "library dry run test".to_string(),
            default_expiry_days: None,
            human: false,
            now,
        },
    )
    .unwrap();

    update(
        &file,
        UpdateOptions {
            actor: "test".to_string(),
            section: "scratch".to_string(),
            section_type: SectionType::Text,
            content: "not persisted".to_string(),
            human: false,
            dry_run: true,
            now,
        },
    )
    .unwrap();
    assert!(!fs::read_to_string(&file).unwrap().contains("not persisted"));

    update(
        &file,
        UpdateOptions {
            actor: "test".to_string(),
            section: "scratch".to_string(),
            section_type: SectionType::Text,
            content: "persisted".to_string(),
            human: false,
            dry_run: false,
            now,
        },
    )
    .unwrap();
    delete_section(&file, "scratch", false, now).unwrap();
    assert!(!fs::read_to_string(&file)
        .unwrap()
        .contains("[SECTION: scratch]"));

    let _ = fs::remove_dir_all(root);
}
