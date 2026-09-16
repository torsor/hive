use std::path::{Path, PathBuf};

use serde_json::Value;
use walkdir::WalkDir;

use super::DiscoverError;

pub fn claude_projects_root() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDE_PROJECTS_ROOT") {
        return PathBuf::from(p);
    }
    let home = std::env::var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".claude")
        });
    home.join("projects")
}

pub fn claude_project_slug(workdir: &str) -> String {
    let path = workdir.trim();
    if path.is_empty() {
        return String::new();
    }
    if let Some(rest) = path.strip_prefix('/') {
        format!("-{}", rest.replace('/', "-"))
    } else {
        path.replace('/', "-")
    }
}

fn stem_matches(path: &Path, session_id: &str) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s == session_id || s.contains(session_id))
}

fn file_has_session(path: &Path, session_id: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("sessionId").and_then(|x| x.as_str()) == Some(session_id) {
            return true;
        }
    }
    false
}

pub fn find_claude_jsonl(
    session_id: &str,
    projects_root: &Path,
    workdir: Option<&str>,
) -> Result<PathBuf, DiscoverError> {
    if session_id.is_empty() {
        return Err(DiscoverError(
            "session_id is required for Claude transcript discovery".into(),
        ));
    }
    if !projects_root.is_dir() {
        return Err(DiscoverError(format!(
            "no Claude projects tree at {}",
            projects_root.display()
        )));
    }
    let mut stem_hits = Vec::new();
    let mut content_hits = Vec::new();
    let walker = WalkDir::new(projects_root).min_depth(2).max_depth(2);
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if stem_matches(path, session_id) {
            stem_hits.push(path.to_path_buf());
        } else if file_has_session(path, session_id) {
            content_hits.push(path.to_path_buf());
        }
    }
    let mut matches = if stem_hits.is_empty() {
        content_hits
    } else {
        stem_hits
    };
    matches.sort();
    if matches.is_empty() {
        return Err(DiscoverError(format!(
            "transcript unavailable: no session file for session_id={session_id}"
        )));
    }
    if matches.len() > 1 {
        let slug = claude_project_slug(workdir.unwrap_or(""));
        if !slug.is_empty() {
            let scoped: Vec<_> = matches
                .iter()
                .filter(|p| {
                    p.parent()
                        .and_then(|d| d.file_name())
                        .and_then(|n| n.to_str())
                        == Some(slug.as_str())
                })
                .cloned()
                .collect();
            if scoped.len() == 1 {
                return Ok(scoped.into_iter().next().unwrap());
            }
            if !scoped.is_empty() {
                matches = scoped;
            }
        }
        return Err(DiscoverError(format!(
            "ambiguous transcript for session_id={session_id}: {} files match",
            matches.len()
        )));
    }
    Ok(matches.into_iter().next().unwrap())
}
