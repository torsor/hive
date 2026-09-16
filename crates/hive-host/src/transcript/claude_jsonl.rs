use super::Block;
use super::common::{parse_obj, reduce_item, user_text, Reduction};

pub(crate) fn claude_jsonl(lines: &[String], include_thinking: bool) -> Reduction {
    let mut state = Reduction {
        include_thinking,
        blocks: Vec::new(),
        tools: Default::default(),
        session_id: None,
    };
    for line in lines {
        let Some(record) = parse_obj(line) else {
            continue;
        };
        if let Some(sid) = record.get("sessionId").and_then(|v| v.as_str()) {
            if !sid.is_empty() {
                state.session_id = Some(sid.to_string());
            }
        }
        let kind = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "user" && kind != "assistant" {
            continue;
        }
        if record.get("isSidechain").and_then(|v| v.as_bool()).unwrap_or(false)
            || record.get("isMeta").and_then(|v| v.as_bool()).unwrap_or(false)
        {
            continue;
        }
        let Some(message) = record.get("message") else {
            continue;
        };
        if !message.is_object() {
            continue;
        }
        let role = if kind == "user" { "user" } else { "assistant" };
        let timestamp = record
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let content = message.get("content");
        if let Some(s) = content.and_then(|v| v.as_str()) {
            if s.trim().is_empty() {
                continue;
            }
            let shown = if role == "user" {
                user_text(s)
            } else {
                Some(s.to_string())
            };
            if let Some(shown) = shown {
                if !shown.trim().is_empty() {
                    state
                        .blocks
                        .push(Block::text("text", role, shown, timestamp.clone()));
                }
            }
            continue;
        }
        let Some(arr) = content.and_then(|v| v.as_array()) else {
            continue;
        };
        for item in arr {
            let Some(obj) = item.as_object() else {
                continue;
            };
            reduce_item(&mut state, obj, role, timestamp.clone());
        }
    }
    state
}
