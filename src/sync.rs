use crate::markdown::convert_markdown_to_rbmem;
use crate::{CompactMode, InferenceStrategy, RbmemDocument, RbmemError};
use crate::document::SourceInfo;
use chrono::Utc;
use notify::{Config as NotifyConfig, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub infer_relations: bool,
    pub min_confidence: f64,
    pub inference_strategy: InferenceStrategy,
    pub dry_run: bool,
    pub default_expiry_days: Option<i64>,
    pub compact_mode: CompactMode,
}

impl SyncOptions {
    pub fn from_folder(
        markdown_folder: &Path,
        infer_relations: bool,
        min_confidence: f64,
        inference_strategy: InferenceStrategy,
        dry_run: bool,
    ) -> Result<Self, RbmemError> {
        let mut options = Self {
            infer_relations,
            min_confidence: min_confidence.clamp(0.0, 1.0),
            inference_strategy,
            dry_run,
            default_expiry_days: None,
            compact_mode: CompactMode::Full,
        };

        let config_path = markdown_folder.join(".rbmemsync");
        if config_path.exists() {
            let text = fs::read_to_string(config_path)?;
            apply_sync_config(&text, &mut options)?;
        }

        Ok(options)
    }
}

pub fn apply_sync_config(text: &str, options: &mut SyncOptions) -> Result<(), RbmemError> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');

        match key {
            "infer_relations" => {
                options.infer_relations = matches!(value, "true" | "yes" | "1");
            }
            "min_confidence" => {
                if let Ok(confidence) = value.parse::<f64>() {
                    options.min_confidence = confidence.clamp(0.0, 1.0);
                }
            }
            "inference_strategy" => {
                options.inference_strategy = <InferenceStrategy as FromStr>::from_str(value)?;
            }
            "default_expiry_days" => {
                options.default_expiry_days = if value.eq_ignore_ascii_case("null") {
                    None
                } else {
                    Some(value.parse::<i64>().map_err(|_| {
                        RbmemError::Parse("invalid default_expiry_days in .rbmemsync".to_string())
                    })?)
                };
            }
            "compact_mode" => {
                options.compact_mode = CompactMode::from_str(value)?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn sync_markdown_folder(
    markdown_folder: &Path,
    output_folder: &Path,
    options: &SyncOptions,
) -> Result<(), RbmemError> {
    let markdown_files = find_markdown_files(markdown_folder)?;
    if markdown_files.is_empty() {
        println!(
            "skipped: no Markdown files under {}",
            markdown_folder.display()
        );
        return Ok(());
    }

    for markdown_file in markdown_files {
        sync_markdown_file(
            markdown_folder,
            output_folder,
            &markdown_file,
            options,
            false,
        )?;
    }

    Ok(())
}

pub fn sync_markdown_file(
    markdown_folder: &Path,
    output_folder: &Path,
    markdown_file: &Path,
    options: &SyncOptions,
    force: bool,
) -> Result<(), RbmemError> {
    let output_file = output_path_for_markdown(markdown_folder, output_folder, markdown_file)?;
    let action = sync_action(markdown_file, &output_file, force)?;

    match action {
        SyncAction::Skip => {
            println!("skipped {}", markdown_file.display());
            return Ok(());
        }
        SyncAction::Create if options.dry_run => {
            println!(
                "would create {} from {}",
                output_file.display(),
                markdown_file.display()
            );
            return Ok(());
        }
        SyncAction::Update if options.dry_run => {
            println!(
                "would update {} from {}",
                output_file.display(),
                markdown_file.display()
            );
            return Ok(());
        }
        SyncAction::Create => println!(
            "created {} from {}",
            output_file.display(),
            markdown_file.display()
        ),
        SyncAction::Update => println!(
            "updated {} from {}",
            output_file.display(),
            markdown_file.display()
        ),
    }

    let now = Utc::now();
    let markdown = fs::read_to_string(markdown_file)?;
    let mut document = convert_markdown_to_rbmem(&markdown, now);
    document.meta.default_expiry_days = options.default_expiry_days;
    document.meta.compact_mode = options.compact_mode;
    let source_path = markdown_file
        .strip_prefix(markdown_folder)
        .unwrap_or(markdown_file)
        .display()
        .to_string();
    stamp_document_source(
        &mut document,
        source_info("markdown", Some(source_path), "sync", Some(&markdown)),
    );
    if options.infer_relations {
        document.infer_relations_with_strategy(
            now,
            options.min_confidence,
            options.inference_strategy,
        );
    }

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    write_document(&output_file, &document, false)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    Create,
    Update,
    Skip,
}

pub fn sync_action(
    markdown_file: &Path,
    output_file: &Path,
    force: bool,
) -> Result<SyncAction, RbmemError> {
    if !output_file.exists() {
        return Ok(SyncAction::Create);
    }
    if force {
        return Ok(SyncAction::Update);
    }

    let markdown_modified = modified_time(markdown_file)?;
    let output_modified = modified_time(output_file)?;
    if markdown_modified > output_modified {
        Ok(SyncAction::Update)
    } else {
        Ok(SyncAction::Skip)
    }
}

pub fn modified_time(path: &Path) -> Result<SystemTime, RbmemError> {
    Ok(fs::metadata(path)?.modified()?)
}

pub fn output_path_for_markdown(
    markdown_folder: &Path,
    output_folder: &Path,
    markdown_file: &Path,
) -> Result<PathBuf, RbmemError> {
    let relative = markdown_file.strip_prefix(markdown_folder).map_err(|_| {
        RbmemError::Parse(format!(
            "{} is not inside {}",
            markdown_file.display(),
            markdown_folder.display()
        ))
    })?;
    Ok(output_folder.join(relative).with_extension("rbmem"))
}

pub fn find_markdown_files(folder: &Path) -> Result<Vec<PathBuf>, RbmemError> {
    let mut files = Vec::new();
    collect_markdown_files(folder, &mut files)?;
    files.sort();
    Ok(files)
}

pub fn collect_markdown_files(folder: &Path, files: &mut Vec<PathBuf>) -> Result<(), RbmemError> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    Ok(())
}

pub fn watch_markdown_folder(
    markdown_folder: PathBuf,
    output_folder: PathBuf,
    options: SyncOptions,
) -> Result<(), RbmemError> {
    println!("watching {}", markdown_folder.display());
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        NotifyConfig::default(),
    )
    .map_err(notify_error)?;
    watcher
        .watch(&markdown_folder, RecursiveMode::Recursive)
        .map_err(notify_error)?;

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if !event_is_relevant(&event.kind) {
                    continue;
                }
                for path in event.paths {
                    if path.extension().is_some_and(|extension| extension == "md") {
                        if let Err(error) = sync_markdown_file(
                            &markdown_folder,
                            &output_folder,
                            &path,
                            &options,
                            true,
                        ) {
                            eprintln!("sync error for {}: {error}", path.display());
                        }
                    }
                }
            }
            Ok(Err(error)) => eprintln!("watch error: {error}"),
            Err(error) => return Err(RbmemError::Io(io::Error::other(error))),
        }
    }
}

fn event_is_relevant(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Create(_) | EventKind::Modify(_))
}

fn notify_error(error: notify::Error) -> RbmemError {
    RbmemError::Io(io::Error::other(error))
}

pub fn stamp_document_source(document: &mut RbmemDocument, source: SourceInfo) {
    for section in &mut document.sections {
        section.source = Some(source.clone());
    }
}

pub fn source_info(kind: impl Into<String>, path: Option<String>, actor: impl Into<String>, hash_input: Option<&str>) -> SourceInfo {
    SourceInfo {
        kind: kind.into(),
        path,
        actor: Some(actor.into()),
        hash: hash_input.map(sha256_hex),
    }
}

fn sha256_hex(input: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, input.as_bytes());
    let mut output = String::from("sha256:");
    for byte in digest.as_ref() {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn write_document(path: &Path, document: &RbmemDocument, human: bool) -> Result<(), RbmemError> {
    let text = if human {
        document.to_human_rbmem_string()
    } else {
        document.to_rbmem_string()
    };
    fs::write(path, text)?;
    Ok(())
}
