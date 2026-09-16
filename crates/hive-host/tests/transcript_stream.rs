//! Transcript stream tick helper tests.

use hive_host::http::transcript_tick_changed;
use hive_protocol::{ChatBlock, TranscriptCursor, TranscriptDoc};

fn sample_doc(blocks: Vec<ChatBlock>, offset: u64, blocked: bool) -> TranscriptDoc {
    TranscriptDoc {
        session_id: Some("sess".into()),
        reset: false,
        blocked_on_prompt: blocked,
        cursor: TranscriptCursor {
            session_id: Some("sess".into()),
            offset,
        },
        blocks,
        has_earlier: false,
        earlier_until: None,
    }
}

#[test]
fn tick_changed_on_first_nonempty() {
    let doc = sample_doc(
        vec![ChatBlock {
            kind: "text".into(),
            role: "user".into(),
            text: "hi".into(),
            ts: None,
            id: Some("1".into()),
            failed: false,
        }],
        1,
        false,
    );
    assert!(transcript_tick_changed(false, false, "", &doc));
}

#[test]
fn tick_unchanged_when_empty_after_bootstrap() {
    let doc = sample_doc(vec![], 5, false);
    assert!(!transcript_tick_changed(true, false, "sess:5", &doc));
}

#[test]
fn tick_changed_on_blocked_flip() {
    let doc = sample_doc(vec![], 5, true);
    assert!(transcript_tick_changed(true, false, "sess:5", &doc));
}

#[test]
fn tick_changed_on_reset() {
    let mut doc = sample_doc(vec![], 0, false);
    doc.reset = true;
    assert!(transcript_tick_changed(true, false, "sess:0", &doc));
}
