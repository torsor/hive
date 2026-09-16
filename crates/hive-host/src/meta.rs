use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

/// Parse a hive `*.meta` file written with `printf '%q'` assignments.
pub fn load_meta(path: &Path) -> Result<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(parse_meta(&text))
}

pub fn parse_meta(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap();
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        out.insert(key.to_string(), unquote(value.trim()));
    }
    out
}

fn unquote(value: &str) -> String {
    if let Some(parts) = shlex::split(value) {
        if parts.len() == 1 {
            return parts.into_iter().next().unwrap();
        }
        if !parts.is_empty() {
            return parts.join(" ");
        }
    }
    value.trim_matches('\'').replace("'\\''", "'")
}

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn write_meta(path: &Path, fields: &[(&str, &str)]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut body = String::new();
    for (k, v) in fields {
        body.push_str(k);
        body.push('=');
        body.push_str(&shell_quote(v));
        body.push('\n');
    }
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))
}
