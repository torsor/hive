use super::Block;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub(crate) fn parse_obj(line: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(line).ok()?;
    v.as_object().cloned().map(Value::Object)
}

pub(crate) fn user_text(text: &str) -> Option<String> {
    let stripped = text.trim();
    for tag in ["local-command-stdout", "task-notification"] {
        if stripped.starts_with(&format!("<{tag}>")) {
            return None;
        }
    }
    static COMMAND_RE: OnceLock<Regex> = OnceLock::new();
    let re = COMMAND_RE.get_or_init(|| {
        Regex::new(
            r"(?s)^\s*(?:<command-message>.*?</command-message>\s*)?<command-name>(?P<name>[^<]*)</command-name>\s*(?:<command-message>.*?</command-message>\s*)?(?:<command-args>(?P<args>.*?)</command-args>\s*)?$",
        )
        .expect("command re")
    });
    if let Some(caps) = re.captures(stripped) {
        let name = caps.name("name").map(|m| m.as_str().trim()).unwrap_or("");
        let args = caps.name("args").map(|m| m.as_str().trim()).unwrap_or("");
        let shown = format!("{name} {args}").trim().to_string();
        return if shown.is_empty() { None } else { Some(shown) };
    }
    Some(text.to_string())
}

pub(crate) fn basename(value: &Value) -> String {
    value
        .as_str()
        .map(|s| {
            s.trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default()
}

pub(crate) fn first_line(value: &Value, limit: usize) -> String {
    let Some(s) = value.as_str() else {
        return String::new();
    };
    let head = s.trim().lines().next().unwrap_or("").to_string();
    if head.len() <= limit {
        head
    } else {
        format!("{}…", &head[..limit.saturating_sub(1)])
    }
}

pub(crate) fn label(name: &str, arguments: &Value) -> String {
    let args = arguments.as_object();
    let get = |k: &str| args.and_then(|m| m.get(k)).cloned().unwrap_or(Value::Null);
    match name {
        "Bash" => format!("ran {}", first_line(&get("command"), 72)).trim().into(),
        "Read" => format!("read {}", basename(&get("file_path"))).trim().into(),
        "Edit" => format!("edited {}", basename(&get("file_path"))).trim().into(),
        "Write" => format!("wrote {}", basename(&get("file_path"))).trim().into(),
        "Glob" | "Grep" => format!("searched {}", first_line(&get("pattern"), 72))
            .trim()
            .into(),
        "Agent" | "Task" => format!("dispatched {}", first_line(&get("description"), 72))
            .trim()
            .into(),
        "WebFetch" => format!("fetched {}", first_line(&get("url"), 72)).trim().into(),
        "Skill" => format!("used the {} skill", first_line(&get("skill"), 72))
            .trim()
            .into(),
        _ => format!("used {name}"),
    }
}

pub(crate) struct Reduction {
    pub include_thinking: bool,
    pub blocks: Vec<Block>,
    pub tools: std::collections::HashMap<String, usize>,
    pub session_id: Option<String>,
}

impl Reduction {
    pub(crate) fn mark_failed(&mut self, tool_id: &str) {
        if let Some(&index) = self.tools.get(tool_id) {
            self.blocks[index].failed = true;
            self.blocks[index].id = Some(tool_id.to_string());
            return;
        }
        self.blocks.push(Block {
            kind: "tool".into(),
            role: "assistant".into(),
            text: String::new(),
            ts: None,
            id: Some(tool_id.into()),
            failed: true,
        });
    }
}

pub(crate) fn reduce_item(
    state: &mut Reduction,
    item: &serde_json::Map<String, Value>,
    role: &str,
    timestamp: Option<String>,
) {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match item_type {
        "text" => {
            let Some(text) = item.get("text").and_then(|v| v.as_str()) else {
                return;
            };
            if text.trim().is_empty() {
                return;
            }
            let shown = if role == "user" {
                user_text(text)
            } else {
                Some(text.to_string())
            };
            if let Some(shown) = shown {
                if !shown.trim().is_empty() {
                    state
                        .blocks
                        .push(Block::text("text", role, shown, timestamp));
                }
            }
        }
        "thinking" => {
            if !state.include_thinking {
                return;
            }
            if let Some(text) = item.get("thinking").and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    state
                        .blocks
                        .push(Block::text("thinking", role, text.to_string(), timestamp));
                }
            }
        }
        "tool_use" => {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("a tool");
            let arguments = item.get("input").cloned().unwrap_or(Value::Null);
            let tool_id = item.get("id").map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            });
            let lbl = label(name, &arguments);
            if let Some(ref id) = tool_id {
                state.tools.insert(id.clone(), state.blocks.len());
            }
            state.blocks.push(Block {
                kind: "tool".into(),
                role: "assistant".into(),
                text: lbl,
                ts: timestamp,
                id: tool_id,
                failed: false,
            });
        }
        "tool_result" => {
            if item.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false) {
                if let Some(id) = item.get("tool_use_id").and_then(|v| v.as_str()) {
                    state.mark_failed(id);
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn is_environment_context(text: &str) -> bool {
    text.trim_start().starts_with("<environment_context>")
}
