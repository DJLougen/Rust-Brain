use crate::commands::{health_report, OutputFormat};
use crate::document::graph_view_to_json;
use crate::parser::parse_document;
use crate::{CompactMode, RbmemDocument, RbmemError, SectionType, SourceInfo, TimestampPolicy};
use chrono::{DateTime, Utc};
use notify::{Config as NotifyConfig, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::mpsc;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct HermesPayload {
    #[serde(default)]
    pub sections: Vec<HermesSectionPatch>,
}

#[derive(Debug, Deserialize)]
pub struct HermesSectionPatch {
    pub path: String,
    #[serde(default = "default_text_type")]
    pub r#type: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub mode: HermesWriteMode,
    #[serde(default)]
    pub source: Option<SourceInfo>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HermesWriteMode {
    #[default]
    Auto,
    Append,
    Replace,
}

fn default_text_type() -> String {
    "text".to_string()
}

// ---------------------------------------------------------------------------
// JSON helpers for the shared rbmem document shape (Hermes `load` output and the
// CLI `context_json` view). Owned here so the two consumers cannot drift apart.
// ---------------------------------------------------------------------------

pub fn document_meta_json(document: &RbmemDocument) -> Value {
    json!({
        "version": document.meta.version,
        "source_version": document.meta.source_version,
        "purpose": document.meta.purpose,
        "compact_mode": document.meta.compact_mode.to_string(),
        "last_updated": document.meta.last_updated,
    })
}

pub fn sections_json(document: &RbmemDocument, resolve: bool) -> Vec<Value> {
    if resolve {
        document
            .resolved_sections()
            .into_iter()
            .map(|section| {
                json!({
                    "path": section.path,
                    "type": section.section_type.to_string(),
                    "content": section.content,
                    "resolved": true,
                    "temporal": section.temporal,
                    "source": section.source,
                    "graph": section.graph,
                })
            })
            .collect()
    } else {
        document
            .sections
            .iter()
            .map(|section| {
                json!({
                    "path": section.path,
                    "type": section.section_type.to_string(),
                    "content": section.content,
                    "resolved": false,
                    "temporal": section.temporal,
                    "source": section.source.clone(),
                    "graph": section.graph,
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

pub fn set_section_source(document: &mut RbmemDocument, path: &str, source: SourceInfo) {
    if let Some(section) = document
        .sections
        .iter_mut()
        .find(|section| section.path == path)
    {
        section.source = Some(source);
    }
}

pub fn validate_or_error(document: &RbmemDocument) -> Result<(), RbmemError> {
    let warnings = document.validate();
    if warnings.is_empty() {
        Ok(())
    } else {
        Err(RbmemError::Parse(warnings.join("; ")))
    }
}

fn event_is_relevant(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Create(_) | EventKind::Modify(_))
}

fn notify_error(error: notify::Error) -> RbmemError {
    RbmemError::Io(io::Error::other(error))
}

pub(crate) fn external_cli_version(path: &Path) -> Result<String, RbmemError> {
    let output = ProcessCommand::new(path).arg("--version").output()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(RbmemError::Parse(format!(
            "{} --version failed: {}",
            path.display(),
            stderr.trim()
        )))
    }
}

// ---------------------------------------------------------------------------
// Payload read / apply
// ---------------------------------------------------------------------------

pub fn read_hermes_payload(
    json: Option<String>,
    json_file: Option<PathBuf>,
) -> Result<HermesPayload, RbmemError> {
    let text = match (json, json_file) {
        (Some(_), Some(_)) => {
            return Err(RbmemError::Parse(
                "use either --json or --json-file, not both".to_string(),
            ))
        }
        (Some(json), None) => json,
        (None, Some(path)) => fs::read_to_string(path)?,
        (None, None) => {
            return Err(RbmemError::Parse(
                "missing --json or --json-file".to_string(),
            ))
        }
    };
    Ok(serde_json::from_str(&text)?)
}

pub fn apply_hermes_payload(
    document: &mut RbmemDocument,
    payload: HermesPayload,
    now: DateTime<Utc>,
) -> Result<(), RbmemError> {
    for patch in payload.sections {
        let section_type = <SectionType as std::str::FromStr>::from_str(&patch.r#type)?;
        // Append-only protection must key off the *existing* section's type, not
        // just the incoming patch type. Otherwise a `{type:"text",mode:"replace"}`
        // patch could silently overwrite a stored hermes:memory section.
        let existing_type = document
            .sections
            .iter()
            .find(|section| section.path == patch.path)
            .map(|section| section.section_type);
        let is_hermes_memory = section_type == SectionType::HermesMemory
            || existing_type == Some(SectionType::HermesMemory);
        if is_hermes_memory && patch.mode == HermesWriteMode::Replace {
            return Err(RbmemError::Parse(format!(
                "hermes:memory section '{}' is append-only; use mode auto or append",
                patch.path
            )));
        }
        // Preserve the protected type so a non-memory patch cannot downgrade it.
        let effective_type = if existing_type == Some(SectionType::HermesMemory) {
            SectionType::HermesMemory
        } else {
            section_type
        };
        let should_append = patch.mode == HermesWriteMode::Append
            || (patch.mode == HermesWriteMode::Auto && is_hermes_memory);

        if should_append {
            append_or_create_section(document, &patch.path, effective_type, &patch.content, now);
        } else {
            document.upsert_section(&patch.path, effective_type, patch.content, now);
        }
        set_section_source(
            document,
            &patch.path,
            patch.source.unwrap_or_else(|| SourceInfo {
                kind: "hermes".to_string(),
                path: None,
                actor: Some("hermes".to_string()),
                hash: None,
            }),
        );
    }
    document.enforce_protected_timestamps(now);
    Ok(())
}

pub fn append_or_create_section(
    document: &mut RbmemDocument,
    path: &str,
    section_type: SectionType,
    content: &str,
    now: DateTime<Utc>,
) {
    if let Some(section) = document
        .sections
        .iter_mut()
        .find(|section| section.path == path)
    {
        if !section.content.contains(content.trim()) {
            if !section.content.trim().is_empty() {
                section.content.push('\n');
            }
            section.content.push_str(content.trim());
        }
        section.section_type = section_type;
        section.temporal.updated_at = now;
        document.meta.last_updated = now;
    } else {
        document.upsert_section(path, section_type, content.trim().to_string(), now);
    }
}

// ---------------------------------------------------------------------------
// Hermes JSON / inject / starter
// ---------------------------------------------------------------------------

pub fn hermes_json(
    document: &RbmemDocument,
    resolve: bool,
    compact: bool,
    minified: bool,
) -> Result<Value, RbmemError> {
    let context = if minified {
        document.to_minified_string(resolve)
    } else if compact {
        document.to_compact_string(resolve, Utc::now())
    } else if resolve {
        document
            .resolved_sections()
            .into_iter()
            .map(|section| {
                format!(
                    "[{}] {}\n{}",
                    section.path, section.section_type, section.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        document.to_rbmem_string()
    };

    Ok(json!({
        "schema": "hermes.rbmem.v1",
        "meta": document_meta_json(document),
        "context": context,
        "sections": sections_json(document, resolve),
        "graph": graph_view_to_json(&document.graph_view()),
        "timeline": document.timeline().into_iter().map(|section| {
            json!({
                "path": section.path,
                "created_at": section.temporal.created_at,
                "updated_at": section.temporal.updated_at,
                "expires_at": section.temporal.expires_at,
                "content": section.content,
            })
        }).collect::<Vec<_>>(),
    }))
}

pub fn hermes_inject_block(
    document: &RbmemDocument,
    resolve: bool,
    compact: bool,
    minified: bool,
) -> Result<String, RbmemError> {
    let payload = hermes_json(document, resolve, compact, minified)?;
    Ok(format!(
        "### HERMES RBMEM CONTEXT\n```json\n{}\n```\n### END HERMES RBMEM CONTEXT\n",
        serde_json::to_string_pretty(&payload)?
    ))
}

pub fn hermes_starter_document(project_name: &str, now: DateTime<Utc>) -> RbmemDocument {
    let mut document = RbmemDocument::new(now, "hermes");
    document.meta.purpose = format!("hermes-agent-memory:{project_name}");
    document.meta.compact_mode = CompactMode::Minified;
    document.upsert_section(
        "goals",
        SectionType::HermesMemory,
        format!("- Maintain working context for {project_name}."),
        now,
    );
    document.upsert_section(
        "rules",
        SectionType::HermesMemory,
        "- Preserve user intent.\n- Prefer append-only memory updates unless replacing stale facts."
            .to_string(),
        now,
    );
    document.upsert_section("memory", SectionType::HermesMemory, String::new(), now);
    document.upsert_section(
        "tasks",
        SectionType::List,
        "- Initialize Hermes RBMEM memory.".to_string(),
        now,
    );
    document.upsert_section(
        "architecture",
        SectionType::Text,
        "Project architecture notes go here.".to_string(),
        now,
    );
    document.upsert_section(
        "timeline",
        SectionType::Timeline,
        format!("{}: Hermes RBMEM memory initialized.", now.to_rfc3339()),
        now,
    );
    document.upsert_section(
        "graph",
        SectionType::Json,
        "{\n  \"notes\": \"Explicit graph relations can be added per section.\"\n}".to_string(),
        now,
    );
    document
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

pub fn document_diagnostics_json(file: &Path) -> Result<(Value, RbmemDocument), RbmemError> {
    let file_exists = file.exists();
    let text = fs::read_to_string(file)?;
    let parsed = parse_document(&text, TimestampPolicy::Preserve)?;
    let mut warnings = parsed.warnings;
    warnings.extend(parsed.document.validate());
    let graph = parsed.document.graph_view();
    let validation_status = if warnings.is_empty() { "ok" } else { "warning" };
    let value = json!({
        "file": file.display().to_string(),
        "file_exists": file_exists,
        "parse": "ok",
        "meta_version": parsed.document.meta.version,
        "sections": parsed.document.sections.len(),
        "graph_edges": graph.edges.len(),
        "validation": {
            "status": validation_status,
            "warnings": warnings,
        },
    });

    Ok((value, parsed.document))
}

pub(crate) fn append_document_diagnostics(
    output: &mut String,
    file: &Path,
) -> Result<RbmemDocument, RbmemError> {
    output.push_str(&format!("file: {}\n", file.display()));
    output.push_str(&format!(
        "file-exists: {}\n",
        if file.exists() { "ok" } else { "missing" }
    ));

    let text = fs::read_to_string(file)?;
    let parsed = parse_document(&text, TimestampPolicy::Preserve)?;
    let mut warnings = parsed.warnings;
    warnings.extend(parsed.document.validate());
    let graph = parsed.document.graph_view();

    output.push_str("parse: ok\n");
    output.push_str(&format!("meta-version: {}\n", parsed.document.meta.version));
    output.push_str(&format!("sections: {}\n", parsed.document.sections.len()));
    output.push_str(&format!("graph-edges: {}\n", graph.edges.len()));
    if warnings.is_empty() {
        output.push_str("validation: ok\n");
    } else {
        output.push_str(&format!("validation: {} warning(s)\n", warnings.len()));
        for warning in warnings {
            output.push_str(&format!("warning: {warning}\n"));
        }
    }

    Ok(parsed.document)
}

// ---------------------------------------------------------------------------
// Doctor reports
// ---------------------------------------------------------------------------

pub fn hermes_doctor_report(
    file: &Path,
    rbmem_cli: Option<&Path>,
    stale_days: u64,
    format: OutputFormat,
) -> Result<String, RbmemError> {
    match format {
        OutputFormat::Text => hermes_doctor_text_report(file, rbmem_cli, stale_days),
        OutputFormat::Json => Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&hermes_doctor_json(file, rbmem_cli, stale_days)?)?
        )),
    }
}

pub fn hermes_doctor_text_report(
    file: &Path,
    rbmem_cli: Option<&Path>,
    stale_days: u64,
) -> Result<String, RbmemError> {
    let mut output = String::new();
    output.push_str("rbmem hermes doctor\n");
    output.push_str(&format!(
        "cli-version: rbmem {}\n",
        env!("CARGO_PKG_VERSION")
    ));

    if let Some(rbmem_cli) = rbmem_cli {
        output.push_str(&format!("configured-rbmem-cli: {}\n", rbmem_cli.display()));
        output.push_str(&format!(
            "configured-rbmem-cli-version: {}\n",
            external_cli_version(rbmem_cli)?
        ));
    }

    let document = append_document_diagnostics(&mut output, file)?;
    let payload = hermes_json(&document, true, false, true)?;
    let context_len = payload
        .get("context")
        .and_then(Value::as_str)
        .map(str::len)
        .unwrap_or(0);

    output.push_str("hermes-load: ok\n");
    output.push_str(&format!("hermes-context-bytes: {context_len}\n"));

    // Health scoring
    match health_report(file, stale_days) {
        Ok(health) => {
            output.push_str(&format!("health-score: {:.1}/100\n", health.score));
            output.push_str(&format!("health-stale-days: {}\n", stale_days));
            output.push_str(&format!(
                "health-total-sections: {}\n",
                health.total_sections
            ));
            output.push_str(&format!(
                "health-stale-sections: {}\n",
                health.stale_sections
            ));
            output.push_str(&format!(
                "health-orphaned-edges: {}\n",
                health.orphaned_edges
            ));
            output.push_str(&format!("health-conflicts: {}\n", health.conflicts));
        }
        Err(e) => {
            output.push_str(&format!("health-score: error: {e}\n"));
        }
    }

    Ok(output)
}

pub fn hermes_doctor_json(
    file: &Path,
    rbmem_cli: Option<&Path>,
    stale_days: u64,
) -> Result<Value, RbmemError> {
    let configured_rbmem_cli = if let Some(rbmem_cli) = rbmem_cli {
        Some(json!({
            "path": rbmem_cli.display().to_string(),
            "version": external_cli_version(rbmem_cli)?,
        }))
    } else {
        None
    };
    let (document, rbmem_document) = document_diagnostics_json(file)?;
    let payload = hermes_json(&rbmem_document, true, false, true)?;
    let context_bytes = payload
        .get("context")
        .and_then(Value::as_str)
        .map(str::len)
        .unwrap_or(0);

    let health = health_report(file, stale_days).ok().map(|h| {
        json!({
            "score": h.score,
            "stale_days": stale_days,
            "total_sections": h.total_sections,
            "stale_sections": h.stale_sections,
            "orphaned_edges": h.orphaned_edges,
            "conflicts": h.conflicts,
        })
    });

    Ok(json!({
        "schema": "rbmem.hermes.doctor.v1",
        "cli_version": format!("rbmem {}", env!("CARGO_PKG_VERSION")),
        "configured_rbmem_cli": configured_rbmem_cli,
        "document": document,
        "hermes_load": {
            "status": "ok",
            "context_bytes": context_bytes,
        },
        "health": health,
    }))
}

// ---------------------------------------------------------------------------
// File watcher
// ---------------------------------------------------------------------------

pub fn watch_hermes_file(file: PathBuf) -> Result<(), RbmemError> {
    let parent = file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    println!("watching {}", file.display());
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        NotifyConfig::default(),
    )
    .map_err(notify_error)?;
    watcher
        .watch(&parent, RecursiveMode::NonRecursive)
        .map_err(notify_error)?;

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if !event_is_relevant(&event.kind) {
                    continue;
                }
                if event.paths.iter().any(|path| path == &file) {
                    match crate::commands::load(&file, TimestampPolicy::Preserve)
                        .and_then(|document| hermes_json(&document, true, true, false))
                    {
                        Ok(payload) => println!("{}", serde_json::to_string_pretty(&payload)?),
                        Err(error) => eprintln!("hermes watch error: {error}"),
                    }
                }
            }
            Ok(Err(error)) => eprintln!("watch error: {error}"),
            Err(error) => return Err(RbmemError::Io(io::Error::other(error))),
        }
    }
}
