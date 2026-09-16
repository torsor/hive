use std::path::{Path, PathBuf};

use serde_json::Value;
use uuid::Uuid;
use walkdir::WalkDir;

use super::DiscoverError;

pub fn codex_home() -> PathBuf {
    if let Ok(p) = std::env::var("CODEX_HOME") {
        return PathBuf::from(p);
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".codex")
}

pub fn codex_sessions_root() -> PathBuf {
    if let Ok(p) = std::env::var("CODEX_SESSIONS_ROOT") {
        return PathBuf::from(p);
    }
    codex_home().join("sessions")
}

pub fn codex_session_index_path() -> PathBuf {
    if let Ok(p) = std::env::var("CODEX_SESSION_INDEX") {
        return PathBuf::from(p);
    }
    codex_home().join("session_index.jsonl")
}

fn is_full_uuid(value: &str) -> bool {
    Uuid::parse_str(value)
        .ok()
        .is_some_and(|u| u.to_string() == value.to_lowercase())
}

fn session_meta_identity(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
            continue;
        };
        let Some(payload) = v.get("payload") else {
            return (None, None);
        };
        let id = payload
            .get("id")
            .or_else(|| payload.get("session_id"))
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let name = payload
            .get("thread_name")
            .or_else(|| payload.get("name"))
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return (id, name);
    }
    (None, None)
}

fn rollouts_matching_id(sessions_root: &Path, needle: &str) -> Vec<PathBuf> {
    if !sessions_root.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(sessions_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("rollout-") || !name.ends_with(".jsonl") || !name.contains(needle) {
            continue;
        }
        out.push(path.to_path_buf());
    }
    out.sort();
    out
}

pub fn ids_for_thread_name(index_path: &Path, name: &str) -> Vec<String> {
    if name.is_empty() || !index_path.is_file() {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(index_path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("thread_name").and_then(|x| x.as_str()) != Some(name) {
            continue;
        }
        let Some(id) = v.get("id").and_then(|x| x.as_str()) else {
            continue;
        };
        if id.is_empty() || !seen.insert(id.to_string()) {
            continue;
        }
        out.push(id.to_string());
    }
    out
}

/// Newest ``session_index`` row for a resume name (cc-run keeps the last match).
pub fn latest_id_for_thread_name(
    index_path: &Path,
    name: &str,
    updated_after: Option<&str>,
) -> Option<String> {
    if name.is_empty() || !index_path.is_file() {
        return None;
    }
    let Ok(text) = std::fs::read_to_string(index_path) else {
        return None;
    };
    let mut latest: Option<String> = None;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("thread_name").and_then(|x| x.as_str()) != Some(name) {
            continue;
        }
        let Some(id) = v.get("id").and_then(|x| x.as_str()) else {
            continue;
        };
        if id.is_empty() || !is_full_uuid(id) {
            continue;
        }
        if let Some(after) = updated_after {
            let updated = v.get("updated_at").and_then(|x| x.as_str()).unwrap_or("");
            if !updated.is_empty() && updated < after {
                continue;
            }
        }
        latest = Some(id.to_string());
    }
    latest
}

fn unique_rollouts_for_uuid(sessions_root: &Path, thread_uuid: &str) -> Vec<(PathBuf, String)> {
    let mut hits = Vec::new();
    for path in rollouts_matching_id(sessions_root, thread_uuid) {
        let (meta_id, _) = session_meta_identity(&path);
        if meta_id.as_deref() == Some(thread_uuid) {
            hits.push((path, thread_uuid.to_string()));
        }
    }
    hits
}

pub fn find_codex_rollout(
    transcript_id: Option<&str>,
    session_id: Option<&str>,
    sessions_root: &Path,
) -> Result<(PathBuf, String), DiscoverError> {
    let tid = transcript_id.filter(|s| !s.is_empty());
    if let Some(tid) = tid {
        if !sessions_root.is_dir() {
            return Err(DiscoverError(format!(
                "no Codex sessions tree at {}",
                sessions_root.display()
            )));
        }
        let mut matches = Vec::new();
        for path in rollouts_matching_id(sessions_root, tid) {
            let (meta_id, _) = session_meta_identity(&path);
            if meta_id.is_none() || meta_id.as_deref() == Some(tid) {
                matches.push((path, meta_id.unwrap_or_else(|| tid.to_string())));
            }
        }
        if matches.is_empty() {
            return Err(DiscoverError(format!(
                "no codex rollout for transcript_id={tid}"
            )));
        }
        if matches.len() > 1 {
            return Err(DiscoverError(format!(
                "ambiguous codex rollout for transcript_id={tid}: {} files",
                matches.len()
            )));
        }
        return Ok(matches.into_iter().next().unwrap());
    }

    let sid = session_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DiscoverError("codex transcript_id or session_id required".into()))?;
    if !sessions_root.is_dir() {
        return Err(DiscoverError(format!(
            "no Codex sessions tree at {}",
            sessions_root.display()
        )));
    }

    let matches = if is_full_uuid(sid) {
        unique_rollouts_for_uuid(sessions_root, sid)
    } else {
        let index = codex_session_index_path();
        let mut matches = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for thread_id in ids_for_thread_name(&index, sid) {
            if !is_full_uuid(&thread_id) {
                continue;
            }
            for (path, bound) in unique_rollouts_for_uuid(sessions_root, &thread_id) {
                if seen.insert(path.clone()) {
                    matches.push((path, bound));
                }
            }
        }
        matches
    };

    if matches.is_empty() {
        return Err(DiscoverError(format!("no codex rollout for id={sid}")));
    }
    if matches.len() > 1 {
        return Err(DiscoverError(format!(
            "ambiguous codex rollout for id={sid}: {} files",
            matches.len()
        )));
    }
    Ok(matches.into_iter().next().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_rollout(path: &Path, transcript_id: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let line = serde_json::json!({
            "type": "session_meta",
            "payload": { "id": transcript_id, "cwd": "/tmp/x" }
        });
        std::fs::File::create(path)
            .unwrap()
            .write_all(format!("{line}\n").as_bytes())
            .unwrap();
    }

    #[test]
    fn finds_rollout_by_transcript_id() {
        let root = std::env::temp_dir().join(format!("hive-codex-tid-{}", std::process::id()));
        let path = root.join("rollout-2026-01-01T00-00-00-abc123.jsonl");
        write_rollout(&path, "abc123");
        let (hit, id) =
            find_codex_rollout(Some("abc123"), None, &root).expect("bind by transcript_id");
        assert_eq!(hit, path);
        assert_eq!(id, "abc123");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resume_name_uses_session_index() {
        let root = std::env::temp_dir().join(format!("hive-codex-idx-{}", std::process::id()));
        let sessions = root.join("sessions");
        let uuid = "019fe863-3499-75a3-9e9f-20918cf711e9";
        let rollout = sessions.join(format!("rollout-2026-01-01T00-00-00-{uuid}.jsonl"));
        write_rollout(&rollout, uuid);
        let index = root.join("session_index.jsonl");
        std::fs::write(
            &index,
            format!(r#"{{"id":"{uuid}","thread_name":"demo-task"}}"#) + "\n",
        )
        .unwrap();
        std::env::set_var("CODEX_SESSIONS_ROOT", &sessions);
        std::env::set_var("CODEX_SESSION_INDEX", &index);
        let (hit, id) =
            find_codex_rollout(None, Some("demo-task"), &sessions).expect("bind by thread_name");
        assert_eq!(hit, rollout);
        assert_eq!(id, uuid);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn latest_id_prefers_newest_after_started() {
        let root = std::env::temp_dir().join(format!("hive-codex-latest-{}", std::process::id()));
        let index = root.join("session_index.jsonl");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &index,
            r#"{"id":"019fe863-3499-75a3-9e9f-20918cf711e9","thread_name":"demo-task","updated_at":"2026-08-14T18:15:13Z"}
{"id":"01a00188-921a-7c43-a771-166299b02f9e","thread_name":"demo-task","updated_at":"2026-08-14T18:28:46Z"}
"#,
        )
        .unwrap();
        let all = ids_for_thread_name(&index, "demo-task");
        assert_eq!(all.len(), 2);
        let latest = latest_id_for_thread_name(&index, "demo-task", Some("2026-08-14T18:28:42Z"));
        assert_eq!(
            latest.as_deref(),
            Some("01a00188-921a-7c43-a771-166299b02f9e")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_filename_substring_for_task_name() {
        let root = std::env::temp_dir().join(format!("hive-codex-no-sub-{}", std::process::id()));
        let sessions = root.join("sessions");
        let rollout = sessions.join("rollout-2026-01-01T00-00-00-demo-task-extra.jsonl");
        write_rollout(&rollout, "other-uuid");
        std::env::set_var("CODEX_SESSIONS_ROOT", &sessions);
        std::env::set_var("CODEX_SESSION_INDEX", root.join("empty.jsonl"));
        let _ = std::fs::File::create(root.join("empty.jsonl")).unwrap();
        let err = find_codex_rollout(None, Some("demo-task"), &sessions).unwrap_err();
        assert!(err.0.contains("no codex rollout"));
        let _ = std::fs::remove_dir_all(root);
    }
}
