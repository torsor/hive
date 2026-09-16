use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::discover::{codex_session_index_path, latest_id_for_thread_name};
use crate::meta;
use crate::tmux;

const RENAME_CMD: &str = "/rename";
const SEED_TEXT: &str = "# hive: session ready";
const READY_TIMEOUT: Duration = Duration::from_secs(90);
const INDEX_TIMEOUT: Duration = Duration::from_secs(45);
const ROLLOUT_WAIT: Duration = Duration::from_secs(5);
const SEED_ROLLOUT_WAIT: Duration = Duration::from_secs(30);

pub fn schedule_codex_rename(tmux_name: String, task: String, meta_path: PathBuf) {
    thread::spawn(move || {
        if let Err(e) = bind_codex_session(&tmux_name, &task, &meta_path) {
            eprintln!("[hive-host] codex bind for {task}: {e}");
        }
    });
}

fn bind_codex_session(tmux_name: &str, task: &str, meta_path: &Path) -> Result<(), String> {
    wait_for_codex_ready(tmux_name)?;
    thread::sleep(Duration::from_secs(2));
    let rename = format!("{RENAME_CMD} {task}");
    tmux::send_literal(tmux_name, &rename)?;
    thread::sleep(Duration::from_millis(300));
    tmux::send_keys(tmux_name, &["Enter"])?;

    let started = meta::load_meta(meta_path)
        .ok()
        .and_then(|m| m.get("started").cloned());

    let deadline = Instant::now() + INDEX_TIMEOUT;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(1));
        let index = codex_session_index_path();
        let Some(uuid) = latest_id_for_thread_name(&index, task, started.as_deref()) else {
            continue;
        };
        if meta::load_meta(meta_path)
            .map(|m| m.get("session_id").map(String::as_str) == Some(uuid.as_str()))
            .unwrap_or(false)
        {
            seed_rollout_if_needed(tmux_name, &uuid)?;
            return Ok(());
        }
        update_meta_session_id(meta_path, &uuid)?;
        eprintln!("[hive-host] codex bound {task} -> {uuid}");
        seed_rollout_if_needed(tmux_name, &uuid)?;
        return Ok(());
    }
    Err(format!(
        "session_index has no thread_name={task} after /rename (chat may work after manual rename)"
    ))
}

fn seed_rollout_if_needed(tmux_name: &str, uuid: &str) -> Result<(), String> {
    let sessions = crate::discover::codex_sessions_root();
    if rollout_exists(&sessions, uuid) {
        return Ok(());
    }
    let deadline = Instant::now() + ROLLOUT_WAIT;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(1));
        if rollout_exists(&sessions, uuid) {
            return Ok(());
        }
    }
    eprintln!("[hive-host] seeding first turn so Codex writes rollout for {uuid}");
    thread::sleep(Duration::from_secs(1));
    tmux::send_literal(tmux_name, SEED_TEXT)?;
    thread::sleep(Duration::from_millis(300));
    tmux::send_keys(tmux_name, &["Enter"])?;
    let deadline = Instant::now() + SEED_ROLLOUT_WAIT;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(1));
        if rollout_exists(&sessions, uuid) {
            eprintln!("[hive-host] codex rollout ready for {uuid}");
            return Ok(());
        }
    }
    eprintln!("[hive-host] note: no rollout yet for {uuid}; chat may stay empty until a user turn");
    Ok(())
}

fn rollout_exists(sessions_root: &Path, uuid: &str) -> bool {
    crate::discover::find_codex_rollout(Some(uuid), None, sessions_root).is_ok()
}

fn wait_for_codex_ready(tmux_name: &str) -> Result<(), String> {
    let deadline = Instant::now() + READY_TIMEOUT;
    let started = Instant::now();
    while Instant::now() < deadline {
        let pane = tmux::capture_tail(tmux_name, 30);
        // Wait for the real input prompt — not the splash (which also mentions "Codex").
        if pane.contains('›') {
            return Ok(());
        }
        if started.elapsed() >= Duration::from_secs(5) && pane.contains("Explain this") {
            return Ok(());
        }
        if pane.contains("trust the contents of this directory") {
            tmux::send_keys(tmux_name, &["1"])?;
            thread::sleep(Duration::from_millis(300));
            tmux::send_keys(tmux_name, &["Enter"])?;
            thread::sleep(Duration::from_secs(1));
            continue;
        }
        if pane.contains("command not found") {
            return Err(format!("provider binary missing in tmux session {tmux_name}"));
        }
        if !tmux::has_session(tmux_name) {
            return Err(format!("tmux session {tmux_name} ended before codex ready"));
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err("codex not ready within timeout".into())
}

fn update_meta_session_id(meta_path: &Path, uuid: &str) -> Result<(), String> {
    let mut map = meta::load_meta(meta_path).map_err(|e| e.to_string())?;
    map.insert("session_id".into(), uuid.to_string());
    let fields: Vec<(&str, &str)> = map
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    meta::write_meta(meta_path, &fields).map_err(|e| e.to_string())
}
