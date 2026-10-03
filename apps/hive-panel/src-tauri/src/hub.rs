//! Thin panel wrapper: fleet cache + Tauri-friendly `String` errors.

use std::sync::{Arc, OnceLock};

use hive_client::Client;
use hive_common::{ClientToml, HiveHome};
use hive_protocol::{
    AttachCommand, BindingsDoc, DirListing, Fleet, SpawnRequest, TranscriptDoc,
};
use tokio::sync::RwLock;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigView {
    pub hive_home: String,
    pub hub: String,
    pub terminal: String,
}

static CLIENT: OnceLock<Arc<Client>> = OnceLock::new();
static FLEET_CACHE: OnceLock<RwLock<Option<Fleet>>> = OnceLock::new();

fn cache() -> &'static RwLock<Option<Fleet>> {
    FLEET_CACHE.get_or_init(|| RwLock::new(None))
}

fn client() -> Result<Arc<Client>, String> {
    if let Some(c) = CLIENT.get() {
        return Ok(c.clone());
    }
    let home = HiveHome::resolve(None).map_err(|e| e.to_string())?;
    let c = Arc::new(Client::new(&home).map_err(|e| e.to_string())?);
    let _ = CLIENT.set(c.clone());
    Ok(c)
}

pub fn load_client() -> ClientToml {
    HiveHome::resolve(None)
        .map(|home| ClientToml::load(&home))
        .unwrap_or_default()
}

pub fn config_view() -> ConfigView {
    let home = HiveHome::resolve(None)
        .map(|h| h.root.display().to_string())
        .unwrap_or_else(|_| "~/.hive".into());
    let c = load_client();
    ConfigView {
        hive_home: home,
        hub: if c.hub.is_empty() {
            "http://127.0.0.1:8788 (local host)".into()
        } else {
            c.hub
        },
        terminal: if c.terminal.is_empty() {
            "iterm2".into()
        } else {
            c.terminal
        },
    }
}

pub fn hub_events_url() -> Option<String> {
    client()
        .ok()
        .map(|c| c.endpoints().hub_events_url())
        .flatten()
}

pub async fn refresh_fleet() -> Result<Fleet, String> {
    let fleet = client()?.fetch_fleet().await.map_err(|e| e.to_string())?;
    *cache().write().await = Some(fleet.clone());
    Ok(fleet)
}

pub async fn cached_fleet() -> Option<Fleet> {
    cache().read().await.clone()
}

pub async fn fetch_fleet() -> Result<Fleet, String> {
    if let Some(cached) = cached_fleet().await {
        return Ok(cached);
    }
    refresh_fleet().await
}

pub async fn list_hosts() -> Result<Vec<String>, String> {
    let fleet = fetch_fleet().await?;
    Ok(fleet.hosts.into_iter().map(|h| h.host).collect())
}

pub async fn list_dirs(host: &str, path: &str) -> Result<DirListing, String> {
    client()?
        .fs(host, Some(path))
        .await
        .map_err(|e| e.to_string())
}

pub async fn spawn(host: &str, req: &SpawnRequest) -> Result<String, String> {
    let out = client()?
        .spawn(host, req)
        .await
        .map_err(|e| e.to_string())?;
    Ok(out.output)
}

pub async fn session_action(host: &str, task: &str, verb: &str) -> Result<String, String> {
    let out = client()?
        .session_action(host, task, verb)
        .await
        .map_err(|e| e.to_string())?;
    Ok(out.output)
}

pub async fn label(host: &str, task: &str, op: &str, tag: Option<String>) -> Result<String, String> {
    let out = client()?
        .label(host, task, op, tag.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(out.output)
}

pub async fn say(host: &str, task: &str, text: &str) -> Result<String, String> {
    let out = client()?
        .say(host, task, text)
        .await
        .map_err(|e| e.to_string())?;
    Ok(out.output)
}

pub async fn attach_argv(host: &str, task: &str) -> Result<AttachCommand, String> {
    client()?
        .attach(host, task)
        .await
        .map_err(|e| e.to_string())
}

pub async fn bindings(host: &str, task: &str) -> Result<BindingsDoc, String> {
    client()?
        .bindings(host, task)
        .await
        .map_err(|e| e.to_string())
}

pub async fn bind(host: &str, task: &str, session_id: &str) -> Result<BindingsDoc, String> {
    client()?
        .bind(host, task, session_id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn transcript(
    host: &str,
    task: &str,
    after: Option<&str>,
    tail: Option<u32>,
    until_offset: Option<u64>,
) -> Result<TranscriptDoc, String> {
    client()?
        .transcript(host, task, after, tail, until_offset)
        .await
        .map_err(|e| e.to_string())
}

pub fn transcript_stream_url(
    host: &str,
    task: &str,
    after: Option<&str>,
    tail: Option<u32>,
) -> Result<String, String> {
    Ok(client()?
        .endpoints()
        .transcript_stream_url(host, task, after, tail))
}
