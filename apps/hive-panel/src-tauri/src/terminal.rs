//! Open an external terminal with hub-provided attach argv. Not a cc-* wrapper.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalMacos {
    AppleTerminal,
    Iterm2,
    Ghostty,
    Warp,
}

impl Default for TerminalMacos {
    fn default() -> Self {
        Self::Iterm2
    }
}

impl TerminalMacos {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "apple_terminal" | "terminal" => Self::AppleTerminal,
            "ghostty" => Self::Ghostty,
            "warp" => Self::Warp,
            _ => Self::Iterm2,
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Quote for the remote shell on the SSH host (not the local macOS launcher).
fn remote_shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn command_line(argv: &[String]) -> String {
    argv.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
}

/// One pasteable `ssh host -t '…'` line for Termius / Terminal.
pub fn ssh_command_line(argv: &[String]) -> String {
    if argv.first().map(String::as_str) == Some("ssh")
        && argv.get(2).map(String::as_str) == Some("-t")
        && argv.len() >= 4
    {
        format!("ssh {} -t {}", shell_quote(&argv[1]), shell_quote(&argv[3]))
    } else {
        command_line(argv)
    }
}

/// SSH to `host` and start a login shell in `dir` (no tmux attach).
pub fn ssh_shell_at_argv(host: &str, dir: &str) -> Vec<String> {
    let dir = dir.trim();
    let remote = if dir.is_empty() {
        "exec ${SHELL:-/bin/zsh} -l".to_string()
    } else {
        format!(
            "cd {} && exec ${{SHELL:-/bin/zsh}} -l",
            remote_shell_quote(dir)
        )
    };
    vec![
        "ssh".into(),
        host.into(),
        "-t".into(),
        remote,
    ]
}

/// Open a plain SSH shell at `dir` on `host` (macOS external terminal).
pub fn open_shell_at(host: &str, dir: &str, label: &str, terminal: TerminalMacos) -> Result<()> {
    let argv = ssh_shell_at_argv(host, dir);
    open_attach(&argv, label, terminal)
}

/// Run `command` in a new external terminal (local laptop — no SSH).
pub fn open_zsh_command(command: &str, label: &str, terminal: TerminalMacos) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        return open_macos_zsh(command, label, terminal);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (label, terminal);
        let st = std::process::Command::new("/bin/zsh")
            .args(["-c", command])
            .spawn()
            .context("spawn local zsh")?;
        let _ = st;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn open_macos_zsh(command: &str, label: &str, terminal: TerminalMacos) -> Result<()> {
    let inner = applescript_escape(command);
    let label = applescript_escape(label);
    let cmd_setup = format!(
        r#"set attachLabel to "{label}"
set theCommand to "{inner}"
set shellCommand to "/bin/zsh -c " & quoted form of theCommand"#
    );
    match terminal {
        TerminalMacos::AppleTerminal => run_osascript(&format!(
            r#"{cmd_setup}
tell application "Terminal"
  activate
  do script shellCommand
  set custom title of front window to attachLabel
end tell"#
        ))
        .context("Terminal.app")?,
        TerminalMacos::Iterm2 => run_osascript(&format!(
            r#"{cmd_setup}
tell application "iTerm"
  activate
  create window with default profile command shellCommand
end tell"#
        ))
        .context("iTerm2")?,
        TerminalMacos::Ghostty | TerminalMacos::Warp => {
            let shell_cmd = format!("/bin/zsh -c {}", shell_quote(command));
            let app = match terminal {
                TerminalMacos::Ghostty => "Ghostty",
                TerminalMacos::Warp => "Warp",
                _ => unreachable!(),
            };
            let st = std::process::Command::new("open")
                .args(["-na", app, "--args", "-e", &shell_cmd])
                .status()
                .with_context(|| format!("open {app}"))?;
            if !st.success() {
                anyhow::bail!("open {app} failed");
            }
        }
    }
    Ok(())
}

fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Run hub attach argv in a new terminal window (macOS).
pub fn open_attach(argv: &[String], label: &str, terminal: TerminalMacos) -> Result<()> {
    if argv.is_empty() {
        anyhow::bail!("empty attach command");
    }
    #[cfg(target_os = "macos")]
    {
        return open_macos(argv, label, terminal);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (label, terminal);
        let st = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .spawn()
            .with_context(|| format!("spawn {}", argv[0]))?;
        let _ = st;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Result<()> {
    let st = std::process::Command::new("osascript")
        .args(["-e", script])
        .status()
        .context("run osascript")?;
    if !st.success() {
        anyhow::bail!("osascript exited with {}", st.code().unwrap_or(-1));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_macos(argv: &[String], label: &str, terminal: TerminalMacos) -> Result<()> {
    let inner = applescript_escape(&ssh_command_line(argv));
    let label = applescript_escape(label);
    let cmd_setup = format!(
        r#"set attachLabel to "{label}"
set theCommand to "{inner}"
set shellCommand to "/bin/zsh -c " & quoted form of theCommand"#
    );
    match terminal {
        TerminalMacos::AppleTerminal => {
            run_osascript(&format!(
                r#"{cmd_setup}
tell application "Terminal"
  activate
  do script shellCommand
  set custom title of front window to attachLabel
end tell"#
            ))
            .context("Terminal.app")?;
        }
        TerminalMacos::Iterm2 => {
            run_osascript(&format!(
                r#"{cmd_setup}
set itermRunning to false
tell application "System Events"
  set itermRunning to (count (every process whose bundle identifier is "com.googlecode.iterm2")) > 0
end tell
tell application "iTerm"
  activate
  if not itermRunning then delay 0.5
  if (count of windows) > 0 then
    tell current window
      create tab with default profile command shellCommand
      tell current tab
        tell current session
          set name to attachLabel
        end tell
      end tell
    end tell
  else
    create window with default profile command shellCommand
    tell current window
      tell current tab
        tell current session
          set name to attachLabel
        end tell
      end tell
    end tell
  end if
end tell"#
            ))
            .context("iTerm2")?;
        }
        TerminalMacos::Ghostty | TerminalMacos::Warp => {
            let shell_cmd = format!("/bin/zsh -c {}", shell_quote(&ssh_command_line(argv)));
            let app = match terminal {
                TerminalMacos::Ghostty => "Ghostty",
                TerminalMacos::Warp => "Warp",
                _ => unreachable!(),
            };
            let st = std::process::Command::new("open")
                .args(["-na", app, "--args", "-e", &shell_cmd])
                .status()
                .with_context(|| format!("open {app}"))?;
            if !st.success() {
                anyhow::bail!("open {app} failed with status {:?}", st.code());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_shell_at_quotes_directory() {
        let argv = ssh_shell_at_argv("hub", "/home/me/my project");
        assert_eq!(argv[0], "ssh");
        assert_eq!(argv[1], "hub");
        assert_eq!(argv[2], "-t");
        assert!(argv[3].contains("cd '/home/me/my project'"));
        assert!(argv[3].contains("exec ${SHELL:-/bin/zsh} -l"));
    }

    #[test]
    fn ssh_command_line_quotes_remote_once() {
        let argv = ssh_shell_at_argv("box-a", "/home/me/my project");
        let line = ssh_command_line(&argv);
        assert_eq!(
            line,
            "ssh 'box-a' -t 'cd '\\''/home/me/my project'\\'' && exec ${SHELL:-/bin/zsh} -l'"
        );
    }
}
