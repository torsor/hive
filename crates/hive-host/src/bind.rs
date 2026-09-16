//! Interactive transcript rebinding: list candidates and persist a pick.

use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::{TimeZone, Utc};
use hive_common::HiveHome;
use hive_protocol::{BindingCandidate, BindingsDoc};
use serde_json::Value;

use crate::discover::{self, claude_project_slug, claude_projects_root};
use crate::meta;
use crate::sessions;
use crate::tmux;

const PANE_MISMATCH: &str = "not the session in the tmux pane";

#[derive(Debug, Clone)]
pub struct RawCandidate {
    pub session_id: String,
    pub thread_name: Option<String>,
    pub updated_at: Option<String>,
}

fn is_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .ok()
        .is_some_and(|u| u.to_string() == value.to_lowercase())
}

pub fn parse_pane_session_id(pane: &str) -> Option<String> {
    let lower = pane.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("session:") {
        let abs = search + rel + "session:".len();
        let rest = pane[abs..].trim_start_matches(|c: char| c.is_whitespace() || c == '│');
        let token: String = rest
            .chars()
            .take_while(|c| c.is_ascii_hexdigit() || *c == '-')
            .collect();
        if is_uuid(&token) {
            return Some(token.to_lowercase());
        }
        search = abs;
    }
    None
}

pub fn assemble_bindings(
    bound: Option<&str>,
    pane: Option<&str>,
    raw: Vec<RawCandidate>,
) -> BindingsDoc {
    let bound = bound
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let pane = pane
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let mut by_id: HashMap<String, RawCandidate> = HashMap::new();
    for cand in raw {
        if cand.session_id.is_empty() {
            continue;
        }
        by_id.insert(cand.session_id.clone(), cand);
    }
    if let Some(id) = bound.clone() {
        by_id.entry(id.clone()).or_insert(RawCandidate {
            session_id: id,
            thread_name: None,
            updated_at: None,
        });
    }
    if let Some(id) = pane.clone() {
        by_id.entry(id.clone()).or_insert(RawCandidate {
            session_id: id,
            thread_name: None,
            updated_at: None,
        });
    }

    let suggested = if let Some(id) = pane.clone() {
        Some(id)
    } else {
        by_id
            .values()
            .filter(|c| bound.as_deref() != Some(c.session_id.as_str()))
            .filter(|c| c.updated_at.as_ref().is_some_and(|s| !s.is_empty()))
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
            .map(|c| c.session_id.clone())
    };

    let mut candidates: Vec<BindingCandidate> = by_id
        .into_values()
        .map(|raw| {
            let live = pane.as_deref() == Some(raw.session_id.as_str());
            let current = bound.as_deref() == Some(raw.session_id.as_str());
            let is_suggested = suggested.as_deref() == Some(raw.session_id.as_str());
            BindingCandidate {
                session_id: raw.session_id,
                thread_name: raw.thread_name,
                updated_at: raw.updated_at,
                current,
                live,
                suggested: is_suggested,
                warning: if pane.is_some() && !live {
                    Some(PANE_MISMATCH.into())
                } else {
                    None
                },
            }
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.suggested
            .cmp(&a.suggested)
            .then(b.live.cmp(&a.live))
            .then(b.updated_at.cmp(&a.updated_at))
            .then(a.session_id.cmp(&b.session_id))
    });

    BindingsDoc {
        bound,
        pane_session_id: pane,
        suggested,
        candidates,
    }
}

pub fn list_codex_raw(index_path: &Path, task: &str) -> Vec<RawCandidate> {
    if task.is_empty() || !index_path.is_file() {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(index_path) else {
        return Vec::new();
    };
    let mut by_id: HashMap<String, RawCandidate> = HashMap::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("thread_name").and_then(|x| x.as_str()) != Some(task) {
            continue;
        }
        let Some(id) = v.get("id").and_then(|x| x.as_str()).filter(|s| !s.is_empty()) else {
            continue;
        };
        let updated_at = v
            .get("updated_at")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        by_id.insert(
            id.to_string(),
            RawCandidate {
                session_id: id.to_string(),
                thread_name: Some(task.to_string()),
                updated_at,
            },
        );
    }
    by_id.into_values().collect()
}

fn file_updated_at(path: &Path) -> Option<String> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Utc.timestamp_opt(secs as i64, 0).single()?.to_rfc3339())
}

pub fn list_claude_raw(projects_root: &Path, workdir: Option<&str>) -> Vec<RawCandidate> {
    let slug = claude_project_slug(workdir.unwrap_or(""));
    if slug.is_empty() {
        return Vec::new();
    }
    let dir = projects_root.join(&slug);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        out.push(RawCandidate {
            session_id: id.to_string(),
            thread_name: None,
            updated_at: file_updated_at(&path),
        });
    }
    out
}

fn provider_of(meta: &HashMap<String, String>) -> &str {
    meta.get("provider")
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("claude")
}

fn raw_for_meta(meta: &HashMap<String, String>, task: &str) -> Vec<RawCandidate> {
    match provider_of(meta) {
        "codex" => list_codex_raw(&discover::codex_session_index_path(), task),
        _ => list_claude_raw(
            &claude_projects_root(),
            meta.get("dir")
                .or_else(|| meta.get("workdir"))
                .map(String::as_str),
        ),
    }
}

fn pane_for_meta(meta: &HashMap<String, String>, task: &str) -> Option<String> {
    let tmux_name = tmux::resolve_tmux(task, meta.get("tmux").map(String::as_str));
    if !tmux::has_session(&tmux_name) {
        return None;
    }
    parse_pane_session_id(&tmux::capture_tail(&tmux_name, 80))
}

pub fn list_bindings(home: &HiveHome, task: &str) -> Result<BindingsDoc, String> {
    let meta = sessions::load_task_meta(home, task)?;
    let bound = meta.get("session_id").map(String::as_str);
    let pane = pane_for_meta(&meta, task);
    Ok(assemble_bindings(
        bound,
        pane.as_deref(),
        raw_for_meta(&meta, task),
    ))
}

fn set_meta_session_id(path: &Path, session_id: &str) -> Result<(), String> {
    let mut map = meta::load_meta(path).map_err(|e| e.to_string())?;
    map.insert("session_id".into(), session_id.to_string());
    let fields: Vec<(&str, &str)> = map
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    meta::write_meta(path, &fields).map_err(|e| e.to_string())
}

pub fn bind_session(home: &HiveHome, task: &str, session_id: &str) -> Result<BindingsDoc, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".into());
    }
    let doc = list_bindings(home, task)?;
    if !doc.candidates.iter().any(|c| c.session_id == session_id) {
        return Err(format!("unknown session_id={session_id}"));
    }
    let path = sessions::meta_path(home, task).ok_or_else(|| format!("no tracked session for '{task}'"))?;
    set_meta_session_id(&path, session_id)?;
    list_bindings(home, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUND: &str = "01a00701-caec-7052-8ccf-376b4002b8e3";
    const LIVE: &str = "019fe737-74ae-7e53-a253-f5f96cb782ff";

    fn pane_status() -> String {
        format!(
            "\
│  Thread name:          period-index                                             │
│  Session:              {LIVE}                     │
╰─────────────────────────────────────────────────────────────────────────────────╯
"
        )
    }

    #[test]
    fn parse_pane_reads_session_label() {
        let id = parse_pane_session_id(&pane_status());
        assert_eq!(id.as_deref(), Some(LIVE));
    }

    #[test]
    fn parse_pane_ignores_uuid_mentions_without_label() {
        let pane = format!("To resume run codex resume ({LIVE})\n› hello");
        assert_eq!(parse_pane_session_id(pane.as_str()), None);
    }

    #[test]
    fn pane_id_is_suggested_and_live() {
        let doc = assemble_bindings(
            Some(BOUND),
            Some(LIVE),
            vec![
                RawCandidate {
                    session_id: BOUND.into(),
                    thread_name: Some("period-index".into()),
                    updated_at: Some("2026-08-15T19:59:16Z".into()),
                },
                RawCandidate {
                    session_id: LIVE.into(),
                    thread_name: Some("period-index".into()),
                    updated_at: Some("2026-08-15T20:24:08Z".into()),
                },
            ],
        );
        assert_eq!(doc.bound.as_deref(), Some(BOUND));
        assert_eq!(doc.pane_session_id.as_deref(), Some(LIVE));
        assert_eq!(doc.suggested.as_deref(), Some(LIVE));
        let live = doc
            .candidates
            .iter()
            .find(|c| c.session_id == LIVE)
            .expect("live candidate");
        assert!(live.live);
        assert!(live.suggested);
        assert!(!live.current);
        assert_eq!(live.warning, None);
        let bound = doc
            .candidates
            .iter()
            .find(|c| c.session_id == BOUND)
            .expect("bound candidate");
        assert!(bound.current);
        assert!(!bound.live);
        assert_eq!(bound.warning.as_deref(), Some(PANE_MISMATCH));
    }

    #[test]
    fn pane_id_missing_from_index_is_still_listed() {
        let doc = assemble_bindings(
            Some(BOUND),
            Some(LIVE),
            vec![RawCandidate {
                session_id: BOUND.into(),
                thread_name: Some("period-index".into()),
                updated_at: Some("2026-08-15T19:59:16Z".into()),
            }],
        );
        assert!(doc.candidates.iter().any(|c| c.session_id == LIVE && c.live));
        assert_eq!(doc.suggested.as_deref(), Some(LIVE));
    }

    #[test]
    fn newest_index_row_is_suggested_when_pane_unknown() {
        let doc = assemble_bindings(
            Some(BOUND),
            None,
            vec![
                RawCandidate {
                    session_id: BOUND.into(),
                    thread_name: Some("period-index".into()),
                    updated_at: Some("2026-08-15T19:59:16Z".into()),
                },
                RawCandidate {
                    session_id: LIVE.into(),
                    thread_name: Some("period-index".into()),
                    updated_at: Some("2026-08-15T20:24:08Z".into()),
                },
            ],
        );
        assert_eq!(doc.suggested.as_deref(), Some(LIVE));
        assert!(doc.candidates.iter().all(|c| c.warning.is_none()));
    }

    #[test]
    fn no_suggestion_when_only_current_is_known() {
        let doc = assemble_bindings(
            Some(BOUND),
            None,
            vec![RawCandidate {
                session_id: BOUND.into(),
                thread_name: Some("period-index".into()),
                updated_at: Some("2026-08-15T19:59:16Z".into()),
            }],
        );
        assert_eq!(doc.suggested, None);
        assert_eq!(doc.candidates.len(), 1);
        assert!(doc.candidates[0].current);
        assert!(!doc.candidates[0].suggested);
    }

    #[test]
    fn later_index_row_wins_for_same_id() {
        let raw = list_codex_raw_from_text(
            r#"{"id":"019fe737-74ae-7e53-a253-f5f96cb782ff","thread_name":"period-index-etc","updated_at":"2026-08-09T16:02:28Z"}
{"id":"01a00701-caec-7052-8ccf-376b4002b8e3","thread_name":"period-index","updated_at":"2026-08-15T19:59:16Z"}
{"id":"019fe737-74ae-7e53-a253-f5f96cb782ff","thread_name":"period-index","updated_at":"2026-08-15T20:24:08Z"}
"#,
            "period-index",
        );
        assert_eq!(raw.len(), 2);
        let live = raw.iter().find(|c| c.session_id == LIVE).unwrap();
        assert_eq!(live.thread_name.as_deref(), Some("period-index"));
        assert_eq!(live.updated_at.as_deref(), Some("2026-08-15T20:24:08Z"));
    }

    fn list_codex_raw_from_text(text: &str, task: &str) -> Vec<RawCandidate> {
        let dir = std::env::temp_dir().join(format!(
            "hive-bind-idx-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session_index.jsonl");
        std::fs::write(&path, text).unwrap();
        let raw = list_codex_raw(&path, task);
        let _ = std::fs::remove_dir_all(dir);
        raw
    }

    #[test]
    fn claude_lists_jsonl_stems_in_project_slug() {
        let root = std::env::temp_dir().join(format!(
            "hive-bind-claude-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let proj = root.join("-tmp-period-index");
        std::fs::create_dir_all(&proj).unwrap();
        let sid = "11111111-2222-3333-4444-555555555555";
        std::fs::write(proj.join(format!("{sid}.jsonl")), "{}\n").unwrap();
        let raw = list_claude_raw(&root, Some("/tmp/period-index"));
        let _ = std::fs::remove_dir_all(root);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].session_id, sid);
    }

    #[test]
    fn bind_persists_session_id_when_candidate_known() {
        let root = std::env::temp_dir().join(format!(
            "hive-bind-meta-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = HiveHome { root: root.clone() };
        std::fs::create_dir_all(home.sessions_dir()).unwrap();
        let index = root.join("session_index.jsonl");
        std::fs::write(
            &index,
            format!(
                r#"{{"id":"{BOUND}","thread_name":"demo","updated_at":"2026-08-15T19:59:16Z"}}
{{"id":"{LIVE}","thread_name":"demo","updated_at":"2026-08-15T20:24:08Z"}}
"#
            ),
        )
        .unwrap();
        meta::write_meta(
            &home.sessions_dir().join("demo.meta"),
            &[
                ("task", "demo"),
                ("dir", "/tmp/x"),
                ("tmux", "hive-demo-missing"),
                ("session_id", BOUND),
                ("provider", "codex"),
            ],
        )
        .unwrap();
        std::env::set_var("CODEX_SESSION_INDEX", &index);
        let doc = bind_session(&home, "demo", LIVE).expect("bind");
        std::env::remove_var("CODEX_SESSION_INDEX");
        assert_eq!(doc.bound.as_deref(), Some(LIVE));
        let map = sessions::load_task_meta(&home, "demo").unwrap();
        assert_eq!(map.get("session_id").map(String::as_str), Some(LIVE));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bind_rejects_unknown_session_id() {
        let root = std::env::temp_dir().join(format!(
            "hive-bind-unknown-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = HiveHome { root: root.clone() };
        std::fs::create_dir_all(home.sessions_dir()).unwrap();
        let index = root.join("empty.jsonl");
        std::fs::write(&index, "").unwrap();
        meta::write_meta(
            &home.sessions_dir().join("demo.meta"),
            &[
                ("task", "demo"),
                ("dir", "/tmp/x"),
                ("tmux", "hive-demo-missing"),
                ("session_id", BOUND),
                ("provider", "codex"),
            ],
        )
        .unwrap();
        std::env::set_var("CODEX_SESSION_INDEX", &index);
        let err = bind_session(&home, "demo", LIVE).unwrap_err();
        std::env::remove_var("CODEX_SESSION_INDEX");
        assert!(err.contains("unknown session_id"), "{err}");
        let _ = std::fs::remove_dir_all(root);
    }
}
