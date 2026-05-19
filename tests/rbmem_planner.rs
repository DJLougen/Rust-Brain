use chrono::{TimeZone, Utc};
use rbmem::parser::parse_document;
use rbmem::{plan_memory, PlanOptions, SatBackend, SatStatus, SectionType, TimestampPolicy};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

fn temp_test_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rbmem-plan-{name}-{suffix}"))
}

#[test]
fn planner_stores_sat_plan_sections_and_graph_edges() {
    let root = temp_test_dir("store");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    fs::write(
        &file,
        r#"meta:
  version: 1.4.0
  purpose: "planner test"
  created_by: "test"

[SECTION: goals]
type: list
content: |
  - ship the release
[END SECTION]

[SECTION: tasks]
type: list
content: |
  - Gather requirements
  - Run tests
  - Deploy release
[END SECTION]

[SECTION: rules]
type: list
content: |
  - deploy release requires run tests
  - gather requirements conflicts with deploy release
[END SECTION]
"#,
    )
    .unwrap();

    let now = Utc.with_ymd_and_hms(2026, 5, 18, 20, 0, 0).unwrap();
    let report = plan_memory(PlanOptions {
        goal: Some("deploy release".to_string()),
        from_memory: false,
        file: Some(file.clone()),
        search_dir: root.clone(),
        context_pack: None,
        solver: SatBackend::Internal,
        proof: true,
        proof_path: None,
        verify_proof: false,
        cube_and_conquer: false,
        dry_run: false,
        now,
    })
    .unwrap();

    assert_eq!(report.status, SatStatus::Sat);
    assert!(report
        .steps
        .iter()
        .any(|step| step.action.contains("Run tests")));
    assert!(report
        .steps
        .iter()
        .any(|step| step.action.contains("Deploy release")));

    let parsed = parse_document(
        &fs::read_to_string(&file).unwrap(),
        TimestampPolicy::Preserve,
    )
    .unwrap()
    .document;
    assert!(parsed.sections.iter().any(|section| {
        section.path.ends_with(".steps")
            && section.section_type == SectionType::List
            && section.content.contains("Deploy release")
    }));
    assert!(parsed.sections.iter().any(|section| {
        section.path.ends_with(".sat")
            && section.section_type == SectionType::Json
            && section
                .content
                .contains("\"schema\": \"rbmem.plan.sat.v1\"")
    }));
    assert!(parsed
        .graph_view()
        .edges
        .iter()
        .any(|edge| { edge.edge_type == "uses_context" && edge.to == "tasks" }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn planner_can_derive_goal_from_memory_without_writing_on_dry_run() {
    let root = temp_test_dir("from-memory");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    fs::write(
        &file,
        r#"meta:
  version: 1.4.0

[SECTION: goals]
type: list
content: |
  - produce a code review
[END SECTION]

[SECTION: tasks]
type: list
content: |
  - Inspect the diff
  - Run focused tests
  - Write the code review
[END SECTION]
"#,
    )
    .unwrap();
    let before = fs::read_to_string(&file).unwrap();

    let report = plan_memory(PlanOptions {
        goal: None,
        from_memory: true,
        file: Some(file.clone()),
        search_dir: root.clone(),
        context_pack: None,
        solver: SatBackend::Internal,
        proof: false,
        proof_path: None,
        verify_proof: false,
        cube_and_conquer: true,
        dry_run: true,
        now: Utc.with_ymd_and_hms(2026, 5, 18, 20, 1, 0).unwrap(),
    })
    .unwrap();

    assert_eq!(report.goal, "produce a code review");
    assert_eq!(report.status, SatStatus::Sat);
    assert!(report.dry_run);
    assert_eq!(fs::read_to_string(&file).unwrap(), before);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn planner_honors_context_pack_includes() {
    let root = temp_test_dir("pack");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    fs::write(
        root.join(".rbmempacks"),
        r#"[pack: release]
include:
  - tasks.release
"#,
    )
    .unwrap();
    fs::write(
        &file,
        r#"meta:
  version: 1.4.0

[SECTION: goals]
type: list
content: |
  - deploy release
[END SECTION]

[SECTION: tasks.release]
type: list
content: |
  - Run release tests
  - Deploy release
[END SECTION]

[SECTION: tasks.unrelated]
type: list
content: |
  - Archive old notes
[END SECTION]

[SECTION: rules.release]
type: list
content: |
  - deploy release requires run release tests
[END SECTION]
"#,
    )
    .unwrap();

    let report = plan_memory(PlanOptions {
        goal: Some("deploy release".to_string()),
        from_memory: false,
        file: Some(file),
        search_dir: root.clone(),
        context_pack: Some("release".to_string()),
        solver: SatBackend::Internal,
        proof: false,
        proof_path: None,
        verify_proof: false,
        cube_and_conquer: false,
        dry_run: true,
        now: Utc.with_ymd_and_hms(2026, 5, 18, 20, 2, 0).unwrap(),
    })
    .unwrap();

    assert!(report
        .context_sections
        .contains(&"tasks.release".to_string()));
    assert!(!report
        .steps
        .iter()
        .any(|step| step.action.contains("Archive old notes")));

    let _ = fs::remove_dir_all(root);
}
