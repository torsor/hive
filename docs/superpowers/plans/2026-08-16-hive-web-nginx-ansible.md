# hive-web nginx `/hive` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve the phone SPA at `http://hub/hive/` via nginx on the hub host (tailnet-only `:80`), retire production `hive-web` on `:8789`, leave hive-hub on `:8787` unchanged.

**Architecture:** Ansible `hive_web` role builds the Vite SPA with `base: '/hive/'`, deploys `~/.hive/web/dist/`, templates `config.json` (hub still `http://<tailscale-ip>:8787`), and installs an nginx site listening **only** on the hub's Tailscale IPv4. The browser loads static files from port 80 and continues cross-origin API/SSE calls to `:8787`. The `hive-web` Rust binary remains in-repo for local `--dev` smoke tests only — not deployed to hub.

**Tech Stack:** Vite 7, vanilla JS, ansible-core, nginx (system package on hub), existing hive-hub CORS.

**Spec:** [docs/superpowers/specs/2026-08-15-hive-phone-console.md](../specs/2026-08-15-hive-phone-console.md) (hosting section updated in Task 7)

## Global Constraints

- Production bind: Tailscale CGNAT `100.64.0.0/10` only — nginx `listen` must use the hub Tailscale IPv4, never `0.0.0.0:80`.
- Auth = Tailscale membership — no login, token, or TLS on the tailnet.
- Clients never SSH on the status/chat path; phone UI is HTTP-only to hive-hub.
- Hub API stays on `:8787`; no same-origin `/v1/` proxy in this plan.
- SPA builds on the ansible control node (laptop); no cross-arch Rust binary copy to hub for static hosting.
- Do not weaken hive-hub/host bind guards elsewhere.

---

## File map (before tasks)

| File | Responsibility |
|------|----------------|
| `apps/hive-web/vite.config.ts` | `base` from `VITE_BASE` env (default `/` for dev) |
| `apps/hive-web/src/hub.js` | Base-aware `config.json` fetch |
| `apps/hive-web/test/hub.test.js` | Tests for `configJsonUrl()` helper |
| `ansible/group_vars/all/vars.yml` | `hive_web_base_path`, drop `hive_web_port` |
| `ansible/roles/hive_web/tasks/main.yml` | SPA deploy + nginx; remove hive-web binary/systemd |
| `ansible/roles/hive_web/templates/nginx-site.conf.j2` | Tailnet-only nginx server block |
| `ansible/roles/hive_web/handlers/main.yml` | `nginx -t` + reload |
| `ansible/roles/hive_web/defaults/main.yml` | `hive_web_nginx_package: nginx` |
| `docs/superpowers/specs/2026-08-15-hive-phone-console.md` | Hosting decision + checklist URL |
| `docs/cheat-sheet.md`, `apps/hive-web/README.md`, `CLAUDE.md` | Bookmark + deploy docs |

---

### Task 1: SPA base path + config.json URL

**Files:**
- Modify: `apps/hive-web/vite.config.ts`
- Modify: `apps/hive-web/src/hub.js`
- Modify: `apps/hive-web/test/hub.test.js`

**Interfaces:**
- Consumes: none
- Produces: `export function configJsonUrl(base = import.meta.env.BASE_URL)` → string ending in `config.json`; `resolveHubUrl()` uses it

- [ ] **Step 1: Write the failing test**

```javascript
import { describe, expect, it } from "vitest";
import { configJsonUrl } from "../src/hub.js";

describe("configJsonUrl", () => {
  it("joins base path for subpath deploy", () => {
    expect(configJsonUrl("/hive/")).toBe("/hive/config.json");
  });

  it("works at site root", () => {
    expect(configJsonUrl("/")).toBe("/config.json");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/hive-web && npm run test -- test/hub.test.js -t configJsonUrl`
Expected: FAIL — `configJsonUrl` not exported

- [ ] **Step 3: Implement**

`vite.config.ts`:

```typescript
export default defineConfig({
  base: process.env.VITE_BASE ?? "/",
  // ...existing keys unchanged
});
```

`hub.js`:

```javascript
export function configJsonUrl(base = import.meta.env.BASE_URL) {
  return new URL("config.json", base.endsWith("/") ? base : `${base}/`).pathname;
}

// in resolveHubUrl():
const res = await fetch(configJsonUrl(), { cache: "no-store" });
```

- [ ] **Step 4: Run tests**

Run: `cd apps/hive-web && npm run test`
Expected: PASS

- [ ] **Step 5: Verify production build locally**

Run:

```bash
cd apps/hive-web
VITE_BASE=/hive/ npm run build
grep -o 'href="/hive/[^"]*"' dist/index.html | head -3
```

Expected: asset links prefixed with `/hive/`

- [ ] **Step 6: Commit**

```bash
git add apps/hive-web/vite.config.ts apps/hive-web/src/hub.js apps/hive-web/test/hub.test.js
git commit -m "feat(hive-web): support /hive/ base path for nginx deploy"
```

---

### Task 2: Ansible variables

**Files:**
- Modify: `ansible/group_vars/all/vars.yml`

**Interfaces:**
- Consumes: none
- Produces: `hive_web_base_path: hive` (no leading/trailing slash — templates add `/{{ hive_web_base_path }}/`)

- [ ] **Step 1: Update group vars**

Replace `hive_web_port: 8789` with:

```yaml
hive_web_base_path: hive          # URL path → http://hub/hive/
hive_web_nginx_port: 80
```

Keep `hive_web_local_repo`, `hive_home`, `hive_hub_port`, `hive_hub_host` unchanged.

- [ ] **Step 2: Commit**

```bash
git add ansible/group_vars/all/vars.yml
git commit -m "chore(ansible): vars for nginx /hive hosting"
```

---

### Task 3: nginx site template + handler

**Files:**
- Create: `ansible/roles/hive_web/templates/nginx-site.conf.j2`
- Create: `ansible/roles/hive_web/handlers/main.yml`
- Create: `ansible/roles/hive_web/defaults/main.yml`

**Interfaces:**
- Consumes: `hive_web_bind_effective`, `hive_home`, `hive_web_base_path`, `hive_web_nginx_port`
- Produces: handler `Reload nginx` notified by site template task

- [ ] **Step 1: Add defaults**

`defaults/main.yml`:

```yaml
hive_web_nginx_package: nginx
hive_web_nginx_site_name: hive-console
```

- [ ] **Step 2: Add handler**

`handlers/main.yml`:

```yaml
- name: Reload nginx
  become: true
  ansible.builtin.service:
    name: nginx
    state: reloaded
```

- [ ] **Step 3: Add nginx site template**

`templates/nginx-site.conf.j2`:

```nginx
# {{ ansible_managed }} — hive phone SPA (static only; API on :8787)
server {
    listen {{ hive_web_bind_effective }}:{{ hive_web_nginx_port }};
    server_name {{ inventory_hostname }};

    location = /{{ hive_web_base_path }} {
        return 301 /{{ hive_web_base_path }}/;
    }

    location /{{ hive_web_base_path }}/ {
        alias {{ hive_home }}/web/dist/;
        index index.html;
    }
}
```

No `autoindex on;` — directory listing stays off (nginx default).

- [ ] **Step 4: Commit**

```bash
git add ansible/roles/hive_web/templates/nginx-site.conf.j2 \
        ansible/roles/hive_web/handlers/main.yml \
        ansible/roles/hive_web/defaults/main.yml
git commit -m "feat(ansible): nginx site template for hive phone SPA"
```

---

### Task 4: Refactor hive_web role tasks

**Files:**
- Modify: `ansible/roles/hive_web/tasks/main.yml`

**Interfaces:**
- Consumes: Task 2 vars, Task 3 template/handler, Task 1 `VITE_BASE=/hive/` build
- Produces: nginx site enabled on hub; `hive-web` user unit stopped/disabled; dist + config.json deployed

- [ ] **Step 1: Remove hive-web binary pipeline**

Delete these tasks (and their tags-only comments if orphaned):

- `Ensure hive-web build staging directory on hub`
- `Sync worktree sources to hub for hive-web build`
- `Build hive-web release binary on hub`
- `Install hive-web binary to stable path`
- `Symlink hive-web for manual debugging`
- `hive-web install (systemd user unit)`
- `Restart hive-web when binary updated`
- `Ensure hive-web is active`

- [ ] **Step 2: Update SPA build command**

Change npm build to pass base path:

```yaml
- name: Build phone SPA assets on control node
  ansible.builtin.command:
    cmd: bash -lc 'npm ci && VITE_BASE=/{{ hive_web_base_path }}/ npm run build'
    chdir: "{{ hive_web_local_repo }}/apps/hive-web"
  delegate_to: localhost
  run_once: true
  become: false
  register: hive_web_npm_build
  changed_when: hive_web_npm_build.rc == 0
  tags: [web, build]
```

- [ ] **Step 3: Add nginx install + site tasks** (after `Set effective hive-web bind address`, rename fact to `hive_web_bind_effective` — keep name for minimal diff)

```yaml
- name: Install nginx on hub
  become: true
  ansible.builtin.apt:
    name: "{{ hive_web_nginx_package }}"
    state: present
    update_cache: true
    cache_valid_time: 86400
  tags: [web]

- name: Deploy hive nginx site
  become: true
  ansible.builtin.template:
    src: nginx-site.conf.j2
    dest: "/etc/nginx/sites-available/{{ hive_web_nginx_site_name }}"
    mode: "0644"
  notify: Reload nginx
  tags: [web]

- name: Enable hive nginx site
  become: true
  ansible.builtin.file:
    src: "/etc/nginx/sites-available/{{ hive_web_nginx_site_name }}"
    dest: "/etc/nginx/sites-enabled/{{ hive_web_nginx_site_name }}"
    state: link
    force: true
  notify: Reload nginx
  tags: [web]

- name: Ensure nginx is enabled and running
  become: true
  ansible.builtin.service:
    name: nginx
    state: started
    enabled: true
  tags: [web]

- name: Stop and disable hive-web user unit (retired on hub)
  ansible.builtin.systemd:
    name: hive-web
    state: stopped
    enabled: false
    scope: user
  failed_when: false
  tags: [web]
```

- [ ] **Step 4: Update role header comment**

```yaml
# Role: hive_web
# Purpose: phone SPA static assets + nginx site on hub host.
# Consumes: hive_home, hive_web_base_path, hive_hub_port, hive_web_local_repo.
# SPA builds on control node; nginx serves ~/.hive/web/dist/ at /hive/.
```

- [ ] **Step 5: Dry-run ansible**

Run: `cd ansible && ansible-playbook site.yml -K --tags web --limit hub --check --diff`
Expected: shows nginx package/site tasks; no hive-web cargo/rsync tasks

- [ ] **Step 6: Commit**

```bash
git add ansible/roles/hive_web/tasks/main.yml
git commit -m "feat(ansible): serve hive-web via nginx, retire hub hive-web unit"
```

---

### Task 5: Deploy + smoke test on hub

**Files:** none (verification only)

- [ ] **Step 1: Preflight on hub**

Run:

```bash
ssh hub 'tailscale ip -4; command -v nginx || echo no-nginx; ss -tlnp | grep ":80 " || true'
```

Note any existing `:80` listeners. If another nginx vhost already owns the tailnet IP on 80, merge manually before converging (unlikely on hub today).

- [ ] **Step 2: Converge web role**

Run: `make deploy-web` (or `cd ansible && ansible-playbook site.yml -K --tags web --limit hub`)

Expected: `ok=… failed=0`

- [ ] **Step 3: HTTP smoke from laptop (on tailnet)**

Run:

```bash
TS=$(ssh hub tailscale ip -4 | head -1)
curl -sf "http://${TS}/hive/" | head -5
curl -sf "http://${TS}/hive/config.json"
curl -o /dev/null -w '%{http_code}\n' "http://${TS}:8787/v1/fleet"
ssh hub systemctl --user is-active hive-web || true   # expect inactive/failed
ssh hub 'systemctl is-active nginx'                   # expect active
```

Expected: HTML loads; config.json has `"hub":"http://<ts-ip>:8787"`; fleet 200; hive-web not active; nginx active.

- [ ] **Step 4: Confirm bind is tailnet-only**

Run: `ssh hub 'ss -tlnp | grep nginx'`

Expected: nginx listens on `100.x.x.x:80`, not `0.0.0.0:80` for the hive site (other vhosts may differ — hive block must be tailnet IP only).

---

### Task 6: iPhone manual QA

**Files:** none

- [ ] **Step 1: Update Home Screen bookmark**

Open `http://hub/hive/` (MagicDNS) on iPhone Safari.

- [ ] **Step 2: Run spec checklist** (from updated spec)

- Fleet loads
- Chat SSE + send
- Spawn, stop/restart
- Copy SSH / attach
- Hub down → readable error

---

### Task 7: Documentation + spec amendment

**Files:**
- Modify: `docs/superpowers/specs/2026-08-15-hive-phone-console.md`
- Modify: `docs/cheat-sheet.md`
- Modify: `apps/hive-web/README.md`
- Modify: `CLAUDE.md`
- Modify: `ansible/README.md`

- [ ] **Step 1: Update spec hosting diagram**

Replace C2 `:8789` lines with:

```
iPhone Safari
    ├─► http://hub/hive/          static SPA (nginx → ~/.hive/web/dist/)
    └─► http://hub:8787/v1/…      hive-hub (fleet, proxy, SSE)
```

Add note: production static host is **nginx** (system, tailnet `:80`); `hive-web serve` is dev/smoke only.

Update verification checklist bookmark to `http://hub/hive/`.

- [ ] **Step 2: Update cheat-sheet phone section**

Bookmark: `http://hub/hive/`. Remove `:8789` from architecture diagram. Note `systemctl is-active nginx` on hub instead of `hive-web`.

- [ ] **Step 3: Update README + CLAUDE one-liners**

Production URL → `http://hub/hive/`. Local dev unchanged (`localhost:5173`).

- [ ] **Step 4: Update ansible README tags blurb**

`| web | hive_web (phone SPA + nginx on hub) |`

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-08-15-hive-phone-console.md \
        docs/cheat-sheet.md apps/hive-web/README.md CLAUDE.md ansible/README.md
git commit -m "docs: phone console at http://hub/hive via nginx"
```

---

## Out of scope (explicit non-goals)

- Same-origin nginx proxy `/v1/` → `:8787` (optional future polish)
- Merging static routes into hive-hub (spec Option A)
- Removing `crates/hive-web` or Makefile `build-web` targets (still used locally)
- TLS on tailnet
- nginx on non-hub hosts

## Rollback

If nginx misbehaves before cutover is trusted:

```bash
ssh hub
systemctl --user start hive-web   # if binary still present from prior deploy
# temporarily bookmark http://hub:8789/ again
```

After this plan ships, re-enable would require re-adding the retired ansible tasks or a one-off `hive-web install` — keep the previous git commit hash handy.

## Self-review (spec coverage)

| Spec requirement | Task |
|------------------|------|
| Tailnet-only bind | Task 3 template `listen {{ hive_web_bind_effective }}` |
| Static dist from ansible | Task 4 deploy dist |
| config.json hub URL | Existing template + Task 4 |
| CORS cross-origin to :8787 | Unchanged — no task |
| No directory listing | Task 3 template (default off) |
| iPhone QA checklist | Task 6 |
| Docs / cheat sheet | Task 7 |

No placeholders remain in task steps.

---

**Plan complete and saved to `docs/superpowers/plans/2026-08-16-hive-web-nginx-ansible.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
