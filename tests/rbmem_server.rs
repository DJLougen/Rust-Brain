use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use rbmem::server::{app, AppState};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tower::ServiceExt;

#[tokio::test]
async fn server_health_endpoint_reports_ok() {
    let response = app(AppState::new(PathBuf::from(".")))
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["status"], "ok");
}

fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rbmem-server-{name}-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn create_memory_rejects_parent_traversal() {
    let dir = temp_dir("create-parent");
    let escaped = dir.parent().unwrap().join("rbmem-escaped-create.rbmem");
    let _ = fs::remove_file(&escaped);

    let response = app(AppState::new(dir.clone()))
        .oneshot(json_request(
            "POST",
            "/memories",
            json!({ "name": "../rbmem-escaped-create" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!escaped.exists(), "traversal name escaped the memory dir");
}

#[tokio::test]
async fn create_memory_rejects_absolute_and_separator_names() {
    for bad in ["/tmp/rbmem-abs", "sub/dir/mem"] {
        let dir = temp_dir("create-bad");
        let response = app(AppState::new(dir))
            .oneshot(json_request("POST", "/memories", json!({ "name": bad })))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "name '{bad}' should be rejected"
        );
    }
}

#[tokio::test]
async fn diff_memory_rejects_traversal_in_other() {
    let dir = temp_dir("diff-other");
    let service = app(AppState::new(dir));

    let created = service
        .clone()
        .oneshot(json_request("POST", "/memories", json!({ "name": "base" })))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);

    let response = service
        .oneshot(json_request(
            "POST",
            "/memories/base/diff",
            json!({ "other": "../../rbmem-escaped-diff" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn valid_memory_name_roundtrips() {
    let dir = temp_dir("valid-roundtrip");
    let service = app(AppState::new(dir.clone()));

    let created = service
        .clone()
        .oneshot(json_request(
            "POST",
            "/memories",
            json!({ "name": "notes" }),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    assert!(dir.join("notes.rbmem").exists());

    let fetched = service
        .oneshot(
            Request::builder()
                .uri("/memories/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
}
