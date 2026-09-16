use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::home::ensure_dir;

pub fn render_unit(description: &str, exec_start: &str) -> String {
    render_unit_with_path(description, exec_start, None)
}

pub fn render_unit_with_path(description: &str, exec_start: &str, path: Option<&str>) -> String {
    let env = path
        .filter(|p| !p.is_empty())
        .map(|p| format!("Environment=\"PATH={p}\"\n"))
        .unwrap_or_default();
    format!(
        r#"[Unit]
Description={description}
After=network.target

[Service]
Type=simple
{env}ExecStart={exec_start}
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
"#
    )
}

/// Quote a token for systemd `ExecStart=` (double-quote style).
fn quote(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '\\' | '"' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn exec_start(bin: &str, bind: &str, port: u16, home: &str, dev: bool) -> String {
    exec_start_with_extra(bin, bind, port, home, dev, &[])
}

pub fn exec_start_web(
    bin: &str,
    bind: &str,
    port: u16,
    home: &str,
    root: &str,
    dev: bool,
) -> String {
    exec_start_with_extra(bin, bind, port, home, dev, &[("--root", root)])
}

fn exec_start_with_extra(
    bin: &str,
    bind: &str,
    port: u16,
    home: &str,
    dev: bool,
    extra: &[(&str, &str)],
) -> String {
    let mut parts = vec![
        quote(bin),
        "serve".into(),
        "--bind".into(),
        quote(bind),
        "--port".into(),
        port.to_string(),
        "--home".into(),
        quote(home),
    ];
    for (flag, value) in extra {
        parts.push((*flag).into());
        parts.push(quote(value));
    }
    if dev {
        parts.push("--dev".into());
    }
    parts.join(" ")
}

pub fn resolve_daemon_bin(explicit: Option<&Path>, env_var: &str) -> Result<String> {
    if let Some(p) = explicit {
        return Ok(p.display().to_string());
    }
    if let Ok(p) = std::env::var(env_var) {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    Ok(std::env::current_exe()?.display().to_string())
}

pub fn install_user_service(service_filename: &str, unit_body: &str) -> Result<PathBuf> {
    let dir = PathBuf::from(std::env::var("HOME")?).join(".config/systemd/user");
    ensure_dir(&dir)?;
    let path = dir.join(service_filename);
    std::fs::write(&path, unit_body).with_context(|| format!("write {}", path.display()))?;
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", service_filename.trim_end_matches(".service")])
        .status();
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_contains_exec_start() {
        let exec = exec_start("/bin/hive-host", "127.0.0.1", 8788, "/tmp/h", false);
        let body = render_unit("hive-host test", &exec);
        assert!(body.contains("Description=hive-host test"));
        assert!(body.contains("ExecStart=\"/bin/hive-host\" serve"));
    }

    #[test]
    fn unit_with_path_sets_environment() {
        let exec = exec_start("/bin/hive-host", "127.0.0.1", 8788, "/tmp/h", false);
        let body = render_unit_with_path("hive-host test", &exec, Some("/home/me/.local/bin:/usr/bin"));
        assert!(body.contains("Environment=\"PATH=/home/me/.local/bin:/usr/bin\""));
    }

    #[test]
    fn exec_start_includes_dev_flag() {
        let exec = exec_start("/bin/hive-host", "127.0.0.1", 8788, "/tmp/h", true);
        assert!(exec.contains("--dev"));
    }

    #[test]
    fn exec_start_quotes_spaced_home() {
        let exec = exec_start("/bin/hive-host", "127.0.0.1", 8788, "/tmp/my home", true);
        assert!(exec.contains("--home \"/tmp/my home\""));
    }
}
