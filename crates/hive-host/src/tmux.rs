use std::process::Command;

pub fn has_session(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn kill_session(name: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() || !has_session(name) {
        Ok(())
    } else {
        Err(format!("tmux kill-session {name} failed"))
    }
}

pub fn capture_last_line(name: &str) -> Option<String> {
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", name, "-S", "-15"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .last()
        .map(|s| s.replace('\t', " "))
}

pub fn capture_tail(name: &str, lines: i32) -> String {
    let out = Command::new("tmux")
        .args([
            "capture-pane",
            "-pt",
            name,
            "-S",
            &format!("-{lines}"),
        ])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

pub fn client_activity_age_secs(name: &str) -> Option<u64> {
    let out = Command::new("tmux")
        .args(["list-clients", "-t", &format!("={name}"), "-F", "#{client_activity}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let newest = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<u64>().ok())
        .max()?;
    Some(now.saturating_sub(newest))
}

pub fn new_session(name: &str, dir: &str, cmd: &str) -> Result<(), String> {
    let wrapped = format!(
        "{cmd}; echo; echo '[hive] agent exited — conversation saved. Ctrl-b d to detach.'; exec ${{SHELL:-/bin/sh}}"
    );
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-c", dir, &wrapped])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux new-session {name} failed"))
    }
}

pub fn send_literal(name: &str, text: &str) -> Result<(), String> {
    if text.starts_with('-') {
        send_keys(name, &["-l", "--", text])
    } else {
        send_keys(name, &["-l", text])
    }
}

const SUBMIT_SETTLE_MS: u64 = 120;

/// Paste literal text, wait for the TUI to absorb it, then submit in an isolated send-keys.
/// Claude Code/Codex TUIs often ignore Enter when it is bundled with the text paste.
pub fn send_literal_enter(name: &str, text: &str) -> Result<(), String> {
    send_literal(name, text)?;
    std::thread::sleep(std::time::Duration::from_millis(SUBMIT_SETTLE_MS));
    send_keys(name, &["Enter"])
}

pub fn send_bracketed_paste_enter(name: &str, text: &str) -> Result<(), String> {
    let paste = format!("\u{1b}[200~{text}\u{1b}[201~");
    send_keys(name, &["-l", &paste])?;
    std::thread::sleep(std::time::Duration::from_millis(SUBMIT_SETTLE_MS));
    send_keys(name, &["Enter"])
}

pub fn send_keys(name: &str, keys: &[&str]) -> Result<(), String> {
    let mut args = vec!["send-keys".to_string(), "-t".into(), name.into()];
    args.extend(keys.iter().map(|s| (*s).to_string()));
    let status = Command::new("tmux")
        .args(&args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("tmux send-keys failed".into())
    }
}

pub fn safe_task(task: &str) -> String {
    let cleaned: String = task
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    cleaned
}

pub fn default_tmux_name(task: &str) -> String {
    format!("hive-{}", safe_task(task))
}

pub fn resolve_tmux(task: &str, meta_tmux: Option<&str>) -> String {
    if let Some(name) = meta_tmux {
        if has_session(name) {
            return name.to_string();
        }
    }
    let hive_name = default_tmux_name(task);
    if has_session(&hive_name) {
        return hive_name;
    }
    meta_tmux
        .map(str::to_string)
        .unwrap_or(hive_name)
}

pub fn blocked_on_prompt(tmux: &str) -> bool {
    let pane = capture_tail(tmux, 20);
    let lower = pane.to_ascii_lowercase();
    lower.contains("do you want to proceed")
        || lower.contains("do you want to make this edit")
        || lower.contains("❯ 1. yes")
        || (lower.contains("1. yes") && lower.contains("2. no"))
}
