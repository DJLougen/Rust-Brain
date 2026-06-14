use crate::{
    context_json, export_graph, merge_documents, query_document, render_context_document,
    ContextOutputRequest, DiffFormat, ExportFormat, MergeStrategy, OutputFormat, RbmemDocument,
    RbmemError, SectionType, TimestampPolicy,
};
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct AppState {
    pub dir: PathBuf,
    memories: Arc<RwLock<HashMap<String, RbmemDocument>>>,
}

#[derive(Debug, Deserialize)]
pub struct MemoryCreateRequest {
    pub name: String,
    pub created_by: Option<String>,
    pub purpose: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SectionUpsertRequest {
    #[serde(default = "default_text_type")]
    pub section_type: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub text: String,
    #[serde(default)]
    pub resolve: bool,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub minified: bool,
    #[serde(default)]
    pub graph_depth: usize,
}

#[derive(Debug, Deserialize)]
pub struct DiffRequest {
    pub other: String,
    #[serde(default = "default_diff_format")]
    pub format: DiffFormat,
}

#[derive(Debug, Deserialize)]
pub struct MergeRequest {
    pub base: String,
    pub remote: String,
    #[serde(default = "default_merge_strategy")]
    pub strategy: MergeStrategy,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub format: ExportFormat,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

impl AppState {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            memories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Resolve a memory name to a path inside `self.dir`, rejecting any name
    /// that is not a single safe filename. This blocks path traversal: `../`,
    /// absolute paths, and drive/UNC prefixes cannot escape the configured
    /// directory.
    fn memory_path(&self, name: &str) -> Result<PathBuf, RbmemError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(RbmemError::Parse("memory name must not be empty".into()));
        }
        // The name must resolve to exactly one normal path component.
        let mut components = Path::new(trimmed).components();
        let single_normal =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if !single_normal {
            return Err(RbmemError::Parse(format!(
                "invalid memory name '{name}': must be a single filename without path separators"
            )));
        }
        let filename = if trimmed.ends_with(".rbmem") {
            trimmed.to_string()
        } else {
            format!("{trimmed}.rbmem")
        };
        Ok(self.dir.join(filename))
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/memories", post(create_memory))
        .route(
            "/memories/:name",
            get(get_memory).put(put_memory).delete(delete_memory),
        )
        .route(
            "/memories/:name/sections/:path",
            get(get_section).put(put_section).delete(delete_section),
        )
        .route("/memories/:name/query", post(query_memory))
        .route("/memories/:name/context", post(context_memory))
        .route("/memories/:name/diff", post(diff_memory))
        .route("/memories/:name/merge", post(merge_memory))
        .route("/memories/:name/export", post(export_memory))
        .with_state(state)
}

pub async fn serve(bind: &str, dir: PathBuf) -> Result<(), RbmemError> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app(AppState::new(dir)))
        .await
        .map_err(|error| RbmemError::Io(std::io::Error::other(error)))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn create_memory(
    State(state): State<AppState>,
    Json(request): Json<MemoryCreateRequest>,
) -> Result<Json<RbmemDocument>, (StatusCode, Json<ErrorResponse>)> {
    let now = Utc::now();
    let mut document = RbmemDocument::new(now, request.created_by.unwrap_or_else(|| "api".into()));
    if let Some(purpose) = request.purpose {
        document.meta.purpose = purpose;
    }
    let path = state.memory_path(&request.name).map_err(server_error)?;
    crate::save(path, &document, false).map_err(server_error)?;
    state
        .memories
        .write()
        .await
        .insert(request.name, document.clone());
    Ok(Json(document))
}

async fn get_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<RbmemDocument>, (StatusCode, Json<ErrorResponse>)> {
    Ok(Json(load_memory(&state, &name).await?))
}

async fn put_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(document): Json<RbmemDocument>,
) -> Result<Json<RbmemDocument>, (StatusCode, Json<ErrorResponse>)> {
    let path = state.memory_path(&name).map_err(server_error)?;
    crate::save(path, &document, false).map_err(server_error)?;
    state.memories.write().await.insert(name, document.clone());
    Ok(Json(document))
}

async fn delete_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.memories.write().await.remove(&name);
    let path = state.memory_path(&name).map_err(server_error)?;
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(RbmemError::from)
            .map_err(server_error)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn get_section(
    State(state): State<AppState>,
    AxumPath((name, path)): AxumPath<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let document = load_memory(&state, &name).await?;
    let section = document
        .sections
        .iter()
        .find(|section| section.path == path)
        .ok_or_else(|| not_found(format!("section '{path}' not found")))?;
    Ok(Json(json!(section)))
}

async fn put_section(
    State(state): State<AppState>,
    AxumPath((name, path)): AxumPath<(String, String)>,
    Json(request): Json<SectionUpsertRequest>,
) -> Result<Json<RbmemDocument>, (StatusCode, Json<ErrorResponse>)> {
    let now = Utc::now();
    let mut document = load_memory(&state, &name).await?;
    let section_type = request
        .section_type
        .parse::<SectionType>()
        .map_err(server_error)?;
    document.upsert_section(&path, section_type, request.content, now);
    let file_path = state.memory_path(&name).map_err(server_error)?;
    crate::save(file_path, &document, false).map_err(server_error)?;
    state.memories.write().await.insert(name, document.clone());
    Ok(Json(document))
}

async fn delete_section(
    State(state): State<AppState>,
    AxumPath((name, path)): AxumPath<(String, String)>,
) -> Result<Json<RbmemDocument>, (StatusCode, Json<ErrorResponse>)> {
    let mut document = load_memory(&state, &name).await?;
    document.sections.retain(|section| section.path != path);
    let file_path = state.memory_path(&name).map_err(server_error)?;
    crate::save(file_path, &document, false).map_err(server_error)?;
    state.memories.write().await.insert(name, document.clone());
    Ok(Json(document))
}

async fn query_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let document = load_memory(&state, &name).await?;
    let context = query_document(
        &document,
        &request.text,
        request.resolve,
        request.graph_depth,
    );
    let file = state.memory_path(&name).map_err(server_error)?;
    Ok(Json(context_json(
        ContextOutputRequest {
            operation: "query".into(),
            file: file.display().to_string(),
            selector_name: "text".into(),
            selector_value: request.text,
            resolve: request.resolve,
            compact: request.compact,
            minified: request.minified,
            graph_depth: request.graph_depth,
            format: OutputFormat::Json,
        },
        &document,
        &context,
    )))
}

async fn context_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let document = load_memory(&state, &name).await?;
    let context = query_document(
        &document,
        &request.text,
        request.resolve,
        request.graph_depth,
    );
    Ok(Json(json!({
        "context": render_context_document(&context, request.resolve, request.compact, request.minified)
    })))
}

async fn diff_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(request): Json<DiffRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let document = load_memory(&state, &name).await?;
    let other = load_memory(&state, &request.other).await?;
    Ok(Json(
        json!({ "diff": crate::diff_with_format(&document, &other, request.format).map_err(server_error)? }),
    ))
}

async fn merge_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(request): Json<MergeRequest>,
) -> Result<Json<RbmemDocument>, (StatusCode, Json<ErrorResponse>)> {
    let local = load_memory(&state, &name).await?;
    let base = load_memory(&state, &request.base).await?;
    let remote = load_memory(&state, &request.remote).await?;
    Ok(Json(merge_documents(
        &base,
        &local,
        &remote,
        request.strategy,
        Utc::now(),
    )))
}

async fn export_memory(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(request): Json<ExportRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let document = load_memory(&state, &name).await?;
    Ok(Json(
        json!({ "export": export_graph(&document, request.format).map_err(server_error)? }),
    ))
}

async fn load_memory(
    state: &AppState,
    name: &str,
) -> Result<RbmemDocument, (StatusCode, Json<ErrorResponse>)> {
    if let Some(document) = state.memories.read().await.get(name).cloned() {
        return Ok(document);
    }
    let path = state.memory_path(name).map_err(server_error)?;
    let document = crate::load(path, TimestampPolicy::Preserve).map_err(server_error)?;
    state
        .memories
        .write()
        .await
        .insert(name.to_string(), document.clone());
    Ok(document)
}

fn default_text_type() -> String {
    "text".to_string()
}

fn default_diff_format() -> DiffFormat {
    DiffFormat::Text
}

fn default_merge_strategy() -> MergeStrategy {
    MergeStrategy::Manual
}

fn server_error(error: RbmemError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn not_found(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse { error: message }),
    )
}
