use chrono::{TimeZone, Utc};
use rbmem::crypto::{decrypt_content, encrypt_content};
use rbmem::{
    create, decrypt_section, encrypt_section, read, update, CreateOptions, EncryptedPayload,
    EncryptionKey, ReadOptions, SectionType, TimestampPolicy, UpdateOptions,
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
    std::env::temp_dir().join(format!("rbmem-enc-edge-{name}-{suffix}"))
}

fn test_key() -> EncryptionKey {
    EncryptionKey::from_bytes([7u8; 32])
}

fn alt_key() -> EncryptionKey {
    EncryptionKey::from_bytes([42u8; 32])
}

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 20, 10, 0, 0).unwrap()
}

/// Helper: create a memory file and add a section with the given content.
fn create_file_with_section(
    root: &Path,
    section_path: &str,
    section_type: SectionType,
    content: &str,
) -> PathBuf {
    let file = root.join("memory.rbmem");
    let now = fixed_time();
    create(
        &file,
        CreateOptions {
            created_by: "test".to_string(),
            purpose: "encryption edge case test".to_string(),
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
            section: section_path.to_string(),
            section_type,
            content: content.to_string(),
            human: false,
            dry_run: false,
            now,
        },
    )
    .unwrap();
    file
}

// ---------------------------------------------------------------------------
// 1. Encrypt / decrypt empty content
// ---------------------------------------------------------------------------

#[test]
fn encrypt_decrypt_empty_section() {
    let root = temp_test_dir("empty");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "empty_section", SectionType::Text, "");
    let now = fixed_time();
    let key = test_key();

    encrypt_section(&file, "empty_section", &key, now).unwrap();

    let raw = fs::read_to_string(&file).unwrap();
    assert!(raw.contains("type: encrypted"));
    assert!(!raw.contains("[SECTION: empty_section]\ntype: text"));

    let doc = decrypt_section(&file, "empty_section", &key, now).unwrap();
    let section = doc
        .sections
        .iter()
        .find(|s| s.path == "empty_section")
        .unwrap();
    assert_eq!(section.content, "");
    assert_eq!(section.section_type, SectionType::Text);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypt_decrypt_empty_content_low_level() {
    let key = test_key();
    let now = fixed_time();
    let payload = encrypt_content("", &key, now).unwrap();
    let decrypted = decrypt_content(&payload, &key).unwrap();
    assert_eq!(decrypted, "");
}

// ---------------------------------------------------------------------------
// 2. Special characters
// ---------------------------------------------------------------------------

#[test]
fn encrypt_decrypt_unicode_content() {
    let key = test_key();
    let now = fixed_time();
    let content = "日本語テスト 🎉 émojis & spëcial çhars — « guillemets » ñ";
    let payload = encrypt_content(content, &key, now).unwrap();
    let decrypted = decrypt_content(&payload, &key).unwrap();
    assert_eq!(decrypted, content);
}

#[test]
fn encrypt_decrypt_content_with_null_bytes_in_string() {
    let key = test_key();
    let now = fixed_time();
    // A valid UTF-8 string that contains the Unicode replacement character and
    // other tricky codepoints (but no actual \0 since Rust strings forbid it).
    let content = "before\u{FFFD}after\u{202E}rtl-override\u{0007}bell";
    let payload = encrypt_content(content, &key, now).unwrap();
    let decrypted = decrypt_content(&payload, &key).unwrap();
    assert_eq!(decrypted, content);
}

#[test]
fn encrypt_decrypt_newlines_tabs_and_whitespace() {
    let key = test_key();
    let now = fixed_time();
    let content = "\t\ttabbed\n\n\nmulti\nline\r\nwindows\t mixed \t  spaces  ";
    let payload = encrypt_content(content, &key, now).unwrap();
    let decrypted = decrypt_content(&payload, &key).unwrap();
    assert_eq!(decrypted, content);
}

#[test]
fn encrypt_decrypt_markdown_and_code_content() {
    let root = temp_test_dir("special-chars");
    fs::create_dir_all(&root).unwrap();
    let content = "# Header\n```rust\nfn main() {\n    let x = \"escaped \\\"quotes\\\" and \\\\backslash\";\n    println!(\"{x}\");\n}\n```\n- bullet with `backticks`\n- math: $E = mc^2$\n> blockquote with <html> & entities";
    let file = create_file_with_section(&root, "code_notes", SectionType::Text, content);
    let now = fixed_time();
    let key = test_key();

    encrypt_section(&file, "code_notes", &key, now).unwrap();
    let doc = decrypt_section(&file, "code_notes", &key, now).unwrap();
    let section = doc
        .sections
        .iter()
        .find(|s| s.path == "code_notes")
        .unwrap();
    assert_eq!(section.content, content);

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 3. Very large sections
// ---------------------------------------------------------------------------

#[test]
fn encrypt_decrypt_large_section() {
    let key = test_key();
    let now = fixed_time();
    // Build a ~256KB string
    let line = "This is a line of text for testing encryption at scale. ";
    let content: String = line.repeat(4600); // ~256KB
    assert!(content.len() > 250_000);

    let payload = encrypt_content(&content, &key, now).unwrap();
    let decrypted = decrypt_content(&payload, &key).unwrap();
    assert_eq!(decrypted, content);
}

#[test]
fn encrypt_decrypt_very_large_section_via_api() {
    let root = temp_test_dir("large");
    fs::create_dir_all(&root).unwrap();
    // ~64KB content through the full file API
    let line = "Large section line for integration testing. ";
    let content: String = line.repeat(1500);
    let file = create_file_with_section(&root, "big_data", SectionType::Text, &content);
    let now = fixed_time();
    let key = test_key();

    encrypt_section(&file, "big_data", &key, now).unwrap();

    let raw = fs::read_to_string(&file).unwrap();
    assert!(!raw.contains("Large section line"));

    let doc = decrypt_section(&file, "big_data", &key, now).unwrap();
    let section = doc.sections.iter().find(|s| s.path == "big_data").unwrap();
    assert_eq!(section.content, content);

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 4. Encrypting an already-encrypted section is a no-op
// ---------------------------------------------------------------------------

#[test]
fn encrypt_already_encrypted_section_is_idempotent() {
    let root = temp_test_dir("idempotent");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "secret", SectionType::Text, "original content");
    let now = fixed_time();
    let key = test_key();

    let doc1 = encrypt_section(&file, "secret", &key, now).unwrap();
    let raw1 = fs::read_to_string(&file).unwrap();

    // Encrypt again — should be a no-op since it's already encrypted
    let doc2 = encrypt_section(&file, "secret", &key, now).unwrap();
    let raw2 = fs::read_to_string(&file).unwrap();

    assert_eq!(raw1, raw2);

    // Both documents should have the same encrypted section
    let s1 = doc1.sections.iter().find(|s| s.path == "secret").unwrap();
    let s2 = doc2.sections.iter().find(|s| s.path == "secret").unwrap();
    assert_eq!(s1.section_type, SectionType::Encrypted);
    assert_eq!(s2.section_type, SectionType::Encrypted);
    assert_eq!(
        s1.encrypted.as_ref().unwrap().ciphertext,
        s2.encrypted.as_ref().unwrap().ciphertext
    );

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 5. Decrypt a non-encrypted section fails
// ---------------------------------------------------------------------------

#[test]
fn decrypt_non_encrypted_section_returns_error() {
    let root = temp_test_dir("decrypt-plain");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "plain", SectionType::Text, "not encrypted");
    let now = fixed_time();
    let key = test_key();

    let result = decrypt_section(&file, "plain", &key, now);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not encrypted") || err_msg.contains("Parse"),
        "unexpected error: {err_msg}"
    );

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 6. Encrypt non-existent section fails
// ---------------------------------------------------------------------------

#[test]
fn encrypt_nonexistent_section_returns_not_found() {
    let root = temp_test_dir("no-section");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "existing", SectionType::Text, "hello");
    let now = fixed_time();
    let key = test_key();

    let result = encrypt_section(&file, "does_not_exist", &key, now);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not found"), "unexpected error: {err_msg}");

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 7. Wrong key decryption fails
// ---------------------------------------------------------------------------

#[test]
fn decrypt_with_wrong_key_returns_crypto_error() {
    let root = temp_test_dir("wrong-key");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "secret", SectionType::Text, "top-secret-data");
    let now = fixed_time();

    encrypt_section(&file, "secret", &test_key(), now).unwrap();

    let result = decrypt_section(&file, "secret", &alt_key(), now);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("decrypt") || err_msg.contains("crypto") || err_msg.contains("Crypto"),
        "unexpected error: {err_msg}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn decrypt_content_with_wrong_key_low_level() {
    let key = test_key();
    let wrong = alt_key();
    let now = fixed_time();
    let payload = encrypt_content("secret data", &key, now).unwrap();
    let result = decrypt_content(&payload, &wrong);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 8. Tampered ciphertext fails
// ---------------------------------------------------------------------------

#[test]
fn tampered_ciphertext_fails_decryption() {
    let key = test_key();
    let now = fixed_time();
    let mut payload = encrypt_content("sensitive info", &key, now).unwrap();

    // Tamper with the ciphertext by flipping a character
    let mut chars: Vec<char> = payload.ciphertext.chars().collect();
    if let Some(c) = chars.get_mut(5) {
        *c = if *c == 'A' { 'B' } else { 'A' };
    }
    payload.ciphertext = chars.into_iter().collect();

    let result = decrypt_content(&payload, &key);
    assert!(result.is_err());
}

#[test]
fn tampered_nonce_fails_decryption() {
    let key = test_key();
    let now = fixed_time();
    let mut payload = encrypt_content("sensitive info", &key, now).unwrap();

    // Tamper with the nonce
    let mut chars: Vec<char> = payload.nonce.chars().collect();
    if let Some(c) = chars.get_mut(0) {
        *c = if *c == 'A' { 'B' } else { 'A' };
    }
    payload.nonce = chars.into_iter().collect();

    let result = decrypt_content(&payload, &key);
    assert!(result.is_err());
}

#[test]
fn invalid_base64_ciphertext_fails() {
    let key = test_key();
    let payload = EncryptedPayload {
        nonce: "AAAAAAAAAAAAAAAA".to_string(), // valid base64, 12 bytes
        ciphertext: "!!!not-valid-base64!!!".to_string(),
        encrypted_at: fixed_time(),
    };
    let result = decrypt_content(&payload, &key);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("ciphertext") || err_msg.contains("invalid"),
        "unexpected error: {err_msg}"
    );
}

#[test]
fn invalid_base64_nonce_fails() {
    let key = test_key();
    let payload = EncryptedPayload {
        nonce: "!!!bad-nonce!!!".to_string(),
        ciphertext: "AAAAAAAAAAAAAAAA".to_string(),
        encrypted_at: fixed_time(),
    };
    let result = decrypt_content(&payload, &key);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("nonce") || err_msg.contains("invalid"),
        "unexpected error: {err_msg}"
    );
}

#[test]
fn nonce_wrong_length_fails() {
    let key = test_key();
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let payload = EncryptedPayload {
        nonce: STANDARD.encode([0u8; 8]), // 8 bytes instead of 12
        ciphertext: STANDARD.encode([0u8; 32]),
        encrypted_at: fixed_time(),
    };
    let result = decrypt_content(&payload, &key);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("12 bytes") || err_msg.contains("nonce"),
        "unexpected error: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// 9. Encryption produces unique nonces (probabilistic guarantee)
// ---------------------------------------------------------------------------

#[test]
fn encrypt_produces_unique_nonces_for_same_content() {
    let key = test_key();
    let now = fixed_time();
    let content = "same content encrypted twice";

    let p1 = encrypt_content(content, &key, now).unwrap();
    let p2 = encrypt_content(content, &key, now).unwrap();

    assert_ne!(p1.nonce, p2.nonce, "nonces must be unique per encryption");
    assert_ne!(
        p1.ciphertext, p2.ciphertext,
        "ciphertexts differ due to unique nonces"
    );

    // Both should decrypt to the same plaintext
    assert_eq!(decrypt_content(&p1, &key).unwrap(), content);
    assert_eq!(decrypt_content(&p2, &key).unwrap(), content);
}

// ---------------------------------------------------------------------------
// 10. Concurrent encryption operations on separate files
// ---------------------------------------------------------------------------

#[test]
fn concurrent_encrypt_decrypt_on_separate_files() {
    let root = temp_test_dir("concurrent");
    fs::create_dir_all(&root).unwrap();
    let now = fixed_time();
    let key = Arc::new(test_key());
    let thread_count = 8;

    let files: Vec<PathBuf> = (0..thread_count)
        .map(|i| {
            let sub = root.join(format!("thread_{i}"));
            fs::create_dir_all(&sub).unwrap();
            create_file_with_section(
                &sub,
                &format!("secret_{i}"),
                SectionType::Text,
                &format!("secret data for thread {i} with some extra padding text"),
            )
        })
        .collect();

    // Phase 1: concurrent encryption
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

    // Verify all files are encrypted
    for (i, file) in files.iter().enumerate() {
        let raw = fs::read_to_string(file).unwrap();
        assert!(raw.contains("type: encrypted"), "file {i} not encrypted");
        assert!(
            !raw.contains(&format!("secret data for thread {i}")),
            "file {i} plaintext leaked"
        );
    }

    // Phase 2: concurrent decryption
    let handles: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let file = file.clone();
            let key = Arc::clone(&key);
            std::thread::spawn(move || {
                let doc = decrypt_section(&file, &format!("secret_{i}"), &key, now).unwrap();
                let section = doc
                    .sections
                    .iter()
                    .find(|s| s.path == format!("secret_{i}"))
                    .unwrap();
                assert_eq!(
                    section.content,
                    format!("secret data for thread {i} with some extra padding text")
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
// 11. Multiple sections encrypted in one file
// ---------------------------------------------------------------------------

#[test]
fn encrypt_multiple_sections_in_same_file() {
    let root = temp_test_dir("multi-section");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("memory.rbmem");
    let now = fixed_time();
    let key = test_key();

    create(
        &file,
        CreateOptions {
            created_by: "test".to_string(),
            purpose: "multi-section encryption".to_string(),
            default_expiry_days: None,
            human: false,
            now,
        },
    )
    .unwrap();

    for i in 0..5 {
        update(
            &file,
            UpdateOptions {
                actor: "test".to_string(),
                section: format!("secret_{i}"),
                section_type: SectionType::Text,
                content: format!("secret content number {i}"),
                human: false,
                dry_run: false,
                now,
            },
        )
        .unwrap();
    }

    // Encrypt all sections
    for i in 0..5 {
        encrypt_section(&file, &format!("secret_{i}"), &key, now).unwrap();
    }

    let raw = fs::read_to_string(&file).unwrap();
    for i in 0..5 {
        assert!(
            !raw.contains(&format!("secret content number {i}")),
            "plaintext for section {i} leaked"
        );
    }

    // Decrypt all sections
    for i in 0..5 {
        let doc = decrypt_section(&file, &format!("secret_{i}"), &key, now).unwrap();
        let section = doc
            .sections
            .iter()
            .find(|s| s.path == format!("secret_{i}"))
            .unwrap();
        assert_eq!(section.content, format!("secret content number {i}"));
    }

    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 12. EncryptionKey edge cases
// ---------------------------------------------------------------------------

#[test]
fn encryption_key_from_env_base64() {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let raw = [55u8; 32];
    let encoded = STANDARD.encode(raw);
    let key = EncryptionKey::from_env_value(&encoded).unwrap();
    assert_eq!(key.as_bytes(), &raw);
}

#[test]
fn encryption_key_from_env_raw_bytes() {
    // Use characters outside the base64 alphabet so base64 decode fails
    // and from_env_value falls through to raw byte interpretation.
    let raw = "!key_for_testing_encryption_32b!";
    assert_eq!(raw.len(), 32);
    let key = EncryptionKey::from_env_value(raw).unwrap();
    assert_eq!(key.as_bytes(), raw.as_bytes());
}

#[test]
fn encryption_key_from_env_trims_whitespace() {
    // Use non-base64 chars so trimming is the relevant behavior
    let raw = "  !key_for_testing_encryption_32b!  ";
    let key = EncryptionKey::from_env_value(raw).unwrap();
    assert_eq!(key.as_bytes(), b"!key_for_testing_encryption_32b!");
}

#[test]
fn encryption_key_from_env_wrong_length_fails() {
    let result = EncryptionKey::from_env_value("tooshort");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 13. Read encrypted file without decryption hides content
// ---------------------------------------------------------------------------

#[test]
fn read_encrypted_file_without_key_hides_sections() {
    let root = temp_test_dir("read-hidden");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "hidden", SectionType::Text, "must not appear");
    let now = fixed_time();
    let key = test_key();

    encrypt_section(&file, "hidden", &key, now).unwrap();

    let output = read(
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

    assert!(!output.contains("must not appear"));
    assert!(!output.contains("[SECTION: hidden]"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn read_encrypted_file_with_key_shows_content() {
    let root = temp_test_dir("read-shown");
    fs::create_dir_all(&root).unwrap();
    let file = create_file_with_section(&root, "visible", SectionType::Text, "should appear now");
    let now = fixed_time();
    let key = test_key();

    encrypt_section(&file, "visible", &key, now).unwrap();

    let output = read(
        &file,
        ReadOptions {
            resolve: false,
            compact: false,
            minified: false,
            hide_empty_temporal: false,
            decrypt: true,
            key: Some(key),
            policy: TimestampPolicy::Preserve,
        },
    )
    .unwrap();

    assert!(output.contains("should appear now"));
    assert!(output.contains("[SECTION: visible]"));

    let _ = fs::remove_dir_all(root);
}
