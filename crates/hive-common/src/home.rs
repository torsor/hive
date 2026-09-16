use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct HiveHome {
    pub root: PathBuf,
}

impl HiveHome {
    pub fn resolve(explicit: Option<&str>) -> Result<Self> {
        if let Some(p) = explicit {
            return Ok(Self {
                root: PathBuf::from(p),
            });
        }
        if let Ok(env) = std::env::var("HIVE_HOME") {
            return Ok(Self {
                root: PathBuf::from(env),
            });
        }
        let home = dirs_home()?;
        Ok(Self {
            root: home.join(".hive"),
        })
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    pub fn host_toml(&self) -> PathBuf {
        self.root.join("host.toml")
    }

    pub fn hub_toml(&self) -> PathBuf {
        self.root.join("hub.toml")
    }

    pub fn web_toml(&self) -> PathBuf {
        self.root.join("web.toml")
    }

    pub fn web_dist_dir(&self) -> PathBuf {
        self.root.join("web/dist")
    }

    pub fn client_toml(&self) -> PathBuf {
        self.root.join("client.toml")
    }

    pub fn hosts_file(&self) -> PathBuf {
        self.root.join("hosts")
    }

    /// Operator account home (`$HOME`), not the hive config root (`~/.hive`).
    pub fn operator_home(&self) -> Result<PathBuf> {
        dirs_home()
    }

    /// Legacy home (`~/.cc`) for optional one-shot migration.
    pub fn legacy_cc() -> PathBuf {
        dirs_home()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(".cc")
    }
}

fn dirs_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .context("HOME is unset")
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("mkdir {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_explicit_and_env() {
        let h = HiveHome::resolve(Some("/tmp/hive-test")).unwrap();
        assert_eq!(h.root, PathBuf::from("/tmp/hive-test"));
        assert_eq!(h.sessions_dir(), PathBuf::from("/tmp/hive-test/sessions"));
    }

    #[test]
    fn resolve_from_hive_home_env() {
        std::env::set_var("HIVE_HOME", "/tmp/env-hive");
        let h = HiveHome::resolve(None).unwrap();
        assert_eq!(h.root, PathBuf::from("/tmp/env-hive"));
    }
}
