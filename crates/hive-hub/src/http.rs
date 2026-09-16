use std::sync::Arc;

use anyhow::Result;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{any, get};
use axum::{Json, Router};
use futures_util::StreamExt;
use hive_common::{ensure_dir, HubConfig, HiveHome};
use hive_protocol::{Fleet, Health};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use crate::poll::poll_loop;
use crate::proxy::proxy;
use crate::state::HubState;

async fn health() -> Json<Health> {
    Json(Health::hub())
}

async fn fleet(State(st): State<Arc<HubState>>) -> Json<Fleet> {
    Json(st.fleet.read().await.clone())
}

async fn events(State(st): State<Arc<HubState>>) -> impl IntoResponse {
    let rx = st.events.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|msg| async move {
        let Ok(ev) = msg else {
            return None;
        };
        let data = serde_json::to_string(&ev).ok()?;
        Some(Ok::<_, std::convert::Infallible>(
            Event::default().event(&ev.kind).data(data),
        ))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub fn router(state: Arc<HubState>) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/fleet", get(fleet))
        .route("/v1/events", get(events))
        .route("/v1/hosts/{host}/{*rest}", any(proxy))
        .route("/v1/hosts/{host}", any(proxy))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve(bind: String, port: u16, home: HiveHome, cfg: HubConfig) -> Result<()> {
    ensure_dir(&home.root)?;
    let (tx, _) = broadcast::channel(64);
    let state = HubState::new(
        home,
        HubConfig {
            bind: bind.clone(),
            port,
            ..cfg
        },
        tx,
    );
    tokio::spawn(poll_loop(state.clone()));
    let addr = format!("{bind}:{port}");
    let listener = TcpListener::bind(&addr).await?;
    eprintln!("hive-hub listening on http://{addr}");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
