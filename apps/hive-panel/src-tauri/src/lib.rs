mod hub;
mod sse;
mod terminal;
mod transcript_stream;

use hive_protocol::{BindingsDoc, Fleet, HubEvent, SpawnRequest, TranscriptDoc};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Listener};
use terminal::TerminalMacos;

#[tauri::command]
fn close_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_config() -> hub::ConfigView {
    hub::config_view()
}

#[tauri::command]
async fn fetch_fleet_cmd() -> Result<Fleet, String> {
    hub::fetch_fleet().await
}

#[tauri::command]
async fn refresh_fleet_cmd() -> Result<Fleet, String> {
    hub::refresh_fleet().await
}

#[tauri::command]
async fn list_hosts_cmd() -> Result<Vec<String>, String> {
    hub::list_hosts().await
}

#[tauri::command]
async fn list_remote_dirs_cmd(host: String, path: String) -> Result<hive_protocol::DirListing, String> {
    hub::list_dirs(&host, &path).await
}

#[derive(Debug, Deserialize)]
struct SpawnArgs {
    host: String,
    task: String,
    dir: String,
    provider: Option<String>,
    auto: bool,
    #[serde(default, alias = "extraArgs")]
    extra_args: String,
}

#[tauri::command]
async fn spawn_session_cmd(args: SpawnArgs) -> Result<String, String> {
    let extra: Vec<String> = args
        .extra_args
        .split_whitespace()
        .map(str::to_string)
        .collect();
    hub::spawn(
        &args.host,
        &SpawnRequest {
            task: args.task,
            dir: Some(args.dir),
            provider: args.provider,
            extra_args: extra,
            auto: args.auto,
            resume: false,
        },
    )
    .await
}

#[tauri::command]
async fn kill_session(host: String, task: String) -> Result<String, String> {
    hub::session_action(&host, &task, "kill").await
}

#[tauri::command]
async fn stop_session(host: String, task: String) -> Result<String, String> {
    hub::session_action(&host, &task, "stop").await
}

#[tauri::command]
async fn restart_session(host: String, task: String) -> Result<String, String> {
    hub::session_action(&host, &task, "restart").await
}

#[tauri::command]
async fn label_session(
    host: String,
    task: String,
    op: String,
    tag: Option<String>,
) -> Result<String, String> {
    hub::label(&host, &task, &op, tag).await
}

#[tauri::command]
async fn open_shell_at_session(host: String, task: String, dir: String) -> Result<String, String> {
    let term = TerminalMacos::parse(&hub::load_client().terminal);
    let label = format!("{host}/{task} shell");
    terminal::open_shell_at(&host, &dir, &label, term).map_err(|e| e.to_string())?;
    Ok(format!("Opened shell at {host}:{dir}"))
}

#[tauri::command]
fn ssh_shell_command(host: String, dir: String) -> String {
    terminal::ssh_command_line(&terminal::ssh_shell_at_argv(&host, &dir))
}

#[tauri::command]
async fn attach_session(host: String, task: String, resume: bool) -> Result<String, String> {
    if resume {
        hub::spawn(
            &host,
            &SpawnRequest {
                task: task.clone(),
                dir: None,
                provider: None,
                extra_args: vec![],
                auto: false,
                resume: true,
            },
        )
        .await?;
    }
    let att = hub::attach_argv(&host, &task).await?;
    let term = TerminalMacos::parse(&hub::load_client().terminal);
    let label = format!("{host}/{task}");
    terminal::open_attach(&att.command, &label, term).map_err(|e| e.to_string())?;
    Ok(format!(
        "Opened terminal for {host}/{task}{}",
        if resume { " (resuming)" } else { "" }
    ))
}

#[tauri::command]
async fn list_bindings_cmd(host: String, task: String) -> Result<BindingsDoc, String> {
    hub::bindings(&host, &task).await
}

#[tauri::command]
async fn bind_session_cmd(
    host: String,
    task: String,
    session_id: String,
) -> Result<BindingsDoc, String> {
    hub::bind(&host, &task, &session_id).await
}

#[tauri::command]
async fn fetch_transcript(
    host: String,
    task: String,
    after: Option<String>,
    tail: Option<u32>,
    until_offset: Option<u64>,
) -> Result<TranscriptDoc, String> {
    hub::transcript(&host, &task, after.as_deref(), tail, until_offset).await
}

#[tauri::command]
async fn say_to_session(host: String, task: String, text: String) -> Result<String, String> {
    hub::say(&host, &task, &text).await
}

#[tauri::command]
fn start_transcript_stream_cmd(
    app: tauri::AppHandle,
    host: String,
    task: String,
    after: Option<String>,
    tail: Option<u32>,
) -> Result<(), String> {
    transcript_stream::start_transcript_stream(app, host, task, after, tail)
}

#[tauri::command]
fn stop_transcript_stream_cmd() {
    transcript_stream::stop_transcript_stream();
}

fn spawn_hub_listener(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            match hub::refresh_fleet().await {
                Ok(fleet) => {
                    let _ = app.emit("fleet", &fleet);
                }
                Err(e) => {
                    let _ = app.emit("fleet-error", e);
                }
            }
            if let Some(url) = hub::hub_events_url() {
                listen_sse(&app, &url).await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        }
    });
}

async fn listen_sse(app: &AppHandle, url: &str) {
    let Ok(resp) = reqwest::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
    else {
        return;
    };
    if !resp.status().is_success() {
        return;
    }
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let Ok(bytes) = chunk else {
            break;
        };
        buf.push_str(&String::from_utf8_lossy(&bytes));
        for data in sse::drain_data_events(&mut buf) {
            if let Ok(ev) = serde_json::from_str::<HubEvent>(&data) {
                let _ = app.emit("hub-event", &ev);
            }
            if let Ok(fleet) = hub::refresh_fleet().await {
                let _ = app.emit("fleet", &fleet);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            spawn_hub_listener(app.handle().clone());
            let handle = app.handle().clone();
            app.listen("hive-refresh", move |_| {
                let handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(fleet) = hub::refresh_fleet().await {
                        let _ = handle.emit("fleet", &fleet);
                    }
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            close_app,
            get_config,
            fetch_fleet_cmd,
            refresh_fleet_cmd,
            list_hosts_cmd,
            list_remote_dirs_cmd,
            spawn_session_cmd,
            kill_session,
            stop_session,
            restart_session,
            label_session,
            attach_session,
            open_shell_at_session,
            ssh_shell_command,
            fetch_transcript,
            list_bindings_cmd,
            bind_session_cmd,
            say_to_session,
            start_transcript_stream_cmd,
            stop_transcript_stream_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running hive-panel");
}
