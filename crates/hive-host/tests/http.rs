//! In-process HTTP integration tests for hive-host router.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use hive_common::HiveHome;
use hive_host::http;
use tower::ServiceExt;

fn temp_home() -> HiveHome {
    let dir = std::env::temp_dir().join(format!(
        "hive-http-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(dir.join("sessions")).unwrap();
    HiveHome { root: dir }
}

#[tokio::test]
async fn health_returns_ok() {
    let home = temp_home();
    let app = http::router(home);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["role"], "host");
}

#[tokio::test]
async fn sessions_empty_list() {
    let home = temp_home();
    let app = http::router(home);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn attach_rejects_bad_task() {
    let home = temp_home();
    let app = http::router(home);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sessions/..%2Fetc/attach")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["code"], "bad_task");
}

#[tokio::test]
async fn spawn_rejects_bad_task() {
    let home = temp_home();
    let dir = home.sessions_dir().display().to_string();
    let app = http::router(home);
    let payload = serde_json::json!({
        "task": "../evil",
        "dir": dir
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sessions")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["code"], "spawn_failed");
}

fn write_codex_demo(home: &HiveHome, bound: &str, live: &str) -> std::path::PathBuf {
    let index = home.root.join("session_index.jsonl");
    std::fs::write(
        &index,
        format!(
            r#"{{"id":"{bound}","thread_name":"demo","updated_at":"2026-08-15T19:59:16Z"}}
{{"id":"{live}","thread_name":"demo","updated_at":"2026-08-15T20:24:08Z"}}
"#
        ),
    )
    .unwrap();
    hive_host::meta::write_meta(
        &home.sessions_dir().join("demo.meta"),
        &[
            ("task", "demo"),
            ("dir", "/tmp/x"),
            ("tmux", "hive-demo-missing"),
            ("session_id", bound),
            ("provider", "codex"),
        ],
    )
    .unwrap();
    index
}

#[tokio::test]
async fn bindings_lists_codex_candidates() {
    let home = temp_home();
    let bound = "01a00701-caec-7052-8ccf-376b4002b8e3";
    let live = "019fe737-74ae-7e53-a253-f5f96cb782ff";
    let index = write_codex_demo(&home, bound, live);
    std::env::set_var("CODEX_SESSION_INDEX", &index);
    let app = http::router(home);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sessions/demo/bindings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    std::env::remove_var("CODEX_SESSION_INDEX");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["bound"], bound);
    assert_eq!(v["suggested"], live);
    assert_eq!(v["candidates"].as_array().map(|a| a.len()), Some(2));
}

#[tokio::test]
async fn bind_updates_session_id() {
    let home = temp_home();
    let bound = "01a00701-caec-7052-8ccf-376b4002b8e3";
    let live = "019fe737-74ae-7e53-a253-f5f96cb782ff";
    let index = write_codex_demo(&home, bound, live);
    std::env::set_var("CODEX_SESSION_INDEX", &index);
    let app = http::router(home.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sessions/demo/bind")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"session_id":"{live}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    std::env::remove_var("CODEX_SESSION_INDEX");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["bound"], live);
    let map = hive_host::sessions::load_task_meta(&home, "demo").unwrap();
    assert_eq!(map.get("session_id").map(String::as_str), Some(live));
}
