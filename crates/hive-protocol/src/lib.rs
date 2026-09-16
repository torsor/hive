//! Shared JSON types for hive-host, hive-hub, hive CLI, and hive-panel.
//!
//! Keep this crate free of HTTP/SSH so every client serializes the same shape.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const API_PREFIX: &str = "/v1";
pub const DEFAULT_HOST_PORT: u16 = 8788;
pub const DEFAULT_HUB_PORT: u16 = 8787;
pub const DEFAULT_WEB_PORT: u16 = 8789;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub ok: bool,
    pub error: String,
    pub code: String,
}

impl ErrorBody {
    pub fn new(code: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: error.into(),
            code: code.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub ok: bool,
    pub role: String,
    pub version: String,
}

impl Health {
    pub fn host() -> Self {
        Self {
            ok: true,
            role: "host".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
    pub fn hub() -> Self {
        Self {
            ok: true,
            role: "hub".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    pub task: String,
    pub state: String,
    #[serde(default)]
    pub started: String,
    #[serde(default)]
    pub dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub starred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFleet {
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub sessions: Vec<SessionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fleet {
    pub hosts: Vec<HostFleet>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptCursor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatBlock {
    pub kind: String,
    pub role: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub reset: bool,
    pub blocked_on_prompt: bool,
    pub cursor: TranscriptCursor,
    pub blocks: Vec<ChatBlock>,
    #[serde(default)]
    pub has_earlier: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earlier_until: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpawnRequest {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SayRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelRequest {
    pub op: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkOutput {
    pub ok: bool,
    pub output: String,
}

impl OkOutput {
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            ok: true,
            output: output.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirListing {
    pub path: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachCommand {
    pub schema: String,
    pub host: String,
    pub task: String,
    pub terminal: String,
    pub command: Vec<String>,
}

impl AttachCommand {
    pub fn ssh_tmux(host: &str, task: &str, tmux: &str) -> Self {
        Self {
            schema: "hive-attach/v1".into(),
            host: host.into(),
            task: task.into(),
            terminal: "external-ssh".into(),
            command: vec![
                "ssh".into(),
                host.into(),
                "-t".into(),
                format!("tmux attach -t {tmux}"),
            ],
        }
    }

    /// Replace the SSH target with the fleet alias the client already knows.
    pub fn with_ssh_host(mut self, host: &str) -> Self {
        self.host = host.to_string();
        if self.command.len() >= 2 {
            self.command[1] = host.to_string();
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubEvent {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindingCandidate {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub current: bool,
    pub live: bool,
    pub suggested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BindingsDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested: Option<String>,
    pub candidates: Vec<BindingCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindRequest {
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_body_json() {
        let e = ErrorBody::new("not_found", "missing");
        let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["code"], "not_found");
        assert_eq!(v["error"], "missing");
    }

    #[test]
    fn attach_with_ssh_host_rewrites_command() {
        let att = AttachCommand::ssh_tmux("box.local", "demo", "hive-demo")
            .with_ssh_host("box");
        assert_eq!(att.host, "box");
        assert_eq!(att.command[1], "box");
    }
}
