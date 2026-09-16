//! Claude JSONL + Codex rollout adapters (port of hive_providers.transcript).

mod claude_jsonl;
mod codex_rollout;
mod common;

use hive_protocol::ChatBlock;

#[derive(Debug, Clone, Default)]
pub struct Cursor {
    pub session_id: Option<String>,
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub kind: String,
    pub role: String,
    pub text: String,
    pub ts: Option<String>,
    pub id: Option<String>,
    pub failed: bool,
}

impl Block {
    pub(crate) fn text(kind: &str, role: &str, text: String, ts: Option<String>) -> Self {
        Self {
            kind: kind.into(),
            role: role.into(),
            text,
            ts,
            id: None,
            failed: false,
        }
    }

    pub fn as_chat(&self) -> ChatBlock {
        ChatBlock {
            kind: self.kind.clone(),
            role: self.role.clone(),
            text: self.text.clone(),
            ts: self.ts.clone(),
            id: self.id.clone(),
            failed: self.failed,
        }
    }
}

#[derive(Debug)]
pub struct AdaptError(pub String);

impl std::fmt::Display for AdaptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for AdaptError {}

pub fn complete_lines(data: &[u8]) -> (Vec<String>, usize) {
    let Some(end) = data.iter().rposition(|&b| b == b'\n') else {
        return (Vec::new(), 0);
    };
    let consumed = &data[..=end];
    let text = String::from_utf8_lossy(consumed);
    (text.lines().map(|s| s.to_string()).collect(), consumed.len())
}

pub fn adapt(
    adapter: &str,
    data: &[u8],
    cursor: Cursor,
    include_thinking: bool,
) -> Result<(Vec<Block>, Cursor), AdaptError> {
    let (lines, consumed) = complete_lines(data);
    let state = match adapter {
        "claude-jsonl" => claude_jsonl::claude_jsonl(&lines, include_thinking),
        "codex-rollout" => codex_rollout::codex_rollout(&lines),
        other => {
            return Err(AdaptError(format!(
                "unknown transcript adapter: {other}"
            )))
        }
    };
    Ok((
        state.blocks,
        Cursor {
            session_id: state.session_id.or(cursor.session_id),
            offset: cursor.offset + consumed as u64,
        },
    ))
}

pub fn adapter_for_provider(provider: &str) -> &'static str {
    match provider {
        "codex" => "codex-rollout",
        _ => "claude-jsonl",
    }
}
