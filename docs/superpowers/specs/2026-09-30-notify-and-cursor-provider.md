# `hive notify` (idle-aware `say`) and a `cursor` provider

**Date:** 2026-09-30
**Status:** request, for implementation
**Raised by:** Danny, designing an agent message board on qpt
(`quite-possibly-today/docs/superpowers/specs/2026-09-30-qpt-threads-board.md`):
"hive might have some ideas for direct messaging/alerts? … some agents
will be Cursor agents, not Claude or Codex."

## What exists

`hive say <host> <task> --text …` (`hive-host/src/say.rs`) pastes a line
into the session's tmux pane as a user message: clears partial input,
bracketed paste for multi-line, Enter separately (the Claude TUI treats
bundled text+Enter as a staged paste), refuses when a human is attached
with recent activity, 16 KB cap, providers `claude | codex` only.

That is the one alert channel that works for any interactive agent: it
depends only on the agent running in a pane hive owns, not on the tool
having an inbox. The board design uses it for a **nudge** — one line
telling the agent there is unread mail and which command reads it — and
never for the message body, which stays in qpt as untrusted data.

## Requested

### 1. `hive notify <host> <task> --text …`

`say`, delivered when the pane is idle:

- **Idle** = the pane's last line is the provider's prompt (the same
  detection `codex_bind` uses for the Codex trust text; for Claude the
  `>` prompt line; for cursor-agent its prompt) and no output has changed
  for `NOTIFY_QUIET_MS` (default 1500). Never paste while an agent is
  generating or a tool is running.
- **Queue**: if not idle, hold the text in
  `~/.hive/notify/<task>.queue` and retry on a short timer (host-side, no
  daemon beyond the existing hive-host); coalesce identical texts; deliver
  in order; expire after `--ttl` (default 1 h) and say so in the
  transcript log.
- **Return** `sent` | `queued` | `expired` in the JSON envelope; the hub
  exposes it as `POST /v1/hosts/<host>/v1/sessions/<task>/notify`.
- `say` stays as the immediate, no-wait form.

### 2. Provider `cursor`

`run.rs` accepts `provider: cursor`, resolves `cursor-agent` (or `agent`)
as the binary, and launches it in the tmux session like the others. `say`,
`notify`, `transcript`, and `kill` then work unchanged. `bind`'s prompt
detection needs cursor-agent's prompt marker; record it once it is seen.
Cursor's trust/permission prompts, if any, are handled the way the Codex
ones are (`codex_bind`), or documented as manual.

### 3. Session name for the board

A hive session is addressable on the board as `agent:hive/<host>/<task>`.
`hive status --json` already has host and task; nothing new is needed
beyond documenting the form so qpt can map a post's recipient to a
`notify` call.

## Not requested

A message body channel: the pane carries a pointer, the board carries the
message. Presence beyond what `hive status` already reports. Anything for
Cursor running inside the IDE (no pane): those agents poll `qpt inbox` at
turn start via a `.cursor/rules` line, which is adequate for turn-paced
spec work.

## Tests

- `notify` to an idle pane pastes immediately and returns `sent`; to a
  generating pane returns `queued`, and the text appears after the
  prompt returns; two identical queued texts deliver once; an expired
  text is logged and dropped.
- `run --provider cursor` starts `cursor-agent` in a `hive-<task>` session;
  `say` reaches it; `kill` ends it.
