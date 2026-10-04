//! macOS: open a login shell running one command (local agents).

use anyhow::{Context, Result};

pub fn launch_shell_command(label: &str, command: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        return launch_macos_terminal(label, command);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (label, command);
        anyhow::bail!("--launch is only supported on macOS; run the printed command manually");
    }
}

#[cfg(target_os = "macos")]
fn launch_macos_terminal(label: &str, command: &str) -> Result<()> {
    let inner = applescript_escape(command);
    let label = applescript_escape(label);
    let script = format!(
        r#"set theCommand to "{inner}"
set shellCommand to "/bin/zsh -c " & quoted form of theCommand
tell application "Terminal"
  activate
  do script shellCommand
  set custom title of front window to "{label}"
end tell"#
    );
    let st = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .context("run osascript for Terminal.app")?;
    if !st.success() {
        anyhow::bail!("Terminal.app launch failed");
    }
    Ok(())
}

fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
