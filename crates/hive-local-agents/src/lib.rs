//! Laptop-side agent bookmarks for hive-panel **Local** section (`local-agents.json`).

use std::path::{Path, PathBuf};

use chrono::Utc;
use hive_common::HiveHome;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FILE_NAME: &str = "local-agents.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),
    #[error("agent not found: {0}")]
    NotFound(String),
    #[error("agent already exists: {0}")]
    Exists(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalAgentDoc {
    pub version: u32,
    #[serde(default)]
    pub agents: Vec<LocalAgent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalAgent {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub resume: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agentmsg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct LocalAgentInput {
    pub title: String,
    pub cwd: String,
    pub resume: String,
    pub notes: Option<String>,
    pub agentmsg: Option<String>,
    pub provider: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LocalAgentPatch {
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub resume: Option<String>,
    pub notes: Option<Option<String>>,
    pub agentmsg: Option<Option<String>>,
    pub provider: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
}

pub fn registry_path(home: &HiveHome) -> PathBuf {
    home.root.join(FILE_NAME)
}

pub fn load(home: &HiveHome) -> Result<LocalAgentDoc, Error> {
    load_path(&registry_path(home))
}

pub fn load_path(path: &Path) -> Result<LocalAgentDoc, Error> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(LocalAgentDoc {
            version: SCHEMA_VERSION,
            agents: vec![],
        });
    };
    let mut doc: LocalAgentDoc = serde_json::from_str(&text).unwrap_or(LocalAgentDoc {
        version: SCHEMA_VERSION,
        agents: vec![],
    });
    if doc.version == 0 {
        doc.version = SCHEMA_VERSION;
    }
    doc.agents.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(doc)
}

pub fn save(home: &HiveHome, doc: &LocalAgentDoc) -> Result<(), Error> {
    save_path(&registry_path(home), doc)
}

pub fn save_path(path: &Path, doc: &LocalAgentDoc) -> Result<(), Error> {
    let mut doc = doc.clone();
    doc.version = SCHEMA_VERSION;
    doc.agents.sort_by(|a, b| a.id.cmp(&b.id));
    for a in &doc.agents {
        validate_agent_record(a)?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(&doc)? + "\n";
    std::fs::write(path, body)?;
    Ok(())
}

pub fn add(home: &HiveHome, id: &str, input: LocalAgentInput) -> Result<LocalAgent, Error> {
    let id = normalize_id(id)?;
    let mut doc = load(home)?;
    if doc.agents.iter().any(|a| a.id == id) {
        return Err(Error::Exists(id));
    }
    let agent = build_agent(id, input)?;
    doc.agents.push(agent.clone());
    save(home, &doc)?;
    Ok(agent)
}

pub fn register(home: &HiveHome, id: &str, input: LocalAgentInput) -> Result<LocalAgent, Error> {
    let id = normalize_id(id)?;
    let mut doc = load(home)?;
    let agent = build_agent(id.clone(), input)?;
    if let Some(slot) = doc.agents.iter_mut().find(|a| a.id == id) {
        *slot = agent.clone();
    } else {
        doc.agents.push(agent.clone());
    }
    save(home, &doc)?;
    Ok(agent)
}

pub fn set(home: &HiveHome, id: &str, patch: LocalAgentPatch) -> Result<LocalAgent, Error> {
    let id = normalize_id(id)?;
    let mut doc = load(home)?;
    let idx = doc
        .agents
        .iter()
        .position(|a| a.id == id)
        .ok_or_else(|| Error::NotFound(id.clone()))?;
    let mut agent = doc.agents[idx].clone();
    apply_patch(&mut agent, patch)?;
    validate_agent_record(&agent)?;
    doc.agents[idx] = agent.clone();
    save(home, &doc)?;
    Ok(agent)
}

pub fn remove(home: &HiveHome, id: &str) -> Result<LocalAgent, Error> {
    let id = normalize_id(id)?;
    let mut doc = load(home)?;
    let pos = doc
        .agents
        .iter()
        .position(|a| a.id == id)
        .ok_or_else(|| Error::NotFound(id.clone()))?;
    let removed = doc.agents.remove(pos);
    save(home, &doc)?;
    Ok(removed)
}

pub fn get(home: &HiveHome, id: &str) -> Result<LocalAgent, Error> {
    let id = normalize_id(id)?;
    load(home)?
        .agents
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| Error::NotFound(id))
}

pub fn open_shell_line(agent: &LocalAgent) -> String {
    format!(
        "cd {} && {}",
        shell_quote(&agent.cwd),
        agent.resume.trim()
    )
}

/// Filter agents by tags. `match_all`: agent must have every tag; otherwise any tag matches.
pub fn agents_matching_tags<'a>(
    agents: &'a [LocalAgent],
    tags: &[String],
    match_all: bool,
) -> Vec<&'a LocalAgent> {
    if tags.is_empty() {
        return agents.iter().collect();
    }
    agents
        .iter()
        .filter(|a| {
            if match_all {
                tags.iter().all(|t| a.tags.iter().any(|at| at == t))
            } else {
                tags.iter().any(|t| a.tags.iter().any(|at| at == t))
            }
        })
        .collect()
}

fn apply_patch(agent: &mut LocalAgent, patch: LocalAgentPatch) -> Result<(), Error> {
    if let Some(title) = patch.title {
        agent.title = validate_title(&title)?;
    }
    if let Some(cwd) = patch.cwd {
        agent.cwd = validate_cwd(&cwd)?;
    }
    if let Some(resume) = patch.resume {
        agent.resume = validate_resume(&resume)?;
    }
    if let Some(notes) = patch.notes {
        agent.notes = optional_text(notes, 4096)?;
    }
    if let Some(agentmsg) = patch.agentmsg {
        agent.agentmsg = optional_agentmsg(agentmsg)?;
    }
    if let Some(provider) = patch.provider {
        agent.provider = optional_provider(provider)?;
    }
    if let Some(tags) = patch.tags {
        agent.tags = normalize_tags(
            tags.into_iter()
                .map(|t| normalize_tag(&t))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    agent.updated_at = Utc::now().to_rfc3339();
    Ok(())
}

fn build_agent(id: String, input: LocalAgentInput) -> Result<LocalAgent, Error> {
    let agent = LocalAgent {
        id,
        title: validate_title(&input.title)?,
        cwd: validate_cwd(&input.cwd)?,
        resume: validate_resume(&input.resume)?,
        notes: optional_text(input.notes, 4096)?,
        agentmsg: optional_agentmsg(input.agentmsg)?,
        provider: optional_provider(input.provider)?,
        tags: normalize_tags(
            input
                .tags
                .into_iter()
                .map(|t| normalize_tag(&t))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        updated_at: Utc::now().to_rfc3339(),
    };
    validate_agent_record(&agent)?;
    Ok(agent)
}

fn validate_agent_record(a: &LocalAgent) -> Result<(), Error> {
    normalize_id(&a.id)?;
    validate_title(&a.title)?;
    validate_cwd(&a.cwd)?;
    validate_resume(&a.resume)?;
    optional_text(a.notes.clone(), 4096)?;
    optional_agentmsg(a.agentmsg.clone())?;
    optional_provider(a.provider.clone())?;
    for t in &a.tags {
        normalize_tag(t)?;
    }
    Ok(())
}

pub fn normalize_id(raw: &str) -> Result<String, Error> {
    let t = raw.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err(Error::Validation("id must not be empty".into()));
    }
    if t.len() > 64 {
        return Err(Error::Validation("id too long (max 64)".into()));
    }
    if !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(Error::Validation(
            "id may only contain letters, digits, -, _".into(),
        ));
    }
    Ok(t)
}

fn validate_title(raw: &str) -> Result<String, Error> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(Error::Validation("title must not be empty".into()));
    }
    if t.len() > 120 {
        return Err(Error::Validation("title too long (max 120)".into()));
    }
    if t.contains('\n') || t.contains('\r') {
        return Err(Error::Validation("title must be a single line".into()));
    }
    Ok(t.to_string())
}

fn validate_cwd(raw: &str) -> Result<String, Error> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(Error::Validation("cwd must not be empty".into()));
    }
    let path = Path::new(t);
    if !path.is_absolute() {
        return Err(Error::Validation("cwd must be an absolute path".into()));
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| Error::Validation(format!("cwd not accessible: {e}")))?;
    Ok(canonical.display().to_string())
}

fn validate_resume(raw: &str) -> Result<String, Error> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(Error::Validation("resume must not be empty".into()));
    }
    if t.len() > 2048 {
        return Err(Error::Validation("resume too long (max 2048)".into()));
    }
    if t.contains('\n') || t.contains('\r') {
        return Err(Error::Validation("resume must be a single line".into()));
    }
    Ok(t.to_string())
}

fn optional_text(raw: Option<String>, max: usize) -> Result<Option<String>, Error> {
    let Some(s) = raw else {
        return Ok(None);
    };
    if s.len() > max {
        return Err(Error::Validation(format!("text too long (max {max})")));
    }
    Ok(Some(s))
}

fn optional_agentmsg(raw: Option<String>) -> Result<Option<String>, Error> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.len() > 128 {
        return Err(Error::Validation("agentmsg too long (max 128)".into()));
    }
    Ok(Some(t.to_string()))
}

const PROVIDERS: &[&str] = &["cursor", "composer", "claude", "codex", "other"];

fn optional_provider(raw: Option<String>) -> Result<Option<String>, Error> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Ok(None);
    }
    if !PROVIDERS.contains(&t.as_str()) {
        return Err(Error::Validation(format!(
            "provider must be one of: {}",
            PROVIDERS.join(", ")
        )));
    }
    Ok(Some(t))
}

pub fn normalize_tag(raw: &str) -> Result<String, Error> {
    let t = raw.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err(Error::Validation("tag must not be empty".into()));
    }
    if t.len() > 32 {
        return Err(Error::Validation("tag too long (max 32)".into()));
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Error::Validation(
            "tag may only contain letters, digits, -, _".into(),
        ));
    }
    Ok(t)
}

pub fn normalize_tags(mut tags: Vec<String>) -> Vec<String> {
    tags.sort();
    tags.dedup();
    if tags.iter().any(|t| t == "evaluation") && !tags.iter().any(|t| t == "room") {
        tags.push("room".into());
        tags.sort();
    }
    tags
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hive_common::HiveHome;

    fn temp_home() -> HiveHome {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("hive-local-agents-{n}-{}", uuid_simple()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        HiveHome { root: dir }
    }

    fn uuid_simple() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    fn sample_input(cwd: &Path) -> LocalAgentInput {
        LocalAgentInput {
            title: "demo".into(),
            cwd: cwd.display().to_string(),
            resume: "cursor-agent resume".into(),
            notes: None,
            agentmsg: Some("agent:demo".into()),
            provider: Some("cursor".into()),
            tags: vec!["hive".into()],
        }
    }

    #[test]
    fn register_upserts() {
        let home = temp_home();
        let cwd = home.root.clone();
        register(&home, "demo", sample_input(&cwd)).unwrap();
        let mut input = sample_input(&cwd);
        input.title = "demo v2".into();
        register(&home, "demo", input).unwrap();
        let got = get(&home, "demo").unwrap();
        assert_eq!(got.title, "demo v2");
    }

    #[test]
    fn add_rejects_duplicate() {
        let home = temp_home();
        let cwd = home.root.clone();
        add(&home, "x", sample_input(&cwd)).unwrap();
        assert!(matches!(
            add(&home, "x", sample_input(&cwd)),
            Err(Error::Exists(_))
        ));
    }

    #[test]
    fn filter_tags_match_all() {
        let a = LocalAgent {
            id: "a".into(),
            title: "a".into(),
            cwd: "/tmp".into(),
            resume: "x".into(),
            notes: None,
            agentmsg: None,
            provider: None,
            tags: vec!["team".into(), "laptop".into()],
            updated_at: String::new(),
        };
        let b = LocalAgent {
            id: "b".into(),
            tags: vec!["team".into()],
            ..a.clone()
        };
        let list = [a, b];
        assert_eq!(
            agents_matching_tags(&list, &["team".into(), "laptop".into()], true).len(),
            1
        );
        assert_eq!(
            agents_matching_tags(&list, &["team".into()], false).len(),
            2
        );
    }

    #[test]
    fn evaluation_tag_adds_room() {
        let home = temp_home();
        let cwd = home.root.clone();
        let mut input = sample_input(&cwd);
        input.tags = vec!["evaluation".into()];
        register(&home, "ev", input).unwrap();
        assert_eq!(get(&home, "ev").unwrap().tags, vec!["evaluation", "room"]);
    }
}
