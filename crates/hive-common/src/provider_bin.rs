use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// PATH for hive-host systemd units and tmux-spawned agents.
pub fn operator_path_env(home: &Path) -> String {
    search_dirs(home)
        .into_iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(":")
}

fn search_dirs(home: &Path) -> Vec<PathBuf> {
    let mut out = vec![home.join(".local/bin")];
    let nvm_root = home.join(".nvm/versions/node");
    if let Ok(rd) = std::fs::read_dir(&nvm_root) {
        let mut vers: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.join("bin").is_dir())
            .collect();
        vers.sort();
        for v in vers {
            out.push(v.join("bin"));
        }
    }
    out.extend([
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);
    out
}

fn env_override(name: &str) -> Option<String> {
    let key = match name {
        "claude" => "CLAUDE_BIN",
        "codex" => "CODEX_BIN",
        _ => return None,
    };
    std::env::var(key)
        .ok()
        .filter(|p| !p.is_empty())
        .filter(|p| Path::new(p).is_file())
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        return std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Resolve an absolute path to `claude` or `codex` for tmux spawn.
pub fn resolve_provider_bin(name: &str) -> Result<String, String> {
    if let Some(p) = env_override(name) {
        return Ok(p);
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let home = PathBuf::from(&home);
    for dir in search_dirs(&home) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Ok(candidate.display().to_string());
        }
    }
    let env_key = match name {
        "claude" => "CLAUDE_BIN",
        "codex" => "CODEX_BIN",
        _ => "PROVIDER_BIN",
    };
    Err(format!(
        "{name} not found (install under ~/.local/bin or set {env_key})"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_path_includes_local_bin() {
        let home = PathBuf::from("/home/me");
        let path = operator_path_env(&home);
        assert!(path.starts_with("/home/me/.local/bin:"));
        assert!(path.contains("/usr/bin"));
    }
}
