# hive-web

Touch-first phone console for hive — Safari on Tailscale, HTTP client of **hive-hub** only.

**Production URL:** `http://<hub>/hive/` (nginx on the hub host; hub API at `http://<hub>:8787`)

## Layout

| URL / port | Service |
|------------|---------|
| **`http://<hub>/hive/`** | nginx — static SPA on the hub host |
| **8787** | hive-hub — fleet + session API (separate origin, CORS enabled) |
| **8789** | `hive-web serve` — local dev/smoke only (not deployed) |

## Hub discovery

At load the SPA picks the hub URL in this order:

1. `VITE_HUB_URL` baked in at build time (local dev).
2. `config.json` next to the SPA, written by the `hive_web` Ansible role from
   `config.json.j2`. See `public/config.example.json` for the shape.
3. Same origin: `http://<page host>:8787`, which is right when nginx on the hub serves the SPA.

## Dev

```bash
cd apps/hive-web    # not repo root — there is no package.json at the top level
npm install
VITE_HUB_URL=http://<hub>:8787 npm run dev
```

Open `http://localhost:5173`.

## Build

```bash
npm run build          # → dist/
make build-web         # dist + release hive-web binary
```

`chat-render.js` is synced from `apps/hive-panel` before build/test (`npm run sync-renderer`).

## Production

Deployed to the hub host via the `hive_web` Ansible role (`make deploy-web`).
See [docs/superpowers/specs/2026-08-15-hive-phone-console.md](../../docs/superpowers/specs/2026-08-15-hive-phone-console.md).
