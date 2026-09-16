//! Real tmux session lifecycle (skipped when tmux is not installed).

use hive_host::tmux;

fn tmux_available() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn tmux_new_session_and_kill() {
    if !tmux_available() {
        eprintln!("skip tmux_new_session_and_kill: tmux not on PATH");
        return;
    }
    let name = format!("hive-test-{}", std::process::id());
    let dir = std::env::temp_dir();
    assert!(!tmux::has_session(&name));
    tmux::new_session(&name, &dir.display().to_string(), "sleep 30")
        .expect("new-session");
    assert!(tmux::has_session(&name));
    tmux::kill_session(&name).expect("kill-session");
    assert!(!tmux::has_session(&name));
}

#[test]
fn tmux_safe_task_sanitizes() {
    assert_eq!(tmux::safe_task("foo/bar"), "foo-bar");
    assert_eq!(tmux::safe_task("ok_name"), "ok_name");
}
