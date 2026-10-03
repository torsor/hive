use chrono::Utc;
use hive_common::{ClientToml, HiveHome};
use hive_protocol::{
    AttachCommand, BindRequest, BindingsDoc, DirListing, Fleet, HostFleet, LabelRequest, OkOutput,
    SayRequest, SessionRow, SpawnRequest, TranscriptDoc,
};
use reqwest::Client as HttpClient;

use crate::encode;
use crate::endpoints::Endpoints;
use crate::error::Error;
use crate::http::{build_http_client, get_json, post_json};

pub struct Client {
    endpoints: Endpoints,
    http: HttpClient,
}

impl Client {
    pub fn new(home: &HiveHome) -> Result<Self, Error> {
        Self::from_client_toml(&ClientToml::load(home))
    }

    pub fn from_client_toml(client: &ClientToml) -> Result<Self, Error> {
        Ok(Self {
            endpoints: Endpoints::from_client(client),
            http: build_http_client()?,
        })
    }

    pub fn from_endpoints(endpoints: Endpoints) -> Result<Self, Error> {
        Ok(Self {
            endpoints,
            http: build_http_client()?,
        })
    }

    pub fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    pub async fn fleet(&self) -> Result<Fleet, Error> {
        let hub = self
            .endpoints
            .hub
            .as_ref()
            .ok_or_else(|| Error::Http {
                status: 400,
                code: "no_hub".into(),
                message: "fleet() requires a configured hub".into(),
            })?;
        get_json(&self.http, &format!("{hub}/v1/fleet")).await
    }

    pub async fn fetch_fleet(&self) -> Result<Fleet, Error> {
        if let Some(hub) = &self.endpoints.hub {
            get_json(&self.http, &format!("{hub}/v1/fleet")).await
        } else {
            let rows: Vec<SessionRow> =
                get_json(&self.http, &format!("{}/v1/sessions", self.endpoints.local_host))
                    .await?;
            Ok(Fleet {
                hosts: vec![HostFleet {
                    host: "local".into(),
                    error: None,
                    sessions: rows,
                }],
                generated_at: Utc::now(),
            })
        }
    }

    pub async fn attach(&self, host: &str, task: &str) -> Result<AttachCommand, Error> {
        let task = encode::path_segment(task);
        let url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/attach"));
        let att: AttachCommand = get_json(&self.http, &url).await?;
        Ok(att.with_ssh_host(host))
    }

    pub async fn say(&self, host: &str, task: &str, text: &str) -> Result<OkOutput, Error> {
        let task = encode::path_segment(task);
        let url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/say"));
        post_json(
            &self.http,
            &url,
            &SayRequest {
                text: text.to_string(),
            },
        )
        .await
    }

    pub async fn session_action(
        &self,
        host: &str,
        task: &str,
        verb: &str,
    ) -> Result<OkOutput, Error> {
        let task = encode::path_segment(task);
        let url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/{verb}"));
        post_json(&self.http, &url, &serde_json::json!({})).await
    }

    pub async fn label(
        &self,
        host: &str,
        task: &str,
        op: &str,
        tag: Option<&str>,
    ) -> Result<OkOutput, Error> {
        let task = encode::path_segment(task);
        let url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/label"));
        post_json(
            &self.http,
            &url,
            &LabelRequest {
                op: op.to_string(),
                tag: tag.map(str::to_string),
            },
        )
        .await
    }

    pub async fn spawn(&self, host: &str, req: &SpawnRequest) -> Result<OkOutput, Error> {
        let url = self.endpoints.host_url(host, "/v1/sessions");
        post_json(&self.http, &url, req).await
    }

    pub async fn transcript(
        &self,
        host: &str,
        task: &str,
        after: Option<&str>,
        tail: Option<u32>,
        until_offset: Option<u64>,
    ) -> Result<TranscriptDoc, Error> {
        let task = encode::path_segment(task);
        let mut url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/transcript"));
        let mut qs = Vec::new();
        if let Some(a) = after.filter(|s| !s.is_empty()) {
            qs.push(format!("after={}", encode::percent_encode(a)));
        }
        if let Some(t) = tail {
            qs.push(format!("tail={t}"));
        }
        if let Some(u) = until_offset {
            qs.push(format!("until_offset={u}"));
        }
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        get_json(&self.http, &url).await
    }

    pub async fn bindings(&self, host: &str, task: &str) -> Result<BindingsDoc, Error> {
        let task = encode::path_segment(task);
        let url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/bindings"));
        get_json(&self.http, &url).await
    }

    pub async fn bind(
        &self,
        host: &str,
        task: &str,
        session_id: &str,
    ) -> Result<BindingsDoc, Error> {
        let task = encode::path_segment(task);
        let url = self
            .endpoints
            .host_url(host, &format!("/v1/sessions/{task}/bind"));
        post_json(
            &self.http,
            &url,
            &BindRequest {
                session_id: session_id.to_string(),
            },
        )
        .await
    }

    pub async fn fs(&self, host: &str, path: Option<&str>) -> Result<DirListing, Error> {
        let mut url = self.endpoints.host_url(host, "/v1/fs");
        if let Some(p) = path.filter(|s| !s.is_empty()) {
            url.push_str(&format!("?path={}", encode::percent_encode(p)));
        }
        get_json(&self.http, &url).await
    }
}
