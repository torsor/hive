use std::net::IpAddr;
use std::process::Command;
use std::str::FromStr;

use anyhow::{bail, Result};

/// Tailscale CGNAT (100.64.0.0/10).
pub fn in_tailnet(host: &str) -> bool {
    let Ok(ip) = IpAddr::from_str(host) else {
        return false;
    };
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (o[1] & 0b1100_0000) == 64
        }
        IpAddr::V6(_) => false,
    }
}

pub fn assert_bind_allowed(role: &str, host: &str, dev: bool) -> Result<()> {
    let host = host.trim();
    if host.is_empty() {
        bail!("{role}: bind address is empty");
    }
    if matches!(host, "0.0.0.0" | "::" | "*") {
        bail!(
            "{role}: refusing to bind {host:?} — Tailscale IP only (or --dev --bind 127.0.0.1)"
        );
    }
    if matches!(host, "127.0.0.1" | "::1" | "localhost") {
        if !dev {
            bail!("{role}: refusing loopback in production — pass --dev --bind 127.0.0.1");
        }
        return Ok(());
    }
    if !in_tailnet(host) {
        bail!("{role}: {host:?} is not a Tailscale CGNAT address (100.64.0.0/10)");
    }
    Ok(())
}

pub fn tailscale_v4() -> Option<String> {
    let out = Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|l| in_tailnet(l))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_requires_dev() {
        assert!(assert_bind_allowed("hive-host", "127.0.0.1", false).is_err());
        assert!(assert_bind_allowed("hive-hub", "127.0.0.1", true).is_ok());
    }

    #[test]
    fn rejects_wildcard() {
        assert!(assert_bind_allowed("hive-host", "0.0.0.0", true).is_err());
    }

    #[test]
    fn accepts_tailnet() {
        assert!(assert_bind_allowed("hive-hub", "100.64.1.2", false).is_ok());
    }

    #[test]
    fn in_tailnet_cgnat() {
        assert!(in_tailnet("100.64.1.2"));
        assert!(!in_tailnet("10.0.0.1"));
        assert!(!in_tailnet("not-an-ip"));
    }
}
