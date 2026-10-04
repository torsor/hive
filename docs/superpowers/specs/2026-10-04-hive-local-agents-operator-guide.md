# Local agents — operator & agent instructions

**Audience:** Dan (morning-after laptop restore), team poll agents (self-register), implementers.  
**Spec (full design):** [2026-10-04-hive-panel-local-agents.md](2026-10-04-hive-panel-local-agents.md)  
**Plan:** [../plans/2026-10-04-hive-local-agents.md](../plans/2026-10-04-hive-local-agents.md)

## What this is

**Local agents** are bookmarks on **this Mac** stored in `~/.hive/local-agents.json`. They are **not** fleet tmux sessions (those stay on `hive status` / panel fleet). Use them for **Cursor/Composer** (and optional local Claude) so after sleep or shutdown you can reopen the **right folder** and run a **resume command** in one step — or restart a **whole group** by tag.

Hive **does not** pick which Cursor chat/agent to open in v1. You get `cursor .` (or similar) on the correct repo; use **`notes`** on the row to remember which sidebar chat to click.

## Quick reference

| Goal | Command |
|------|---------|
| See all local agents | `hive local list` |
| See one cohort | `hive local list --tag my-team` |
| Register (agent or human, upsert) | `hive local register --id … --title … --cwd "$PWD" --resume '…' --tags laptop,my-team` |
| One-shot create (fail if id exists) | `hive local add …` |
| Print resume line | `hive local open <id>` |
| Open in Terminal.app (macOS) | `hive local open <id> --launch` |
| Restart whole group (macOS) | `hive local open-group --tag my-team --launch` |
| JSON | append `--json` to `list` or `show` |

Help: `hive local --help`, `hive local register --help`, `hive local open-group --help`.

## Agent self-registration (recommended)

Run at **session start** (or in a wrapper) on the laptop — safe every time:

```bash
hive local register \
  --id my-agent \
  --title "hive — panel & local registry" \
  --cwd "$PWD" \
  --resume 'cursor .' \
  --provider cursor \
  --agentmsg agent:my-agent \
  --tags laptop,my-team \
  --notes 'Cursor: reopen this Composer chat after window opens'
```

| Flag | Rule |
|------|------|
| `--id` | Stable slug, lowercase `[a-z0-9_-]`, usually matches board name without `agent:` |
| `--cwd` | **`$PWD`** at project root; must exist when the command runs |
| `--resume` | One line, no newlines. **Cursor on laptop:** `'cursor .'` until CLI resume exists |
| `--tags` | **`laptop,my-team`** for “bring back with the cohort” |
| `--agentmsg` | Optional; hive does not poll the board for local rows |
| `--notes` | Human hint for which Cursor chat to select |

Use **`register`**, not **`add`**, so restarts update the row without errors.

## Morning-after (laptop was shut down)

1. Build or install a `hive` binary that includes `hive local` (see plan).
2. `hive local list --tag my-team` — confirm rows exist (if empty, register manually or wake agents once).
3. `hive local open-group --tag my-team --launch` — one Terminal window per agent, each runs `cd <cwd> && <resume>`.
4. In each **Cursor** window, open the chat named in **`notes`** / **`title`**.
5. Remote fleet: unchanged — panel **Refresh** or `hive status` for box sessions.

## Groups = tags

There is no separate “group” field. Agents that should restart together share a tag, e.g. **`my-team`**. Optional **`laptop`** marks “this row is for the Mac, not devbox-only bookkeeping.”

Filter logic:

- `hive local list --tag a --tag b` — agents with **any** of the tags (default).
- `hive local list --tag a --tag b --match-all` — agents with **all** tags.

## Cursor vs fleet Claude/Codex

| Situation | Use |
|-----------|-----|
| Cursor/Composer on laptop | **`hive local register`** + `cursor .` |
| Claude/Codex in tmux on a box | **`hive run`** / panel fleet (not local registry) |
| Future Cursor on devbox via CLI | Update **`resume`** when `cursor-agent resume …` is proven; keep same **`id`** / **tags** |

## Panel (when shipped)

Same file (`~/.hive/local-agents.json`). **Local** section below fleet; **Open** / **Open group** use your configured terminal (iTerm/Ghostty/etc.), not only Terminal.app.

## Troubleshooting

| Problem | Check |
|---------|--------|
| `command not found: hive local` | Rebuild/install `hive` from hive-repo `main` |
| `cwd not accessible` | Path must exist; use absolute path |
| `agent already exists` on add | Use **`register`** or **`remove`** then **`add`** |
| `--launch` fails | macOS only today; without `--launch`, run printed `cd && …` yourself |
| Empty list after reboot | Registry is on **this machine’s** `~/.hive/`; re-register or restore from backup |
