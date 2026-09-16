use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use hive_protocol::{
    AttachCommand, BindRequest, ErrorBody, Health, LabelRequest, OkOutput, SayRequest,
    SpawnRequest, TranscriptCursor, TranscriptDoc,
};
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::discover::{self, TranscriptCursorQuery};
use hive_common::{ensure_dir, validate_task_name, HiveHome};
use crate::{bind, fs, labels, run, say, sessions, tmux};

#[derive(Clone)]
struct App {
    home: Arc<HiveHome>,
}

fn err(code: StatusCode, key: &str, msg: impl Into<String>) -> impl IntoResponse {
    (code, Json(ErrorBody::new(key, msg)))
}

fn parse_task(task: String) -> Result<String, (StatusCode, Json<ErrorBody>)> {
    validate_task_name(&task)
        .map(|t| t.to_string())
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorBody::new("bad_task", e))))
}

async fn health() -> Json<Health> {
    Json(Health::host())
}

async fn list_sessions(State(app): State<App>) -> impl IntoResponse {
    Json(sessions::list_sessions(&app.home))
}

async fn spawn_session(
    State(app): State<App>,
    Json(req): Json<SpawnRequest>,
) -> impl IntoResponse {
    match run::spawn(&app.home, &req) {
        Ok(out) => (StatusCode::OK, Json(OkOutput::new(out))).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, "spawn_failed", e).into_response(),
    }
}

#[derive(Deserialize, Default)]
struct TranscriptQuery {
    after: Option<String>,
    tail: Option<u32>,
    until_offset: Option<u64>,
}

fn parse_after(raw: Option<&str>) -> Option<TranscriptCursorQuery> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    let cur: TranscriptCursor = serde_json::from_str(s).ok()?;
    Some(TranscriptCursorQuery {
        session_id: cur.session_id,
        offset: cur.offset,
    })
}

const DEFAULT_STREAM_TAIL: u32 = 150;

struct TranscriptStreamState {
    after: Option<TranscriptCursorQuery>,
    initial_after: Option<TranscriptCursorQuery>,
    initial_tail: Option<u32>,
    bootstrapped: bool,
    last_blocked: bool,
    last_cursor_key: String,
}

impl TranscriptStreamState {
    fn new(initial_after: Option<TranscriptCursorQuery>, initial_tail: Option<u32>) -> Self {
        Self {
            after: initial_after.clone(),
            initial_after,
            initial_tail,
            bootstrapped: false,
            last_blocked: false,
            last_cursor_key: String::new(),
        }
    }
}

fn cursor_key(doc: &TranscriptDoc) -> String {
    format!(
        "{}:{}",
        doc.cursor.session_id.as_deref().unwrap_or(""),
        doc.cursor.offset
    )
}

/// Whether an SSE tick should be emitted for this document vs prior stream state.
pub fn transcript_tick_changed(
    bootstrapped: bool,
    last_blocked: bool,
    last_cursor_key: &str,
    doc: &TranscriptDoc,
) -> bool {
    if doc.reset {
        return true;
    }
    if !bootstrapped {
        return !doc.blocks.is_empty() || doc.has_earlier || doc.blocked_on_prompt;
    }
    if doc.blocked_on_prompt != last_blocked {
        return true;
    }
    if !doc.blocks.is_empty() {
        return true;
    }
    cursor_key(doc) != last_cursor_key
}

fn apply_stream_tick(state: &mut TranscriptStreamState, doc: &TranscriptDoc) {
    state.last_blocked = doc.blocked_on_prompt;
    state.last_cursor_key = cursor_key(doc);
    state.after = Some(TranscriptCursorQuery {
        session_id: doc.session_id.clone(),
        offset: doc.cursor.offset,
    });
    state.bootstrapped = true;
}

async fn transcript(
    State(app): State<App>,
    Path(task): Path<String>,
    Query(q): Query<TranscriptQuery>,
) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    let meta = match sessions::load_task_meta(&app.home, &task) {
        Ok(m) => m,
        Err(e) => return err(StatusCode::NOT_FOUND, "not_found", e).into_response(),
    };
    let after = parse_after(q.after.as_deref());
    match discover::transcript_document(&meta, after.as_ref(), q.tail, q.until_offset, true) {
        Ok(doc) => Json(doc).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, "transcript", e.0).into_response(),
    }
}

async fn transcript_stream(
    State(app): State<App>,
    Path(task): Path<String>,
    Query(q): Query<TranscriptQuery>,
) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    let initial_after = parse_after(q.after.as_deref());
    let initial_tail = if initial_after.is_some() {
        q.tail
    } else {
        Some(q.tail.unwrap_or(DEFAULT_STREAM_TAIL))
    };
    let home = app.home.clone();
    let shared = Arc::new(Mutex::new(TranscriptStreamState::new(
        initial_after,
        initial_tail,
    )));
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
        Duration::from_secs(2),
    ))
    .filter_map(move |_| {
        let home = home.clone();
        let task = task.clone();
        let shared = shared.clone();
        async move {
            let meta = sessions::load_task_meta(&home, &task).ok()?;
            let mut state = shared.lock().ok()?;
            let (after, tail) = if !state.bootstrapped && state.after.is_none() {
                (None, state.initial_tail)
            } else if !state.bootstrapped {
                (state.initial_after.clone(), None)
            } else {
                (state.after.clone(), None)
            };
            let doc = discover::transcript_document(&meta, after.as_ref(), tail, None, true).ok()?;
            if !transcript_tick_changed(
                state.bootstrapped,
                state.last_blocked,
                &state.last_cursor_key,
                &doc,
            ) {
                return None;
            }
            apply_stream_tick(&mut state, &doc);
            let data = serde_json::to_string(&doc).ok()?;
            Some(Ok::<_, std::convert::Infallible>(
                Event::default().event("tick").data(data),
            ))
        }
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn say_handler(
    State(app): State<App>,
    Path(task): Path<String>,
    Json(req): Json<SayRequest>,
) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    match say::say(&app.home, &task, &req.text) {
        Ok(out) => Json(OkOutput::new(out)).into_response(),
        Err(e) => err(StatusCode::CONFLICT, "say_failed", e).into_response(),
    }
}

async fn stop_handler(State(app): State<App>, Path(task): Path<String>) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    match run::stop(&app.home, &task) {
        Ok(out) => Json(OkOutput::new(out)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, "stop_failed", e).into_response(),
    }
}

async fn restart_handler(State(app): State<App>, Path(task): Path<String>) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    match run::restart(&app.home, &task) {
        Ok(out) => Json(OkOutput::new(out)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, "restart_failed", e).into_response(),
    }
}

async fn kill_handler(State(app): State<App>, Path(task): Path<String>) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    match run::kill(&app.home, &task) {
        Ok(out) => Json(OkOutput::new(out)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, "kill_failed", e).into_response(),
    }
}

async fn label_handler(
    State(app): State<App>,
    Path(task): Path<String>,
    Json(req): Json<LabelRequest>,
) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    if sessions::meta_path(&app.home, &task).is_none() {
        return err(StatusCode::NOT_FOUND, "not_found", format!("no session {task}"))
            .into_response();
    }
    let starred = match req.op.as_str() {
        "star" => true,
        "unstar" => false,
        _ => {
            return err(StatusCode::BAD_REQUEST, "bad_op", "op must be star or unstar")
                .into_response()
        }
    };
    match labels::set_starred(&sessions::labels_path(&app.home, &task), starred) {
        Ok(l) => Json(OkOutput::new(if l.starred { "starred" } else { "unstarred" }))
            .into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, "label_failed", e).into_response(),
    }
}

#[derive(Deserialize)]
struct FsQuery {
    path: Option<String>,
}

async fn fs_handler(Query(q): Query<FsQuery>) -> impl IntoResponse {
    match fs::list_dir(q.path.as_deref().unwrap_or("")) {
        Ok(listing) => Json(listing).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, "fs", e).into_response(),
    }
}

async fn bindings_handler(State(app): State<App>, Path(task): Path<String>) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    match bind::list_bindings(&app.home, &task) {
        Ok(doc) => Json(doc).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, "not_found", e).into_response(),
    }
}

async fn bind_handler(
    State(app): State<App>,
    Path(task): Path<String>,
    Json(req): Json<BindRequest>,
) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    match bind::bind_session(&app.home, &task, &req.session_id) {
        Ok(doc) => Json(doc).into_response(),
        Err(e) => {
            let code = if e.starts_with("unknown session_id") || e.contains("required") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            let key = if code == StatusCode::BAD_REQUEST {
                "bind_failed"
            } else {
                "not_found"
            };
            err(code, key, e).into_response()
        }
    }
}

async fn attach_handler(State(app): State<App>, Path(task): Path<String>) -> impl IntoResponse {
    let task = match parse_task(task) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    let meta = match sessions::load_task_meta(&app.home, &task) {
        Ok(m) => m,
        Err(e) => return err(StatusCode::NOT_FOUND, "not_found", e).into_response(),
    };
    let tmux_name = tmux::resolve_tmux(&task, meta.get("tmux").map(|s| s.as_str()));
    let host = hostname();
    Json(AttachCommand::ssh_tmux(&host, &task, &tmux_name)).into_response()
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "localhost".into())
        })
}

pub fn router(home: HiveHome) -> Router {
    let app = App {
        home: Arc::new(home),
    };
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/sessions", get(list_sessions).post(spawn_session))
        .route("/v1/sessions/{task}/transcript", get(transcript))
        .route(
            "/v1/sessions/{task}/transcript/stream",
            get(transcript_stream),
        )
        .route("/v1/sessions/{task}/say", post(say_handler))
        .route("/v1/sessions/{task}/stop", post(stop_handler))
        .route("/v1/sessions/{task}/restart", post(restart_handler))
        .route("/v1/sessions/{task}/kill", post(kill_handler))
        .route("/v1/sessions/{task}/label", post(label_handler))
        .route("/v1/sessions/{task}/bindings", get(bindings_handler))
        .route("/v1/sessions/{task}/bind", post(bind_handler))
        .route("/v1/sessions/{task}/attach", get(attach_handler))
        .route("/v1/fs", get(fs_handler))
        .layer(CorsLayer::permissive())
        .with_state(app)
}

pub async fn serve(home: HiveHome, bind: String, port: u16) -> anyhow::Result<()> {
    ensure_dir(&home.sessions_dir())?;
    let addr = format!("{bind}:{port}");
    let listener = TcpListener::bind(&addr).await?;
    eprintln!("hive-host listening on http://{addr}");
    axum::serve(listener, router(home)).await?;
    Ok(())
}
