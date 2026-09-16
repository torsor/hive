//! SSE frame splitting for hive-hub `/v1/events`.

/// Pull complete SSE frames from `buf` (delimited by blank line) and return `data:` payloads.
pub fn drain_data_events(buf: &mut String) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(idx) = buf.find("\n\n") {
        let frame = buf[..idx].to_string();
        *buf = buf[idx + 2..].to_string();
        if let Some(data) = data_from_frame(&frame) {
            out.push(data);
        }
    }
    out
}

fn data_from_frame(frame: &str) -> Option<String> {
    let mut data = String::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim());
        }
    }
    if data.is_empty() {
        None
    } else {
        Some(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parse_multi_frame() {
        let mut buf =
            "event: fleet\ndata: {\"kind\":\"fleet\"}\n\nevent: ping\ndata: {\"kind\":\"ping\"}\n\n"
                .to_string();
        let events = drain_data_events(&mut buf);
        assert_eq!(events.len(), 2);
        assert!(events[0].contains("fleet"));
        assert!(events[1].contains("ping"));
        assert!(buf.is_empty());
    }

    #[test]
    fn sse_holds_partial_frame() {
        let mut buf = "event: fleet\ndata: {\"kind\":\"fleet\"}\n".to_string();
        assert!(drain_data_events(&mut buf).is_empty());
        buf.push_str("\n\n");
        assert_eq!(drain_data_events(&mut buf).len(), 1);
    }

    #[test]
    fn sse_skips_empty_data() {
        let mut buf = "event: keepalive\n\n".to_string();
        assert!(drain_data_events(&mut buf).is_empty());
    }
}
