// Phone chat: SSE + poll fallback. Markup rules live in chat-render.js (parity-tested).

import { renderMarkdownWithMath } from "./chat-render.js";
import { fleetUrl, hubFetch, resolveHubUrl, transcriptStreamUrl } from "./hub.js";
import { composeEnabledForState, flattenFleet } from "./fleet.js";
import { openTranscriptStream } from "./sse.js";

const FAST_MS = 4000;
const SLOW_MS = 20000;
const POLL_FALLBACK_MS = 2500;
const DECAY = 1.5;
const CHAT_TAIL = 150;

export function nextDelay(current, sawNew) {
  if (sawNew) return FAST_MS;
  return Math.min(Math.round(current * DECAY), SLOW_MS);
}

export function absorbBlocks(existing, incoming) {
  const keyOf = (block) => (block.id ? `id:${block.id}` : null);
  let changed = 0;
  for (const block of incoming || []) {
    const key = keyOf(block);
    const at = key ? existing.findIndex((b) => keyOf(b) === key) : -1;
    if (at < 0) {
      const last = existing[existing.length - 1];
      if (
        !key &&
        last &&
        last.kind === block.kind &&
        last.role === block.role &&
        last.text === block.text &&
        Boolean(last.failed) === Boolean(block.failed)
      ) {
        continue;
      }
      existing.push(block);
      changed += 1;
      continue;
    }
    const kept = existing[at];
    const merged = {
      ...kept,
      ...block,
      text: block.text || kept.text,
      failed: block.failed || kept.failed,
    };
    if (JSON.stringify(merged) !== JSON.stringify(kept)) changed += 1;
    existing[at] = merged;
  }
  return changed;
}

function normalizeMessageText(text) {
  return String(text ?? "").trim();
}

function outboundAlreadyInBlocks(blocks, outboundItem) {
  const text = typeof outboundItem === "string" ? outboundItem : outboundItem.text;
  const sentAt =
    typeof outboundItem === "string" ? null : outboundItem.sentAtBlockCount;
  if (sentAt != null && blocks.length <= sentAt) return false;
  const want = normalizeMessageText(text);
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.role !== "user") continue;
    return normalizeMessageText(block.text) === want;
  }
  return false;
}

export function mergeOutboundBlocks(blocks, outbound) {
  if (!outbound?.length) return blocks;
  const pending = outbound.filter(
    (o) => o.status === "failed" || !outboundAlreadyInBlocks(blocks, o),
  );
  if (!pending.length) return blocks;
  return [
    ...blocks,
    ...pending.map((o) => ({
      kind: "text",
      role: "user",
      text: o.text,
      pending: o.status,
      pendingError: o.error,
      failed: o.status === "failed",
    })),
  ];
}

function pruneOutbound() {
  chat.outbound = chat.outbound.filter(
    (o) => o.status === "failed" || !outboundAlreadyInBlocks(chat.blocks, o),
  );
}

export function prependBlocks(existing, incoming) {
  const keyOf = (block) => (block.id ? `id:${block.id}` : null);
  const existingKeys = new Set(existing.map((b) => keyOf(b)).filter(Boolean));
  const unique = [];
  for (const block of incoming || []) {
    const key = keyOf(block);
    if (key && existingKeys.has(key)) continue;
    unique.push(block);
  }
  if (!unique.length) return 0;
  existing.splice(0, 0, ...unique);
  return unique.length;
}

const $ = (id) => document.getElementById(id);

function el(tag, cls, text) {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text != null) node.textContent = text;
  return node;
}

const chat = {
  row: null,
  sessionApi: null,
  onAttach: null,
  cursor: "",
  session: null,
  blocks: [],
  timer: null,
  delay: FAST_MS,
  busy: false,
  halted: false,
  streaming: false,
  closeStream: null,
  prevBlocked: false,
  fetched: false,
  hasEarlier: false,
  earlierUntil: null,
  loadingEarlier: false,
  outbound: [],
  drainBusy: false,
  stickBottom: true,
};

const SCROLL_BOTTOM_THRESHOLD = 80;

function isAtBottom(log) {
  if (!log) return true;
  return log.scrollHeight - log.scrollTop - log.clientHeight < SCROLL_BOTTOM_THRESHOLD;
}

function scrollLogToBottom() {
  const log = $("chat-log");
  if (!log) return;
  const go = () => {
    log.scrollTop = log.scrollHeight;
  };
  go();
  requestAnimationFrame(() => {
    go();
    requestAnimationFrame(go);
  });
}

function updateJumpButton() {
  const btn = $("chat-jump");
  const log = $("chat-log");
  if (!btn || !log) return;
  const canScroll = log.scrollHeight > log.clientHeight + 4;
  btn.hidden = chat.stickBottom || !canScroll;
}

function onLogScroll() {
  const log = $("chat-log");
  if (!log) return;
  chat.stickBottom = isAtBottom(log);
  updateJumpButton();
}

let tailObserver = null;

function ensureTailSentinel(log) {
  let tail = log.querySelector("#chat-tail");
  if (!tail) {
    tail = el("div", "chat-tail-sentinel");
    tail.id = "chat-tail";
    tail.setAttribute("aria-hidden", "true");
    log.append(tail);
  } else if (tail !== log.lastElementChild) {
    log.append(tail);
  }
  return tail;
}

function syncTailObserver() {
  const log = $("chat-log");
  if (!log) return;
  const tail = ensureTailSentinel(log);
  if (!tailObserver) {
    tailObserver = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (!entry) return;
        chat.stickBottom = entry.isIntersecting;
        updateJumpButton();
      },
      { root: log, threshold: 0, rootMargin: `0px 0px ${SCROLL_BOTTOM_THRESHOLD}px 0px` },
    );
  }
  tailObserver.disconnect();
  tailObserver.observe(tail);
}

function say(text, error = false) {
  const node = $("chat-msg");
  node.textContent = text || "";
  node.classList.toggle("error", error);
}

function blockNode(block) {
  if (block.kind === "tool") {
    const node = el("div", `chat-tool${block.failed ? " chat-tool--failed" : ""}`);
    node.append(
      el("span", "chat-tool-mark", block.failed ? "✕" : "·"),
      el("span", "chat-tool-text", block.text || "working"),
    );
    return node;
  }
  const mine = block.role === "user";
  const pending =
    block.pending === "queued" || block.pending === "sending"
      ? "Sending…"
      : block.pending === "failed"
        ? block.pendingError || "Failed to send"
        : null;
  const pendingCls =
    block.pending === "failed"
      ? " chat-msg--pending-failed"
      : block.pending
        ? " chat-msg--pending"
        : "";
  const node = el(
    "div",
    `chat-msg ${mine ? "chat-msg--you" : "chat-msg--agent"}${
      block.kind === "thinking" ? " chat-msg--thinking" : ""
    }${pendingCls}`,
  );
  node.append(el("div", "chat-who", mine ? "you" : chat.row?.task || "agent"));
  const body = el("div", "chat-body");
  body.innerHTML = renderMarkdownWithMath(block.text, window.katex);
  node.append(body);
  if (pending) {
    const label = el("div", "chat-pending-label");
    label.setAttribute("role", "status");
    label.textContent = pending;
    node.append(label);
  }
  return node;
}

function outboundBusy() {
  return chat.outbound.some(
    (o) => o.status === "queued" || o.status === "sending",
  );
}

function updateComposeStatus() {
  const send = $("chat-send");
  const field = $("chat-text");
  const status = $("chat-send-status");
  const compose = $("chat-compose");
  if (!send || !field) return;
  const busy = outboundBusy();
  if (compose) compose.setAttribute("aria-busy", busy ? "true" : "false");
  if (status) {
    status.hidden = !busy;
    status.textContent = busy ? "Sending…" : "";
  }
  send.disabled = field.disabled || busy;
}

export function setComposeEnabled(enabled, { offline = false } = {}) {
  const field = $("chat-text");
  const send = $("chat-send");
  const hint = $("chat-compose-hint");
  if (!field || !send) return;
  field.disabled = !enabled;
  send.disabled = !enabled || outboundBusy();
  field.placeholder = enabled
    ? "Message the agent…"
    : "Session is offline — history only";
  if (hint) {
    if (!enabled && offline) {
      hint.hidden = false;
      hint.textContent = "Session is offline — history only.";
    } else {
      hint.hidden = true;
      hint.textContent = "";
    }
  }
  updateComposeStatus();
}

function focusComposeField() {
  const field = $("chat-text");
  if (!field || field.disabled) return;
  window.requestAnimationFrame(() => {
    try {
      field.focus({ preventScroll: true });
    } catch (_) {
      field.focus();
    }
  });
}

async function refreshRowState(row) {
  if (!row?.host || !row?.task) return row;
  try {
    const hub = await resolveHubUrl();
    const fleet = await hubFetch(fleetUrl(hub));
    const fresh = flattenFleet(fleet).find(
      (r) => r.host === row.host && r.task === row.task,
    );
    if (fresh) {
      chat.row = fresh;
      return fresh;
    }
  } catch (_) {
    /* keep snapshot row */
  }
  return row;
}

function render(opts = {}) {
  const log = $("chat-log");
  const preserveScroll = !opts.forceBottom && !chat.stickBottom;
  const scrollTop = preserveScroll ? log.scrollTop : 0;
  log.innerHTML = "";
  if (chat.hasEarlier) {
    const wrap = el("div", "chat-load-earlier");
    const btn = el("button", "btn btn-compact", chat.loadingEarlier ? "Loading…" : "Load earlier");
    btn.type = "button";
    btn.disabled = chat.loadingEarlier;
    btn.onclick = () => void loadEarlier();
    wrap.append(btn);
    log.append(wrap);
  }
  if (!chat.blocks.length && !chat.outbound.length) {
    log.append(el("p", "empty muted", "No conversation yet."));
  }
  for (const block of mergeOutboundBlocks(chat.blocks, chat.outbound)) {
    log.append(blockNode(block));
  }
  ensureTailSentinel(log);
  syncTailObserver();
  updateComposeStatus();
  if (preserveScroll) log.scrollTop = scrollTop;
  else scrollLogToBottom();
  updateJumpButton();
}

function applyDoc(doc, opts = {}) {
  if (!doc) {
    if (!opts.quiet) say("No transcript returned", true);
    return 0;
  }
  const firstFetch = !chat.fetched;
  chat.fetched = true;
  const resetting = Boolean(
    !opts.prepend &&
      (doc.reset ||
        (chat.session && doc.session_id && doc.session_id !== chat.session)),
  );
  if (resetting) chat.blocks = [];
  chat.session = doc.session_id || chat.session;
  let changed = 0;
  if (opts.prepend) {
    changed = prependBlocks(chat.blocks, doc.blocks);
    chat.hasEarlier = false;
    chat.earlierUntil = null;
  } else {
    changed = absorbBlocks(chat.blocks, doc.blocks);
    if (doc.cursor) chat.cursor = JSON.stringify(doc.cursor);
    if (opts.tail != null) {
      chat.hasEarlier = Boolean(doc.has_earlier);
      chat.earlierUntil = doc.earlier_until ?? null;
    }
  }
  const blockedNow = Boolean(doc.blocked_on_prompt);
  const blockedChanged = blockedNow !== chat.prevBlocked;
  chat.prevBlocked = blockedNow;
  $("chat-blocked").hidden = !blockedNow;
  if (changed > 0 || blockedChanged || resetting || firstFetch || opts.prepend) {
    pruneOutbound();
    render();
  }
  if (!opts.quiet) say("");
  return changed;
}

async function fetchTranscript(quiet = false, opts = {}) {
  if (!chat.row || chat.busy || !chat.sessionApi) return 0;
  chat.busy = true;
  if (!quiet) say(opts.prepend ? "Loading earlier…" : "Reading…");
  try {
    const query = {};
    if (opts.untilOffset != null) query.until_offset = String(opts.untilOffset);
    else if (opts.tail != null) query.tail = String(opts.tail);
    else if (chat.cursor) query.after = chat.cursor;
    const doc = await chat.sessionApi("transcript", {}, query);
    const changed = applyDoc(doc, { quiet, prepend: opts.prepend, tail: opts.tail });
    return changed;
  } catch (e) {
    say(String(e.message || e), true);
    chat.halted = true;
    if (!opts.prepend) chat.cursor = "";
    stopTransport();
    return 0;
  } finally {
    chat.busy = false;
    chat.loadingEarlier = false;
  }
}

async function loadEarlier() {
  if (chat.earlierUntil == null || chat.loadingEarlier) return;
  chat.loadingEarlier = true;
  chat.stickBottom = false;
  render();
  await fetchTranscript(false, { untilOffset: chat.earlierUntil, prepend: true });
}

async function pumpOutbound() {
  if (chat.drainBusy || !chat.sessionApi) return;
  chat.drainBusy = true;
  try {
    while (true) {
      const next = chat.outbound.find((o) => o.status === "queued");
      if (!next) break;
      next.status = "sending";
      render({ forceBottom: true });
      try {
        await chat.sessionApi("say", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ text: next.text }),
          signal: AbortSignal.timeout(15000),
        });
        next.status = "sent";
        next.sentAtBlockCount = chat.blocks.length;
        pruneOutbound();
        void fetchTranscript(true);
        if (!chat.streaming) {
          window.setTimeout(resume, 500);
        }
      } catch (e) {
        next.status = "failed";
        next.error = String(e.message || e);
        say(next.error, true);
        render();
        break;
      }
    }
  } finally {
    chat.drainBusy = false;
    if (chat.outbound.some((o) => o.status === "queued")) {
      void pumpOutbound();
    }
    render();
  }
}

function enqueueSend(text) {
  const trimmed = text.trim();
  if (!trimmed || !chat.sessionApi) return;
  chat.stickBottom = true;
  chat.outbound.push({
    id: `pending:${Date.now()}:${Math.random().toString(36).slice(2, 8)}`,
    text: trimmed,
    status: "queued",
  });
  $("chat-text").value = "";
  render({ forceBottom: true });
  void pumpOutbound();
}

async function send() {
  const field = $("chat-text");
  const text = field.value;
  if (!text.trim() || outboundBusy()) return;
  enqueueSend(text);
}

function stopPolling() {
  if (chat.timer) {
    clearTimeout(chat.timer);
    chat.timer = null;
  }
}

function stopStream() {
  if (chat.closeStream) {
    chat.closeStream();
    chat.closeStream = null;
  }
  chat.streaming = false;
}

function stopTransport() {
  stopPolling();
  stopStream();
}

function schedulePoll(delay) {
  stopPolling();
  chat.delay = delay;
  chat.timer = setTimeout(async () => {
    const added = await fetchTranscript(true);
    if (chat.row && !chat.halted && !chat.streaming) {
      schedulePoll(nextDelay(chat.delay, added > 0));
    }
  }, delay);
}

async function startStream() {
  if (!chat.row || chat.halted) return;
  stopStream();
  const hub = await resolveHubUrl();
  const url = transcriptStreamUrl(
    hub,
    chat.row.host,
    chat.row.task,
    chat.cursor || null,
    null,
  );
  chat.streaming = true;
  chat.closeStream = openTranscriptStream(url, {
    onDoc: (doc) => {
      applyDoc(doc, { quiet: true });
    },
    onError: () => {
      chat.streaming = false;
      chat.closeStream = null;
      schedulePoll(POLL_FALLBACK_MS);
    },
  });
}

function resume() {
  chat.halted = false;
  void fetchTranscript(true).then(() => {
    void startStream();
  });
}

function onVisibility() {
  if (!chat.row) return;
  if (document.hidden) {
    stopTransport();
    return;
  }
  resume();
}

function syncViewport() {
  const height = window.visualViewport?.height;
  if (height) document.documentElement.style.setProperty("--chat-h", `${height}px`);
}

let wired = false;
function wire() {
  if (wired) return;
  wired = true;
  $("chat-close").onclick = () => closeChat();
  $("chat-refresh").onclick = () => {
    chat.halted = false;
    void fetchTranscript(false, { tail: CHAT_TAIL }).then(() => {
      void startStream();
    });
  };
  $("chat-compose").onsubmit = (event) => {
    event.preventDefault();
    send();
  };
  const log = $("chat-log");
  if (log && !log.dataset.scrollBound) {
    log.dataset.scrollBound = "1";
    log.addEventListener("scroll", onLogScroll, { passive: true });
    syncTailObserver();
  }
  $("chat-jump").onclick = () => {
    chat.stickBottom = true;
    scrollLogToBottom();
    updateJumpButton();
  };
  $("chat-attach").onclick = () => {
    const { row, onAttach } = chat;
    closeChat();
    onAttach?.(row);
  };
  $("chat").oncancel = (event) => {
    event.preventDefault();
    closeChat();
  };
}

export function openChat(row, { sessionApi, onAttach } = {}) {
  chat.row = row;
  chat.sessionApi = sessionApi;
  chat.onAttach = onAttach;
  chat.cursor = "";
  chat.session = null;
  chat.blocks = [];
  chat.halted = false;
  chat.delay = FAST_MS;
  chat.prevBlocked = false;
  chat.fetched = false;
  chat.hasEarlier = false;
  chat.earlierUntil = null;
  chat.loadingEarlier = false;
  chat.outbound = [];
  chat.drainBusy = false;
  chat.stickBottom = true;
  $("chat-title").textContent = row.task || "agent";
  const provider = row.provider ? ` · ${row.provider}` : "";
  $("chat-where").textContent = `${row.host || "?"}${provider}`;
  $("chat-blocked").hidden = true;
  $("chat-log").innerHTML = "";
  $("chat-text").value = "";
  const running = composeEnabledForState(row.state);
  setComposeEnabled(running, { offline: !running });
  say("");
  wire();
  syncViewport();
  window.visualViewport?.addEventListener("resize", syncViewport);
  document.addEventListener("visibilitychange", onVisibility);
  if (!$("chat").open) $("chat").showModal();
  syncViewport();
  void refreshRowState(row).then((fresh) => {
    if (!chat.row || chat.row.host !== fresh.host || chat.row.task !== fresh.task) {
      return;
    }
    const live = composeEnabledForState(fresh.state);
    setComposeEnabled(live, { offline: !live });
    if (live) focusComposeField();
  });
  void fetchTranscript(false, { tail: CHAT_TAIL }).then(() => {
    if (chat.row && composeEnabledForState(chat.row.state)) {
      setComposeEnabled(true);
      focusComposeField();
    }
    chat.stickBottom = true;
    render({ forceBottom: true });
    void startStream();
  });
}

export function closeChat() {
  if (outboundBusy()) {
    const ok = window.confirm("A message is still sending. Close anyway?");
    if (!ok) return;
  }
  stopTransport();
  chat.row = null;
  chat.outbound = [];
  chat.drainBusy = false;
  window.visualViewport?.removeEventListener("resize", syncViewport);
  document.removeEventListener("visibilitychange", onVisibility);
  document.documentElement.style.removeProperty("--chat-h");
  if ($("chat").open) $("chat").close();
}
