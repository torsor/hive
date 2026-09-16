use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use hive_common::{read_host_entries, HostConfig, HostEntry, HiveHome};
use hive_protocol::{Fleet, HostFleet, HubEvent};

use crate::state::HubState;

pub fn host_base(name: &str, port: u16) -> String {
    if name.starts_with("http://") || name.starts_with("https://") {
        name.trim_end_matches('/').to_string()
    } else {
        format!("http://{name}:{port}")
    }
}

/// Resolve the HTTP base URL for polling a fleet host's hive-host API.
pub fn poll_base(entry: &HostEntry, port: u16, home: &HiveHome) -> String {
    if entry.target.starts_with("http://") || entry.target.starts_with("https://") {
        return entry.target.trim_end_matches('/').to_string();
    }
    if hive_common::is_local_hostname(&entry.name)
        || hive_common::is_local_hostname(&entry.target)
    {
        let cfg = HostConfig::load(home).unwrap_or_default();
        return format!("http://{}:{}", cfg.bind, port);
    }
    host_base(&entry.target, port)
}

/// Resolve poll/proxy base URL for a fleet host name (`box-a`, `hub`, …).
pub fn resolve_host_base(name: &str, port: u16, home: &HiveHome) -> String {
    for entry in read_host_entries(home) {
        if entry.name == name {
            return poll_base(&entry, port, home);
        }
    }
    if hive_common::is_local_hostname(name) {
        let cfg = HostConfig::load(home).unwrap_or_default();
        return format!("http://{}:{}", cfg.bind, port);
    }
    host_base(name, port)
}

pub async fn refresh_once(state: &HubState) {
    let entries = read_host_entries(&state.home);
    let mut out = Vec::new();
    for entry in entries {
        let base = poll_base(&entry, state.cfg.host_port, &state.home);
        let url = format!("{base}/v1/sessions");
        match state.http.get(&url).timeout(Duration::from_secs(8)).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Vec<hive_protocol::SessionRow>>().await {
                    Ok(mut sessions) => {
                        for s in &mut sessions {
                            s.host = Some(entry.name.clone());
                        }
                        out.push(HostFleet {
                            host: entry.name.clone(),
                            error: None,
                            sessions,
                        });
                    }
                    Err(e) => out.push(HostFleet {
                        host: entry.name.clone(),
                        error: Some(e.to_string()),
                        sessions: vec![],
                    }),
                }
            }
            Ok(resp) => out.push(HostFleet {
                host: entry.name.clone(),
                error: Some(format!("http {}", resp.status())),
                sessions: vec![],
            }),
            Err(e) => out.push(HostFleet {
                host: entry.name.clone(),
                error: Some(e.to_string()),
                sessions: vec![],
            }),
        }
    }
    let fleet = Fleet {
        hosts: out,
        generated_at: Utc::now(),
    };
    *state.fleet.write().await = fleet;
    let _ = state.events.send(HubEvent {
        kind: "fleet".into(),
        host: None,
        task: None,
    });
}

pub async fn poll_loop(state: Arc<HubState>) {
    let secs = state.cfg.poll_secs.max(1);
    loop {
        refresh_once(&state).await;
        tokio::time::sleep(Duration::from_secs(secs)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hive_common::HiveHome;
    use tokio::sync::broadcast;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn host_base_adds_default_port() {
        assert_eq!(host_base("hub", 8788), "http://hub:8788");
        assert_eq!(host_base("http://box:9", 8788), "http://box:9");
    }

    #[test]
    fn poll_base_uses_explicit_url() {
        let home = HiveHome {
            root: std::env::temp_dir(),
        };
        let entry = HostEntry {
            name: "box-a".into(),
            target: "http://100.64.0.2:8788".into(),
        };
        assert_eq!(
            poll_base(&entry, 8788, &home),
            "http://100.64.0.2:8788"
        );
    }

    #[test]
    fn resolve_host_base_uses_hosts_file() {
        let dir = std::env::temp_dir().join(format!("hive-hub-resolve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("hosts"),
            "hub http://100.64.0.1:8788\nbox-a http://100.64.0.2:8788\n",
        )
        .unwrap();
        let home = HiveHome { root: dir.clone() };
        assert_eq!(
            resolve_host_base("hub", 8788, &home),
            "http://100.64.0.1:8788"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn poll_merges_host_errors() {
        let good = MockServer::start().await;
        let bad = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&good)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/sessions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&bad)
            .await;

        let dir = std::env::temp_dir().join(format!("hive-hub-poll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("hosts"),
            format!("good {}\nbad {}\n", good.uri(), bad.uri()),
        )
        .unwrap();

        let home = HiveHome { root: dir.clone() };
        let (tx, _) = broadcast::channel(4);
        let state = HubState::new(home, hive_common::HubConfig::default(), tx);

        refresh_once(&state).await;

        let fleet = state.fleet.read().await;
        assert_eq!(fleet.hosts.len(), 2);
        let ok = fleet.hosts.iter().find(|h| h.error.is_none()).expect("one ok host");
        let err = fleet
            .hosts
            .iter()
            .find(|h| h.error.is_some())
            .expect("one failed host");
        assert_eq!(ok.host, "good");
        assert!(ok.sessions.is_empty());
        assert_eq!(err.host, "bad");
        assert!(err.error.as_ref().unwrap().contains("500"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
