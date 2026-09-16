# Design: Hive phone console (Tailscale web SPA)

**Date:** 2026-08-15  
**Status:** approved (hosting choice locked)  
**Scope:** iPhone Safari (and any mobile browser on the tailnet) — fleet view, chat, stop/restart, attach recipe copy  
**See also:** the older toolkit's `hive web` phone-console plan, [2026-08-14-hive-desktop-app-design.md](2026-08-14-hive-desktop-app-design.md), `ansible/README.md`

## Intent

Port the older toolkit's **`hive web`** phone console to hive. The hive stack already exposes the needed HTTP API on **hive-hub**; this track adds a touch-first static SPA and a production static host on the hub machine.

**Not a native iOS app.** Safari (optionally Add to Home Screen). Attach on phone copies an SSH/tmux argv for **Terminus** (or similar) — no in-browser terminal.

**Do not port** the older toolkit's Python `hive_web` server or shell-out to `cc-*`. Clients talk to **hive-hub** only.

---

## Hosting decision: **C2**

Static UI and hive-hub API both live on **hub**, different ports:

```
iPhone Safari
    ├─► http://hub/hive/          static SPA (nginx → ~/.hive/web/dist/)
    └─► http://hub:8787/v1/…      hive-hub (fleet, proxy, SSE)
            └── HTTP ──► each fleet host :8788 (hive-host)
```

Production static host is **nginx** (system, tailnet `:80`); `hive-web serve` is dev/smoke only.

| Port / path | Service | Process |
|-------------|---------|---------|
| **8787** | hive-hub API | `hive-hub` systemd user unit |
| **:80 /hive/** | phone SPA | nginx (system unit) → `~/.hive/web/dist/` |

**Why C2 (not C1):** one always-on entry host (hub); avoids depending on box-b for UI when that box flaps; UI and hub deploys can still be versioned together via ansible on hub.

**Why not Option A (hub serves UI on 8787):** deferred — C2 keeps API and static concerns separate without a second machine. Option A remains a future simplification (single URL) if we merge static routes into hive-hub later.

### Optional dev layout (non-essential)

During UI work, serve from a laptop without deploying to hub:

```
vite dev (localhost:5173)  ──CORS──►  http://hub:8787/v1/…
```

hive-hub already uses permissive CORS. Hub URL comes from env or a dev `config.json`.

---

## Trust and bind rules

Same as hub/host (see CLAUDE.md):

- Production bind: Tailscale CGNAT `100.64.0.0/10` only on **8787** and nginx tailnet **:80**.
- Loopback / `0.0.0.0` refused outside `--dev`.
- **Auth = Tailscale membership** — no login, token, or TLS in this track.
- nginx must not expose directory listings or serve files outside the SPA root.

Cross-origin: UI origin `http://hub` or `http://hub/hive/` calling API `http://hub:8787` requires CORS on POST and SSE. Hub already ships `CorsLayer::permissive()`.

---

## Older toolkit → hive API mapping

Phone JS calls **hub** directly (no `/api/v1` prefix on the UI host).

| older toolkit `hive web` | hive hub |
|---------------|--------|
| `GET /api/v1/fleet` | `GET /v1/fleet` |
| `GET /api/v1/sessions/{host}/{task}/attach-command` | `GET /v1/hosts/{host}/v1/sessions/{task}/attach` |
| `GET /api/v1/sessions/{host}/{task}/transcript` | `GET /v1/hosts/{host}/v1/sessions/{task}/transcript` |
| `POST …/say` | `POST /v1/hosts/{host}/v1/sessions/{task}/say` |
| `POST …/stop` | `POST …/stop` |
| `POST …/restart` | `POST …/restart` |
| `POST …/label` | `POST …/label` |

**Attach schema:** `hive-mobile-attach/v1` → **`hive-attach/v1`** (`crates/hive-protocol`).

**Chat transport upgrade:** prefer SSE `GET …/transcript/stream` over the older toolkit's HTTP polling (hive-panel already uses this pattern via Tauri). Fallback poll at 2.5s if stream drops.

**Fleet JSON:** hub fleet shape (same wire types as hive-panel / `hive-protocol`). Adapt the older toolkit's `flattenFleet` in `app.js` to match.

---

## UI source and parity

**Starting point:** the older toolkit's static assets (`index.html`, `app.js`, CSS):

- `index.html`, `app.js`, `chat.js`, `console.css`, `tokens.css`
- Touch layout: `100dvh`, safe-area insets, 16px textarea, Send-only compose, 44px targets

**Canonical renderer:** `apps/hive-panel/src/chat-render.js` — keep a parity hash test (the older toolkit had `test_static_parity.py`; hive should use vitest or a small node script in the web app package).

**Themes:** same six browse themes as panel (`thing`, `thing-light`, `torsor`, …).

### In scope (MVP)

- Fleet list: host filter, starred filter, refresh
- Session actions: Chat, Stop, Restart, copy attach command
- **Spawn** from header (host, root browse, provider)
- **SSH:** copy path / copy SSH command on fleet cards (Termius)
- Full-screen chat dialog (Claude + Codex)
- Blocked-on-prompt banner + attach hint
- Default hub URL baked for production (`http://hub:8787` or relative discovery via same-host config)

### Out of scope (this track)

- Native iOS / Capacitor / Tauri iOS
- PWA manifest, service worker, offline
- Open SSH in-browser (copy-only on phone; laptop panel opens iTerm)
- Workshop verbs (mode/box/guides/…)
- Kill (can add later; panel has it)
- SSH or ansible from the phone UI

---

## Static hosting on hub

**Production:** nginx (system unit) on tailnet `:80`, location `/hive/` → `~/.hive/web/dist/`. Deployed via `ansible/roles/hive_web` (`--tags web`).

**Dev/smoke only:** `hive-web serve` (local bind guards, e.g. `--dev --bind 127.0.0.1 --port 8789`) or vite dev on the laptop — not used in production.

Production requirements:

- nginx listens on tailnet IP only (`100.64.0.0/10`)
- Serves pre-built `dist/` from ansible deploy
- Optional `config.json` at SPA root: `{ "hub": "http://100.x.x.x:8787" }` for phones that need explicit hub URL

---

## Repo layout (proposed)

```
apps/hive-web/
  index.html
  src/app.js          # fleet UI, hub client
  src/chat.js         # SSE + compose
  src/console.css
  src/tokens.css
  vite.config.ts      # build → dist/
  package.json

ansible/roles/hive_web/   # deploy dist + systemd unit on hub
```

Build output deployed to hub host, e.g. `~/.hive/web/dist/`.

---

## Configuration surfaces

| File | Where | Purpose |
|------|-------|---------|
| `~/.hive/client.toml` | laptop | `hub = "hub"` — panel/CLI (unchanged) |
| SPA `config.json` or build-time default | hub `/hive/` | `{ "hub": "http://<hub-tailscale-ip>:8787" }` |

Phone bookmark: **`http://hub/hive/`** (MagicDNS) or tailnet IP + `/hive/`.

---

## Implementation phases

### Phase 1 — MVP

1. Create `apps/hive-web/`; port the older toolkit's static UI
2. Replace API adapter (`/api/v1` → hub `/v1/…`)
3. Wire fleet, stop, restart, label, attach copy
4. Chat with SSE + poll fallback
5. `chat-render.js` parity test vs hive-panel
6. Local dev: vite → hub

### Phase 2 — Production deploy

1. `ansible/roles/hive_web` on hub (build, install dist, systemd, bind tailnet)
2. Document in `ansible/README.md` (phone section)
3. Manual iPhone QA on tailnet (checklist below)

### Phase 3 — Polish (optional)

- Load-earlier transcript, outbound send queue (panel parity)
- Add to Home Screen icons / meta tags
- Consider merging UI into hive-hub (Option A) for single URL

---

## Verification checklist (iPhone on tailnet)

- [ ] Open `http://hub/hive/` — fleet loads
- [ ] Filter by host / starred
- [ ] Open Claude chat — transcript renders markdown
- [ ] Send a line — appears in session on box
- [ ] Open Codex chat — same
- [ ] Stop / Restart a session
- [ ] Copy attach command — valid `hive-attach/v1` argv for Terminus
- [ ] Star / unstar session
- [ ] Off-tailnet bind on nginx :80 fails closed
- [ ] Hub unreachable — UI shows clear error (not blank crash)

---

## Non-goals

- Replacing hive-panel on macOS
- lab-agents unified mobile gateway (separate program; see `lab-agents/docs/mobile/`)
- TLS termination on the tailnet

---

## Files (expected touch)

| Area | Path |
|------|------|
| SPA | `apps/hive-web/**` |
| Renderer parity | `apps/hive-panel/src/chat-render.js`, test |
| Hub CORS | `crates/hive-hub/src/http.rs` (already permissive) |
| Deploy | `ansible/roles/hive_web/**`, `ansible/site.yml`, `group_vars/all/vars.yml` |
| Docs | `ansible/README.md`, `CLAUDE.md` (one line) |
