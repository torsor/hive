use crate::home::HiveHome;

/// One fleet host line from `~/.hive/hosts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    /// Short name for display and client routes (`box-a`).
    pub name: String,
    /// Poll target: bare hostname or `http://tailscale-ip:port`.
    pub target: String,
}

pub fn parse_host_line(line: &str) -> Option<HostEntry> {
    let line = line.split('#').next()?.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split_whitespace();
    let first = parts.next()?.to_string();
    if let Some(second) = parts.next() {
        return Some(HostEntry {
            name: first,
            target: second.to_string(),
        });
    }
    Some(HostEntry {
        name: first.clone(),
        target: first,
    })
}

pub fn read_host_entries(home: &HiveHome) -> Vec<HostEntry> {
    let path = home.hosts_file();
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_host_line).collect()
}

pub fn read_hosts(home: &HiveHome) -> Vec<String> {
    read_host_entries(home)
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

pub fn host_allowed(home: &HiveHome, host: &str) -> bool {
    read_host_entries(home)
        .iter()
        .any(|entry| entry.name == host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_comments_and_blanks() {
        let dir = std::env::temp_dir().join(format!("hive-common-hosts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hosts"), "alpha\n# skip\n  beta  extra\n\n").unwrap();
        let home = HiveHome {
            root: dir.clone(),
        };
        assert_eq!(read_hosts(&home), vec!["alpha", "beta"]);
        let entries = read_host_entries(&home);
        assert_eq!(entries[1].name, "beta");
        assert_eq!(entries[1].target, "extra");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn host_allowed_matches_hosts_file() {
        let dir = std::env::temp_dir().join(format!("hive-common-allow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("hosts"),
            "box\nbox-a http://100.64.0.2:8788\nhttp://other:9\n",
        )
        .unwrap();
        let home = HiveHome { root: dir.clone() };
        assert!(host_allowed(&home, "box"));
        assert!(host_allowed(&home, "box-a"));
        assert!(host_allowed(&home, "http://other:9"));
        assert!(!host_allowed(&home, "evil"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
