# Local agent registry (panel + CLI)

**Date:** 2026-10-04  
**Status:** approved (Dan); CLI + library partial; panel Local UI pending  
**Operator guide (how-to):** [2026-10-04-hive-local-agents-operator-guide.md](2026-10-04-hive-local-agents-operator-guide.md)  
**Implementation plan:** [../plans/2026-10-04-hive-local-agents.md](../plans/2026-10-04-hive-local-agents.md)  
**Related:** [2026-08-14-hive-desktop-app-design.md](2026-08-14-hive-desktop-app-design.md), [2026-10-03-hive-session-tags.md](2026-10-03-hive-session-tags.md), [2026-09-30-notify-idle-wait.md](2026-09-30-notify-idle-wait.md)

## Problem

Fleet rows cover **remote** `hive-host` tmux sessions. On the laptop, Cursor/Composer
and other CLI agents run outside hive. Dan wants a **hive-owned registry** (under
`HIVE_HOME`), a **Local** section in the panel, and **`hive local …`** subcommands —
without merging these rows into hub fleet poll.

**Cursor `provider` on hive-host** stays deferred until the Cursor CLI pilot on devbox.

## Storage

**Path:** `$HIVE_HOME/local-agents.json` (default `~/.hive/local-agents.json`).

**Top level:**

```json
{
  "version": 1,
  "agents": [ … ]
}
```

| Field | Type | Notes |
|-------|------|--------|
| `version` | `1` | Required; bump only on breaking schema changes |
| `agents` | array | Sorted by `id` on write |

Missing file or invalid JSON → treat as `{ "version": 1, "agents": [] }` (CLI/panel).

### Agent record

| Field | Required | Type | Limits / rules |
|-------|----------|------|----------------|
| `id` | yes | string | Slug: lowercase `[a-z0-9_-]`, 1–64 chars, unique in file |
| `title` | yes | string | 1–120 chars; no newlines |
| `cwd` | yes | string | Absolute path; must exist at **write** time (CLI/panel validate) |
| `resume` | yes | string | 1–2048 chars; single logical shell command (no newlines) |
| `notes` | no | string | Max 4096 chars |
| `agentmsg` | no | string | Board slug, e.g. `agent:my-agent` (stored as given; trim whitespace) |
| `provider` | no | string | One of: `cursor`, `composer`, `claude`, `codex`, `other` |
| `tags` | no | string[] | Same normalization as session tags (`hive-host` labels rules) |
| `updated_at` | auto | string | RFC3339 UTC; set on every create/update |

**Not stored:** `created_at` (optional v2); **`starred`** (use `tags` if needed later).

**Open behavior (panel; CLI optional):** external terminal runs  
`cd <cwd> && <resume>` in a login shell (macOS backends in `terminal.rs`). No SSH, no tmux.

Panel is the primary editor; CLI and panel both read/write the same file (no separate stores).

## CLI — `hive local`

Global: respects `--home` / `HIVE_HOME` like other commands.

| Subcommand | Args / flags | Behavior |
|------------|--------------|----------|
| **`list`** | `[--json]` `[--tag …]` `[--match-all]` | Table `id`, `title`, `tags` (or full doc with `--json`). Filter: any `--tag` matches unless `--match-all` |
| **`show`** | `<id>` `[--json]` | One agent; exit 1 if missing |
| **`add`** | `--id`, `--title`, `--cwd`, `--resume` plus optional fields below | Create; fail if `id` exists |
| **`register`** | same flags as **`add`** | **Upsert** by `id` — preferred when an **agent registers itself** on each wake |
| **`set`** | `<id>` plus any optional field flags | Partial update; fail if missing |
| **`remove`** | `<id>` | Delete; fail if missing |
| **`open`** | `<id>` `[--launch]` | Print `cd … && …`; **`--launch`** opens Terminal.app (macOS) |
| **`open-group`** | `--tag …` (required) `[--match-all]` `[--launch]` | **Open** every agent whose tags match |

### Shared optional flags (`add` / `set`)

| Flag | Maps to |
|------|---------|
| `--title` | `title` |
| `--cwd` | `cwd` |
| `--resume` | `resume` |
| `--notes` | `notes` (empty string clears on `set`) |
| `--agentmsg` | `agentmsg` (empty clears on `set`) |
| `--provider` | `provider` |
| `--tags` | `tags` (comma-separated, repeatable ok; on `set`, replaces whole list) |
| `--tag-add` / `--tag-remove` | **not** in v1 (use `--tags` on `set`; mirror session labels in v2 if needed) |

**Help:** `hive local --help`, `hive local register --help`, `hive local open-group --help`.  
**Plain-language steps:** [operator guide](2026-10-04-hive-local-agents-operator-guide.md).

Examples:

```bash
# Human one-shot
hive local add --id my-agent --title "hive session tags" \
  --cwd ~/lab/things/software/hive/hive-repo \
  --resume 'cursor .' \
  --agentmsg agent:my-agent --provider cursor --tags laptop,my-team

# Agent self-registration (safe to run every session start)
hive local register --id my-agent --title "hive — panel" \
  --cwd "$PWD" --resume 'cursor .' \
  --agentmsg agent:my-agent --provider cursor --tags laptop,my-team

hive local open-group --tag my-team --launch
hive local set my-agent --title "hive — panel + tags"
hive local list --tag my-team
hive local open my-agent --launch
```

### Agent self-registration

Yes — agents **should** register themselves with **`hive local register`**, not `add`, so restarts refresh `cwd`, `resume`, and `updated_at` without “already exists” errors. Typical pattern at end of agent bootstrap or in a wrapper:

- **`--id`** — stable slug (often matches board name without `agent:`, e.g. `my-agent`).
- **`--cwd "$PWD"`** — project root for **Open**.
- **`--resume`** — whatever command resumes that agent in a terminal (tool-specific; update when the CLI changes).
- **`--agentmsg`** — optional link to board slug (`agent:…`); hive does not poll agentmsg for local rows.

No hub, no hive-host, no tokens — only writes `local-agents.json` on the machine where the command runs.

## Groups (labels = tags)

No separate “group” table. **Shared tags** define a cohort — same normalization as fleet session tags.

**Convention (Dan, 2026-10-04):**

| Tag | Meaning |
|-----|---------|
| `laptop` | Registered for resume on the Mac (vs devbox-only bookkeeping) |
| `my-team` | All team board poll agents on this laptop |
| (project tags) | Optional: `hive`, `room`, `qpt`, … |

Every agent that should come back after a **laptop shutdown** should **`hive local register`** with the same group tag(s), e.g. `--tags laptop,my-team`.

**After reboot — restart a group:**

```bash
hive local list --tag my-team
hive local open-group --tag my-team --launch    # one Terminal per agent (macOS)
```

Panel (later): filter Local by tag + **Open group** button (same semantics).

**Partial restart:** `--tag hive --tag laptop --match-all` or a dedicated tag per sub-cohort.

## Provider-specific resume (honest limits)

Hive opens **directory + command**; it does not replace each tool’s UI for picking threads/agents.

| Provider | Typical `resume` | What hive cannot do (v1) |
|----------|------------------|---------------------------|
| **cursor** / **composer** | `cursor .` or `cursor <path>` (after `cd` to `cwd`) | **Choose which Cursor agent / Composer chat** — still the local GUI after the window opens |
| **claude** | `claude` (local terminal) | Trust/folder prompts — human or wrapper |
| **codex** | `codex` in `cwd` | Same class as fleet tmux when hive owns the session; local registry optional |
| **fleet tmux** | Use **`hive run` / panel Attach**, not local registry | — |

**Cursor wrinkle (Dan, 2026-10-04):** for team-style **Cursor** agents on the laptop, fully automatic “resume the same agent thread” is not available without a future **Cursor CLI** (`cursor-agent resume …` on devbox is the experiment). What *is* available and worth standardizing:

```bash
hive local register --id my-agent --provider cursor \
  --cwd "$PWD" --resume 'cursor .' \
  --tags laptop,my-team --agentmsg agent:my-agent \
  --notes 'Re-open the hive chat in Cursor sidebar after window opens'
```

That gives **correct folder context** and opens the project in Cursor in one gesture (`open-group --launch` does it for the whole tag). You still **pick the agent/conversation in Cursor’s UI** — document which one in **`notes`** (or title) so morning-after is “open window → click the right chat,” not “find the repo path again.”

When headless resume works, update **`resume`** via `register` without changing ids or tags.

## Laptop shutdown / morning-after workflow

1. **Before shutdown:** agents already registered with shared tags (or run once manually).
2. **After reboot:** `hive local open-group --tag my-team --launch` (or panel equivalent).
3. **Cursor rows:** each window opens on the right repo; **select the agent/chat in Cursor** (see `notes` on the row).
4. **Fleet boxes:** unchanged — `hive status` / panel fleet for remote tmux sessions.

Implementation: crate **`hive-local-agents`** (load/save/validate) used by **hive-cli** and **hive-panel** Tauri (panel UI not yet wired).

## Panel UX (v1)

- **Local** section — separate from fleet list/tiles; same sort of row actions where applicable.
- CRUD forms use the fields above (required: id, title, cwd, resume).
- **Open** as defined; fleet refresh unchanged.
- Load file on startup; after each save, refresh local list.

Optional later: copy resume, last-opened time, import/export file — not v1.

## Relationship to other “registration”

| Mechanism | Role |
|-----------|------|
| **`hive run` + hive-host** | Full session (tmux, chat, say, tags on host). Use for Claude/Codex hive owns. |
| **`local-agents.json`** | Bookmark + resume on **this machine** only. |
| **Hub fleet / poll** | Do **not** include registry rows. |
| **agentmsg register** | Independent; optional `agentmsg` field links a row to a board slug. |

## Not in scope (v1)

- Hub or host API for local agents.
- Auto-discovery of Cursor sessions.
- `hive local` on remote SSH host (file is always local `HIVE_HOME` on the machine running the command).
- **`provider: cursor`** spawn on fleet hosts.

## Tests

- Validate `id`, tags, provider enum, cwd exists.
- add / set / remove / list round-trip on temp `HIVE_HOME`.
- Duplicate `id`, missing agent errors.
- Corrupt JSON → empty registry.

## Open questions (defer)

- `hive local add` upsert flag vs strict add-only (pinned: **add-only**; use `set`).
- Linux/Windows **open** in CLI beyond printing the command.
