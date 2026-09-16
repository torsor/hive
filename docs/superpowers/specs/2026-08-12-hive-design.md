# hive design notes

## hive-host transcript stream

`GET /v1/sessions/{task}/transcript/stream` — Server-Sent Events for live transcript deltas.

### Query parameters

| Param | Type | Default | Meaning |
|-------|------|---------|---------|
| `after` | JSON cursor (URL-encoded) | — | Resume after this cursor; omit on first connect to bootstrap |
| `tail` | u32 | **150** | When no `after`, return the last N blocks on the first tick |

Same cursor shape as `GET /v1/sessions/{task}/transcript` (`TranscriptCursor`).

### Tick semantics

- The host polls the transcript adapter every ~2s per open connection.
- Each tick calls `discover::transcript_document(..., after: Some(last_cursor), tail: None)` once bootstrapped.
- An SSE frame is emitted **only when** something changed: new/changed blocks, cursor advanced, session reset, or `blocked_on_prompt` flipped.
- Empty ticks are skipped (no keepalive spam).
- First tick without `after` uses `tail=150` and sets `has_earlier` like the GET handler.

### Event format

```
event: tick
data: <TranscriptDoc JSON>
```

### Client path via hub

Panel/Tauri: `GET {hub}/v1/hosts/{host}/v1/sessions/{task}/transcript/stream?after=…&tail=…`

The hub proxy must **stream** `text/event-stream` bodies (no full-body buffer) so ticks reach subscribers promptly.
