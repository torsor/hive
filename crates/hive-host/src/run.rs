use chrono::Utc;
use uuid::Uuid;

use hive_common::{ensure_dir, resolve_provider_bin, validate_task_name, HiveHome};
use crate::codex_bind;
use crate::meta;
use crate::sessions;
use crate::tmux;
use hive_protocol::SpawnRequest;

fn codex_projects_override(workdir: &str) -> String {
    let workdir_toml =
        serde_json::to_string(workdir).unwrap_or_else(|_| format!("\"{workdir}\""));
    format!(r#"projects={{{workdir_toml}={{trust_level="trusted"}}}}"#)
}

fn codex_base_args(workdir: &str) -> Vec<String> {
    vec![
        "--sandbox".into(),
        "danger-full-access".into(),
        "--ask-for-approval".into(),
        "never".into(),
        "-c".into(),
        codex_projects_override(workdir),
    ]
}

fn quote_cmd(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| meta::shell_quote(p))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn spawn(home: &HiveHome, req: &SpawnRequest) -> Result<String, String> {
    if req.resume {
        return resume(home, &req.task);
    }
    let task = validate_task_name(req.task.trim())?;
    let dir = req
        .dir
        .as_deref()
        .ok_or("dir is required for a fresh start")?;
    let dir = std::fs::canonicalize(dir).map_err(|e| format!("no such directory: {dir} ({e})"))?;
    let provider = req
        .provider
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("claude");
    if provider != "claude" && provider != "codex" {
        return Err(format!("unsupported provider {provider}"));
    }
    let tmux_name = format!("hive-{}", tmux::safe_task(task));
    if tmux::has_session(&tmux_name) {
        return Err(format!("session already active for {task}"));
    }
    let bin = resolve_provider_bin(provider)?;
    let session_id = if provider == "claude" {
        Uuid::new_v4().to_string()
    } else {
        tmux::safe_task(task)
    };
    let mut parts = vec![bin.clone()];
    if provider == "claude" {
        parts.push("--session-id".into());
        parts.push(session_id.clone());
    } else {
        parts.extend(codex_base_args(&dir.to_string_lossy()));
    }
    parts.extend(req.extra_args.iter().cloned());
    if req.auto && provider == "claude" {
        parts.extend(["--permission-mode".into(), "acceptEdits".into()]);
    }
    let cmd = quote_cmd(&parts);
    let started = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let meta_path = home.sessions_dir().join(format!("{task}.meta"));
    ensure_dir(&home.sessions_dir()).map_err(|e| e.to_string())?;
    meta::write_meta(
        &meta_path,
        &[
            ("task", task),
            ("dir", &dir.to_string_lossy()),
            ("tmux", &tmux_name),
            ("started", &started),
            ("cmd", &cmd),
            ("session_id", &session_id),
            ("provider", provider),
        ],
    )
    .map_err(|e| e.to_string())?;
    tmux::new_session(&tmux_name, &dir.to_string_lossy(), &cmd)?;
    if provider == "codex" {
        codex_bind::schedule_codex_rename(tmux_name.clone(), task.to_string(), meta_path);
    }
    Ok(format!("started {tmux_name} in {} (provider {provider})", dir.display()))
}

pub fn resume(home: &HiveHome, task: &str) -> Result<String, String> {
    let task = validate_task_name(task)?;
    let map = sessions::load_task_meta(home, task)?;
    let dir = map.get("dir").cloned().ok_or("no dir in meta")?;
    let session_id = map
        .get("session_id")
        .cloned()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("'{task}' has no recorded session id"))?;
    let provider = map
        .get("provider")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "claude".into());
    let tmux_name = map
        .get("tmux")
        .cloned()
        .unwrap_or_else(|| format!("hive-{}", tmux::safe_task(task)));
    if tmux::has_session(&tmux_name) {
        return Err(format!("session already active: {tmux_name}"));
    }
    if !std::path::Path::new(&dir).is_dir() {
        return Err(format!("recorded directory is gone: {dir}"));
    }
    let bin = resolve_provider_bin(&provider)?;
    let mut parts = vec![bin];
    if provider == "claude" {
        parts.extend(["--resume".into(), session_id.clone()]);
    } else {
        parts.extend(codex_base_args(&dir));
        parts.extend(["resume".into(), session_id.clone()]);
    }
    let cmd = quote_cmd(&parts);
    let started = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let meta_path = sessions::meta_path(home, task).ok_or("meta vanished")?;
    meta::write_meta(
        &meta_path,
        &[
            ("task", task),
            ("dir", &dir),
            ("tmux", &tmux_name),
            ("started", &started),
            ("cmd", &cmd),
            ("session_id", &session_id),
            ("provider", &provider),
        ],
    )
    .map_err(|e| e.to_string())?;
    tmux::new_session(&tmux_name, &dir, &cmd)?;
    Ok(format!("resumed {tmux_name} in {dir}"))
}

pub fn stop(home: &HiveHome, task: &str) -> Result<String, String> {
    let task = validate_task_name(task)?;
    let map = sessions::load_task_meta(home, task)?;
    let tmux_name = tmux::resolve_tmux(task, map.get("tmux").map(|s| s.as_str()));
    if tmux::has_session(&tmux_name) {
        tmux::kill_session(&tmux_name)?;
    }
    Ok(format!("stopped {tmux_name} — offline, meta kept"))
}

pub fn kill(home: &HiveHome, task: &str) -> Result<String, String> {
    let task = validate_task_name(task)?;
    let _ = stop(home, task);
    if let Some(meta) = sessions::meta_path(home, task) {
        let _ = std::fs::remove_file(&meta);
        let _ = std::fs::remove_file(meta.with_extension("labels"));
    }
    Ok(format!("killed {task}"))
}

pub fn restart(home: &HiveHome, task: &str) -> Result<String, String> {
    let task = validate_task_name(task)?;
    let _ = stop(home, task);
    resume(home, task)
}
