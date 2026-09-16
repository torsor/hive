mod claude;
mod codex;
mod io;

use std::collections::HashMap;
use std::path::PathBuf;

pub use claude::{claude_project_slug, claude_projects_root};
pub use codex::{codex_session_index_path, codex_sessions_root, find_codex_rollout, ids_for_thread_name, latest_id_for_thread_name};

use crate::transcript::{adapt, adapter_for_provider, Cursor};

use self::claude::find_claude_jsonl;
use self::io::{adapt_tail, read_after, read_until};

#[derive(Debug)]
pub struct DiscoverError(pub String);

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for DiscoverError {}

#[derive(Clone, Debug, Default)]
pub struct TranscriptCursorQuery {
    pub session_id: Option<String>,
    pub offset: u64,
}

pub fn resolve_source(
    meta: &HashMap<String, String>,
) -> Result<(PathBuf, Option<String>, String), DiscoverError> {
    let provider = meta
        .get("provider")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("claude");
    let session_id = meta
        .get("session_id")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty());
    let transcript_id = meta
        .get("transcript_id")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty());
    let workdir = meta
        .get("dir")
        .or_else(|| meta.get("workdir"))
        .map(|s| s.as_str());
    match provider {
        "codex" => {
            let root = codex_sessions_root();
            let (path, id) = match find_codex_rollout(transcript_id, session_id, &root) {
                Ok(found) => found,
                Err(err) => {
                    let stale_uuid = session_id.is_some_and(|sid| {
                        uuid::Uuid::parse_str(sid)
                            .ok()
                            .is_some_and(|u| u.to_string() == sid.to_lowercase())
                    });
                    if stale_uuid {
                        if let Some(task) = meta.get("task").map(|s| s.as_str()).filter(|s| !s.is_empty())
                        {
                            find_codex_rollout(transcript_id, Some(task), &root)?
                        } else {
                            return Err(err);
                        }
                    } else {
                        return Err(err);
                    }
                }
            };
            Ok((path, Some(id), "codex-rollout".into()))
        }
        _ => {
            let sid = session_id.or(transcript_id).ok_or_else(|| {
                DiscoverError("session_id is required for Claude transcript discovery".into())
            })?;
            let path = find_claude_jsonl(sid, &claude_projects_root(), workdir)?;
            Ok((path, Some(sid.to_string()), "claude-jsonl".into()))
        }
    }
}

pub fn transcript_document(
    meta: &HashMap<String, String>,
    after: Option<&TranscriptCursorQuery>,
    tail: Option<u32>,
    until_offset: Option<u64>,
    check_pane: bool,
) -> Result<hive_protocol::TranscriptDoc, DiscoverError> {
    let (path, session_id, adapter) = resolve_source(meta)?;
    let tmux = meta.get("tmux").cloned().unwrap_or_else(|| {
        crate::tmux::resolve_tmux(
            meta.get("task").map(|s| s.as_str()).unwrap_or(""),
            None,
        )
    });
    let blocked = if check_pane {
        crate::tmux::blocked_on_prompt(&tmux)
    } else {
        false
    };

    let mut reset = false;
    let mut has_earlier = false;
    let mut earlier_until = None;

    let (blocks, advanced) = if let Some(until) = until_offset {
        let data = read_until(&path, until)?;
        let (b, c) = adapt(
            &adapter,
            &data,
            Cursor {
                session_id: session_id.clone(),
                offset: 0,
            },
            false,
        )
        .map_err(|e| DiscoverError(e.0))?;
        (
            b,
            Cursor {
                session_id: c.session_id.or(session_id.clone()),
                offset: until,
            },
        )
    } else if let Some(n) = tail.filter(|n| *n > 0) {
        let use_tail = after.map(|a| a.offset == 0).unwrap_or(true);
        if use_tail {
            reset = true;
            let (b, c, earlier, until) =
                adapt_tail(&path, session_id.as_deref(), &adapter, n as usize)?;
            has_earlier = earlier;
            earlier_until = Some(until);
            (b, c)
        } else {
            read_after(&path, &adapter, session_id.clone(), after, &mut reset)?
        }
    } else {
        read_after(&path, &adapter, session_id.clone(), after, &mut reset)?
    };

    if let Some(cur) = after {
        if let (Some(a), Some(b)) = (&cur.session_id, &session_id) {
            if a != b {
                reset = true;
            }
        }
    }

    Ok(hive_protocol::TranscriptDoc {
        session_id,
        reset,
        blocked_on_prompt: blocked,
        cursor: hive_protocol::TranscriptCursor {
            session_id: advanced.session_id,
            offset: advanced.offset,
        },
        blocks: blocks.into_iter().map(|b| b.as_chat()).collect(),
        has_earlier,
        earlier_until,
    })
}

pub fn adapter_name(provider: &str) -> &'static str {
    adapter_for_provider(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_missing_session() {
        let meta = HashMap::from([("provider".into(), "claude".into())]);
        let err = resolve_source(&meta).unwrap_err();
        assert!(err.0.contains("session_id is required"));
    }

    #[test]
    fn tail_respects_has_earlier() {
        let sid = "11111111-2222-3333-4444-555555555555";
        let root = std::env::temp_dir().join(format!(
            "hive-discover-tail-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let proj = root.join("-tmp-x");
        std::fs::create_dir_all(&proj).unwrap();
        let dest = proj.join(format!("{sid}.jsonl"));
        std::fs::copy(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/transcript-claude-session.jsonl"),
            &dest,
        )
        .unwrap();
        // Pad file so tail window starts after byte 0.
        let mut padded = std::fs::read(&dest).unwrap();
        padded.splice(0..0, vec![b' '; 300_000]);
        std::fs::write(&dest, &padded).unwrap();

        std::env::set_var("CLAUDE_PROJECTS_ROOT", &root);
        let mut meta = HashMap::new();
        meta.insert("provider".into(), "claude".into());
        meta.insert("session_id".into(), sid.into());
        meta.insert("dir".into(), "/tmp/x".into());
        meta.insert("task".into(), "demo".into());

        let doc =
            transcript_document(&meta, None, Some(2), None, false).expect("tail transcript");
        assert!(doc.has_earlier, "expected has_earlier when file is padded");
        assert!(doc.earlier_until.unwrap_or(0) > 0);

        let _ = std::fs::remove_dir_all(root);
    }
}
