# `hive notify`: idle-aware `say`

**Date:** 2026-09-30
**Status:** request, for implementation
**Raised by:** Danny, designing an agent message board on qpt
(`quite-possibly-today/docs/superpowers/specs/2026-09-30-qpt-threads-board.md`).
**Scope (ruled 2026-09-30):** only the idle-wait. `say` is already the
messaging capability and nothing about it changes. Hive does not launch
Cursor agents and the board does not depend on hive, so the `cursor`
provider request that was here is withdrawn; agents outside hive learn of
posts by polling (`qpt inbox`) or their own tool's messaging.

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

### 2. Session name for the board

A hive session is addressable on the board as `agent:hive/<host>/<task>`;
a convention only, nothing to build.

## Not requested

A message body channel: the pane carries a pointer, the board carries the
message. Presence beyond what `hive status` already reports. A `cursor`
provider (withdrawn). Anything for agents not launched by hive.

## Tests

- `notify` to an idle pane pastes immediately and returns `sent`; to a
  generating pane returns `queued`, and the text appears after the
  prompt returns; two identical queued texts deliver once; an expired
  text is logged and dropped.
