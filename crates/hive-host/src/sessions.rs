use std::path::PathBuf;

use hive_protocol::SessionRow;

use hive_common::HiveHome;
use crate::labels;
use crate::meta;
use crate::tmux;

pub fn meta_path(home: &HiveHome, task: &str) -> Option<PathBuf> {
    let p = home.sessions_dir().join(format!("{task}.meta"));
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

pub fn labels_path(home: &HiveHome, task: &str) -> PathBuf {
    if let Some(meta) = meta_path(home, task) {
        return meta.with_extension("labels");
    }
    home.sessions_dir().join(format!("{task}.labels"))
}

pub fn list_sessions(home: &HiveHome) -> Vec<SessionRow> {
    let dir = home.sessions_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("meta") {
            continue;
        }
        let Ok(map) = meta::load_meta(&path) else {
            continue;
        };
        let task = map
            .get("task")
            .cloned()
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string()
            });
        let tmux_name = map.get("tmux").cloned();
        let resolved = tmux::resolve_tmux(&task, tmux_name.as_deref());
        let running = tmux::has_session(&resolved);
        let last = if running {
            tmux::capture_last_line(&resolved)
        } else {
            None
        };
        let starred = labels::load(&path.with_extension("labels")).starred;
        rows.push(SessionRow {
            host: None,
            task,
            state: if running {
                "running".into()
            } else {
                "exited".into()
            },
            started: map.get("started").cloned().unwrap_or_default(),
            dir: map.get("dir").cloned().unwrap_or_default(),
            last,
            provider: map
                .get("provider")
                .cloned()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "claude".into()),
            starred,
            tmux: Some(resolved),
        });
    }
    rows.sort_by(|a, b| a.task.cmp(&b.task));
    rows
}

pub fn load_task_meta(
    home: &HiveHome,
    task: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let path = meta_path(home, task).ok_or_else(|| format!("no tracked session for '{task}'"))?;
    meta::load_meta(&path).map_err(|e| e.to_string())
}
