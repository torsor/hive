use chrono::Utc;
use hive_client::{Client, Endpoints, Error};
use hive_protocol::{Fleet, HostFleet, SessionRow};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_json_maps_error_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/fleet"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "ok": false,
            "code": "not_found",
            "error": "session gone"
        })))
        .mount(&server)
        .await;

    let client = Client::from_endpoints(Endpoints {
        hub: Some(server.uri()),
        local_host: "http://127.0.0.1:8788".into(),
    })
    .unwrap();

    let err = client.fetch_fleet().await.unwrap_err();
    match err {
        Error::Http { status, code, message } => {
            assert_eq!(status, 404);
            assert_eq!(code, "not_found");
            assert_eq!(message, "session gone");
        }
        other => panic!("expected Http error, got {other:?}"),
    }
}

#[tokio::test]
async fn fleet_parses() {
    let server = MockServer::start().await;
    let fleet = Fleet {
        hosts: vec![HostFleet {
            host: "box".into(),
            error: None,
            sessions: vec![SessionRow {
                host: Some("box".into()),
                task: "demo".into(),
                state: "running".into(),
                started: String::new(),
                dir: "/tmp".into(),
                last: None,
                provider: "claude".into(),
                starred: false,
                tags: vec![],
                tmux: None,
            }],
        }],
        generated_at: Utc::now(),
    };
    Mock::given(method("GET"))
        .and(path("/v1/fleet"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&fleet))
        .mount(&server)
        .await;

    let client = Client::from_endpoints(Endpoints {
        hub: Some(server.uri()),
        local_host: "http://127.0.0.1:8788".into(),
    })
    .unwrap();

    let got = client.fetch_fleet().await.unwrap();
    assert_eq!(got.hosts.len(), 1);
    assert_eq!(got.hosts[0].host, "box");
    assert_eq!(got.hosts[0].sessions[0].task, "demo");
}

#[tokio::test]
async fn local_fetch_fleet_uses_sessions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![SessionRow {
            host: None,
            task: "solo".into(),
            state: "idle".into(),
            started: String::new(),
            dir: "/".into(),
            last: None,
            provider: "claude".into(),
            starred: false,
            tags: vec![],
            tmux: None,
        }]))
        .mount(&server)
        .await;

    let client = Client::from_endpoints(Endpoints {
        hub: None,
        local_host: server.uri(),
    })
    .unwrap();

    let got = client.fetch_fleet().await.unwrap();
    assert_eq!(got.hosts.len(), 1);
    assert_eq!(got.hosts[0].host, "local");
    assert_eq!(got.hosts[0].sessions[0].task, "solo");
}

#[tokio::test]
async fn attach_rewrites_ssh_host() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/sessions/demo/attach"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "schema": "hive-attach/v1",
            "host": "wrong-hostname",
            "task": "demo",
            "terminal": "external-ssh",
            "command": ["ssh", "wrong-hostname", "-t", "tmux attach -t hive-demo"]
        })))
        .mount(&server)
        .await;

    let client = Client::from_endpoints(Endpoints {
        hub: None,
        local_host: server.uri(),
    })
    .unwrap();

    let att = client.attach("fleet-alias", "demo").await.unwrap();
    assert_eq!(att.host, "fleet-alias");
    assert_eq!(att.command[1], "fleet-alias");
}
