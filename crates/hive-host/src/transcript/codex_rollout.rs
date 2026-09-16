use super::Block;
use super::common::{first_line, is_environment_context, parse_obj, Reduction};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn codex_join_text_parts(content: &Value) -> Option<String> {
    let arr = content.as_array()?;
    let mut parts = Vec::new();
    for item in arr {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let ty = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !matches!(ty, "input_text" | "output_text" | "text") {
            continue;
        }
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            if text.trim().is_empty() || is_environment_context(text) {
                continue;
            }
            parts.push(text.to_string());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn unescape_json_string(value: &str) -> String {
    if let Ok(Value::String(s)) = serde_json::from_str::<Value>(&format!("\"{value}\"")) {
        s
    } else {
        value
            .replace("\\n", "\n")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    }
}

fn codex_label(name: &str, payload: &Value) -> String {
    if matches!(name, "exec" | "shell" | "exec_command") {
        if let Some(raw) = payload.get("input").and_then(|v| v.as_str()) {
            static CMD_RE: OnceLock<Regex> = OnceLock::new();
            let re = CMD_RE.get_or_init(|| {
                Regex::new(r#""cmd"\s*:\s*"((?:\\.|[^"\\])*)""#).expect("cmd re")
            });
            if let Some(caps) = re.captures(raw) {
                let cmd = unescape_json_string(&caps[1]);
                return format!("ran {}", first_line(&Value::String(cmd), 72))
                    .trim()
                    .into();
            }
        }
        if let Some(arguments) = payload.get("arguments").and_then(|v| v.as_str()) {
            if let Ok(parsed) = serde_json::from_str::<Value>(arguments) {
                if let Some(cmd) = parsed.get("cmd") {
                    return format!("ran {}", first_line(cmd, 72)).trim().into();
                }
            }
        }
        return format!("ran {name}");
    }
    if let Some(arguments) = payload.get("arguments").and_then(|v| v.as_str()) {
        if !arguments.trim().is_empty() {
            return format!(
                "used {name}: {}",
                first_line(&Value::String(arguments.to_string()), 72)
            )
            .trim()
            .into();
        }
    }
    format!("used {name}")
}

pub(crate) fn codex_rollout(lines: &[String]) -> Reduction {
    let mut state = Reduction {
        include_thinking: false,
        blocks: Vec::new(),
        tools: Default::default(),
        session_id: None,
    };
    let mut last_user: Option<String> = None;
    for line in lines {
        let Some(record) = parse_obj(line) else {
            continue;
        };
        let Some(payload) = record.get("payload") else {
            continue;
        };
        if !payload.is_object() {
            continue;
        }
        let timestamp = record
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let kind = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "session_meta" {
            for key in ["id", "session_id"] {
                if let Some(v) = payload.get(key).and_then(|v| v.as_str()) {
                    if !v.is_empty() {
                        state.session_id = Some(v.to_string());
                        break;
                    }
                }
            }
            continue;
        }
        if kind == "event_msg" {
            if payload.get("type").and_then(|v| v.as_str()) != Some("user_message") {
                continue;
            }
            let Some(text) = payload.get("message").and_then(|v| v.as_str()) else {
                continue;
            };
            if text.trim().is_empty() || is_environment_context(text) {
                continue;
            }
            if last_user.as_deref() == Some(text) {
                continue;
            }
            state
                .blocks
                .push(Block::text("text", "user", text.to_string(), timestamp));
            last_user = Some(text.to_string());
            continue;
        }
        if kind != "response_item" {
            continue;
        }
        let item_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if item_type == "message" {
            let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role == "developer" {
                continue;
            }
            if role != "user" && role != "assistant" {
                continue;
            }
            let Some(text) = payload.get("content").and_then(codex_join_text_parts) else {
                continue;
            };
            if role == "user" {
                if last_user.as_deref() == Some(text.as_str()) {
                    continue;
                }
                state
                    .blocks
                    .push(Block::text("text", "user", text.clone(), timestamp));
                last_user = Some(text);
            } else {
                state
                    .blocks
                    .push(Block::text("text", "assistant", text, timestamp));
            }
            continue;
        }
        if item_type == "custom_tool_call" || item_type == "function_call" {
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("a tool");
            let lbl = codex_label(name, payload);
            let tool_id = payload
                .get("call_id")
                .or_else(|| payload.get("id"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let status = payload
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let failed = status == "failed" || status == "error";
            if let Some(ref id) = tool_id {
                state.tools.insert(id.clone(), state.blocks.len());
            }
            state.blocks.push(Block {
                kind: "tool".into(),
                role: "assistant".into(),
                text: lbl,
                ts: timestamp,
                id: tool_id,
                failed,
            });
        }
    }
    state
}
