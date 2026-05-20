use chrono::{TimeZone, Utc};
use rbmem::parser::parse_document;
use rbmem::{
    context, create, encrypt_section, query, read, update, ContextOptions, CreateOptions,
    EncryptionKey, OutputFormat, RbmemDocument, ReadOptions, SectionType, TimestampPolicy,
    UpdateOptions,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

fn temp_test_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rbmem-concurrent-{name}-{suffix}"))
}

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 20, 10, 0, 0).unwrap()
}

fn test_key() -> EncryptionKey {
    EncryptionKey::from_bytes([7u8; 32])
}

/// Helper: create a fresh rbmem file in a subdirectory.
fn make_file(dir: &Path, name: &str) -> PathBuf {
    let file = dir.join(format!("{name}.rbmem"));
    create(
        &file,
        CreateOptions {
            created_by: "concurrent-test".to_string(),
            purpose: format!("concurrent test: {name}"),
            default_expiry_days: None,
            human: false,
            now: fixed_time(),
        },
    )
    .unwrap();
    file
}

// ---------------------------------------------------------------------------
// 1. Concurrent reads of the same file
// ---------------------------------------------------------------------------

#[test]
fn concurrent_reads_same_file() {
    let root = temp_test_dir("reads");
    fs::create_dir_all(&root).unwrap();
    let file = make_file(&root, "shared");
    let now = fixed_time();

    // Populate with several sections
    for i in 0..20 {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: format!("section_{i}"),
                section_type: SectionType::Text,
                content: format!(
                    "Content for section {i} with some searchable text about topic {i}"
                ),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    let file = Arc::new(file);
    let thread_count = 16;

    let handles: Vec<_> = (0..thread_count)
        .map(|t| {
            let file = Arc::clone(&file);
            std::thread::spawn(move || {
                // Each thread reads the file and verifies content
                let output = read(
                    file.as_ref(),
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

                assert!(output.contains("section_0"));
                assert!(output.contains("section_19"));
                assert!(output.contains("Content for section 5"));
                t // return thread id for verification
            })
        })
        .collect();

    for h in handles {
        let tid = h.join().unwrap();
        assert!(tid < thread_count);
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 2. Concurrent reads with query
// ---------------------------------------------------------------------------

#[test]
fn concurrent_query_same_file() {
    let root = temp_test_dir("query");
    fs::create_dir_all(&root).unwrap();
    let file = make_file(&root, "queryable");
    let now = fixed_time();

    for i in 0..10 {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: format!("topic_{i}"),
                section_type: SectionType::Text,
                content: format!("Detailed information about topic {i} for querying"),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    let file = Arc::new(file);
    let thread_count = 12;

    let handles: Vec<_> = (0..thread_count)
        .map(|t| {
            let file = Arc::clone(&file);
            std::thread::spawn(move || {
                let result = query(
                    file.as_ref(),
                    "topic",
                    ContextOptions {
                        resolve: true,
                        compact: false,
                        minified: true,
                        graph_depth: 0,
                        decrypt: false,
                        key: None,
                        format: OutputFormat::Text,
                        policy: TimestampPolicy::Preserve,
                        max_tokens: None,
                    },
                )
                .unwrap();
                assert!(
                    result.contains("topic"),
                    "thread {t}: query result missing expected content"
                );
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 3. Concurrent writes to separate files
// ---------------------------------------------------------------------------

#[test]
fn concurrent_writes_separate_files() {
    let root = temp_test_dir("writes");
    fs::create_dir_all(&root).unwrap();
    let thread_count = 12;

    // Pre-create files
    let files: Vec<PathBuf> = (0..thread_count)
        .map(|i| {
            let sub = root.join(format!("writer_{i}"));
            fs::create_dir_all(&sub).unwrap();
            make_file(&sub, &format!("file_{i}"))
        })
        .collect();

    let handles: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let file = file.clone();
            std::thread::spawn(move || {
                let now = fixed_time();
                for j in 0..10 {
                    update(
                        &file,
                        UpdateOptions {
                            actor: format!("thread-{i}"),
                            section: format!("data_{j}"),
                            section_type: SectionType::Text,
                            content: format!("Written by thread {i}, entry {j}"),
                            human: false,
                            dry_run: false,
                            now,
                        },
                    )
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify each file has all its sections
    for (i, file) in files.iter().enumerate() {
        let output = read(
            file,
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

        for j in 0..10 {
            assert!(
                output.contains(&format!("Written by thread {i}, entry {j}")),
                "file {i} missing section data_{j}"
            );
        }
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 4. Concurrent updates to the same section in separate files
// ---------------------------------------------------------------------------

#[test]
fn concurrent_section_updates_separate_files() {
    let root = temp_test_dir("section-updates");
    fs::create_dir_all(&root).unwrap();
    let thread_count = 8;

    let files: Vec<PathBuf> = (0..thread_count)
        .map(|i| {
            let sub = root.join(format!("upd_{i}"));
            fs::create_dir_all(&sub).unwrap();
            make_file(&sub, &format!("upd_{i}"))
        })
        .collect();

    let handles: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let file = file.clone();
            std::thread::spawn(move || {
                let now = fixed_time();
                // Each thread repeatedly updates the same section
                for round in 0..5 {
                    update(
                        &file,
                        UpdateOptions {
                            actor: format!("thread-{i}"),
                            section: "shared_name".to_string(),
                            section_type: SectionType::List,
                            content: format!("- Round {round} from thread {i}"),
                            human: false,
                            dry_run: false,
                            now,
                        },
                    )
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Each file should have the shared_name section with accumulated content
    for (i, file) in files.iter().enumerate() {
        let output = read(
            file,
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
        assert!(
            output.contains("[SECTION: shared_name]"),
            "file {i} missing shared_name section"
        );
        // Last round should be present
        assert!(
            output.contains(&format!("Round 4 from thread {i}")),
            "file {i} missing final round"
        );
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 5. Concurrent graph view operations
// ---------------------------------------------------------------------------

#[test]
fn concurrent_graph_view_operations() {
    let root = temp_test_dir("graph");
    fs::create_dir_all(&root).unwrap();
    let file = make_file(&root, "graph-test");
    let now = fixed_time();

    // Build a document with hierarchical sections and graph relations
    for i in 0..15 {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: format!("project.module_{i}"),
                section_type: SectionType::Text,
                content: format!("Module {i} handles functionality area {i}"),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    let file = Arc::new(file);
    let thread_count = 10;

    let handles: Vec<_> = (0..thread_count)
        .map(|_| {
            let file = Arc::clone(&file);
            std::thread::spawn(move || {
                // Parse the document and compute graph view
                let raw = fs::read_to_string(file.as_ref()).unwrap();
                let parsed = parse_document(&raw, TimestampPolicy::Preserve)
                    .unwrap()
                    .document;
                let view = parsed.graph_view();

                // All sections should appear as nodes
                assert!(view.nodes.len() >= 15);
                // Parent "project" should have "contains" edges
                assert!(view.edges.iter().any(|e| e.edge_type == "contains"));

                view.nodes.len()
            })
        })
        .collect();

    let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // All threads should see the same graph
    for count in &results {
        assert_eq!(*count, results[0]);
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 6. Concurrent encrypt operations on separate files
// ---------------------------------------------------------------------------

#[test]
fn concurrent_encrypt_separate_files() {
    let root = temp_test_dir("encrypt-concurrent");
    fs::create_dir_all(&root).unwrap();
    let thread_count = 8;
    let key = Arc::new(test_key());
    let now = fixed_time();

    let files: Vec<PathBuf> = (0..thread_count)
        .map(|i| {
            let sub = root.join(format!("enc_{i}"));
            fs::create_dir_all(&sub).unwrap();
            let file = make_file(&sub, &format!("enc_{i}"));
            update(
                &file,
                UpdateOptions {
                    actor: "test".to_string(),
                    section: format!("secret_{i}"),
                    section_type: SectionType::Text,
                    content: format!(
                        "Confidential data from thread {i} with padding text for size"
                    ),
                    human: false,
                    dry_run: false,
                    now,
                },
            )
            .unwrap();
            file
        })
        .collect();

    let handles: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let file = file.clone();
            let key = Arc::clone(&key);
            std::thread::spawn(move || {
                encrypt_section(&file, &format!("secret_{i}"), &key, now).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify all sections are encrypted
    for (i, file) in files.iter().enumerate() {
        let raw = fs::read_to_string(file).unwrap();
        assert!(
            raw.contains("type: encrypted"),
            "file {i} section not encrypted"
        );
        assert!(
            !raw.contains(&format!("Confidential data from thread {i}")),
            "file {i} plaintext leaked"
        );
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 7. Concurrent context operations
// ---------------------------------------------------------------------------

#[test]
fn concurrent_context_operations() {
    let root = temp_test_dir("context");
    fs::create_dir_all(&root).unwrap();
    let file = make_file(&root, "context-test");
    let now = fixed_time();

    // Populate with diverse sections
    for i in 0..10 {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: format!("knowledge.area_{i}"),
                section_type: SectionType::Text,
                content: format!(
                    "Knowledge area {i} covers important domain concepts for context retrieval"
                ),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    let file = Arc::new(file);
    let thread_count = 8;

    let handles: Vec<_> = (0..thread_count)
        .map(|t| {
            let file = Arc::clone(&file);
            std::thread::spawn(move || {
                let result = context(
                    file.as_ref(),
                    "domain concepts",
                    ContextOptions {
                        resolve: true,
                        compact: false,
                        minified: true,
                        graph_depth: 1,
                        decrypt: false,
                        key: None,
                        format: OutputFormat::Json,
                        policy: TimestampPolicy::Preserve,
                        max_tokens: None,
                    },
                )
                .unwrap();

                assert!(
                    result.contains("\"schema\""),
                    "thread {t}: context output missing schema"
                );
                result.len()
            })
        })
        .collect();

    let sizes: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // All threads should get consistent results
    for size in &sizes {
        assert_eq!(*size, sizes[0], "context output sizes should be consistent");
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 8. Mixed concurrent operations: read + query + context
// ---------------------------------------------------------------------------

#[test]
fn mixed_concurrent_read_query_context() {
    let root = temp_test_dir("mixed");
    fs::create_dir_all(&root).unwrap();
    let file = make_file(&root, "mixed-ops");
    let now = fixed_time();

    for i in 0..10 {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: format!("data_{i}"),
                section_type: SectionType::Text,
                content: format!("Data section {i} with searchable content about widgets"),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    let file = Arc::new(file);
    let mut handles = Vec::new();

    // Readers
    for _ in 0..5 {
        let file = Arc::clone(&file);
        handles.push(std::thread::spawn(move || {
            read(
                file.as_ref(),
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
            .unwrap()
        }));
    }

    // Queriers
    for _ in 0..5 {
        let file = Arc::clone(&file);
        handles.push(std::thread::spawn(move || {
            query(
                file.as_ref(),
                "widgets",
                ContextOptions {
                    resolve: true,
                    compact: false,
                    minified: true,
                    graph_depth: 0,
                    decrypt: false,
                    key: None,
                    format: OutputFormat::Text,
                    policy: TimestampPolicy::Preserve,
                    max_tokens: None,
                },
            )
            .unwrap()
        }));
    }

    // Context builders
    for _ in 0..5 {
        let file = Arc::clone(&file);
        handles.push(std::thread::spawn(move || {
            context(
                file.as_ref(),
                "data analysis",
                ContextOptions {
                    resolve: true,
                    compact: false,
                    minified: true,
                    graph_depth: 0,
                    decrypt: false,
                    key: None,
                    format: OutputFormat::Json,
                    policy: TimestampPolicy::Preserve,
                    max_tokens: None,
                },
            )
            .unwrap()
        }));
    }

    for h in handles {
        let result = h.join().unwrap();
        assert!(
            !result.is_empty(),
            "all operations should return non-empty output"
        );
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 9. Concurrent document parsing and graph building
// ---------------------------------------------------------------------------

#[test]
fn concurrent_parse_and_graph_build() {
    let root = temp_test_dir("parse-graph");
    fs::create_dir_all(&root).unwrap();
    let file = make_file(&root, "parse-test");
    let now = fixed_time();

    // Create a hierarchical document
    let sections = [
        ("arch", "System architecture overview"),
        ("arch.api", "API layer details"),
        ("arch.api.auth", "Authentication module"),
        ("arch.api.routes", "Route definitions"),
        ("arch.db", "Database layer"),
        ("arch.db.schema", "Schema definitions"),
        ("arch.db.migrations", "Migration scripts"),
        ("config", "Configuration settings"),
        ("config.env", "Environment variables"),
        ("config.secrets", "Secret management"),
    ];

    for (path, content) in &sections {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: path.to_string(),
                section_type: SectionType::Text,
                content: content.to_string(),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    let raw = Arc::new(fs::read_to_string(&file).unwrap());
    let thread_count = 20;

    let handles: Vec<_> = (0..thread_count)
        .map(|_| {
            let raw = Arc::clone(&raw);
            std::thread::spawn(move || {
                let doc = parse_document(&raw, TimestampPolicy::Preserve)
                    .unwrap()
                    .document;

                // Verify section count
                assert_eq!(doc.sections.len(), sections.len());

                // Build graph view
                let view = doc.graph_view();
                assert!(!view.nodes.is_empty());
                assert!(!view.edges.is_empty());

                // Verify contains edges for hierarchical sections
                let contains_edges: Vec<_> = view
                    .edges
                    .iter()
                    .filter(|e| e.edge_type == "contains")
                    .collect();
                assert!(contains_edges.len() >= 5);

                (doc.sections.len(), view.nodes.len(), view.edges.len())
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // All threads should produce identical results
    for result in &results {
        assert_eq!(
            *result, results[0],
            "concurrent parses should produce identical graphs"
        );
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 10. Concurrent upsert on in-memory documents
// ---------------------------------------------------------------------------

#[test]
fn concurrent_in_memory_document_upserts() {
    let now = fixed_time();
    let thread_count = 8;
    let sections_per_thread = 20;

    let handles: Vec<_> = (0..thread_count)
        .map(|t| {
            std::thread::spawn(move || {
                let mut doc = RbmemDocument::new(now, format!("thread-{t}"));
                for i in 0..sections_per_thread {
                    doc.upsert_section(
                        &format!("thread_{t}.section_{i}"),
                        SectionType::Text,
                        format!("Content from thread {t}, section {i}"),
                        now,
                    );
                }

                // Verify all sections are present
                assert_eq!(doc.sections.len(), sections_per_thread);

                // Build graph view
                let view = doc.graph_view();
                assert!(!view.nodes.is_empty());

                doc.sections.len()
            })
        })
        .collect();

    for h in handles {
        let count = h.join().unwrap();
        assert_eq!(count, sections_per_thread);
    }
}
