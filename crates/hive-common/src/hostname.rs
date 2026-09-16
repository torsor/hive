use std::process::Command;

/// Short hostname of this machine (`hub` from `hub.example`).
pub fn local_short_hostname() -> Option<String> {
    if let Ok(name) = std::env::var("HOSTNAME") {
        let short = short_hostname(&name);
        if !short.is_empty() {
            return Some(short);
        }
    }
    let out = Command::new("hostname").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(short_hostname(&name))
    }
}

pub fn is_local_hostname(name: &str) -> bool {
    let Some(local) = local_short_hostname() else {
        return false;
    };
    name.eq_ignore_ascii_case(&local) || name.eq_ignore_ascii_case(&short_hostname(name))
}

fn short_hostname(name: &str) -> String {
    name.split('.').next().unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_hostname_strips_domain() {
        assert_eq!(short_hostname("hub.tail.net"), "hub");
    }
}
