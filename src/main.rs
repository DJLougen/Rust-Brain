use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use rbmem::commands as api;
use rbmem::document::graph_view_to_json;
#[cfg(test)]
use rbmem::document::GraphInfo;
use rbmem::hermes;
use rbmem::markdown;
use rbmem::md_sync;
use rbmem::pack;
use rbmem::parser::parse_document;
#[cfg(test)]
use rbmem::GraphRelation;
#[cfg(test)]
use rbmem::SourceInfo;
use rbmem::{
    CompactMode, DiffFormat, InferenceStrategy, MergeStrategy, OutputFormat, PlanOptions,
    PlanReport, RbmemDocument, RbmemError, SatBackend, SatStatus, SectionType, TimestampPolicy,
};
use serde_json::{json, Value};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::SystemTime;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "rbmem")]
#[command(version)]
#[command(about = "Rust-Brain Memory Format (.rbmem) CLI")]
struct Cli {
    #[arg(long, value_enum, default_value_t = LogFormat::Text, global = true)]
    log_format: LogFormat,
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
    DeleteSection {
        file: PathBuf,
        #[arg(long)]
        section: String,
        #[arg(long)]
        dry_run: bool,
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
        decrypt: bool,
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
        #[arg(long, default_value = "me")]
        actor: String,
        #[arg(long)]
        human: bool,
        #[arg(long)]
        dry_run: bool,
    },
    Prune {
        file: PathBuf,
    },
    Graph {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
        format: GraphFormat,
    },
    Export {
        file: PathBuf,
        #[arg(long, value_enum)]
        format: rbmem::ExportFormat,
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
        #[arg(long, value_enum, default_value_t = InferenceStrategy::Balanced)]
        inference_strategy: InferenceStrategy,
    },
    Infer {
        file: PathBuf,
        #[arg(long, default_value_t = 0.6)]
        min_confidence: f64,
        #[arg(long, value_enum, default_value_t = InferenceStrategy::Balanced)]
        inference_strategy: InferenceStrategy,
    },
    Query {
        file: PathBuf,
        text: String,
        #[arg(long)]
        resolve: bool,
        #[arg(long)]
        compact: bool,
        #[arg(long, alias = "tiny")]
        minified: bool,
        #[arg(long, default_value_t = 0)]
        graph_depth: usize,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        decrypt: bool,
        #[arg(long)]
        max_tokens: Option<usize>,
    },
    Context {
        file: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long)]
        resolve: bool,
        #[arg(long)]
        compact: bool,
        #[arg(long, alias = "tiny")]
        minified: bool,
        #[arg(long, default_value_t = 1)]
        graph_depth: usize,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        decrypt: bool,
        #[arg(long)]
        max_tokens: Option<usize>,
    },
    Plan {
        goal: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        from_memory: bool,
        #[arg(long)]
        pack: Option<String>,
        #[arg(long, value_enum, default_value_t = SatBackend::Auto)]
        solver: SatBackend,
        #[arg(long)]
        proof: bool,
        #[arg(long)]
        proof_path: Option<PathBuf>,
        #[arg(long)]
        verify_proof: bool,
        #[arg(long)]
        cube_and_conquer: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Encrypt {
        file: PathBuf,
        #[arg(long)]
        section: String,
    },
    Decrypt {
        file: PathBuf,
        #[arg(long)]
        section: String,
    },
    Diff {
        before: PathBuf,
        after: PathBuf,
        #[arg(long, value_enum, default_value_t = DiffFormat::Text)]
        format: DiffFormat,
    },
    Merge {
        base: PathBuf,
        local: PathBuf,
        remote: PathBuf,
        #[arg(long, value_enum, default_value_t = MergeStrategy::Manual)]
        strategy: MergeStrategy,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Migrate {
        file: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    Review {
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Doctor {
        file: Option<PathBuf>,
        #[arg(long, default_value_t = 30)]
        stale_days: u64,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Pack {
        file: PathBuf,
        name: String,
        #[arg(long)]
        pack_file: Option<PathBuf>,
        #[arg(long)]
        resolve: bool,
        #[arg(long)]
        compact: bool,
        #[arg(long, alias = "tiny")]
        minified: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        max_tokens: Option<usize>,
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
        #[arg(long, value_enum, default_value_t = InferenceStrategy::Balanced)]
        inference_strategy: InferenceStrategy,
        #[arg(long)]
        dry_run: bool,
    },
    Serve {
        #[arg(long, default_value = "localhost:3000")]
        bind: String,
        #[arg(long)]
        dir: PathBuf,
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
        json: Option<String>,
        #[arg(long)]
        json_file: Option<PathBuf>,
    },
    Plan {
        file: PathBuf,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long)]
        from_memory: bool,
        #[arg(long)]
        pack: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    Init {
        project_name: String,
    },
    Watch {
        file: PathBuf,
    },
    Doctor {
        file: PathBuf,
        #[arg(long, default_value_t = 30)]
        stale_days: u64,
        #[arg(long)]
        rbmem_cli: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
    Dot,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

fn init_tracing(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let result = match format {
        LogFormat::Text => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .without_time()
            .try_init(),
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .try_init(),
    };
    let _ = result;
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        let mut source = error.source();
        while let Some(error) = source {
            eprintln!("caused by: {error}");
            source = error.source();
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), RbmemError> {
    let cli = Cli::parse();
    init_tracing(cli.log_format);
    tracing::info!(command = ?cli.command, "rbmem_command_start");

    match cli.command {
        Command::Create {
            file,
            created_by,
            purpose,
            default_expiry_days,
            human,
        } => {
            let now = Utc::now();
            api::create(
                &file,
                api::CreateOptions {
                    created_by,
                    purpose,
                    default_expiry_days,
                    human,
                    now,
                },
            )?;
            println!("created {}", file.display());
        }
        Command::Read {
            file,
            resolve,
            compact,
            minified,
            hide_empty_temporal,
            decrypt,
            hermes,
            hermes_inject,
        } => {
            if hermes_inject {
                let document = api::load(&file, TimestampPolicy::Preserve)?;
                print!(
                    "{}",
                    hermes::hermes_inject_block(&document, resolve, compact, minified)?
                );
            } else if hermes {
                let document = api::load(&file, TimestampPolicy::Preserve)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&hermes::hermes_json(
                        &document, resolve, compact, minified
                    )?)?
                );
            } else {
                print!(
                    "{}",
                    api::read(
                        &file,
                        api::ReadOptions {
                            resolve,
                            compact,
                            minified,
                            hide_empty_temporal,
                            decrypt,
                            key: None,
                            policy: TimestampPolicy::Preserve,
                        },
                    )?
                );
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
            actor,
            human,
            dry_run,
        } => {
            let now = Utc::now();
            let section_type = <SectionType as std::str::FromStr>::from_str(&r#type)?;
            let body = api::read_content_argument(content, content_file)?;
            api::update(
                &file,
                api::UpdateOptions {
                    section,
                    section_type,
                    content: body,
                    actor,
                    human,
                    dry_run,
                    now,
                },
            )?;
            if dry_run {
                println!("would update {}", file.display());
            } else {
                println!("updated {}", file.display());
            }
        }
        Command::DeleteSection {
            file,
            section,
            dry_run,
        } => {
            api::delete_section(&file, &section, dry_run, Utc::now())?;
            if dry_run {
                println!("would delete {}#{}", file.display(), section);
            } else {
                println!("deleted {}#{}", file.display(), section);
            }
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
        Command::Export { file, format } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            print!("{}", rbmem::export_graph(&document, format)?);
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
                println!("valid RBMEM v1.4.0");
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
            inference_strategy,
        } => {
            let now = Utc::now();
            let text = fs::read_to_string(&markdown)?;
            let mut document = markdown::convert_markdown_to_rbmem(&text, now);
            md_sync::stamp_document_source(
                &mut document,
                md_sync::source_info(
                    "markdown",
                    Some(markdown.display().to_string()),
                    "sync",
                    Some(&text),
                ),
            );
            if infer_relations {
                document.infer_relations_with_strategy(now, min_confidence, inference_strategy);
            }
            write_document(&output, &document, false)?;
            println!("converted {} -> {}", markdown.display(), output.display());
        }
        Command::Infer {
            file,
            min_confidence,
            inference_strategy,
        } => {
            let now = Utc::now();
            let mut document = read_document(&file, TimestampPolicy::Preserve)?;
            let added =
                document.infer_relations_with_strategy(now, min_confidence, inference_strategy);
            write_document(&file, &document, false)?;
            println!("added {added} inferred relation(s)");
        }
        Command::Query {
            file,
            text,
            resolve,
            compact,
            minified,
            graph_depth,
            format,
            decrypt,
            max_tokens,
        } => {
            print!(
                "{}",
                api::query(
                    &file,
                    &text,
                    api::ContextOptions {
                        resolve,
                        compact,
                        minified,
                        graph_depth,
                        decrypt,
                        key: None,
                        format,
                        policy: TimestampPolicy::Preserve,
                        max_tokens,
                    },
                )?
            );
            if format == OutputFormat::Json {
                println!();
            }
        }
        Command::Context {
            file,
            task,
            resolve,
            compact,
            minified,
            graph_depth,
            format,
            decrypt,
            max_tokens,
        } => {
            print!(
                "{}",
                api::context(
                    &file,
                    &task,
                    api::ContextOptions {
                        resolve,
                        compact,
                        minified,
                        graph_depth,
                        decrypt,
                        key: None,
                        format,
                        policy: TimestampPolicy::Preserve,
                        max_tokens,
                    },
                )?
            );
            if format == OutputFormat::Json {
                println!();
            }
        }
        Command::Plan {
            goal,
            file,
            from_memory,
            pack,
            solver,
            proof,
            proof_path,
            verify_proof,
            cube_and_conquer,
            dry_run,
            format,
        } => {
            let report = rbmem::plan_memory(PlanOptions {
                goal,
                from_memory,
                file,
                search_dir: std::env::current_dir().map_err(RbmemError::Io)?,
                context_pack: pack,
                solver,
                proof,
                proof_path,
                verify_proof,
                cube_and_conquer,
                dry_run,
                now: Utc::now(),
            })?;
            match format {
                OutputFormat::Text => print!("{}", render_plan_report(&report)),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
        }
        Command::Encrypt { file, section } => {
            let key = rbmem::EncryptionKey::resolve()?;
            api::encrypt_section(&file, &section, &key, Utc::now())?;
            println!("encrypted {}#{}", file.display(), section);
        }
        Command::Decrypt { file, section } => {
            let key = rbmem::EncryptionKey::resolve()?;
            api::decrypt_section(&file, &section, &key, Utc::now())?;
            println!("decrypted {}#{}", file.display(), section);
        }
        Command::Diff {
            before,
            after,
            format,
        } => {
            print!("{}", api::diff_file_with_format(&before, &after, format)?);
        }
        Command::Merge {
            base,
            local,
            remote,
            strategy,
            output,
        } => {
            let base_document = api::load(&base, TimestampPolicy::Preserve)?;
            let local_document = api::load(&local, TimestampPolicy::Preserve)?;
            let remote_document = api::load(&remote, TimestampPolicy::Preserve)?;
            let merged = rbmem::merge_documents(
                &base_document,
                &local_document,
                &remote_document,
                strategy,
                Utc::now(),
            );
            if let Some(output) = output {
                api::save(&output, &merged, false)?;
                println!("merged {}", output.display());
            } else {
                print!("{}", merged.to_rbmem_string());
            }
        }
        Command::Migrate {
            file,
            output,
            dry_run,
        } => {
            let raw = fs::read_to_string(&file)?;
            let parsed = parse_document(&raw, TimestampPolicy::Preserve)?;
            let target = output.unwrap_or_else(|| file.clone());
            let source_version = parsed
                .document
                .meta
                .source_version
                .clone()
                .unwrap_or_else(|| parsed.document.meta.version.clone());
            if dry_run {
                println!(
                    "would migrate {} from RBMEM {} to RBMEM 1.4.0",
                    file.display(),
                    source_version
                );
                for warning in parsed.warnings {
                    println!("warning: {warning}");
                }
                print!("{}", parsed.document.to_rbmem_string());
            } else {
                write_document(&target, &parsed.document, false)?;
                println!(
                    "migrated {} -> {} from RBMEM {} to RBMEM 1.4.0",
                    file.display(),
                    target.display(),
                    source_version
                );
            }
        }
        Command::Review { file, dry_run } => {
            let text = fs::read_to_string(&file)?;
            let parsed = parse_document(&text, TimestampPolicy::Preserve)?;
            let warning_count = parsed.warnings.len();
            print!("{}", review_document(&parsed.document, parsed.warnings));
            if !dry_run && warning_count > 0 {
                return Err(RbmemError::Parse(format!(
                    "review found {} warning(s)",
                    warning_count
                )));
            }
        }
        Command::Doctor {
            file,
            stale_days,
            format,
        } => {
            print!("{}", doctor_report(file.as_deref(), stale_days, format)?);
        }
        Command::Pack {
            file,
            name,
            pack_file,
            resolve,
            compact,
            minified,
            format,
            max_tokens,
        } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            let config_path = pack_file.unwrap_or_else(|| pack::default_pack_file(&file));
            let config_text = fs::read_to_string(&config_path)?;
            let pack = pack::parse_pack_config(&config_text, &name)?;
            let effective_max_tokens = max_tokens.or(pack.max_tokens);
            let context = pack::pack_document(&document, &pack, resolve, effective_max_tokens);
            let compact = compact || (!minified && pack.mode == Some(CompactMode::Compact));
            let minified = minified || pack.mode == Some(CompactMode::Minified);
            print_context_output(
                ContextOutputRequest {
                    operation: "pack",
                    file: &file,
                    selector_name: "name",
                    selector_value: &name,
                    resolve,
                    compact,
                    minified,
                    graph_depth: pack.graph_depth,
                    format,
                },
                &document,
                &context,
            )?;
        }
        Command::Sync {
            markdown_folder,
            output_folder,
            watch,
            infer_relations,
            min_confidence,
            inference_strategy,
            dry_run,
        } => {
            let options = md_sync::SyncOptions::from_folder(
                &markdown_folder,
                infer_relations,
                min_confidence,
                inference_strategy,
                dry_run,
            )?;
            md_sync::sync_markdown_folder(&markdown_folder, &output_folder, &options)?;
            if watch {
                md_sync::watch_markdown_folder(markdown_folder, output_folder, options)?;
            }
        }
        Command::Serve { bind, dir } => {
            tokio::runtime::Runtime::new()
                .map_err(RbmemError::Io)?
                .block_on(rbmem::server::serve(&bind, dir))?;
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
                    serde_json::to_string_pretty(&hermes::hermes_json(
                        &document, resolve, compact, minified
                    )?)?
                );
            }
            HermesCommand::Save {
                file,
                json,
                json_file,
            } => {
                let now = Utc::now();
                let mut document = if file.exists() {
                    read_document(&file, TimestampPolicy::Preserve)?
                } else {
                    RbmemDocument::new(now, "hermes")
                };
                let payload = hermes::read_hermes_payload(json, json_file)?;
                hermes::apply_hermes_payload(&mut document, payload, now)?;
                write_document(&file, &document, false)?;
                hermes::validate_or_error(&document)?;
                println!("saved {}", file.display());
            }
            HermesCommand::Plan {
                file,
                goal,
                from_memory,
                pack,
                dry_run,
                format,
            } => {
                let search_dir = file.parent().map(Path::to_path_buf).unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                });
                let report = rbmem::plan_memory(PlanOptions {
                    goal,
                    from_memory,
                    file: Some(file),
                    search_dir,
                    context_pack: pack,
                    solver: SatBackend::Auto,
                    proof: false,
                    proof_path: None,
                    verify_proof: false,
                    cube_and_conquer: false,
                    dry_run,
                    now: Utc::now(),
                })?;
                match format {
                    OutputFormat::Text => print!("{}", render_plan_report(&report)),
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                }
            }
            HermesCommand::Init { project_name } => {
                let now = Utc::now();
                let document = hermes::hermes_starter_document(&project_name, now);
                let file =
                    PathBuf::from(format!("{}.rbmem", markdown::title_to_path(&project_name)));
                write_document(&file, &document, false)?;
                println!("created {}", file.display());
            }
            HermesCommand::Watch { file } => {
                hermes::watch_hermes_file(file)?;
            }
            HermesCommand::Doctor {
                file,
                stale_days,
                rbmem_cli,
                format,
            } => {
                print!(
                    "{}",
                    hermes::hermes_doctor_report(&file, rbmem_cli.as_deref(), stale_days, format)?
                );
            }
        },
        Command::Timeline { file } => {
            let document = read_document(&file, TimestampPolicy::Preserve)?;
            for section in document.timeline() {
                println!(
                    "{}  {}  {}",
                    section.temporal.created_at.to_rfc3339(),
                    section.path,
                    markdown::first_line(&section.content)
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

fn render_plan_report(report: &PlanReport) -> String {
    let mut output = String::new();
    output.push_str("rbmem SAT plan\n");
    output.push_str(&format!("goal: {}\n", report.goal));
    output.push_str(&format!("status: {:?}\n", report.status));
    output.push_str(&format!("solver: {}\n", report.solver));
    output.push_str(&format!("memory: {}\n", report.memory_file));
    output.push_str(&format!("stored: {}\n", report.plan_path));
    output.push_str(&format!(
        "cnf: {} variable(s), {} clause(s)\n",
        report.variables, report.clauses
    ));
    if report.dry_run {
        output.push_str("dry-run: plan was not written\n");
    }
    if report.proof.requested {
        output.push_str(&format!(
            "proof: {} ({}, verified: {})\n",
            report.proof.path.as_deref().unwrap_or("not written"),
            report.proof.verifier,
            report.proof.verified
        ));
    }

    match report.status {
        SatStatus::Sat => {
            output.push_str("\nsteps:\n");
            if report.steps.is_empty() {
                output.push_str("- no concrete steps selected\n");
            } else {
                for step in &report.steps {
                    if let Some(source) = &step.source {
                        output.push_str(&format!("{}. {} [{}]\n", step.order, step.action, source));
                    } else {
                        output.push_str(&format!("{}. {}\n", step.order, step.action));
                    }
                }
            }
        }
        SatStatus::Unsat => {
            output.push_str(
                "\nNo satisfying plan found. Review conflicting rules and constraints.\n",
            );
        }
    }

    output
}

fn print_context_document(document: &RbmemDocument, resolve: bool, compact: bool, minified: bool) {
    print!(
        "{}",
        render_context_document(document, resolve, compact, minified)
    );
}

fn render_context_document(
    document: &RbmemDocument,
    resolve: bool,
    compact: bool,
    minified: bool,
) -> String {
    if minified || (!compact && document.meta.compact_mode == CompactMode::Minified) {
        document.to_minified_string(resolve)
    } else if compact || document.meta.compact_mode == CompactMode::Compact {
        document.to_compact_string(resolve, Utc::now())
    } else {
        document.to_rbmem_string_hiding_empty_temporal()
    }
}

struct ContextOutputRequest<'a> {
    operation: &'a str,
    file: &'a Path,
    selector_name: &'a str,
    selector_value: &'a str,
    resolve: bool,
    compact: bool,
    minified: bool,
    graph_depth: usize,
    format: OutputFormat,
}

fn print_context_output(
    request: ContextOutputRequest<'_>,
    source: &RbmemDocument,
    context: &RbmemDocument,
) -> Result<(), RbmemError> {
    match request.format {
        OutputFormat::Text => {
            print_context_document(context, request.resolve, request.compact, request.minified)
        }
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&context_json(request, source, context))?
        ),
    }
    Ok(())
}

fn context_json(
    request: ContextOutputRequest<'_>,
    source: &RbmemDocument,
    context: &RbmemDocument,
) -> Value {
    json!({
        "schema": "rbmem.context.v1",
        "operation": request.operation,
        "file": request.file.display().to_string(),
        "selector": {
            "kind": request.selector_name,
            "value": request.selector_value,
        },
        "options": {
            "resolve": request.resolve,
            "compact": request.compact,
            "minified": request.minified,
            "graph_depth": request.graph_depth,
        },
        "source": {
            "meta": document_meta_json(source),
            "section_count": source.sections.len(),
        },
        "context": render_context_document(
            context,
            request.resolve,
            request.compact,
            request.minified,
        ),
        "context_meta": document_meta_json(context),
        "sections": sections_json(context, request.resolve),
        "graph": graph_view_to_json(&context.graph_view()),
    })
}

fn review_document(document: &RbmemDocument, mut warnings: Vec<String>) -> String {
    warnings.extend(document.validate());
    let mut output = String::new();

    if warnings.is_empty() {
        output.push_str("valid RBMEM v1.4.0\n");
    } else {
        for warning in warnings {
            output.push_str(&format!("warning: {warning}\n"));
        }
    }

    for section in &document.sections {
        if let Some(source) = &section.source {
            if matches!(source.kind.as_str(), "agent" | "hermes") {
                output.push_str(&format!(
                    "review source: {} from {}\n",
                    section.path, source.kind
                ));
            }
        }

        if let Some(graph) = &section.graph {
            for relation in &graph.relations {
                if relation.inferred {
                    output.push_str(&format!(
                        "review inferred edge: {} -> {} ({})\n",
                        section.path, relation.to, relation.relation_type
                    ));
                }
            }
        }
    }

    output
}

fn doctor_report(
    file: Option<&Path>,
    stale_days: u64,
    format: OutputFormat,
) -> Result<String, RbmemError> {
    match format {
        OutputFormat::Text => doctor_text_report(file, stale_days),
        OutputFormat::Json => Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&doctor_json(file, stale_days)?)?
        )),
    }
}

fn doctor_text_report(file: Option<&Path>, stale_days: u64) -> Result<String, RbmemError> {
    let mut output = String::new();
    output.push_str("rbmem doctor\n");
    output.push_str(&format!(
        "cli-version: rbmem {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    output.push_str("document-format: RBMEM v1.4.0\n");

    if let Some(file) = file {
        append_document_diagnostics(&mut output, file)?;

        // Health scoring with configurable stale threshold
        match api::health_report(file, stale_days) {
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
    } else {
        output.push_str("file: not provided\n");
    }

    Ok(output)
}

fn doctor_json(file: Option<&Path>, stale_days: u64) -> Result<Value, RbmemError> {
    let document = if let Some(file) = file {
        Some(document_diagnostics_json(file)?.0)
    } else {
        None
    };

    let health = if let Some(file) = file {
        api::health_report(file, stale_days).ok().map(|h| {
            json!({
                "score": h.score,
                "stale_days": stale_days,
                "total_sections": h.total_sections,
                "stale_sections": h.stale_sections,
                "orphaned_edges": h.orphaned_edges,
                "conflicts": h.conflicts,
            })
        })
    } else {
        None
    };

    Ok(json!({
        "schema": "rbmem.doctor.v1",
        "cli_version": format!("rbmem {}", env!("CARGO_PKG_VERSION")),
        "document_format": "RBMEM v1.4.0",
        "document": document,
        "health": health,
    }))
}

fn document_diagnostics_json(file: &Path) -> Result<(Value, RbmemDocument), RbmemError> {
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

fn append_document_diagnostics(
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

fn document_meta_json(document: &RbmemDocument) -> Value {
    json!({
        "version": document.meta.version,
        "source_version": document.meta.source_version,
        "purpose": document.meta.purpose,
        "compact_mode": document.meta.compact_mode.to_string(),
        "last_updated": document.meta.last_updated,
    })
}

fn sections_json(document: &RbmemDocument, resolve: bool) -> Vec<Value> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 27, 13, 10, 0).unwrap()
    }

    #[test]
    fn markdown_converter_preserves_heading_hierarchy() {
        let document = markdown::convert_markdown_to_rbmem(
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
        let document = markdown::convert_markdown_to_rbmem(
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
            vec!["inhibition.of.return.how.it.works.theoretical.mechanisms"]
        );
    }

    #[test]
    fn markdown_converter_preserves_frontmatter_as_meta_section() {
        let document = markdown::convert_markdown_to_rbmem(
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

        let options = md_sync::SyncOptions::from_folder(
            &markdown,
            false,
            0.6,
            InferenceStrategy::Balanced,
            false,
        )
        .unwrap();
        md_sync::sync_markdown_folder(&markdown, &output, &options).unwrap();

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
        let source = parsed.document.sections[0].source.as_ref().unwrap();
        assert_eq!(source.kind, "markdown");
        let expected_source_path = Path::new("concepts").join("agent.md").display().to_string();
        assert_eq!(source.path.as_deref(), Some(expected_source_path.as_str()));
        assert!(source
            .hash
            .as_ref()
            .is_some_and(|hash| hash.starts_with("sha256:")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_dry_run_does_not_write_outputs() {
        let root = temp_test_dir("sync-dry-run");
        let markdown = root.join("md");
        let output = root.join("out");
        fs::create_dir_all(&markdown).unwrap();
        fs::write(markdown.join("note.md"), "# Note\n\nBody.").unwrap();

        let options = md_sync::SyncOptions::from_folder(
            &markdown,
            false,
            0.6,
            InferenceStrategy::Balanced,
            true,
        )
        .unwrap();
        md_sync::sync_markdown_folder(&markdown, &output, &options).unwrap();

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

        let options = md_sync::SyncOptions::from_folder(
            &markdown,
            false,
            0.6,
            InferenceStrategy::Balanced,
            false,
        )
        .unwrap();
        md_sync::sync_markdown_folder(&markdown, &output, &options).unwrap();
        let generated = output.join("note.rbmem");
        let first_modified = fs::metadata(&generated).unwrap().modified().unwrap();
        md_sync::sync_markdown_folder(&markdown, &output, &options).unwrap();
        let second_modified = fs::metadata(&generated).unwrap().modified().unwrap();

        assert_eq!(first_modified, second_modified);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hermes_starter_has_standard_sections() {
        let document = hermes::hermes_starter_document("Demo Project", fixed_time());
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
        let document = hermes::hermes_starter_document("Demo Project", fixed_time());
        let payload = hermes::hermes_json(&document, true, false, true).unwrap();

        assert_eq!(payload["schema"], "hermes.rbmem.v1");
        assert!(payload["sections"].as_array().unwrap().len() >= 6);
        assert!(payload["graph"]["nodes"].as_array().is_some());
        assert!(payload["timeline"].as_array().unwrap().len() == 1);
        assert!(payload["context"].as_str().unwrap().contains("[goals]"));
    }

    #[test]
    fn query_context_includes_matches_parents_and_graph_neighbors() {
        let now = fixed_time();
        let mut document = RbmemDocument::new(now, "me");
        document.upsert_section(
            "rules",
            SectionType::List,
            "- Preserve user intent.".to_string(),
            now,
        );
        document.upsert_section(
            "rules.review",
            SectionType::Text,
            "Review pull requests with tests in mind.".to_string(),
            now,
        );
        document.upsert_section(
            "memory.testing",
            SectionType::Text,
            "Run focused Rust checks before handing work back.".to_string(),
            now,
        );

        let review = document
            .sections
            .iter_mut()
            .find(|section| section.path == "rules.review")
            .unwrap();
        review.graph = Some(GraphInfo {
            node_type: None,
            relations: vec![GraphRelation {
                to: "memory.testing".to_string(),
                relation_type: "uses".to_string(),
                valid_from: Some(now),
                valid_until: None,
                inferred: false,
                confidence: None,
            }],
        });

        let context = api::query_document(&document, "pull requests", true, 1);
        let paths = context
            .sections
            .iter()
            .map(|section| section.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["memory.testing", "rules", "rules.review"]);
        assert!(context
            .to_minified_string(true)
            .contains("Preserve user intent"));
    }

    #[test]
    fn diff_reports_added_and_changed_sections() {
        let now = fixed_time();
        let mut before = RbmemDocument::new(now, "me");
        before.upsert_section("rules", SectionType::Text, "old".to_string(), now);

        let mut after = before.clone();
        after.upsert_section("rules", SectionType::Text, "new".to_string(), now);
        after.upsert_section("memory", SectionType::Text, "added".to_string(), now);

        let diff = api::diff_documents(&before, &after);

        assert!(diff.contains("added: memory"));
        assert!(diff.contains("changed content: rules"));
    }

    #[test]
    fn review_flags_hermes_sources_and_inferred_edges() {
        let now = fixed_time();
        let mut document = RbmemDocument::new(now, "me");
        document.upsert_section(
            "memory",
            SectionType::HermesMemory,
            "- fact".to_string(),
            now,
        );
        let memory = document
            .sections
            .iter_mut()
            .find(|section| section.path == "memory")
            .unwrap();
        memory.source = Some(SourceInfo {
            kind: "hermes".to_string(),
            path: None,
            actor: Some("hermes".to_string()),
            hash: None,
        });
        memory.graph = Some(GraphInfo {
            node_type: None,
            relations: vec![GraphRelation {
                to: "rules".to_string(),
                relation_type: "references".to_string(),
                valid_from: Some(now),
                valid_until: None,
                inferred: true,
                confidence: Some(0.72),
            }],
        });

        let review = review_document(&document, Vec::new());

        assert!(review.contains("review source: memory from hermes"));
        assert!(review.contains("review inferred edge: memory -> rules"));
    }

    #[test]
    fn doctor_report_summarizes_file_health() {
        let root = temp_test_dir("doctor");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("memory.rbmem");
        let document = hermes::hermes_starter_document("Demo Project", fixed_time());
        fs::write(&file, document.to_rbmem_string()).unwrap();

        let report = doctor_report(Some(&file), 30, OutputFormat::Text).unwrap();

        assert!(report.contains("rbmem doctor"));
        assert!(report.contains("cli-version: rbmem"));
        assert!(report.contains("document-format: RBMEM v1.4.0"));
        assert!(report.contains("file-exists: ok"));
        assert!(report.contains("parse: ok"));
        assert!(report.contains("validation: ok"));
        assert!(report.contains("health-score:"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hermes_doctor_report_checks_loadable_context() {
        let root = temp_test_dir("hermes-doctor");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("memory.rbmem");
        let document = hermes::hermes_starter_document("Demo Project", fixed_time());
        fs::write(&file, document.to_rbmem_string()).unwrap();

        let report = hermes::hermes_doctor_report(&file, None, 30, OutputFormat::Text).unwrap();

        assert!(report.contains("rbmem hermes doctor"));
        assert!(report.contains("parse: ok"));
        assert!(report.contains("validation: ok"));
        assert!(report.contains("hermes-load: ok"));
        assert!(report.contains("hermes-context-bytes:"));
        assert!(report.contains("health-score:"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn doctor_json_reports_machine_readable_health() {
        let root = temp_test_dir("doctor-json");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("memory.rbmem");
        let document = hermes::hermes_starter_document("Demo Project", fixed_time());
        fs::write(&file, document.to_rbmem_string()).unwrap();

        let report = doctor_report(Some(&file), 30, OutputFormat::Json).unwrap();
        let value: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(value["schema"], "rbmem.doctor.v1");
        assert_eq!(value["document"]["parse"], "ok");
        assert_eq!(value["document"]["validation"]["status"], "ok");
        assert!(value["health"].as_object().is_some());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_json_contains_selected_sections() {
        let now = fixed_time();
        let mut document = RbmemDocument::new(now, "me");
        document.upsert_section("rules", SectionType::List, "- Base rule".to_string(), now);
        document.upsert_section(
            "rules.review",
            SectionType::Text,
            "Review pull requests.".to_string(),
            now,
        );

        let context = api::query_document(&document, "pull requests", true, 0);
        let value = context_json(
            ContextOutputRequest {
                operation: "query",
                file: Path::new("memory.rbmem"),
                selector_name: "text",
                selector_value: "pull requests",
                resolve: true,
                compact: false,
                minified: true,
                graph_depth: 0,
                format: OutputFormat::Json,
            },
            &document,
            &context,
        );

        assert_eq!(value["schema"], "rbmem.context.v1");
        assert_eq!(value["operation"], "query");
        assert_eq!(value["sections"].as_array().unwrap().len(), 2);
        assert!(value["context"]
            .as_str()
            .unwrap()
            .contains("[rules.review]"));
    }

    #[test]
    fn pack_config_selects_includes_query_and_mode() {
        let now = fixed_time();
        let mut document = RbmemDocument::new(now, "me");
        document.upsert_section("rules", SectionType::List, "- Base rule".to_string(), now);
        document.upsert_section(
            "rules.review",
            SectionType::Text,
            "Review pull requests.".to_string(),
            now,
        );
        document.upsert_section(
            "memory.testing",
            SectionType::Text,
            "Run Rust tests.".to_string(),
            now,
        );

        let pack = pack::parse_pack_config(
            r#"[pack: code_review]
include:
  - rules
query: "Rust tests"
graph_depth: 0
mode: minified

[pack: other]
include:
  - memory
"#,
            "code_review",
        )
        .unwrap();
        let context = pack::pack_document(&document, &pack, true, None);
        let paths = context
            .sections
            .iter()
            .map(|section| section.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["memory.testing", "rules", "rules.review"]);
        assert_eq!(pack.mode, Some(CompactMode::Minified));
    }

    #[test]
    fn hermes_save_payload_appends_hermes_memory() {
        let now = fixed_time();
        let mut document = hermes::hermes_starter_document("Demo Project", now);
        let payload = hermes::read_hermes_payload(
            Some(
                r#"{
              "sections": [
                {
                  "path": "memory",
                  "type": "hermes:memory",
                  "content": "- User prefers compact context.",
                  "mode": "auto"
                }
              ]
            }"#
                .to_string(),
            ),
            None,
        )
        .unwrap();

        hermes::apply_hermes_payload(&mut document, payload, now).unwrap();
        hermes::apply_hermes_payload(
            &mut document,
            hermes::read_hermes_payload(
                Some(r#"{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers compact context."}]}"#.to_string()),
                None,
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

    #[test]
    fn hermes_save_rejects_replace_for_append_only_memory() {
        let now = fixed_time();
        let mut document = hermes::hermes_starter_document("Demo Project", now);
        let payload = hermes::read_hermes_payload(
            Some(
                r#"{"sections":[{"path":"memory","type":"hermes:memory","content":"replacement","mode":"replace"}]}"#
                    .to_string(),
            ),
            None,
        )
        .unwrap();

        let error = hermes::apply_hermes_payload(&mut document, payload, now).unwrap_err();

        assert!(error.to_string().contains("append-only"));
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rbmem-{name}-{suffix}"))
    }
}
