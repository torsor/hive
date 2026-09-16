use hive_host::transcript::{adapt, Cursor};

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    std::fs::read(&root).unwrap_or_else(|_| panic!("missing fixture {}", root.display()))
}

use std::path::PathBuf;

const CLAUDE: &str = "transcript-claude-session.jsonl";
const CODEX: &str = "transcript-codex-rollout.jsonl";
const SESSION: &str = "11111111-2222-3333-4444-555555555555";

#[test]
fn claude_cursor_consumes_fixture() {
    let data = fixture(CLAUDE);
    let (blocks, cur) = adapt("claude-jsonl", &data, Cursor::default(), false).unwrap();
    assert_eq!(cur.offset, data.len() as u64);
    assert_eq!(cur.session_id.as_deref(), Some(SESSION));
    assert!(!blocks.is_empty());
    assert!(blocks.iter().all(|b| b.kind != "thinking"));
}

#[test]
fn claude_user_and_tools() {
    let data = fixture(CLAUDE);
    let (blocks, _) = adapt("claude-jsonl", &data, Cursor::default(), false).unwrap();
    let text: Vec<_> = blocks.iter().filter(|b| b.kind == "text").collect();
    assert!(text.iter().any(|b| b.role == "user"));
    let tools: Vec<_> = blocks.iter().filter(|b| b.kind == "tool").collect();
    assert!(!tools.is_empty());
    assert!(tools.iter().any(|b| b.text.contains("ran ") || b.text.contains("read ")));
}

#[test]
fn claude_thinking_opt_in() {
    let data = fixture(CLAUDE);
    let (plain, _) = adapt("claude-jsonl", &data, Cursor::default(), false).unwrap();
    let (with, _) = adapt("claude-jsonl", &data, Cursor::default(), true).unwrap();
    let t0 = plain.iter().filter(|b| b.kind == "thinking").count();
    let t1 = with.iter().filter(|b| b.kind == "thinking").count();
    assert_eq!(t0, 0);
    assert!(t1 >= t0);
}

#[test]
fn complete_lines_holds_partial() {
    let (lines, n) = hive_host::transcript::complete_lines(b"{\"a\":1}\n{\"b\":");
    assert_eq!(lines.len(), 1);
    assert_eq!(n, b"{\"a\":1}\n".len());
}

#[test]
fn unknown_adapter_refuses() {
    let err = adapt("nope", b"\n", Cursor::default(), false).unwrap_err();
    assert!(err.0.contains("unknown"));
}

#[test]
fn codex_fixture_round_trip() {
    let data = fixture(CODEX);
    let (blocks, cur) = adapt("codex-rollout", &data, Cursor::default(), false).unwrap();
    assert_eq!(cur.offset, data.len() as u64);
    assert!(cur.session_id.is_some());
    assert!(blocks.iter().any(|b| b.kind == "text" && b.role == "user"));
    assert!(blocks.iter().any(|b| b.kind == "text" && b.role == "assistant"));
}

#[test]
fn bind_loopback_requires_dev() {
    use hive_common::bind::assert_bind_allowed;
    assert!(assert_bind_allowed("hive-host", "127.0.0.1", false).is_err());
    assert!(assert_bind_allowed("hive-host", "127.0.0.1", true).is_ok());
    assert!(assert_bind_allowed("hive-host", "0.0.0.0", true).is_err());
    assert!(assert_bind_allowed("hive-host", "100.64.1.2", false).is_ok());
}

#[test]
fn meta_round_trip() {
    let dir = std::env::temp_dir().join(format!("hive-meta-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.meta");
    hive_host::meta::write_meta(&path, &[("task", "demo"), ("dir", "/tmp/x")]).unwrap();
    let map = hive_host::meta::load_meta(&path).unwrap();
    assert_eq!(map.get("task").unwrap(), "demo");
    assert_eq!(map.get("dir").unwrap(), "/tmp/x");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn claude_slug() {
    assert_eq!(
        hive_host::discover::claude_project_slug("/home/x/y"),
        "-home-x-y"
    );
}

#[test]
fn transcript_tail_and_until_offset() {
    let sid = SESSION;
    let root = std::env::temp_dir().join(format!(
        "hive-proj-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let proj = root.join("-tmp-x");
    std::fs::create_dir_all(&proj).unwrap();
    let dest = proj.join(format!("{sid}.jsonl"));
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(CLAUDE),
        &dest,
    )
    .unwrap();
    std::env::set_var("CLAUDE_PROJECTS_ROOT", &root);
    let mut meta = std::collections::HashMap::new();
    meta.insert("provider".into(), "claude".into());
    meta.insert("session_id".into(), sid.into());
    meta.insert("dir".into(), "/tmp/x".into());
    meta.insert("task".into(), "demo".into());
    let tailed =
        hive_host::discover::transcript_document(&meta, None, Some(2), None, false).unwrap();
    assert!(tailed.reset);
    assert!(!tailed.blocks.is_empty());
    assert!(tailed.blocks.len() <= 8);
    let until = hive_host::discover::transcript_document(
        &meta,
        None,
        None,
        Some(80),
        false,
    )
    .unwrap();
    assert!(until.cursor.offset <= 80);
    let _ = std::fs::remove_dir_all(root);
}
