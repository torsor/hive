import { openChat } from "./chat.js";
import {
  fleetUrl,
  hubFetch,
  makeSessionApi,
  resolveHubUrl,
} from "./hub.js";
import { chatCapable, flattenFleet } from "./fleet.js";
import { openSpawn } from "./spawn.js";
import {
  appendPathCopyBlock,
  copyText,
  formatCommand,
  resolveRemotePath,
  sshShellAtCommand,
} from "./paths.js";

const enc = encodeURIComponent;
const $ = (id) => document.getElementById(id);
function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}
function message(node, text, error = false) {
  node.textContent = text || "";
  node.classList.toggle("error", error);
}

const THEMES = [
  "thing",
  "thing-light",
  "torsor",
  "torsor-dark",
  "torsor-b",
  "torsor-b-dark",
];
function applyBrowseTheme(id, persist = false) {
  const theme = THEMES.includes(id) ? id : "torsor-b";
  document.documentElement.removeAttribute("data-theme");
  if (theme === "torsor") document.documentElement.removeAttribute("data-browse-theme");
  else document.documentElement.dataset.browseTheme = theme;
  if (persist) localStorage.setItem("hiveBrowseTheme", theme);
  $("theme").value = theme;
}
function defaultBrowseTheme() {
  try {
    const saved = localStorage.getItem("hiveBrowseTheme");
    if (THEMES.includes(saved)) return saved;
  } catch (_) {}
  return "torsor-b";
}

let sheetResolve = null;
function openSheet(title, fill, { okLabel = "OK", cancelHidden = false } = {}) {
  return new Promise((resolve) => {
    sheetResolve = resolve;
    $("sheet-title").textContent = title;
    $("sheet-body").innerHTML = "";
    fill($("sheet-body"));
    $("sheet-ok").textContent = okLabel;
    $("sheet-cancel").hidden = cancelHidden;
    if (!$("sheet").open) $("sheet").showModal();
  });
}
function closeSheet(ok) {
  document.querySelector("#sheet .sheet-actions").hidden = false;
  if ($("sheet").open) $("sheet").close();
  if (sheetResolve) {
    sheetResolve(ok);
    sheetResolve = null;
  }
}

function say(text, error = false) {
  message($("msg"), text, error);
}

const state = { sessions: [], hostFilter: "", starredOnly: false, busy: false };

async function loadFleet() {
  if (state.busy) return;
  state.busy = true;
  $("refresh").disabled = true;
  say("Loading…");
  try {
    const hub = await resolveHubUrl();
    const doc = await hubFetch(fleetUrl(hub));
    state.sessions = flattenFleet(doc);
    say("");
  } catch (e) {
    say(String(e.message || e), true);
  } finally {
    state.busy = false;
    $("refresh").disabled = false;
  }
  renderFleet();
}

function formatCommandLine(cmd) {
  return formatCommand(cmd);
}

async function copyAttachCommand(text, field) {
  return copyText(text, field);
}

async function showAttach(row) {
  const host = String(row.host || "");
  const task = String(row.task || "");
  openSheet(`${task} · attach`, (body) =>
    body.append(el("p", "muted", "Checking current fleet state…")),
  );
  try {
    const sessionApi = makeSessionApi(host, task);
    const doc = await sessionApi("attach");
    const text = formatCommandLine(doc.command);
    openSheet(
      `${task} · attach`,
      (body) => {
        body.append(el("p", "sheet-line", `${host} / ${task}`));
        body.append(
          el(
            "p",
            "muted",
            "Copy this command, open Terminus (or a terminal), then paste and run it.",
          ),
        );
        const field = el("textarea", "attach-command mono");
        field.readOnly = true;
        field.value = text;
        field.rows = 5;
        body.append(field);
        const actions = el("div", "attach-actions");
        const copy = el("button", "btn", "copy command");
        const result = el("span", "muted", "");
        copy.onclick = async () => {
          const copied = await copyAttachCommand(text, field);
          result.textContent = copied
            ? "Copied."
            : "Selected — use Copy from the selection menu.";
        };
        actions.append(copy, result);
        body.append(actions);
      },
      { okLabel: "Done", cancelHidden: true },
    );
  } catch (e) {
    say(String(e.message || e), true);
    closeSheet(false);
  }
}

async function showSsh(row) {
  const host = String(row.host || "");
  const task = String(row.task || "");
  const dir = String(row.dir || "").trim();
  if (!dir) {
    say("No project path recorded for this session.", true);
    return;
  }
  const resolved = await resolveRemotePath(host, dir);
  await openSheet(
    `${task} · ssh`,
    (body) => {
      body.append(el("p", "sheet-line", `${host} / ${task}`));
      appendPathCopyBlock(body, {
        host,
        dir: resolved,
        hint: "Copy path or SSH command, then paste in Termius.",
      });
    },
    { okLabel: "Done", cancelHidden: true },
  );
}

async function copySshQuick(row) {
  const host = String(row.host || "");
  const dir = String(row.dir || "").trim();
  if (!dir) {
    say("No project path recorded for this session.", true);
    return;
  }
  say("Copying SSH command…");
  try {
    const resolved = await resolveRemotePath(host, dir);
    const cmd = sshShellAtCommand(host, resolved);
    const copied = await copyText(cmd);
    say(
      copied ? "SSH command copied — paste in Termius." : "Select the command to copy.",
      !copied,
    );
  } catch (e) {
    say(String(e.message || e), true);
  }
}

async function mutate(verb, row) {
  const host = String(row.host || "");
  const task = String(row.task || "");
  const label = verb === "stop" ? "Stop" : "Restart";
  const ok = await openSheet(`${label} ${task}?`, (body) => {
    body.append(el("p", "sheet-line", `${host} / ${task}`));
  }, { okLabel: label });
  if (!ok) return;
  say(`${label}ing…`);
  try {
    const sessionApi = makeSessionApi(host, task);
    await sessionApi(verb, { method: "POST" });
    say(`${label}ed.`);
    await loadFleet();
  } catch (e) {
    say(String(e.message || e), true);
  }
}

function renderFleet() {
  const hosts = [...new Set(state.sessions.map((r) => r.host).filter(Boolean))].sort();
  const chips = $("chips");
  chips.innerHTML = "";
  const starredChip = el(
    "button",
    `chip${state.starredOnly ? " active" : ""}`,
    "★ starred",
  );
  starredChip.onclick = () => {
    state.starredOnly = !state.starredOnly;
    renderFleet();
  };
  chips.append(starredChip);
  for (const host of hosts) {
    const b = el("button", `chip${state.hostFilter === host ? " active" : ""}`, host);
    b.onclick = () => {
      state.hostFilter = state.hostFilter === host ? "" : host;
      renderFleet();
    };
    chips.append(b);
  }

  const list = $("agents-list");
  list.innerHTML = "";
  let rows = state.hostFilter
    ? state.sessions.filter((r) => r.host === state.hostFilter)
    : state.sessions;
  if (state.starredOnly) rows = rows.filter((r) => r.starred);
  $("agents-empty").hidden = rows.length > 0;

  for (const row of rows) {
    const warn = row.state === "unreachable" || row.state === "idle" || row._pseudo;
    const card = el("article", `card${warn ? " card-warn" : ""}`);
    const title = row.starred ? `★ ${row.task || "?"}` : row.task || "?";
    card.append(el("h3", "card-title", title));
    const provider = row.provider ? ` · ${row.provider}` : "";
    const sub =
      row.state === "idle"
        ? `${row.host || "?"} · idle`
        : `${row.host || "?"} · ${row.state || "?"}${provider}`;
    card.append(el("p", "card-sub muted", sub));
    if (row.dir && !row._pseudo) {
      const dirLine = el("p", "card-dir mono muted", row.dir);
      dirLine.title = row.dir;
      card.append(dirLine);
    }
    if (row.error) card.append(el("p", "muted", String(row.error)));
    const actions = el("div", "card-actions");
    if (!row._pseudo && row.state !== "idle" && row.state !== "unreachable") {
      const star = el(
        "button",
        `btn btn--sm${row.starred ? " starred" : ""}`,
        row.starred ? "★" : "☆",
      );
      star.onclick = async () => {
        const op = row.starred ? "unstar" : "star";
        try {
          const sessionApi = makeSessionApi(row.host, row.task);
          await sessionApi("label", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ op }),
          });
          await loadFleet();
        } catch (e) {
          say(String(e.message || e), true);
        }
      };
      actions.append(star);
      if (row.dir) {
        const sshBtn = el("button", "btn btn--sm", "ssh");
        sshBtn.title = "Copy path or SSH command for Termius";
        sshBtn.onclick = () => showSsh(row);
        actions.append(sshBtn);
        const copySshBtn = el("button", "btn btn--sm", "copy ssh");
        copySshBtn.title = "Copy SSH command to clipboard";
        copySshBtn.onclick = () => void copySshQuick(row);
        actions.append(copySshBtn);
      }
      if (chatCapable(row.provider)) {
        const talk = el("button", "btn btn--sm", "chat");
        talk.onclick = () =>
          openChat(row, {
            sessionApi: makeSessionApi(row.host, row.task),
            onAttach: showAttach,
          });
        actions.append(talk);
      }
      if (row.state === "running") {
        const attach = el("button", "btn btn--sm", "attach");
        attach.onclick = () => showAttach(row);
        actions.append(attach);
        const stop = el("button", "btn btn--sm", "stop");
        stop.onclick = () => mutate("stop", row);
        actions.append(stop);
      }
      const restart = el("button", "btn btn--sm", "restart");
      restart.onclick = () => mutate("restart", row);
      actions.append(restart);
    }
    card.append(actions);
    list.append(card);
  }
  $("count").textContent = `${rows.length} session${rows.length === 1 ? "" : "s"}`;
}

$("refresh").onclick = () => loadFleet();
$("spawn").onclick = () => {
  const hosts = [...new Set(state.sessions.map((r) => r.host).filter(Boolean))].sort();
  openSpawn({
    say,
    hosts,
    onClosed: (spawned) => {
      if (spawned) void loadFleet();
    },
  });
};
$("theme").onchange = () => applyBrowseTheme($("theme").value, true);
$("sheet-ok").onclick = () => closeSheet(true);
$("sheet-cancel").onclick = () => closeSheet(false);
$("sheet").oncancel = (e) => {
  e.preventDefault();
  closeSheet(false);
};

applyBrowseTheme(defaultBrowseTheme());
loadFleet();
setInterval(() => loadFleet(), 5 * 60 * 1000);
