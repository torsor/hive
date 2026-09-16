use hive_common::HiveHome;
use crate::sessions;
use crate::tmux;

const MAX_LEN: usize = 16000;
const HUMAN_SECS: u64 = 2;

pub fn say(home: &HiveHome, task: &str, text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("empty say text".into());
    }
    if text.len() > MAX_LEN {
        return Err(format!("say text longer than {MAX_LEN} bytes"));
    }
    let meta = sessions::load_task_meta(home, task)?;
    let provider = meta
        .get("provider")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("claude");
    if provider != "claude" && provider != "codex" {
        return Err(format!("{provider} chat read is not available"));
    }
    let tmux_name = tmux::resolve_tmux(task, meta.get("tmux").map(|s| s.as_str()));
    if !tmux::has_session(&tmux_name) {
        return Err(format!("session {tmux_name} is not running"));
    }
    if tmux::client_activity_age_secs(&tmux_name).is_some_and(|age| age < HUMAN_SECS) {
        return Err("a human is attached with recent activity; not pasting".into());
    }
    // Clear partial input, paste text, then submit Enter in a separate send-keys (Claude TUI
    // treats bundled text+Enter as paste and leaves the line staged at the prompt).
    tmux::send_keys(&tmux_name, &["Escape", "Escape"])?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    if text.contains('\n') {
        tmux::send_bracketed_paste_enter(&tmux_name, text)?;
    } else {
        tmux::send_literal_enter(&tmux_name, text)?;
    }
    Ok("said".into())
}
