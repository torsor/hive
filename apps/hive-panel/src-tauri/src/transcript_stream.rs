//! Per-session transcript SSE subscriber for the chat panel.

use std::sync::Mutex;

use futures_util::StreamExt;
use hive_protocol::TranscriptDoc;
use tauri::{async_runtime::JoinHandle, AppHandle, Emitter};

use crate::hub;
use crate::sse;

static STREAM_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

pub fn stop_transcript_stream() {
    if let Some(handle) = STREAM_TASK.lock().unwrap().take() {
        handle.abort();
    }
}

pub fn start_transcript_stream(
    app: AppHandle,
    host: String,
    task: String,
    after: Option<String>,
    tail: Option<u32>,
) -> Result<(), String> {
    stop_transcript_stream();
    let url = hub::transcript_stream_url(&host, &task, after.as_deref(), tail)?;
    let handle = tauri::async_runtime::spawn(async move {
        if let Err(e) = run_stream(&app, &url).await {
            let _ = app.emit("transcript-stream-error", e);
        }
    });
    *STREAM_TASK.lock().unwrap() = Some(handle);
    Ok(())
}

async fn run_stream(app: &AppHandle, url: &str) -> Result<(), String> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("stream http {}", resp.status()));
    }
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        for data in sse::drain_data_events(&mut buf) {
            let doc: TranscriptDoc =
                serde_json::from_str(&data).map_err(|e| format!("parse tick: {e}"))?;
            app.emit("transcript-tick", &doc)
                .map_err(|e| e.to_string())?;
        }
    }
    Err("stream ended".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse;

    #[test]
    fn parses_transcript_tick_from_sse_bytes() {
        let sample = r#"event: tick
data: {"reset":false,"blocked_on_prompt":false,"cursor":{"offset":1},"blocks":[{"kind":"text","role":"user","text":"hi"}]}

"#;
        let mut buf = sample.to_string();
        let events = sse::drain_data_events(&mut buf);
        assert_eq!(events.len(), 1);
        let doc: TranscriptDoc = serde_json::from_str(&events[0]).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].text, "hi");
    }
}
