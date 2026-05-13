use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use rbmem::server::{app, AppState};
use serde_json::Value;
use std::path::PathBuf;
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
