use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use notify::{Config as NotifyConfig, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rbmem::document::graph_view_to_json;
use rbmem::parser::parse_document;
use rbmem::{CompactMode, RbmemDocument, RbmemError, SectionType, TimestampPolicy};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc;
use std::time::SystemTime;

#[derive(Debug, Parser)]
#[command(name = "rbmem")]
#[command(about = "Rust-Brain Memory Format (.rbmem) v1.3 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Create {
        file: PathBuf,
        #[arg(long, default_value = "me")]
        created_by: String,
        #[arg(long, default_value = "personal-agent-memory")]
        purpose: String,
        #[arg(long)]
        default_expiry_days: Option<i64>,
        #[arg(long)]
        human: bool,
    },
    Read {
        file: PathBuf,
        #[arg(long)]
        resolve: bool,
        #[arg(long)]
        compact: bool,
        #[arg(long, alias = "tiny")]
        minified: bool,
        #[arg(long)]
        hide_empty_temporal: bool,
        #[arg(long)]
        hermes: bool,
        #[arg(long)]
        hermes_inject: bool,
    },
    Resolve {
        file: PathBuf,
        #[arg(long)]
        compact: bool,
        #[arg(long, alias = "tiny")]
        minified: bool,
        #[arg(long)]
        hide_empty_temporal: bool,
    },
    Update {
        file: PathBuf,
        #[arg(long)]
        section: String,
        #[arg(long, default_value = "text")]
        r#type: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        content_file: Option<PathBuf>,
        #[arg(long)]
        human: bool,
    },
    Prune {
        file: PathBuf,
    },
    Graph {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
        format: GraphFormat,
    },
    Tree {
        file: PathBuf,
    },
    Validate {
        file: PathBuf,
    },
    ConvertFromMd {
        markdown: PathBuf,
        output: PathBuf,
        #[arg(long)]
        infer_relations: bool,
        #[arg(long, default_value_t = 0.6)]
        min_confidence: f64,
    },
    Infer {
        file: PathBuf,
        #[arg(long, default_value_t = 0.6)]
        min_confidence: f64,
    },
    Sync {
        markdown_folder: PathBuf,
        output_folder: PathBuf,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        infer_relations: bool,
        #[arg(long, default_value_t = 0.6)]
        min_confidence: f64,
        #[arg(long)]
        dry_run: bool,
    },
    Hermes {
        #[command(subcommand)]
        command: HermesCommand,
    },
    Timeline {
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum HermesCommand {
    Load {
        file: PathBuf,
        #[arg(long)]
        compact: bool,
        #[arg(long, alias = "tiny")]
        minified: bool,
        #[arg(long)]
        resolve: bool,
    },
    Save {
        file: PathBuf,
        #[arg(long)]
        json: String,
    },
    Init {
        project_name: String,
    },
    Watch {
        file: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
    Dot,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), RbmemError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Create {
            file,
            created_by,
            purpose,
            default_expiry_days,
            human,
        } => {
            let now = Utc::now();
            let mut document = RbmemDocument::new(now, created_by);
            document.meta.purpose = purpose;
            document.meta.default_expiry_days = default_expiry_days;
            write_document(&file, &document, human)?;
            println!("created {}", file.display());
        }
        Command::Read {
            file,
            resolve,
            compact,
            minified,
            hide_empty_temporal,
            hermes,
            hermes_inject,
        } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            if hermes_inject {
                print!(
                    "{}",
                    hermes_inject_block(&document, resolve, compact, minified)?
                );
            } else if hermes {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&hermes_json(
                        &document, resolve, compact, minified
                    )?)?
                );
            } else if minified
                || (resolve && !compact && document.meta.compact_mode == CompactMode::Minified)
            {
                print!("{}", document.to_minified_string(resolve));
            } else if compact || (resolve && document.meta.compact_mode == CompactMode::Compact) {
                print!("{}", document.to_compact_string(resolve, Utc::now()));
            } else if resolve {
                for section in document.resolved_sections() {
                    println!("[{}] type={}", section.path, section.section_type);
                    println!("{}", section.content);
                    println!();
                }
            } else {
                print_full_document(&document, hide_empty_temporal);
            }
        }
        Command::Resolve {
            file,
            compact,
            minified,
            hide_empty_temporal,
        } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            if minified || (!compact && document.meta.compact_mode == CompactMode::Minified) {
                print!("{}", document.to_minified_string(true));
            } else if compact || document.meta.compact_mode == CompactMode::Compact {
                print!("{}", document.to_compact_string(true, Utc::now()));
            } else {
                let _ = hide_empty_temporal;
                for section in document.resolved_sections() {
                    println!("[{}] type={}", section.path, section.section_type);
                    println!("{}", section.content);
                    println!();
                }
            }
        }
        Command::Update {
            file,
            section,
            r#type,
            content,
            content_file,
            human,
        } => {
            let now = Utc::now();
            let mut document = if file.exists() {
                read_document(&file, TimestampPolicy::Preserve)?
            } else {
                RbmemDocument::new(now, "me")
            };
            let section_type = SectionType::from_str(&r#type)?;
            let body = read_content_argument(content, content_file)?;
            document.upsert_section(&section, section_type, body, now);
            document.enforce_protected_timestamps(now);
            write_document(&file, &document, human)?;
            println!("updated {}", file.display());
        }
        Command::Prune { file } => {
            let now = Utc::now();
            let mut document = read_document(&file, TimestampPolicy::Preserve)?;
            let removed = document.prune_expired(now);
            write_document(&file, &document, false)?;
            println!("removed {removed} expired section(s)");
        }
        Command::Graph { file, format } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            match format {
                GraphFormat::Json => {
                    let json = graph_view_to_json(&document.graph_view());
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                GraphFormat::Dot => print!("{}", document.graph_as_dot()),
            }
        }
        Command::Tree { file } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            print!("{}", document.tree());
        }
        Command::Validate { file } => {
            let parsed = parse_document(&fs::read_to_string(&file)?, TimestampPolicy::Preserve)?;
            let mut warnings = parsed.warnings;
            warnings.extend(parsed.document.validate());
            if warnings.is_empty() {
                println!("valid RBMEM v1.3");
            } else {
                for warning in warnings {
                    println!("warning: {warning}");
                }
            }
        }
        Command::ConvertFromMd {
            markdown,
            output,
            infer_relations,
            min_confidence,
        } => {
            let now = Utc::now();
            let text = fs::read_to_string(&markdown)?;
            let mut document = convert_markdown_to_rbmem(&text, now);
            if infer_relations {
                document.infer_relations(now, min_confidence);
            }
            write_document(&output, &document, false)?;
            println!("converted {} -> {}", markdown.display(), output.display());
        }
        Command::Infer {
            file,
            min_confidence,
        } => {
            let now = Utc::now();
            let mut document = read_document(&file, TimestampPolicy::Preserve)?;
            let added = document.infer_relations(now, min_confidence);
            write_document(&file, &document, false)?;
            println!("added {added} inferred relation(s)");
        }
        Command::Sync {
            markdown_folder,
            output_folder,
            watch,
            infer_relations,
            min_confidence,
            dry_run,
        } => {
            let options = SyncOptions::from_folder(
                &markdown_folder,
                infer_relations,
                min_confidence,
                dry_run,
            )?;
            sync_markdown_folder(&markdown_folder, &output_folder, &options)?;
            if watch {
                watch_markdown_folder(markdown_folder, output_folder, options)?;
            }
        }
        Command::Hermes { command } => match command {
            HermesCommand::Load {
                file,
                compact,
                minified,
                resolve,
            } => {
                let document = read_document(&file, TimestampPolicy::Preserve)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&hermes_json(
                        &document, resolve, compact, minified
                    )?)?
                );
            }
            HermesCommand::Save { file, json } => {
                let now = Utc::now();
                let mut document = if file.exists() {
                    read_document(&file, TimestampPolicy::Preserve)?
                } else {
                    RbmemDocument::new(now, "hermes")
                };
                let payload = read_hermes_payload(&json)?;
                apply_hermes_payload(&mut document, payload, now)?;
                write_document(&file, &document, false)?;
                validate_or_error(&document)?;
                println!("saved {}", file.display());
            }
            HermesCommand::Init { project_name } => {
                let now = Utc::now();
                let document = hermes_starter_document(&project_name, now);
                let file = PathBuf::from(format!("{}.rbmem", title_to_path(&project_name)));
                write_document(&file, &document, false)?;
                println!("created {}", file.display());
            }
            HermesCommand::Watch { file } => {
                watch_hermes_file(file)?;
            }
        },
        Command::Timeline { file } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            for section in document.timeline() {
                println!(
                    "{}  {}  {}",
                    section.temporal.created_at.to_rfc3339(),
                    section.path,
                    first_line(&section.content)
                );
            }
        }
    }

    Ok(())
}

fn read_document(path: &Path, policy: TimestampPolicy) -> Result<RbmemDocument, RbmemError> {
    let input = fs::read_to_string(path)?;
    Ok(parse_document(&input, policy)?.document)
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

fn print_full_document(document: &RbmemDocument, hide_empty_temporal: bool) {
    if hide_empty_temporal {
        print!("{}", document.to_rbmem_string_hiding_empty_temporal());
    } else {
        print!("{}", document.to_rbmem_string());
    }
}

fn read_content_argument(
    content: Option<String>,
    content_file: Option<PathBuf>,
) -> Result<String, RbmemError> {
    match (content, content_file) {
        (Some(content), None) => Ok(content),
        (None, Some(path)) => Ok(fs::read_to_string(path)?),
        (None, None) => Ok(String::new()),
        (Some(_), Some(_)) => Err(RbmemError::Parse(
            "use either --content or --content-file, not both".to_string(),
        )),
    }
}

fn convert_markdown_to_rbmem(markdown: &str, now: chrono::DateTime<Utc>) -> RbmemDocument {
    let mut document = RbmemDocument::new(now, "me");
    let mut heading_stack: Vec<String> = Vec::new();
    let mut current_path = "meta.markdown".to_string();
    let mut current_lines = Vec::new();

    for line in markdown.lines() {
        if let Some((level, title)) = markdown_heading(line) {
            flush_markdown_section(&mut document, &current_path, &mut current_lines, now);
            heading_stack.truncate(level.saturating_sub(1));
            heading_stack.push(title_to_path(title));
            current_path = heading_stack.join(".");
        } else {
            current_lines.push(line.to_string());
        }
    }

    flush_markdown_section(&mut document, &current_path, &mut current_lines, now);
    document
}

fn flush_markdown_section(
    document: &mut RbmemDocument,
    path: &str,
    lines: &mut Vec<String>,
    now: chrono::DateTime<Utc>,
) {
    let content = lines.join("\n").trim().to_string();
    if !content.is_empty() {
        document.upsert_section(path, SectionType::Text, content, now);
    }
    lines.clear();
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let after_hashes = trimmed.get(hashes..)?;
    if !after_hashes.starts_with(' ') {
        return None;
    }

    let title = after_hashes.trim();
    (!title.is_empty()).then_some((hashes, title))
}

fn title_to_path(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            // Spaces and punctuation belong inside the current heading segment.
            // Dots are reserved for real Markdown heading depth.
            slug.push('-');
            last_was_separator = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "document".to_string()
    } else {
        slug
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

#[derive(Debug, Clone)]
struct SyncOptions {
    infer_relations: bool,
    min_confidence: f64,
    dry_run: bool,
    default_expiry_days: Option<i64>,
    compact_mode: CompactMode,
}

impl SyncOptions {
    fn from_folder(
        markdown_folder: &Path,
        infer_relations: bool,
        min_confidence: f64,
        dry_run: bool,
    ) -> Result<Self, RbmemError> {
        let mut options = Self {
            infer_relations,
            min_confidence: min_confidence.clamp(0.0, 1.0),
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

fn apply_sync_config(text: &str, options: &mut SyncOptions) -> Result<(), RbmemError> {
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

fn sync_markdown_folder(
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

fn sync_markdown_file(
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
    if options.infer_relations {
        document.infer_relations(now, options.min_confidence);
    }

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    write_document(&output_file, &document, false)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncAction {
    Create,
    Update,
    Skip,
}

fn sync_action(
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

fn modified_time(path: &Path) -> Result<SystemTime, RbmemError> {
    Ok(fs::metadata(path)?.modified()?)
}

fn output_path_for_markdown(
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

fn find_markdown_files(folder: &Path) -> Result<Vec<PathBuf>, RbmemError> {
    let mut files = Vec::new();
    collect_markdown_files(folder, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown_files(folder: &Path, files: &mut Vec<PathBuf>) -> Result<(), RbmemError> {
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

fn watch_markdown_folder(
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

fn hermes_json(
    document: &RbmemDocument,
    resolve: bool,
    compact: bool,
    minified: bool,
) -> Result<Value, RbmemError> {
    let sections = if resolve {
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
                    "graph": section.graph,
                })
            })
            .collect::<Vec<_>>()
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
                    "graph": section.graph,
                })
            })
            .collect::<Vec<_>>()
    };

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
        "meta": {
            "version": document.meta.version,
            "purpose": document.meta.purpose,
            "compact_mode": document.meta.compact_mode.to_string(),
            "last_updated": document.meta.last_updated,
        },
        "context": context,
        "sections": sections,
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

fn hermes_inject_block(
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

#[derive(Debug, Deserialize)]
struct HermesPayload {
    #[serde(default)]
    sections: Vec<HermesSectionPatch>,
}

#[derive(Debug, Deserialize)]
struct HermesSectionPatch {
    path: String,
    #[serde(default = "default_text_type")]
    r#type: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    mode: HermesWriteMode,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum HermesWriteMode {
    #[default]
    Auto,
    Append,
    Replace,
}

fn default_text_type() -> String {
    "text".to_string()
}

fn read_hermes_payload(input: &str) -> Result<HermesPayload, RbmemError> {
    let text = if Path::new(input).exists() {
        fs::read_to_string(input)?
    } else {
        input.to_string()
    };
    Ok(serde_json::from_str(&text)?)
}

fn apply_hermes_payload(
    document: &mut RbmemDocument,
    payload: HermesPayload,
    now: chrono::DateTime<Utc>,
) -> Result<(), RbmemError> {
    for patch in payload.sections {
        let section_type = SectionType::from_str(&patch.r#type)?;
        let should_append = patch.mode == HermesWriteMode::Append
            || (patch.mode == HermesWriteMode::Auto && section_type == SectionType::HermesMemory);

        if should_append {
            append_or_create_section(document, &patch.path, section_type, &patch.content, now);
        } else {
            document.upsert_section(&patch.path, section_type, patch.content, now);
        }
    }
    document.enforce_protected_timestamps(now);
    Ok(())
}

fn append_or_create_section(
    document: &mut RbmemDocument,
    path: &str,
    section_type: SectionType,
    content: &str,
    now: chrono::DateTime<Utc>,
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

fn validate_or_error(document: &RbmemDocument) -> Result<(), RbmemError> {
    let warnings = document.validate();
    if warnings.is_empty() {
        Ok(())
    } else {
        Err(RbmemError::Parse(warnings.join("; ")))
    }
}

fn hermes_starter_document(project_name: &str, now: chrono::DateTime<Utc>) -> RbmemDocument {
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

fn watch_hermes_file(file: PathBuf) -> Result<(), RbmemError> {
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
                    match read_document(&file, TimestampPolicy::Preserve)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 27, 13, 10, 0).unwrap()
    }

    #[test]
    fn markdown_converter_preserves_heading_hierarchy() {
        let document = convert_markdown_to_rbmem(
            r#"# Agents
Root body.

## Reader
Reader body.

### Capabilities
Uses Writer.

## Writer
Writer body.
"#,
            fixed_time(),
        );

        let paths = document
            .sections
            .iter()
            .map(|section| section.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec![
                "agents",
                "agents.reader",
                "agents.reader.capabilities",
                "agents.writer"
            ]
        );
    }

    #[test]
    fn markdown_heading_words_stay_inside_one_path_segment() {
        let document = convert_markdown_to_rbmem(
            r#"# Inhibition of Return

## How It Works

### Theoretical Mechanisms
Body.
"#,
            fixed_time(),
        );

        let paths = document
            .sections
            .iter()
            .map(|section| section.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec!["inhibition-of-return.how-it-works.theoretical-mechanisms"]
        );
    }

    #[test]
    fn markdown_converter_preserves_frontmatter_as_meta_section() {
        let document = convert_markdown_to_rbmem(
            r#"---
title: Test
---

# Agents
Body.
"#,
            fixed_time(),
        );

        let meta = document
            .sections
            .iter()
            .find(|section| section.path == "meta.markdown")
            .unwrap();

        assert!(meta.content.contains("title: Test"));
    }

    #[test]
    fn sync_non_watch_converts_nested_markdown_files() {
        let root = temp_test_dir("sync-nested");
        let markdown = root.join("md");
        let output = root.join("out");
        fs::create_dir_all(markdown.join("concepts")).unwrap();
        fs::write(
            markdown.join("concepts").join("agent.md"),
            "# Agent\n\nRoot.\n\n## Memory\n\nUses Tools.",
        )
        .unwrap();
        fs::write(markdown.join(".rbmemsync"), "compact_mode: minified\n").unwrap();

        let options = SyncOptions::from_folder(&markdown, false, 0.6, false).unwrap();
        sync_markdown_folder(&markdown, &output, &options).unwrap();

        let generated = output.join("concepts").join("agent.rbmem");
        assert!(generated.exists());
        let parsed = parse_document(
            &fs::read_to_string(generated).unwrap(),
            TimestampPolicy::Preserve,
        )
        .unwrap();
        assert_eq!(parsed.document.meta.compact_mode, CompactMode::Minified);
        assert!(parsed
            .document
            .sections
            .iter()
            .any(|section| section.path == "agent.memory"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_dry_run_does_not_write_outputs() {
        let root = temp_test_dir("sync-dry-run");
        let markdown = root.join("md");
        let output = root.join("out");
        fs::create_dir_all(&markdown).unwrap();
        fs::write(markdown.join("note.md"), "# Note\n\nBody.").unwrap();

        let options = SyncOptions::from_folder(&markdown, false, 0.6, true).unwrap();
        sync_markdown_folder(&markdown, &output, &options).unwrap();

        assert!(!output.join("note.rbmem").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_skips_unchanged_outputs() {
        let root = temp_test_dir("sync-skip");
        let markdown = root.join("md");
        let output = root.join("out");
        fs::create_dir_all(&markdown).unwrap();
        fs::write(markdown.join("note.md"), "# Note\n\nBody.").unwrap();

        let options = SyncOptions::from_folder(&markdown, false, 0.6, false).unwrap();
        sync_markdown_folder(&markdown, &output, &options).unwrap();
        let generated = output.join("note.rbmem");
        let first_modified = fs::metadata(&generated).unwrap().modified().unwrap();
        sync_markdown_folder(&markdown, &output, &options).unwrap();
        let second_modified = fs::metadata(&generated).unwrap().modified().unwrap();

        assert_eq!(first_modified, second_modified);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hermes_starter_has_standard_sections() {
        let document = hermes_starter_document("Demo Project", fixed_time());
        let paths = document
            .sections
            .iter()
            .map(|section| section.path.as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"goals"));
        assert!(paths.contains(&"rules"));
        assert!(paths.contains(&"memory"));
        assert!(paths.contains(&"tasks"));
        assert!(paths.contains(&"architecture"));
        assert!(paths.contains(&"timeline"));
        assert!(paths.contains(&"graph"));
        assert!(document
            .sections
            .iter()
            .any(|section| section.section_type == SectionType::HermesMemory));
    }

    #[test]
    fn hermes_json_contains_sections_graph_timeline_and_context() {
        let document = hermes_starter_document("Demo Project", fixed_time());
        let payload = hermes_json(&document, true, false, true).unwrap();

        assert_eq!(payload["schema"], "hermes.rbmem.v1");
        assert!(payload["sections"].as_array().unwrap().len() >= 6);
        assert!(payload["graph"]["nodes"].as_array().is_some());
        assert!(payload["timeline"].as_array().unwrap().len() == 1);
        assert!(payload["context"].as_str().unwrap().contains("[goals]"));
    }

    #[test]
    fn hermes_save_payload_appends_hermes_memory() {
        let now = fixed_time();
        let mut document = hermes_starter_document("Demo Project", now);
        let payload = read_hermes_payload(
            r#"{
              "sections": [
                {
                  "path": "memory",
                  "type": "hermes:memory",
                  "content": "- User prefers compact context.",
                  "mode": "auto"
                }
              ]
            }"#,
        )
        .unwrap();

        apply_hermes_payload(&mut document, payload, now).unwrap();
        apply_hermes_payload(
            &mut document,
            read_hermes_payload(
                r#"{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers compact context."}]}"#,
            )
            .unwrap(),
            now,
        )
        .unwrap();

        let memory = document
            .sections
            .iter()
            .find(|section| section.path == "memory")
            .unwrap();
        assert_eq!(
            memory
                .content
                .matches("User prefers compact context")
                .count(),
            1
        );
        assert_eq!(memory.section_type, SectionType::HermesMemory);
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rbmem-{name}-{suffix}"))
    }
}
