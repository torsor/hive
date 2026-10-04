# Agent registration boundary (hive-centric)

**Date:** 2026-10-04  
**Status:** approved direction — **not implemented** (beyond proto `local-agents.json`)  
**Related:** [2026-10-04-hive-panel-local-agents.md](2026-10-04-hive-panel-local-agents.md), [2026-09-30-notify-idle-wait.md](2026-09-30-notify-idle-wait.md), qpt-repo task stub `docs/superpowers/specs/2026-10-04-qpt-hive-agent-registry.md`

## Problem

team-style agents exist in three places today:

- **agentmsg** — white-pages registration, poll, `--on` text  
- **qpt** — threads, sync, participant lists  
- **hive** — fleet tmux sessions, laptop **local** bookmarks (`local-agents.json`)

Dan wants **shared vocabulary** (slugs, tags) and is leaning toward **hive as the canonical registration** so qpt and agentmsg become **hive-aware**, without an ever-growing tangle of mutual dependencies.

## Decision (Dan, 2026-10-04)

- **Hive owns the agent record** (identity + where/how to run + optional board pointer).  
- **qpt** and **agentmsg** **read** that record (and may **optional publish** to the board); they do **not** define a competing laptop roster.  
- **agentmsg → qpt** for delivery/sync remains acceptable.  
- **Hive must not** require qpt or agentmsg to run (fleet, local register, panel fleet section).

Registration is **not** messaging: thread bodies stay on qpt; hive stores pointers and metadata only.

## Dependency rules (anti-tangle)

```text
agentmsg  →  qpt           transport / sync (existing)
qpt       →  hive           read registry (future)
agentmsg  →  hive           optional publish from registry (future)

hive      ↛  agentmsg/qpt   no hard dependency on board for core ops
```

**Hive-aware** means: resolve `agent:slug` and cohort tags from hive’s store; display and route using that data. It does **not** mean embedding qpt or agentmsg inside hive-host/hive-hub.

## Canonical record (target shape)

Evolution of today’s `local-agents.json` → **`~/.hive/agents.json`** (name TBD) with **facets** on one slug:

| Facet | Fields (conceptual) |
|-------|---------------------|
| **Identity** | `id` (slug), `title`, `tags[]`, `updated_at` |
| **Local** | `cwd`, `resume`, `provider`, `notes` |
| **Board** | `agentmsg` slug, optional poll hint (not thread content) |
| **Fleet** | optional `host`, `task` when hive-host owns tmux |

Same **`id`** as board `agent:<id>` without prefix. Tags (`laptop`, `my-team`, project tags) shared with session label vocabulary where applicable.

**Write API (future):** `hive agent register` (supersedes `hive local register`); single upsert.  
**Read API (future):** CLI/panel today; optional **`GET /v1/agents`** on hub only if fleet-wide roster on hubhost is needed (phase B — see local-agents plan).

## Optional publish (later)

`hive agent register --publish-agentmsg` (or a hubhost sync job):

- Upserts white-pages entry from hive record (`--on`, endpoint, host).  
- Failure to publish does **not** fail local registry write.  
- agentmsg does **not** become source of truth for cwd/resume.

## Phased rollout

| Phase | Hive | qpt | agentmsg |
|-------|------|-----|----------|
| **Now** | `hive local register`, `local-agents.json`, panel Local | unchanged | independent register |
| **A** | Unified schema + `hive agent register`; deprecate duplicate fields in docs | read `agents.json` for slug metadata in thread UI / CLI hints | optional validate slug against hive file on laptop |
| **B** | Hub aggregate roster (if laptops + hubhost need one view) | sync facet from hub | publish from hive on hubhost |

Implement **A** after team cohort lives on `hive local register`; **B** only if operational pain justifies it.

## Non-goals

- Hive as message bus or thread store.  
- Merging local rows into `GET /v1/fleet` poll (local stays client-side or separate API).  
- Forcing all agents through hive-host tmux (Cursor laptop agents stay local facet).  
- agentmsg registration without qpt on the sync host (unchanged product boundary).

## Open questions

- File name: `agents.json` vs keep `local-agents.json` until schema grows.  
- Whether hubhost holds a **merged** roster or each machine only holds **local facet** + fleet from hub.  
- Cursor CLI resume: update `resume` field only; same `id`.

## Tests (when implemented)

- Register upsert; facet merge; corrupt JSON → empty.  
- qpt read-only: unknown slug vs known slug from temp `HIVE_HOME`.  
- Publish optional path fails open (registry still saved).
