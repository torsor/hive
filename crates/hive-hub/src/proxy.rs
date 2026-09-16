use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use hive_common::host_allowed;
use hive_protocol::ErrorBody;

use crate::poll::resolve_host_base;
use crate::state::HubState;

/// Parse `/v1/hosts/{host}/…` into upstream host id and normalized REST path.
pub fn upstream_path(uri_path: &str) -> Option<(String, String)> {
    let after_hosts = uri_path.strip_prefix("/v1/hosts/")?;
    let (host_raw, rest_raw) = match after_hosts.split_once('/') {
        Some((h, r)) => (h, format!("/{r}")),
        None => (after_hosts, String::new()),
    };
    Some((decode_path_segment(host_raw), normalize_rest(&rest_raw)))
}

fn decode_path_segment(raw: &str) -> String {
    let mut out = String::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn normalize_rest(rest_raw: &str) -> String {
    if rest_raw.is_empty() || rest_raw == "/" {
        "/v1/health".to_string()
    } else if rest_raw.starts_with("/v1/") {
        rest_raw.to_string()
    } else {
        format!("/v1{rest_raw}")
    }
}

pub fn build_upstream_url(
    home: &hive_common::HiveHome,
    host: &str,
    rest: &str,
    query: Option<&str>,
    host_port: u16,
) -> String {
    let qs = query.map(|q| format!("?{q}")).unwrap_or_default();
    format!(
        "{}{rest}{qs}",
        resolve_host_base(host, host_port, home)
    )
}

pub fn should_stream_response(headers: &HeaderMap) -> bool {
    if headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/event-stream"))
    {
        return true;
    }
    headers
        .get(axum::http::header::TRANSFER_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|te| te.eq_ignore_ascii_case("chunked"))
}

fn copy_stream_headers(upstream: &HeaderMap, dest: &mut HeaderMap) {
    for key in [axum::http::header::CONTENT_TYPE, axum::http::header::CACHE_CONTROL] {
        if let Some(v) = upstream.get(&key) {
            if let Ok(hv) = HeaderValue::from_bytes(v.as_bytes()) {
                dest.insert(key, hv);
            }
        }
    }
}

pub async fn proxy(State(st): State<Arc<HubState>>, req: Request) -> impl IntoResponse {
    let uri = req.uri().clone();
    let Some((host, rest)) = upstream_path(uri.path()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody::new("proxy", "expected /v1/hosts/{host}/…")),
        )
            .into_response();
    };
    if !host_allowed(&st.home, &host) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorBody::new("forbidden_host", format!("host not in fleet: {host}"))),
        )
            .into_response();
    }
    let url = build_upstream_url(&st.home, &host, &rest, uri.query(), st.cfg.host_port);
    let method = req.method().clone();
    let body = axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let mut b = st.http.request(method.clone(), &url);
    if !body.is_empty() {
        b = b
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
    }
    match b.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            if should_stream_response(resp.headers()) {
                let mut headers = HeaderMap::new();
                copy_stream_headers(resp.headers(), &mut headers);
                let stream = resp.bytes_stream().map(|chunk| {
                    chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                });
                let body = Body::from_stream(stream);
                let mut response = Response::new(body);
                *response.status_mut() = status;
                *response.headers_mut() = headers;
                response
            } else {
                let bytes = resp.bytes().await.unwrap_or_default();
                (status, bytes).into_response()
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorBody::new("proxy", e.to_string())),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_encoded_host() {
        assert_eq!(
            upstream_path("/v1/hosts/my%20box/v1/sessions/foo"),
            Some(("my box".into(), "/v1/sessions/foo".into()))
        );
        assert_eq!(
            upstream_path("/v1/hosts/http%3A%2F%2Fbox%3A9/v1/health"),
            Some(("http://box:9".into(), "/v1/health".into()))
        );
    }

    #[test]
    fn proxy_rewrites_path() {
        assert_eq!(
            upstream_path("/v1/hosts/box/v1/sessions/foo"),
            Some(("box".into(), "/v1/sessions/foo".into()))
        );
        assert_eq!(
            upstream_path("/v1/hosts/box"),
            Some(("box".into(), "/v1/health".into()))
        );
        assert_eq!(
            upstream_path("/v1/hosts/box/sessions/foo"),
            Some(("box".into(), "/v1/sessions/foo".into()))
        );
        assert_eq!(upstream_path("/v1/fleet"), None);
    }

    #[test]
    fn build_upstream_url_uses_hosts_file() {
        use hive_common::HiveHome;

        let dir = std::env::temp_dir().join(format!("hive-hub-proxy-url-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hosts"), "hub http://100.64.0.1:8788\n").unwrap();
        let home = HiveHome { root: dir.clone() };
        assert_eq!(
            build_upstream_url(&home, "hub", "/v1/fs", None, 8788),
            "http://100.64.0.1:8788/v1/fs"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_event_stream_content_type() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        );
        assert!(should_stream_response(&headers));
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        assert!(!should_stream_response(&headers));
    }

    #[tokio::test]
    async fn proxy_rejects_unknown_host() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use http_body_util::BodyExt;
        use hive_common::HiveHome;
        use tokio::sync::broadcast;
        use tower::ServiceExt;

        use crate::http;
        use crate::state::HubState;

        let dir = std::env::temp_dir().join(format!("hive-hub-proxy-no-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hosts"), "box\n").unwrap();
        let home = HiveHome { root: dir.clone() };
        let (tx, _) = broadcast::channel(4);
        let state = HubState::new(home, hive_common::HubConfig::default(), tx);
        let app = http::router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/hosts/evil/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["code"], "forbidden_host");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn proxy_passthrough_sse_stream() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use http_body_util::BodyExt;
        use hive_common::HiveHome;
        use tokio::sync::broadcast;
        use tower::ServiceExt;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        use crate::http;
        use crate::state::HubState;

        let upstream = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/sessions/demo/transcript/stream"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(
                    "event: tick\ndata: {\"ok\":true}\n\n",
                    "text/event-stream",
                ),
            )
            .mount(&upstream)
            .await;

        let dir = std::env::temp_dir().join(format!("hive-hub-proxy-sse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let upstream_uri = upstream.uri();
        std::fs::write(dir.join("hosts"), format!("{upstream_uri}\n")).unwrap();
        let home = HiveHome { root: dir.clone() };
        let (tx, _) = broadcast::channel(4);
        let state = HubState::new(home, hive_common::HubConfig::default(), tx);
        let app = http::router(state);

        let encoded_host = upstream_uri
            .replace('%', "%25")
            .replace(':', "%3A")
            .replace('/', "%2F");
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/v1/hosts/{encoded_host}/v1/sessions/demo/transcript/stream"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(body.windows(6).any(|w| w == b"event:"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
