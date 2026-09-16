use hive_common::ClientToml;
use hive_protocol::{DEFAULT_HOST_PORT, DEFAULT_HUB_PORT};

use crate::encode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub hub: Option<String>,
    pub local_host: String,
}

impl Endpoints {
    pub fn from_client(client: &ClientToml) -> Self {
        if !client.hub.is_empty() {
            let hub = normalize_hub_url(&client.hub);
            return Self {
                hub: Some(hub),
                local_host: format!("http://127.0.0.1:{DEFAULT_HOST_PORT}"),
            };
        }
        Self {
            hub: None,
            local_host: format!("http://127.0.0.1:{DEFAULT_HOST_PORT}"),
        }
    }

    pub fn hub_events_url(&self) -> Option<String> {
        self.hub.as_ref().map(|h| format!("{h}/v1/events"))
    }

    pub fn transcript_stream_url(
        &self,
        host: &str,
        task: &str,
        after: Option<&str>,
        tail: Option<u32>,
    ) -> String {
        let task = encode::path_segment(task);
        let mut url = self.host_url(host, &format!("/v1/sessions/{task}/transcript/stream"));
        let mut qs = Vec::new();
        if let Some(a) = after.filter(|s| !s.is_empty()) {
            qs.push(format!("after={}", encode::percent_encode(a)));
        }
        if let Some(t) = tail {
            qs.push(format!("tail={t}"));
        }
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        url
    }

    pub fn host_url(&self, host: &str, rest: &str) -> String {
        let rest = if rest.starts_with('/') {
            rest.to_string()
        } else {
            format!("/{rest}")
        };
        if let Some(hub) = &self.hub {
            format!("{hub}/v1/hosts/{}{rest}", encode::path_segment(host))
        } else {
            format!("{}{rest}", self.local_host)
        }
    }
}

fn normalize_hub_url(raw: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.trim_end_matches('/').to_string()
    } else if raw.contains(':') {
        format!("http://{raw}")
    } else {
        format!("http://{raw}:{DEFAULT_HUB_PORT}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hive_common::ClientToml;

    #[test]
    fn hub_hostname_adds_port() {
        let ep = Endpoints::from_client(&ClientToml {
            hub: "hub".into(),
            ..Default::default()
        });
        assert_eq!(ep.hub.as_deref(), Some("http://hub:8787"));
    }

    #[test]
    fn hub_url_passthrough() {
        let ep = Endpoints::from_client(&ClientToml {
            hub: "http://box:9999/".into(),
            ..Default::default()
        });
        assert_eq!(ep.hub.as_deref(), Some("http://box:9999"));
    }

    #[test]
    fn host_url_via_hub_proxy() {
        let ep = Endpoints::from_client(&ClientToml {
            hub: "http://hub:8787".into(),
            ..Default::default()
        });
        assert_eq!(
            ep.host_url("box", "/v1/sessions/foo"),
            "http://hub:8787/v1/hosts/box/v1/sessions/foo"
        );
    }

    #[test]
    fn host_url_encodes_host_segment() {
        let ep = Endpoints::from_client(&ClientToml {
            hub: "http://hub:8787".into(),
            ..Default::default()
        });
        assert_eq!(
            ep.host_url("my box", "/v1/sessions"),
            "http://hub:8787/v1/hosts/my%20box/v1/sessions"
        );
    }

    #[test]
    fn host_url_local_direct() {
        let ep = Endpoints::from_client(&ClientToml::default());
        assert_eq!(
            ep.host_url("ignored", "/v1/sessions"),
            "http://127.0.0.1:8788/v1/sessions"
        );
    }

    #[test]
    fn transcript_stream_url_builds_query() {
        let ep = Endpoints::from_client(&ClientToml {
            hub: "http://hub:8787".into(),
            ..Default::default()
        });
        let url = ep.transcript_stream_url("box", "my task", Some(r#"{"offset":1}"#), Some(150));
        assert!(url.contains("/transcript/stream"));
        assert!(url.contains("tail=150"));
        assert!(url.contains("after="));
        assert!(url.contains("my%20task"));
    }
}
