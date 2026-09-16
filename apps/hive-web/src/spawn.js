import { fleetUrl, hubFetch, listRemoteDirs, resolveHubUrl, spawnSession } from "./hub.js";
import { flattenFleet } from "./fleet.js";
import { appendPathCopyBlock, resolveRemotePath } from "./paths.js";

const $ = (id) => document.getElementById(id);

function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}

export function basenameOf(path) {
  const trimmed = String(path || "").replace(/\/+$/, "");
  const parts = trimmed.split("/");
  const last = parts[parts.length - 1] || "";
  if (last === "~" || last === "") return "";
  if (last.startsWith("~")) return last.slice(1) || "";
  return last;
}

function parentPath(cwd) {
  if (!cwd || cwd === "/") return null;
  const trimmed = cwd.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  if (idx <= 0) return "/";
  return trimmed.slice(0, idx) || "/";
}

function joinPath(cwd, name) {
  if (cwd === "/") return `/${name}`;
  return `${cwd.replace(/\/+$/, "")}/${name}`;
}

export function parseExtraArgs(raw) {
  return String(raw || "")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
}

function setSheetActionsVisible(visible) {
  const actions = document.querySelector("#sheet .sheet-actions");
  if (actions) actions.hidden = !visible;
}

async function loadFleetHosts() {
  const hub = await resolveHubUrl();
  const doc = await hubFetch(fleetUrl(hub));
  return [...new Set(flattenFleet(doc).map((r) => r.host).filter(Boolean))].sort();
}

/**
 * Open the spawn bottom sheet. Uses the shared #sheet dialog; hides default OK/Cancel.
 */
export function openSpawn({ say, hosts: presetHosts, onClosed }) {
  let hosts = presetHosts?.length ? [...presetHosts] : [];
  let host = hosts[0] || "";
  let root = "";
  let resolvedRoot = "";
  let resolvingRoot = false;
  let task = "";
  let taskTouched = false;
  let provider = "claude";
  let auto = false;
  let extraOpen = false;
  let extraArgs = "";
  let error = null;
  let spawning = false;
  let loadingHosts = !hosts.length;

  let browseOpen = false;
  let browsePath = "";
  let listing = null;
  let browseLoading = false;
  let browseError = null;
  let resolveTimer = null;

  const sheet = $("sheet");
  const prevCancel = sheet.oncancel;

  function cleanup(spawned) {
    setSheetActionsVisible(true);
    sheet.oncancel = prevCancel;
    if (sheet.open) sheet.close();
    onClosed?.(spawned);
  }

  sheet.oncancel = (e) => {
    e.preventDefault();
    cleanup(false);
  };

  setSheetActionsVisible(false);

  async function ensureHosts() {
    if (hosts.length) return;
    loadingHosts = true;
    render();
    try {
      hosts = await loadFleetHosts();
      host = hosts[0] || "";
    } catch (e) {
      error = String(e.message || e);
      hosts = [];
    } finally {
      loadingHosts = false;
      render();
    }
  }

  function onRootChange(value, { fromBrowse = false } = {}) {
    root = value;
    if (!taskTouched) task = basenameOf(value);
    if (fromBrowse) {
      resolvedRoot = value;
      resolvingRoot = false;
      render();
      return;
    }
    resolvedRoot = "";
    if (resolveTimer) clearTimeout(resolveTimer);
    const h = host.trim();
    const d = root.trim();
    if (!h || !d) {
      resolvingRoot = false;
      render();
      return;
    }
    resolvingRoot = true;
    render();
    resolveTimer = setTimeout(() => {
      void resolveRemotePath(h, d).then((path) => {
        if (root.trim() !== d) return;
        resolvedRoot = path;
        resolvingRoot = false;
        render();
      });
    }, 350);
  }

  async function loadBrowse(path) {
    if (!host.trim()) {
      browseError = "Select a host first";
      render();
      return;
    }
    browseLoading = true;
    browseError = null;
    render();
    try {
      listing = await listRemoteDirs(host.trim(), path);
      browsePath = listing.path;
    } catch (e) {
      browseError = String(e.message || e);
      listing = null;
    } finally {
      browseLoading = false;
      render();
    }
  }

  function openBrowse() {
    browseOpen = true;
    browseError = null;
    void loadBrowse("");
  }

  function selectBrowsePath() {
    if (!listing) return;
    onRootChange(listing.path, { fromBrowse: true });
    browseOpen = false;
    render();
  }

  async function onSpawn() {
    const h = host.trim();
    const d = root.trim();
    const t = task.trim();
    if (!h) {
      error = "Select a host";
      render();
      return;
    }
    if (!d) {
      error = "Enter a project root";
      render();
      return;
    }
    if (!t) {
      error = "Enter a task name";
      render();
      return;
    }
    spawning = true;
    error = null;
    render();
    try {
      const claudeOnly = provider === "claude";
      const msg = await spawnSession(h, {
        task: t,
        dir: d,
        provider,
        extra_args: parseExtraArgs(extraArgs),
        auto: claudeOnly ? auto : false,
        resume: false,
      });
      say(msg);
      cleanup(true);
    } catch (e) {
      error = String(e.message || e);
      spawning = false;
      render();
    }
  }

  function render() {
    const body = $("sheet-body");
    body.innerHTML = "";
    $("sheet-title").textContent = "Spawn session";

    if (browseOpen) {
      const panel = el("div", "spawn-browse");
      const toolbar = el("div", "spawn-browse-toolbar");
      const parent = listing ? parentPath(listing.path) : null;
      const up = el("button", "btn btn--sm", "Up");
      up.type = "button";
      up.disabled = !parent || browseLoading;
      up.onclick = () => void loadBrowse(parent);
      const cwd = el("span", "spawn-browse-cwd mono", browsePath || "…");
      cwd.title = browsePath;
      const back = el("button", "btn btn--sm", "Back");
      back.type = "button";
      back.onclick = () => {
        browseOpen = false;
        render();
      };
      toolbar.append(up, cwd, back);
      panel.append(toolbar);

      if (browseError) panel.append(el("p", "form-error", browseError));
      if (browseLoading) panel.append(el("p", "muted", "Loading…"));

      if (!browseLoading && listing?.path) {
        appendPathCopyBlock(panel, {
          host: host.trim(),
          dir: listing.path,
          hint: "Copy path or SSH command for Termius.",
        });
      }

      if (!browseLoading && listing) {
        const dirs = listing.entries.filter((e) => e.is_dir);
        const list = el("ul", "spawn-browse-list");
        if (dirs.length === 0) {
          list.append(el("li", "muted", "No subdirectories"));
        } else {
          for (const entry of dirs) {
            const li = el("li");
            const btn = el("button", "spawn-browse-entry", `${entry.name}/`);
            btn.type = "button";
            btn.onclick = () => void loadBrowse(joinPath(listing.path, entry.name));
            li.append(btn);
            list.append(li);
          }
        }
        panel.append(list);
      }

      const actions = el("div", "spawn-actions");
      const select = el("button", "btn", "Select");
      select.type = "button";
      select.disabled = !listing || browseLoading;
      select.onclick = selectBrowsePath;
      actions.append(select);
      panel.append(actions);
      body.append(panel);
      return;
    }

    const form = el("div", "spawn-form");

    const hostField = el("label", "form-field");
    hostField.append(el("span", null, "Host"));
    const hostSelect = el("select", "field");
    hostSelect.disabled = loadingHosts || hosts.length === 0;
    if (hosts.length === 0) {
      hostSelect.append(el("option", null, loadingHosts ? "Loading…" : "No hosts"));
    } else {
      for (const h of hosts) {
        const opt = el("option", null, h);
        opt.value = h;
        if (h === host) opt.selected = true;
        hostSelect.append(opt);
      }
    }
    hostSelect.onchange = (e) => {
      host = e.target.value;
      if (root.trim()) onRootChange(root);
      else render();
    };
    hostField.append(hostSelect);
    form.append(hostField);

    const rootField = el("label", "form-field");
    rootField.append(el("span", null, "Root"));
    const rootRow = el("div", "spawn-root-row");
    const rootInput = el("input", "field mono");
    rootInput.type = "text";
    rootInput.placeholder = "~/project";
    rootInput.value = root;
    rootInput.oninput = (e) => onRootChange(e.target.value);
    const browseBtn = el("button", "btn btn--sm", "Browse…");
    browseBtn.type = "button";
    browseBtn.disabled = !host.trim();
    browseBtn.onclick = openBrowse;
    rootRow.append(rootInput, browseBtn);
    rootField.append(rootRow);
    form.append(rootField);

    const pathForCopy = (resolvedRoot || root).trim();
    if (host.trim() && pathForCopy) {
      if (resolvingRoot && !resolvedRoot) {
        form.append(el("p", "muted path-copy-pending", "Resolving path…"));
      } else {
        appendPathCopyBlock(form, {
          host: host.trim(),
          dir: pathForCopy,
          hint: "Copy path or SSH command for Termius.",
        });
      }
    }

    const taskField = el("label", "form-field");
    taskField.append(el("span", null, "Task"));
    const taskInput = el("input", "field mono");
    taskInput.type = "text";
    taskInput.placeholder = "task-name";
    taskInput.value = task;
    taskInput.oninput = (e) => {
      taskTouched = true;
      task = e.target.value;
    };
    taskField.append(taskInput);
    form.append(taskField);

    const providerField = el("label", "form-field");
    providerField.append(el("span", null, "Provider"));
    const providerSelect = el("select", "field");
    for (const [value, label] of [
      ["claude", "Claude"],
      ["codex", "Codex"],
    ]) {
      const opt = el("option", null, label);
      opt.value = value;
      if (value === provider) opt.selected = true;
      providerSelect.append(opt);
    }
    providerSelect.onchange = (e) => {
      provider = e.target.value;
      if (provider === "codex") auto = false;
      render();
    };
    providerField.append(providerSelect);
    form.append(providerField);

    const checks = el("div", "form-checks");
    const autoLabel = el("label", "form-check");
    const autoBox = el("input");
    autoBox.type = "checkbox";
    autoBox.checked = auto;
    autoBox.disabled = provider !== "claude";
    autoBox.onchange = (e) => {
      auto = e.target.checked;
    };
    autoLabel.append(
      autoBox,
      el("span", null, "Auto"),
      el(
        "span",
        "muted form-hint",
        provider === "claude" ? "--permission-mode acceptEdits" : "Claude only",
      ),
    );
    checks.append(autoLabel);
    form.append(checks);

    const extraWrap = el("div", "form-extra");
    const extraToggle = el("button", "btn btn--sm btn--quiet", extraOpen ? "Hide extra args" : "Extra args…");
    extraToggle.type = "button";
    extraToggle.onclick = () => {
      extraOpen = !extraOpen;
      render();
    };
    extraWrap.append(extraToggle);
    if (extraOpen) {
      const extraInput = el("input", "field mono");
      extraInput.type = "text";
      extraInput.placeholder = "e.g. --model opus";
      extraInput.value = extraArgs;
      extraInput.oninput = (e) => {
        extraArgs = e.target.value;
      };
      extraWrap.append(extraInput);
    }
    form.append(extraWrap);

    if (error) form.append(el("p", "form-error", error));

    const actions = el("div", "spawn-actions");
    const cancel = el("button", "btn btn--quiet", "Cancel");
    cancel.type = "button";
    cancel.onclick = () => cleanup(false);
    const spawnBtn = el("button", "btn", spawning ? "Spawning…" : "Spawn");
    spawnBtn.type = "button";
    spawnBtn.disabled = spawning || loadingHosts;
    spawnBtn.onclick = () => void onSpawn();
    actions.append(cancel, spawnBtn);
    form.append(actions);

    body.append(form);
  }

  render();
  if (!sheet.open) sheet.showModal();
  void ensureHosts();
}
