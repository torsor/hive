# hive-panel (agents)

HTTP client of hive-hub. Do not add `Command` to legacy tools or `ssh` on the fleet/chat path.

- Rust: `src-tauri/src/hub.rs` talks to `~/.hive/client.toml` `hub`. Cache last fleet; emit `fleet` events.
- Attach is the exception: GET hub attach argv, then open an external terminal (macOS). That argv is `ssh -t host tmux attach`.
- React chat: `ChatPanel.tsx` uses `@tanstack/react-virtual` for long transcripts, module-level render/transcript caches, and an outbound send queue. Live updates subscribe to per-session transcript SSE via Tauri (`start_transcript_stream_cmd` → `transcript-tick` events); HTTP poll at 2.5s is the fallback when the stream drops.
- Shared JSON: `crates/hive-protocol`.
