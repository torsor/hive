# Session tags and panel filtering

**Date:** 2026-10-03  
**Status:** approved (Dan); wrapper details may refine via qpt thread `hive:labels`  
**Thread:** `01M41JMCZ46F2605Q6ZNXNEBED`

## Problem

Fleet rows only support **starred**. Room, evaluation, and workshop agents share hosts with
other sessions; the panel needs project tags and a way to keep ephemeral rows off the default
view without killing sessions.

## Storage

Unchanged sidecar: `~/.hive/sessions/<task>.labels`

```json
{ "starred": false, "tags": ["room", "evaluation"] }
```

`tags` is a sorted unique list of short strings (lowercase). No secrets, no paths.

## Tag vocabulary (convention)

| Tag | Meaning |
|-----|---------|
| `room` | thinglab room agent |
| `evaluation` | evaluation-room / evaluate wrapper (always includes `room`) |
| `workshop` | workshop agent when used |
| `hidden` | omit from default panel list |

Kind-specific tags (`research`, `editorial`, …) optional; wrappers may add later.

**Dan (2026-10-03):** visibility tag is `hidden` (not `hide`). Evaluation spawns carry
`room` and `evaluation`.

## API

### Fleet row

`SessionRow.tags: string[]` from the sidecar at list time.

### Spawn

`SpawnRequest.tags: string[]` — on fresh spawn, write tags to the sidecar (preserve
`starred` if the file already exists). Resume does not change tags unless a future op
requests it.

### Label

`POST /v1/sessions/{task}/label` body `{ "op": "star"|"unstar"|"tag-add"|"tag-remove", "tag": "…" }`
(`tag` required for tag ops).

## CLI

- `hive run … --tags room,evaluation` (comma-separated, repeatable ok)
- `hive label <host> <task> star|unstar`
- `hive label <host> <task> tag-add <tag>` / `tag-remove <tag>`

## Panel

- Row badges for tags (compact).
- Filters: multi-select **tags** (include if row has any selected tag).
- **Default:** exclude rows with tag `hidden`.
- Toggle **Show hidden** (persisted in localStorage with other filters).
- Starred-only filter unchanged.

## Wrappers (out of repo)

Thread consensus (`agent:bobby-rooms`, `agent:jan-evaluation`, 2026-10-03):

- **room start:** pass `--tags` mirroring the thing's `identity.tags` from thinglab
  (template declares `room` + kind: `research`, `editorial`, `evaluation`, …). No slug
  in tags — task name is the slug.
- **evaluate / jan-evaluation:** `room`, `evaluation` only; `--hidden` opt-in. Gate on
  `hive run --help` listing `--tags` until fleet binaries are rebuilt.
- **hidden:** running sessions only; down/archive kills the session — do not reuse `hidden`
  for finished work. Optional wrapper policy: auto-`hidden` for `trial-*` / `rr-test-*`
  slugs (wrapper-only, not hive).
- **room** and **evaluate** wrappers landed in room-repo / thinglab-templates (local until
  hive deploy).

## Not requested

- Hub poll skipping hidden sessions.
- Tags inferred from task-name prefix in hive-host.
- agentmsg board naming (`agent:hive/<host>/<task>`).

## Tests

- Sidecar round-trip; spawn sets tags; tag-add/remove; corrupt JSON → empty tags.
- List sessions exposes tags.
- Label handler rejects bad op/tag.
