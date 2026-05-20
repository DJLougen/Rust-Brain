use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{TimeZone, Utc};
use rbmem::{
    create, encrypt_section, query, read, update, ContextOptions, CreateOptions, EncryptionKey,
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
    std::env::temp_dir().join(format!("rbmem-encryption-{name}-{suffix}"))
}

fn test_key() -> EncryptionKey {
    EncryptionKey::from_bytes([7u8; 32])
}

#[test]
fn encrypted_sections_are_skipped_unless_decrypt_is_enabled() {
    let root = temp_test_dir("read");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    let now = Utc.with_ymd_and_hms(2026, 5, 7, 12, 30, 0).unwrap();

    create(
        &file,
        CreateOptions {
            created_by: "api-test".to_string(),
            purpose: "encryption smoke test".to_string(),
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
            section: "secrets.api".to_string(),
            section_type: SectionType::Text,
            content: "token=super-secret".to_string(),
            human: false,
            dry_run: false,
            now,
        },
    )
    .unwrap();

    let encrypted = encrypt_section(&file, "secrets.api", &test_key(), now).unwrap();
    assert_eq!(encrypted.sections[0].section_type, SectionType::Encrypted);

    let raw = fs::read_to_string(&file).unwrap();
    assert!(raw.contains("type: encrypted"));
    assert!(raw.contains("nonce:"));
    assert!(raw.contains("ciphertext:"));
    assert!(!raw.contains("super-secret"));

    let hidden = read(
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
    assert!(!hidden.contains("secrets.api"));
    assert!(!hidden.contains("super-secret"));

    let decrypted = read(
        &file,
        ReadOptions {
            resolve: false,
            compact: false,
            minified: false,
            hide_empty_temporal: false,
            decrypt: true,
            key: Some(test_key()),
            policy: TimestampPolicy::Preserve,
        },
    )
    .unwrap();
    assert!(decrypted.contains("[SECTION: secrets.api]"));
    assert!(decrypted.contains("super-secret"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_sections_are_queryable_only_with_decryption() {
    let root = temp_test_dir("query");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    let now = Utc.with_ymd_and_hms(2026, 5, 7, 12, 45, 0).unwrap();

    create(
        &file,
        CreateOptions {
            created_by: "api-test".to_string(),
            purpose: "encryption query test".to_string(),
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
            section: "secrets.review".to_string(),
            section_type: SectionType::Text,
            content: "github review secret phrase".to_string(),
            human: false,
            dry_run: false,
            now,
        },
    )
    .unwrap();
    encrypt_section(&file, "secrets.review", &test_key(), now).unwrap();

    let hidden = query(
        &file,
        "secret phrase",
        ContextOptions {
            resolve: false,
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
    assert!(!hidden.contains("secret phrase"));

    let decrypted = query(
        &file,
        "secret phrase",
        ContextOptions {
            resolve: false,
            compact: false,
            minified: true,
            graph_depth: 0,
            decrypt: true,
            key: Some(test_key()),
            format: OutputFormat::Text,
            policy: TimestampPolicy::Preserve,
            max_tokens: None,
        },
    )
    .unwrap();
    assert!(decrypted.contains("secret phrase"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn encryption_key_decodes_from_environment_style_base64() {
    let encoded = STANDARD.encode([9u8; 32]);
    let key = EncryptionKey::from_env_value(&encoded).unwrap();

    assert_eq!(key.as_bytes(), &[9u8; 32]);
}
