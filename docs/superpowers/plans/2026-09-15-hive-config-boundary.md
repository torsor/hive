# Config Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove every trace of one particular fleet from the repository so it can be published, while the author's own deploy keeps working from config under `~/.hive/fleet/`.

**Architecture:** Ansible reads its inventory and shared vars from `~/.hive/fleet/` instead of the tree; the repo ships example copies. Code, scripts, tests, fixtures, and docs use the placeholder names `hub`, `box-a`, `box-b`, `box-c` and addresses in `100.64.0.0/24`. The project is named `hive` everywhere; the string `hive-v2` disappears from the tree.

**Tech Stack:** Rust workspace (cargo), Tauri 2 + React + vitest (hive-panel), Vite + vitest (hive-web), Ansible, POSIX sh.

**Spec:** `docs/superpowers/specs/2026-09-15-hive-config-boundary.md`

**Private companion:** `../notes/2026-09-15-public-split-runbook.md` (one level above the repo, outside git). It holds the real hostnames and the **substitution table**. When a step says "apply the substitution table", read that file. Never write a real hostname, real tailnet address, or the old forge URL into any file inside the repo.

## Global Constraints

- Placeholder host names are exactly `hub`, `box-a`, `box-b`, `box-c`.
- Placeholder tailnet addresses are in `100.64.0.0/24` (`100.64.0.1` for the hub, `100.64.0.2` for box-a).
- Placeholder repo URL in examples is `https://github.com/OWNER/hive.git`.
- Placeholder home directory is `/home/user`.
- `/srv/lab/hive` stays as the generic default checkout path.
- The project name is `hive`. After Task 8 the string `hive-v2` appears nowhere in the tree except the 2026-09-15 spec and this plan, which describe its removal. Every leak grep below excludes those two files with `SELF=':!docs/superpowers/specs/2026-09-15-*' ':!docs/superpowers/plans/2026-09-15-*'`.
- Tauri: `productName` is `hive-panel`, `identifier` is `dev.torsor.hive`, window title is `hive`.
- Work on a branch named `public-split` off `hive-v2`. Commit after every task.
- Do not push anywhere. Do not create the GitHub repo. Do not run Ansible against real hosts without `--check`. Task 10 is the only exception and requires the user's explicit go-ahead at its start.

---

### Task 0: Branch

**Files:** none

- [ ] **Step 1: Create the working branch**

```bash
cd "$(git rev-parse --show-toplevel)"
git checkout -b public-split hive-v2
git status -sb
```

Expected: `## public-split`, clean tree.

---

### Task 1: Rust test data and fixtures

**Files:**
- Modify: `crates/hive-hub/src/poll.rs:32,114,124-129,139-145`
- Modify: `crates/hive-hub/src/proxy.rs:188-192`
- Modify: `crates/hive-common/src/hosts.rs:6,77,82`
- Modify: `crates/hive-common/src/hostname.rs:3,40`
- Modify: `crates/hive-cli/src/lib.rs:388,391,415`
- Modify: `crates/hive-client/src/endpoints.rs:86,89`
- Modify: `tests/fixtures/transcript-codex-rollout.jsonl`
- Modify: `tests/fixtures/transcript-claude-session.jsonl`

**Interfaces:** none. Behaviour is unchanged; only string literals in tests, doc comments, and fixtures change.

- [ ] **Step 1: Confirm the suite is green before touching anything**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: `test result: ok` for every crate.

- [ ] **Step 2: Replace host names and addresses in hub tests**

In `crates/hive-hub/src/poll.rs`:

```rust
// line 32, doc comment
/// Resolve poll/proxy base URL for a fleet host name (`box-a`, `hub`, …).

// host_base_adds_default_port
assert_eq!(host_base("hub", 8788), "http://hub:8788");

// poll_base_uses_explicit_url
let entry = HostEntry {
    name: "box-a".into(),
    target: "http://100.64.0.2:8788".into(),
};
assert_eq!(
    poll_base(&entry, 8788, &home),
    "http://100.64.0.2:8788"
);

// resolve_host_base_uses_hosts_file
std::fs::write(
    dir.join("hosts"),
    "hub http://100.64.0.1:8788\nbox-a http://100.64.0.2:8788\n",
)
.unwrap();
let home = HiveHome { root: dir.clone() };
assert_eq!(
    resolve_host_base("hub", 8788, &home),
    "http://100.64.0.1:8788"
);
```

In `crates/hive-hub/src/proxy.rs`, test `build_upstream_url_uses_hosts_file`:

```rust
std::fs::write(dir.join("hosts"), "hub http://100.64.0.1:8788\n").unwrap();
let home = HiveHome { root: dir.clone() };
assert_eq!(
    build_upstream_url(&home, "hub", "/v1/fs", None, 8788),
    "http://100.64.0.1:8788/v1/fs"
);
```

- [ ] **Step 3: Replace in common, cli, and client crates**

`crates/hive-common/src/hosts.rs`:

```rust
// line 6
    /// Short name for display and client routes (`box-a`).

// host_allowed_matches_hosts_file
    "box\nbox-a http://100.64.0.2:8788\nhttp://other:9\n",
// ...
    assert!(host_allowed(&home, "box-a"));
```

`crates/hive-common/src/hostname.rs`:

```rust
// line 3
/// Short hostname of this machine (`hub` from `hub.example`).

// short_hostname_strips_domain
        assert_eq!(short_hostname("hub.tail.net"), "hub");
```

`crates/hive-cli/src/lib.rs`, tests `bind_subcommand_lists_without_to` and `print_bindings_marks_suggested`:

```rust
        let cli = Cli::parse_from(["hive", "bind", "hub", "period-index"]);
// ...
                assert_eq!(host, "hub");
// ...
        print_bindings("hub", "period-index", &doc);
```

`crates/hive-client/src/endpoints.rs`, test `hub_hostname_adds_port`:

```rust
            hub: "hub".into(),
// ...
        assert_eq!(ep.hub.as_deref(), Some("http://hub:8787"));
```

- [ ] **Step 4: Neutralise fixture working directories**

```bash
sed -i '' 's#"cwd":"/home/[a-z]*"#"cwd":"/home/user"#g' tests/fixtures/transcript-codex-rollout.jsonl
sed -i '' 's#"cwd":"/srv/workshops/dec/work"#"cwd":"/home/user/work"#g' tests/fixtures/transcript-claude-session.jsonl
grep -o '"cwd":"[^"]*"' tests/fixtures/*.jsonl | sort | uniq -c
```

Expected: only `/home/user`, `/home/user/work`, and `/tmp`.

- [ ] **Step 5: Run the suite and the leak check**

```bash
cargo test --workspace 2>&1 | grep -E '^test result|FAILED' | sort | uniq -c
git grep -n -E '100\.(6[5-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.[0-9]+\.[0-9]+' -- crates tests
```

Expected: every line `test result: ok`, and the grep prints nothing (only `100.64.x.x` remains).

- [ ] **Step 6: Commit**

```bash
git add crates tests
git commit -m "test: use placeholder hosts, addresses, and paths in test data"
```

---

### Task 2: hive-panel identity and tests

**Files:**
- Modify: `apps/hive-panel/src-tauri/tauri.conf.json:3,5,15`
- Modify: `apps/hive-panel/index.html:6`
- Modify: `apps/hive-panel/src/main.tsx:20`
- Modify: `apps/hive-panel/src/fleet.test.ts:23,29,38,53`
- Modify: `apps/hive-panel/src/fleetView.test.ts:7`
- Modify: `apps/hive-panel/src-tauri/src/terminal.rs:198,200,208,212`

**Interfaces:** none.

- [ ] **Step 1: Set the app identity**

`apps/hive-panel/src-tauri/tauri.conf.json`:

```json
  "productName": "hive-panel",
  "version": "0.2.0",
  "identifier": "dev.torsor.hive",
```

and in `app.windows[0]`:

```json
        "title": "hive",
```

`apps/hive-panel/index.html` line 6:

```html
    <title>hive</title>
```

`apps/hive-panel/src/main.tsx` line 20:

```ts
const APP_LABEL = "hive";
```

- [ ] **Step 2: Rename test hosts**

`apps/hive-panel/src/fleet.test.ts`:

```ts
        { host: "box-a", sessions: [] },
// ...
    expect(rows[0]).toMatchObject({ host: "box-a", state: "idle", actionable: false });
// ...
          host: "hub",
// ...
    expect(rows[0].host).toBe("hub");
```

`apps/hive-panel/src/fleetView.test.ts` line 7:

```ts
    host: "box-a",
```

`apps/hive-panel/src-tauri/src/terminal.rs` tests:

```rust
        let argv = ssh_shell_at_argv("hub", "/home/me/my project");
        assert_eq!(argv[0], "ssh");
        assert_eq!(argv[1], "hub");
// ...
        let argv = ssh_shell_at_argv("box-a", "/home/me/my project");
        let line = ssh_command_line(&argv);
        assert_eq!(
            line,
            "ssh 'box-a' -t 'cd '\\''/home/me/my project'\\'' && exec ${SHELL:-/bin/zsh} -l'"
        );
```

- [ ] **Step 3: Run the panel checks**

```bash
cd apps/hive-panel && { [ -d node_modules ] || npm install; } && npm run check && npm test 2>&1 | tail -6
cd src-tauri && cargo test 2>&1 | grep -E '^test result'
```

Expected: tsc clean, vitest all passing, `test result: ok`.

- [ ] **Step 4: Commit**

```bash
git add apps/hive-panel
git commit -m "hive-panel: name the app hive, drop the working name from identity and tests"
```

---

### Task 3: hive-web phone console

**Files:**
- Modify: `apps/hive-web/src/hub.js:22-44`
- Modify: `apps/hive-web/test/hub.test.js`
- Modify: `apps/hive-web/test/fleet.test.js:7,16,25`
- Modify: `apps/hive-web/test/paths.test.js:13,15,22,23`
- Modify: `apps/hive-web/index.html:11,19`
- Rename: `apps/hive-web/public/config.json` → `apps/hive-web/public/config.example.json`
- Modify: `.gitignore`
- Modify: `apps/hive-web/README.md`
- Modify: `crates/hive-web/Cargo.toml:7`

**Interfaces:**
- Produces: `sameOriginHub(hostname: string): string` exported from `apps/hive-web/src/hub.js`. Returns `http://<hostname>:8787`, or `""` for an empty hostname.

- [ ] **Step 1: Write the failing test**

Append to `apps/hive-web/test/hub.test.js` (and add `sameOriginHub` to the import list at the top):

```js
describe("sameOriginHub", () => {
  it("points at the hub port on the page host", () => {
    expect(sameOriginHub("hub")).toBe("http://hub:8787");
  });

  it("is empty without a hostname", () => {
    expect(sameOriginHub("")).toBe("");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd apps/hive-web && npm test 2>&1 | tail -15
```

Expected: FAIL, `sameOriginHub is not a function` or not exported.

- [ ] **Step 3: Implement the same-origin fallback**

In `apps/hive-web/src/hub.js`, add after `normalizeHubUrl`:

```js
/** Hub URL on the host that served this page (production: nginx on the hub). */
export function sameOriginHub(hostname) {
  const name = String(hostname || "").trim();
  return name ? `http://${name}:${DEFAULT_HUB_PORT}` : "";
}
```

and replace the last two lines of `resolveHubUrl` (`hubBase = normalizeHubUrl("...");` and its `return`) with:

```js
  const pageHost = typeof window !== "undefined" ? window.location.hostname : "";
  hubBase = sameOriginHub(pageHost) || `http://localhost:${DEFAULT_HUB_PORT}`;
  return hubBase;
```

- [ ] **Step 4: Rename hosts in the remaining web tests**

`apps/hive-web/test/hub.test.js`:

```js
    expect(normalizeHubUrl("hub")).toBe("http://hub:8787");
// ...
  const hub = "http://hub:8787";
// ...
    expect(fleetUrl(hub)).toBe("http://hub:8787/v1/fleet");
// ...
    expect(hostUrl(hub, "box-a", "/v1/sessions/demo/attach")).toBe(
      "http://hub:8787/v1/hosts/box-a/v1/sessions/demo/attach",
    );
// ...
      "http://hub:8787/v1/hosts/my%20box/v1/sessions/my%20task/say",
```

`apps/hive-web/test/fleet.test.js`: `"<REAL_BOX_B>"` → `"box-b"` (line 7), `"<REAL_BOX_A>"` → `"box-a"` (lines 16, 25). Fill the tokens from the runbook's substitution table.

`apps/hive-web/test/paths.test.js`:

```js
    expect(sshShellAtArgv("box-a", "/srv/lab/hive")).toEqual([
      "ssh",
      "box-a",
// ...
    expect(sshShellAtCommand("hub", "/home/me/proj")).toBe(
      "ssh 'hub' -t 'cd '\\''/home/me/proj'\\'' && exec ${SHELL:-/bin/zsh} -l'",
```

- [ ] **Step 5: Title, badge, example config, gitignore, crate description**

`apps/hive-web/index.html`:

```html
    <title>hive</title>
<!-- ... -->
    <span class="app-label-badge" aria-hidden="true">hive</span>
```

```bash
git mv apps/hive-web/public/config.json apps/hive-web/public/config.example.json
printf '{\n  "hub": "http://hub:8787"\n}\n' > apps/hive-web/public/config.example.json
```

Append to `.gitignore`:

```
# hive-web hub config is generated by ansible (roles/hive_web/templates/config.json.j2)
apps/hive-web/public/config.json
```

`crates/hive-web/Cargo.toml` line 7:

```toml
description = "Static phone console server for hive"
```

- [ ] **Step 6: Rewrite the hive-web README**

Replace `apps/hive-web/README.md` with:

````markdown
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
````

(The spec link target is renamed in Task 6; the link is correct after that task.)

- [ ] **Step 7: Run the web checks**

```bash
cd "$(git rev-parse --show-toplevel)" && make web-check 2>&1 | grep -E 'Tests|Test Files|test result|FAIL'
```

Expected: vitest `Tests … passed`, no FAIL, `test result: ok` for hive-web and hive-common.

- [ ] **Step 8: Commit**

```bash
git add -A apps/hive-web crates/hive-web .gitignore
git commit -m "hive-web: same-origin hub fallback, example config, neutral names"
```

---

### Task 4: Ansible inventory and vars move to `~/.hive/fleet/`

**Files:**
- Modify: `ansible/ansible.cfg`
- Create: `ansible/inventory.example.yml`
- Create: `ansible/group_vars.example/all/vars.yml`
- Delete: `ansible/inventory.yml`, `ansible/group_vars/all/vars.yml`
- Modify: `ansible/tests/render_hostsfile.yml`
- Modify: `ansible/roles/hive_web/tasks/main.yml` (task order only)
- Create (outside repo): `~/.hive/fleet/inventory.yml`, `~/.hive/fleet/group_vars/all/vars.yml`

**Interfaces:**
- Produces: inventory group `hive_hub` (the hub host), used by the Makefile in Task 5.
- Produces: `~/.hive/fleet/` as the inventory location every Ansible entry point relies on.

- [ ] **Step 1: Copy the real inventory and vars out of the tree first**

This preserves the author's deploy. It copies tracked files verbatim; it does not invent hostnames.

```bash
mkdir -p ~/.hive/fleet/group_vars/all
cp ansible/inventory.yml ~/.hive/fleet/inventory.yml
cp ansible/group_vars/all/vars.yml ~/.hive/fleet/group_vars/all/vars.yml
ls -la ~/.hive/fleet ~/.hive/fleet/group_vars/all
```

Then edit `~/.hive/fleet/inventory.yml` by hand: add a top-level `hive_hub` group whose only host is the one that currently carries `hive_hub_enabled: true`, and move that flag into the group's `vars` (exact shape in Step 3, with the real name in place of `hub`). Confirm:

```bash
cd ansible && ansible-inventory -i ~/.hive/fleet/inventory.yml --graph
ansible-inventory -i ~/.hive/fleet/inventory.yml --host "$(ansible-inventory -i ~/.hive/fleet/inventory.yml --graph hive_hub | sed -n 's/^ *|--//p' | head -1)" | grep hive_hub_enabled
```

Expected: `@hive_hub` group with one host, and that host reports `"hive_hub_enabled": true`.

- [ ] **Step 2: Point `ansible.cfg` at it**

`ansible/ansible.cfg`, `[defaults]`:

```ini
# Fleet inventory lives outside the repo. Copy inventory.example.yml and
# group_vars.example/ to ~/.hive/fleet/ and edit (see README.md).
inventory = ~/.hive/fleet/inventory.yml
```

(Replace the existing `inventory = inventory.yml` line. Keep the other keys.)

- [ ] **Step 3: Write the example inventory**

`ansible/inventory.example.yml`:

```yaml
# Example hive fleet inventory.
# Copy to ~/.hive/fleet/inventory.yml and replace the placeholder names with
# your Tailscale / SSH hostnames. ~/.hive/hosts on the laptop and on the hub
# are GENERATED from that file by `ansible-playbook site.yml`.
hive_hosts:
  hosts:
    hub:
    box-a:
    box-b:

# The one always-on host that runs hive-hub and serves the phone console.
hive_hub:
  hosts:
    hub:
  vars:
    hive_hub_enabled: true

control:
  hosts:
    localhost:
      ansible_connection: local
```

- [ ] **Step 4: Write the example vars**

`ansible/group_vars.example/all/vars.yml`:

```yaml
# Example shared vars. Copy to ~/.hive/fleet/group_vars/all/vars.yml and edit.
# Ansible loads group_vars/ from the directory that holds the inventory file.
hive_repo_url: https://github.com/OWNER/hive.git   # what the boxes clone
hive_ref: main                                     # branch, tag, or SHA to converge
hive_checkout_dir: /srv/lab/hive
hive_bin_dir: "{{ hive_checkout_dir }}/bin"
# Control-node repo root (ansible/..). SPA builds here; Rust builds on the boxes.
hive_web_local_repo: "{{ playbook_dir }}/.."
hive_web_dist_dir: "{{ hive_checkout_dir }}/web/dist"
hive_home: "{{ ansible_facts.env.HOME }}/.hive"
hive_host_port: 8788
hive_hub_port: 8787
hive_web_base_path: hive          # URL path → http://<hub>/hive/
hive_web_nginx_port: 80
hive_hub_host: hub                # written to the laptop's client.toml

client_hostsfile_path: "{{ ansible_facts.env.HOME }}/.hive/hosts"
client_config_path: "{{ ansible_facts.env.HOME }}/.hive/client.toml"
```

- [ ] **Step 5: Remove the real files from the tree**

```bash
git rm -q ansible/inventory.yml ansible/group_vars/all/vars.yml
git status -s ansible
```

Expected: two `D` lines, two `??` lines for the example files.

- [ ] **Step 6: Point the render test at the example inventory**

`ansible/tests/render_hostsfile.yml`:

```yaml
# Prove hosts-file generation against the example inventory, writing only to /tmp.
# Run from ansible/: ansible-playbook -i inventory.example.yml tests/render_hostsfile.yml
- hosts: localhost
  connection: local
  gather_facts: true
  tasks:
    - ansible.builtin.include_role:
        name: client_hostsfile
      vars:
        client_hostsfile_path: /tmp/hive-ansible-test-hosts.d/hosts
    - name: Show generated hosts file
      ansible.builtin.command: cat /tmp/hive-ansible-test-hosts.d/hosts
      changed_when: false
      register: hosts_out
    - ansible.builtin.assert:
        that:
          - "'hub' in hosts_out.stdout"
          - "'box-a' in hosts_out.stdout"
          - "'box-b' in hosts_out.stdout"
          - "'GENERATED' in hosts_out.stdout"
        fail_msg: "generated hosts file incomplete — inspect /tmp/hive-ansible-test-hosts.d/hosts"
```

- [ ] **Step 7: Write `config.json` before mirroring the SPA**

In `ansible/roles/hive_web/tasks/main.yml`, move the three tasks `Resolve Tailscale IPv4 bind address for hive-web`, `Set effective hive-web bind address`, and `Phone SPA hub config.json` so they come immediately **after** `Deploy phone SPA dist to hub` and **before** `Mirror phone SPA dist to legacy nginx docroot`. No task text changes. Reason: the tracked `config.json` no longer rides along in `dist/`, so the mirror must copy the templated one.

Resulting task order: Ensure bin dir → Build SPA → Ensure dist dir → Deploy dist → Resolve IP → Set bind → config.json → Mirror → Install nginx → Deploy site → Enable site → Ensure nginx → Stop hive-web unit.

- [ ] **Step 8: Verify syntax, the render test, and the real inventory**

```bash
cd ansible
ansible-playbook --syntax-check site.yml
ansible-playbook -i inventory.example.yml tests/render_hostsfile.yml 2>&1 | tail -4
ansible-inventory --graph            # uses ansible.cfg → ~/.hive/fleet
ansible-playbook site.yml --check --diff --limit localhost 2>&1 | tail -8
```

Expected: syntax ok; render test `failed=0`; the graph shows the real hosts and an `@hive_hub` group; the localhost check run reports `failed=0` and `changed=0` (the laptop's `~/.hive/hosts` and `client.toml` are already what the inventory generates).

- [ ] **Step 9: Commit**

```bash
cd "$(git rev-parse --show-toplevel)"
git add -A ansible
git commit -m "ansible: read inventory and vars from ~/.hive/fleet, ship examples"
```

---

### Task 5: Scripts, Makefile, hosts.example

**Files:**
- Modify: `Makefile:48,139-140`
- Modify: `bin/hive-deploy:2-8,26-30,83-85`
- Modify: `install-host.sh:39`, `install-hub.sh:33`
- Modify: `hosts.example:5`

**Interfaces:**
- Consumes: inventory group `hive_hub` from Task 4.

- [ ] **Step 1: Makefile**

Line 48:

```make
	@echo "  make deploy-web        ansible --tags web --limit hive_hub (phone SPA)"
```

Target:

```make
deploy-web:
	cd ansible && ansible-playbook site.yml -K --tags web --limit hive_hub
```

- [ ] **Step 2: hive-deploy header, inventory check, footer**

Header (lines 2-8):

```sh
# Push merged code to hosts: git pull in the fleet checkout, rebuild, reinstall units.
#
#   hive-deploy box-a               one host
#   hive-deploy box-a box-b         several
#   hive-deploy --all               every host in ~/.hive/fleet/inventory.yml
#   hive-deploy -n box-a            dry run (--check --diff)
#   hive-deploy -K box-a box-b --tags linger   one-time: enable-linger (sudo)
```

After the `ansible-playbook` PATH check (after line 35), add:

```sh
if [ -z "${ANSIBLE_INVENTORY:-}" ] && [ ! -f "$HOME/.hive/fleet/inventory.yml" ]; then
  echo "hive-deploy: no ~/.hive/fleet/inventory.yml — copy ansible/inventory.example.yml there and edit" >&2
  exit 1
fi
```

Footer heredoc:

```sh
cat <<'NOTE'

Deployed. Confirm from the laptop:

    hive status
    ssh HOST 'systemctl --user is-active hive-host'

Hub:

    ssh HUB 'systemctl --user is-active hive-hub'
NOTE
```

- [ ] **Step 3: Install shortcuts and hosts.example**

`install-host.sh` line 39 and `install-hub.sh` line 33: replace the real hostname after `--migrate` with `hub`.

`hosts.example` line 5: `# hub`.

- [ ] **Step 4: Verify**

```bash
sh -n bin/hive-deploy && bash -n install-host.sh && bash -n install-hub.sh
bin/hive-deploy -h | head -8
make -n deploy-web
git grep -n -i -E '\b(<REAL_HUB>|<REAL_BOX_A>|<REAL_BOX_B>|<REAL_BOX_C>)\b' -- Makefile bin install-host.sh install-hub.sh hosts.example; echo "grep exit $?"
```

(fill the tokens from the runbook's substitution table)

Expected: no syntax errors, help shows `box-a` examples, `make -n` shows `--limit hive_hub`, grep exit `1` (no matches). If the grep exit is `0`, fix the listed lines.

- [ ] **Step 5: Commit**

```bash
git add Makefile bin/hive-deploy install-host.sh install-hub.sh hosts.example
git commit -m "scripts: placeholder hosts in usage text, deploy-web targets the hive_hub group"
```

---

### Task 6: Rename the August spec files and fix links

**Files:**
- Rename: `docs/superpowers/specs/2026-08-12-hive-v2-cutover.md` → `2026-08-12-hive-cutover.md`
- Rename: `docs/superpowers/specs/2026-08-12-hive-v2-design.md` → `2026-08-12-hive-design.md`
- Rename: `docs/superpowers/specs/2026-08-12-hive-v2-refactor-plan.md` → `2026-08-12-hive-refactor-plan.md`
- Rename: `docs/superpowers/specs/2026-08-15-hive-v2-phone-console.md` → `2026-08-15-hive-phone-console.md`
- Modify: every file that links to them (`CLAUDE.md`, `README.md`, `apps/hive-web/README.md`, `docs/cheat-sheet.md`, `docs/superpowers/specs/2026-08-13-hive-install-locations.md`, `docs/superpowers/specs/2026-08-14-hive-desktop-app-design.md`, `docs/superpowers/plans/2026-08-16-hive-web-nginx-ansible.md`)

- [ ] **Step 1: Rename**

```bash
cd docs/superpowers/specs
git mv 2026-08-12-hive-v2-cutover.md 2026-08-12-hive-cutover.md
git mv 2026-08-12-hive-v2-design.md 2026-08-12-hive-design.md
git mv 2026-08-12-hive-v2-refactor-plan.md 2026-08-12-hive-refactor-plan.md
git mv 2026-08-15-hive-v2-phone-console.md 2026-08-15-hive-phone-console.md
cd "$(git rev-parse --show-toplevel)"
```

- [ ] **Step 2: Rewrite the links**

```bash
SELF=":!docs/superpowers/specs/2026-09-15-* :!docs/superpowers/plans/2026-09-15-*"
git grep -l -- '-hive-v2-' -- . $SELF | xargs sed -i '' \
  -e 's/2026-08-12-hive-v2-cutover\.md/2026-08-12-hive-cutover.md/g' \
  -e 's/2026-08-12-hive-v2-design\.md/2026-08-12-hive-design.md/g' \
  -e 's/2026-08-12-hive-v2-refactor-plan\.md/2026-08-12-hive-refactor-plan.md/g' \
  -e 's/2026-08-15-hive-v2-phone-console\.md/2026-08-15-hive-phone-console.md/g'
```

- [ ] **Step 3: Check every relative Markdown link resolves**

Save as a scratch script and run it from the repo root:

```bash
python3 - <<'EOF'
import re, subprocess, pathlib
files = subprocess.check_output(["git","ls-files","*.md"], text=True).split()
bad = 0
for f in files:
    text = pathlib.Path(f).read_text()
    for m in re.finditer(r'\]\(([^)#\s]+)(?:#[^)]*)?\)', text):
        t = m.group(1)
        if re.match(r'https?://|mailto:', t): continue
        p = (pathlib.Path(f).parent / t).resolve()
        if not p.exists():
            print(f"{f}: broken link -> {t}"); bad += 1
print("broken:", bad)
EOF
git grep -n -- '-hive-v2-' -- . $SELF ; echo "grep exit $?"
```

Expected: `broken: 0` and grep exit `1`.

- [ ] **Step 4: Commit**

```bash
git add -A docs CLAUDE.md README.md apps/hive-web/README.md
git commit -m "docs: drop the working name from spec filenames and links"
```

---

### Task 7: Neutralise prose in docs, README, CLAUDE.md, Ansible README

**Files:**
- Modify: `docs/superpowers/specs/*.md` (except `2026-09-15-hive-config-boundary.md`), `docs/superpowers/plans/2026-08-16-hive-web-nginx-ansible.md`, `README.md`, `CLAUDE.md`, `ansible/README.md`, `apps/hive-web/README.md`

**Interfaces:** none. Text only. `docs/cheat-sheet.md` is left alone here; Task 8 removes it.

- [ ] **Step 1: Apply the substitution table mechanically**

Read the substitution table in `../notes/2026-09-15-public-split-runbook.md`. For each `Real → Placeholder` hostname row, run over the files listed above (not the cheat sheet):

```bash
FILES=$(git ls-files 'docs/superpowers/**/*.md' README.md CLAUDE.md ansible/README.md apps/hive-web/README.md | grep -v config-boundary)
# one -e per row of the table; word boundaries so "hub" inside other words is untouched
sed -i '' -e 's/\b<REAL_HUB>\b/hub/g' -e 's/\b<REAL_BOX_A>\b/box-a/g' -e 's/\b<REAL_BOX_B>\b/box-b/g' -e 's/\b<REAL_BOX_C>\b/box-c/g' $FILES
```

(BSD sed lacks `\b`; use `[[:<:]]` and `[[:>:]]` on macOS: `s/[[:<:]]<REAL_HUB>[[:>:]]/hub/g`.)

Then the URL and login rows:

```bash
sed -i '' -e 's#http://<REAL_FORGE_HOST>:3000/[a-z]*/hive\.git#https://github.com/OWNER/hive.git#g' $FILES
sed -i '' -e 's#/home/<REAL_LOGIN>#/home/user#g' -e 's/operator (`<REAL_LOGIN>`)/operator (your login)/' $FILES
```

- [ ] **Step 2: Project-name wording**

```bash
sed -i '' \
  -e 's/# hive v2 design notes/# hive design notes/' \
  -e 's/# Hive v2 refactor + testing plan/# Hive refactor + testing plan/' \
  -e 's/# Design: Hive v2 phone console/# Design: Hive phone console/' \
  -e 's/\.worktrees\/hive-v2/.worktrees\/hive/' \
  -e 's/git push origin hive-v2/git push origin main/' \
  -e 's/after pushing hive-v2 to forge/after pushing main to your git remote/' \
  -e 's/| cc-\* (legacy) | hive v2 |/| older toolkit | hive |/' \
  -e 's/| | cc-\* | hive v2 |/| | older toolkit | hive |/' \
  -e 's/Hive v2 mixes/Hive mixes/' \
  -e 's/Hive v2 already separates/Hive already separates/' \
  -e 's/desktop application for hive v2/desktop application for hive/' \
  -e 's/phone console for hive v2/phone console for hive/' \
  -e 's/No tokens in v1\./No tokens./' \
  $FILES
```

Then edit by hand in `docs/superpowers/specs/2026-08-15-hive-phone-console.md` line 10:

```markdown
Port the older toolkit's **`hive web`** phone console to hive. The hive stack already exposes the needed HTTP API on **hive-hub**; this track adds a touch-first static SPA and a production static host on the hub machine.
```

- [ ] **Step 3: Hand-read the four fleet-heavy docs**

Open and read top to bottom, fixing sentences the substitution left awkward (for example a list of three placeholder boxes where "each box" reads better, or a parenthetical about which real host needed sudo fixes, which becomes "some hosts"):

- `docs/superpowers/specs/2026-08-13-hive-install-locations.md`
- `docs/superpowers/specs/2026-08-15-hive-phone-console.md`
- `docs/superpowers/plans/2026-08-16-hive-web-nginx-ansible.md`
- `ansible/README.md` (the `-K` paragraph about root-owned `/srv/lab` becomes: "`-K` is required on first converge when `/srv/lab` is root-owned or when `base_packages` must install apt packages." The Ubuntu sudo-rs note stays, without the parenthetical naming a host.)

- [ ] **Step 4: Leak check**

Run the grep from the runbook's **Verification** section, restricted to `$FILES`, plus:

```bash
git grep -n -i -E 'hive-v2|hive v2|\bv1\b|\bv2\b' -- $FILES ; echo "grep exit $?"
```

Expected: exit `1` for both. The only allowed `v2` is inside a URL path such as `schema.tauri.app/config/2`, which this grep does not match.

- [ ] **Step 5: Commit**

```bash
git add -A docs README.md CLAUDE.md ansible/README.md apps/hive-web/README.md
git commit -m "docs: describe machines by role, name the project hive"
```

---

### Task 8: Cheat sheet out, operations section in, README and CLAUDE.md, LICENSE

**Files:**
- Move (out of repo): `docs/cheat-sheet.md` → `../notes/cheat-sheet.md`
- Modify: `ansible/README.md` (full rewrite below)
- Modify: `README.md` (full rewrite below)
- Modify: `CLAUDE.md` (targeted edits)
- Create: `LICENSE`
- Modify (outside repo): `../CLAUDE.md` (the thing-level file)

- [ ] **Step 1: Move the cheat sheet**

```bash
cp docs/cheat-sheet.md ../notes/cheat-sheet.md
git rm -q docs/cheat-sheet.md
ls ../notes
```

- [ ] **Step 2: Rewrite `ansible/README.md`**

Keep the existing **Tags** table verbatim. Replace everything else with:

````markdown
# Hive fleet provisioning

`site.yml` is the runbook. Design: [install locations](../docs/superpowers/specs/2026-08-13-hive-install-locations.md), [desktop app](../docs/superpowers/specs/2026-08-14-hive-desktop-app-design.md), [config boundary](../docs/superpowers/specs/2026-09-15-hive-config-boundary.md).

Control node: your laptop with `ansible-playbook` on PATH (`uv tool install ansible-core`).

## One-time: describe your fleet

Inventory and shared vars live **outside the repo**, under `~/.hive/fleet/`:

```bash
mkdir -p ~/.hive/fleet/group_vars/all
cp inventory.example.yml ~/.hive/fleet/inventory.yml
cp group_vars.example/all/vars.yml ~/.hive/fleet/group_vars/all/vars.yml
$EDITOR ~/.hive/fleet/inventory.yml ~/.hive/fleet/group_vars/all/vars.yml
```

- `inventory.yml`: your Tailscale / SSH hostnames in `hive_hosts`; the always-on hub in `hive_hub`.
- `vars.yml`: `hive_repo_url` (what the boxes clone) and `hive_ref` (branch, tag, or SHA).

`ansible.cfg` points at that inventory, so run every command below from this
directory. `~/.hive/hosts` and `~/.hive/client.toml` on the laptop, and
`~/.hive/hosts` on the hub, are GENERATED from the inventory. Edit the inventory,
not those files.

## Prerequisites on each box

- Reachable over SSH by the inventory hostname; Tailscale up.
- `hive_ref` pushed to `hive_repo_url` and reachable from the box.

### Ubuntu 25.10+ / 26.04: classic sudo

Those releases may route `sudo` through **sudo-rs**, which Ansible's `-K` become
cannot drive (timeout waiting for privilege escalation prompt). Classic sudo is
already installed; flip once per host **before** the first converge:

```bash
ssh -t <box> 'sudo update-alternatives --set sudo /usr/bin/sudo.ws'
```

## Converge

```bash
ansible-playbook site.yml -K                        # full fleet + laptop client files
ansible-playbook site.yml -K --limit box-a          # one host only (no laptop hosts file)
ansible-playbook site.yml -K --limit box-a,localhost
ansible-playbook site.yml -K --check --diff         # drift preview
ansible-playbook site.yml -K --tags web --limit hive_hub   # phone SPA (builds on laptop, copies to hub)
```

From the repo root: `make deploy-web` is the last line. Day-to-day code updates:

```bash
hive-deploy box-a box-b    # or: hive-deploy --all
```

`-K` (sudo password) is required on first converge when `/srv/lab` is root-owned
or when `base_packages` must install apt packages. Later runs usually need no `-K`.

Recommended first converge: one low-stakes box plus `localhost`, then the fleet,
then `hive status` from the laptop.

## After converge

On each box:

```bash
systemctl --user is-active hive-host                 # active
curl -s http://$(tailscale ip -4):8788/v1/sessions   # []
```

On the hub: `systemctl --user is-active hive-hub` and `~/.hive/hosts` lists every box.

User units need **linger** or `hive-host` stops when nobody is logged in:

```bash
sudo loginctl enable-linger $USER      # or: hive-deploy -K <box> --tags linger
```

## Procedures

- **Add a host:** one entry in `~/.hive/fleet/inventory.yml`, then `--limit newhost,localhost` (or a full run).
- **Pin a rollout:** set `hive_ref` in `~/.hive/fleet/group_vars/all/vars.yml` to a tag or SHA.
- **Move the hub:** move the host under `hive_hub` in the inventory, then rerun `--tags hub,hive,web`.
- **Change where boxes fetch code:** set `hive_repo_url`; the next converge updates each checkout's remote.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Host `(unreachable)` from hub | `hive-host` down, wrong poll URL, or tailnet path | `systemctl --user status hive-host`; hub `~/.hive/hosts` uses Tailscale IPs; `curl http://<box-ip>:8788/v1/sessions` from the hub |
| Host flaps reachable/unreachable | linger off, so the user manager stops | `sudo loginctl enable-linger $USER`; restart `hive-host` |
| Ansible `-K` hangs | sudo-rs | `update-alternatives --set sudo /usr/bin/sudo.ws` |
| `hive status` shows stale sessions | old binary on box | `hive-deploy <host>` |

## Tags

(unchanged table)

## Tests

```bash
ansible-playbook -i inventory.example.yml tests/render_hostsfile.yml
```
````

- [ ] **Step 3: Rewrite `README.md`**

````markdown
# hive

Rust fleet console for Claude/Codex agents running in tmux on always-on boxes.

Laptop may sleep. Boxes and one designated **hub** stay up. Clients talk HTTP
to the hub; the hub polls each box's **hive-host**. SSH is only for install and
attach.

## Pieces

| Binary | Role |
|--------|------|
| `hive-host` | Per-box daemon (`~/.hive/sessions`, tmux, transcripts) — port **8788** |
| `hive-hub` | Always-on aggregator + action proxy — port **8787** |
| `hive` | CLI (`status`, `say`, `stop`, `run`, …) |
| `hive-panel` | Tauri GUI — hub HTTP/SSE only |
| `hive-web` | Phone Safari console, served by nginx on the hub |

Shared types: `crates/hive-protocol`. Specs: [design](docs/superpowers/specs/2026-08-12-hive-design.md), [install locations](docs/superpowers/specs/2026-08-13-hive-install-locations.md), [config boundary](docs/superpowers/specs/2026-09-15-hive-config-boundary.md).

## Layout

```
crates/hive-protocol|common|client|host|hub|web|cli
apps/hive-panel/       # desktop GUI (Tauri)
apps/hive-web/         # phone SPA
ansible/               # fleet provisioning (authoritative deploy path)
bin/hive-deploy        # day-2 code push wrapper
install-host.sh        # dev/bootstrap shortcut only
install-hub.sh         # dev/bootstrap shortcut only
hosts.example          # seed for ~/.hive/hosts when not using Ansible
tests/fixtures/        # transcript golden files
```

## Bootstrap (fleet)

Production deploy uses Ansible. Your fleet's inventory lives in `~/.hive/fleet/`,
never in this repo. See [ansible/README.md](ansible/README.md) for the copy step,
converge order, and operations.

```bash
# control node: install deploy helper
make install-ops

# describe your fleet (see ansible/README.md), then converge
cd ansible
ansible-playbook site.yml -K --limit box-a,localhost
ansible-playbook site.yml -K

# laptop CLI
cargo install --path crates/hive-cli
hive status

# day-2 code updates
hive-deploy --all

# GUI
cd apps/hive-panel && npm install && npm run tauri dev
```

Dev/bootstrap without Ansible (single host, quick iteration):

```bash
./install-host.sh box-a
./install-hub.sh hub
```

Local smoke (no Tailscale):

```bash
cargo run -p hive-host -- serve --dev --bind 127.0.0.1
cargo run -p hive-hub  -- serve --dev --bind 127.0.0.1
printf 'hub = "http://127.0.0.1:8787"\n' > ~/.hive/client.toml
cargo run -p hive-cli -- status
```

## Config (`HIVE_HOME`, default `~/.hive/`)

| Path | Where |
|------|--------|
| `fleet/inventory.yml`, `fleet/group_vars/all/vars.yml` | laptop (Ansible control node) |
| `sessions/<task>.meta` (+ `.labels`) | each box |
| `host.toml` | each box (`bind`, `port`) |
| `hosts` | hub (GENERATED from the inventory) |
| `hub.toml` | hub |
| `client.toml` | laptop (`hub = "…"`, GENERATED from the inventory) |

Bind production addresses to Tailscale CGNAT (`100.64.0.0/10`) or use `--dev` + loopback.

## Trust

Tailscale membership is the auth boundary. No tokens.

## Attach

External terminal only: hub returns `ssh -t host tmux attach …`. No embedded terminal in the panel.

## License

MIT, see [LICENSE](LICENSE).
````

- [ ] **Step 4: Edit `CLAUDE.md` (repo root)**

Line 5: `at \`http://<hub>/hive/\` via nginx on the hub; **hive-web** is local dev/smoke only.`

In the commands block:

```bash
hive-web serve                         # phone SPA local dev/smoke only (prod: http://<hub>/hive/)
cd apps/hive-web && npm run dev          # phone UI dev → VITE_HUB_URL=http://<hub>:8787
```

Replace the `Cheat sheet:` line at the bottom of the Architecture section with:

```markdown
Config boundary: [docs/superpowers/specs/2026-09-15-hive-config-boundary.md](docs/superpowers/specs/2026-09-15-hive-config-boundary.md). Operations: [ansible/README.md](ansible/README.md).
```

Add to **Do not**:

```markdown
- Put any real hostname, tailnet address, or repo URL in the tree. Fleet config lives in `~/.hive/fleet/`; examples use `hub`, `box-a`, `box-b`.
```

- [ ] **Step 5: LICENSE**

Create `LICENSE` with the MIT text. The copyright line is `Copyright (c) 2026 Danny Krashen` (the Cargo workspace already declares `license = "MIT"`; the author confirmed publishing under their own name).

```
MIT License

Copyright (c) 2026 Danny Krashen

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 6: Thing-level `../CLAUDE.md` (outside repo)**

Rewrite the file one level above the repo as:

```markdown
# hive

Fleet console for agent sessions (Claude Code, Codex) running in tmux on always-on boxes.
The laptop is a **client** and may sleep; the boxes and one designated **hub** stay up.

## Layout

```
hive-repo/          the code (git; branch `main`; public on GitHub)
notes/              this installation: cheat sheet, runbooks, fleet-specific reminders
thing.yaml          this thing's metadata
```

Work happens in `hive-repo/`. It carries its own `CLAUDE.md` with the architecture rules and
the commands agents should know — **read that first** when touching code. This file is the
thing-level orientation only.

The repo is public and must never contain this fleet's hostnames, addresses, or repo URL.
Fleet config lives in `~/.hive/fleet/` (see `hive-repo/ansible/README.md`); everything
about *this* installation that is not a config file lives in `notes/`.

## Architecture in one paragraph

(keep the existing paragraph unchanged)

## Trust boundary

(keep unchanged)

## Deploy

Ansible is the authoritative path (`ansible/site.yml`, see `hive-repo/ansible/README.md`);
`hive-deploy` is the day-2 code push wrapper; `install-host.sh` / `install-hub.sh` are
dev/bootstrap shortcuts only. `notes/cheat-sheet.md` is the operational quick reference for
this fleet, and the design specs live in `hive-repo/docs/superpowers/specs/`.

## Managed binaries

`thing install apply` here builds the laptop-side pair: `hive` (from `crates/hive-cli`) and
`hive-panel-app` (Tauri bundle, macOS). `hive-host` and `hive-hub` are deliberately not
declared — the boxes get those from Ansible.

## Related things

- `hive-old` — the retired SSH/tmux `cc-*` toolkit this replaced
- `lab-agents` — agent handoff registry and desktop panel across machines
- `skald` — ships writing work to a box and starts a detached Claude session
- `thing` — the project-organization CLI this thing is registered with
```

- [ ] **Step 7: Verify**

```bash
git ls-files docs/cheat-sheet.md | wc -l     # 0
test -f ../notes/cheat-sheet.md && echo notes-ok
python3 - <<'EOF'
import re, subprocess, pathlib
files = subprocess.check_output(["git","ls-files","*.md"], text=True).split()
bad = 0
for f in files:
    for m in re.finditer(r'\]\(([^)#\s]+)(?:#[^)]*)?\)', pathlib.Path(f).read_text()):
        t = m.group(1)
        if re.match(r'https?://|mailto:', t): continue
        if not (pathlib.Path(f).parent / t).resolve().exists():
            print(f"{f}: broken link -> {t}"); bad += 1
print("broken:", bad)
EOF
```

Expected: `0`, `notes-ok`, `broken: 0`.

- [ ] **Step 8: Commit**

```bash
git add -A LICENSE README.md CLAUDE.md ansible/README.md docs
git commit -m "docs: generic operations guide, MIT license, cheat sheet moved out of the repo"
```

---

### Task 9: Final verification on the branch

**Files:** none modified.

- [ ] **Step 1: Full test run**

```bash
cargo test --workspace 2>&1 | grep -E '^test result' | sort | uniq -c
make web-check 2>&1 | grep -E 'Test Files|test result|FAIL'
cd apps/hive-panel && npm run check && npm test 2>&1 | grep -E 'Test Files|FAIL'; cd src-tauri && cargo test 2>&1 | grep -E '^test result'; cd "$(git rev-parse --show-toplevel)"
cd ansible && ansible-playbook --syntax-check site.yml && ansible-playbook -i inventory.example.yml tests/render_hostsfile.yml 2>&1 | tail -3; cd ..
```

Expected: all `ok`, no `FAIL`, render test `failed=0`.

- [ ] **Step 2: Leak grep over the whole tree**

Run the grep in the runbook's **Verification** section over the tree (excluding `$SELF`), plus:

```bash
SELF=":!docs/superpowers/specs/2026-09-15-* :!docs/superpowers/plans/2026-09-15-*"
git grep -n -i -E 'hive-v2|hive v2' -- . $SELF ; echo "grep exit $?"
git grep -n -E '100\.(6[5-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.[0-9]+\.[0-9]+' -- . $SELF ; echo "grep exit $?"
```

Expected: exit `1` for every grep.

- [ ] **Step 3: Drift check against the real fleet (read-only)**

```bash
cd ansible && ansible-playbook site.yml --check --diff 2>&1 | tail -25
```

Expected: `failed=0` on every host. Acceptable `changed` items: the SPA `config.json` (now templated before the mirror), comment text in generated files, and the checkout task if `hive_ref` differs. Anything else is a regression: stop and report.

- [ ] **Step 4: Report**

Summarise for the user: test results, grep results, and the drift-check diff. Do not proceed to Task 10 without their explicit go-ahead.

---

### Task 10: Cutover (requires the user's explicit go-ahead)

**Files:**
- Modify (outside repo): `~/.hive/fleet/group_vars/all/vars.yml`, `../thing.yaml`

**Interfaces:**
- Produces: a public GitHub repo `hive` under the user's account, branch `main`, one commit.

- [ ] **Step 1: Orphan `main` with the scrubbed tree**

```bash
git checkout --orphan main
git rm -r -q --cached .
git add -A
git status -s | grep -v '^A ' ; echo "(anything above is unexpected)"
git commit -q -m "hive: fleet console for agent sessions on always-on boxes"
git log --oneline | cat        # exactly one line
```

- [ ] **Step 2: Remotes and GitHub**

```bash
git remote rename origin forge
gh repo create hive --public --source . --remote origin --push \
  --description "Fleet console for Claude Code and Codex agent sessions running in tmux on always-on boxes. Rust host/hub daemons, CLI, Tauri panel, phone console."
git remote -v
gh repo view --web
```

- [ ] **Step 3: Switch the boxes to the new remote**

In `~/.hive/fleet/group_vars/all/vars.yml` set `hive_repo_url` to the HTTPS clone URL `gh repo view --json url -q .url` prints, with `.git` appended, and `hive_ref: main`. Then:

```bash
hive-deploy -n --all          # dry run: expect the checkout task to show the remote/ref change
hive-deploy --all
hive status
```

Confirm on each box that `git -C /srv/lab/hive remote get-url origin` is the GitHub URL and `git -C /srv/lab/hive rev-parse --short HEAD` matches the new `main`.

Rollback: restore the previous `hive_repo_url` and `hive_ref` in the private vars and run `hive-deploy --all` again.

- [ ] **Step 4: Registry and notes**

In `../thing.yaml`, set `repos[0].origin` to the GitHub URL. Append a dated "done" line to `../notes/2026-09-15-public-split-runbook.md`. Optionally create a pull-mirror repo on the forge from its web UI (Migration → mirror from the GitHub URL); the old forge repo stays untouched as the archive.
