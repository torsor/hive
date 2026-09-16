import {
  createContext,
  memo,
  startTransition,
  useCallback,
  useContext,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { renderMarkdownWithMath } from "./chat-render.js";
import {
  getKatex,
  getPlain,
  renderCacheKey,
  setKatex,
  setPlain,
} from "./chatRenderCache";
import {
  getTranscriptCache,
  setTranscriptCache,
} from "./chatTranscriptCache";
import type { BindingCandidate, BindingsDoc, ChatBlock, TranscriptDoc } from "./types";

const CHAT_POLL_FALLBACK_MS = 2500;
const CHAT_MAX_KEY = "hivePanelChatMaximized";
const CHAT_TAIL = 150;
const KATEX_IDLE_TIMEOUT_MS = 8000;

const KatexPausedContext = createContext(false);

export type ChatPanelProps = {
  host: string;
  task: string;
  provider: string;
  running: boolean;
  onClose: () => void;
};

function blockKey(block: ChatBlock): string | null {
  return block.id ? `id:${block.id}` : null;
}

function blocksEquivalent(a: ChatBlock, b: ChatBlock): boolean {
  return (
    a.kind === b.kind &&
    a.role === b.role &&
    a.text === b.text &&
    Boolean(a.failed) === Boolean(b.failed)
  );
}

function tailHasEquivalent(blocks: ChatBlock[], block: ChatBlock): boolean {
  const last = blocks[blocks.length - 1];
  return last != null && blocksEquivalent(last, block);
}

function normalizeMessageText(text: string): string {
  return text.trim();
}

function outboundAlreadyInBlocks(
  blocks: ChatBlock[],
  outbound: { text: string; sentAtBlockCount?: number | null },
): boolean {
  if (
    outbound.sentAtBlockCount != null &&
    blocks.length <= outbound.sentAtBlockCount
  ) {
    return false;
  }
  const want = normalizeMessageText(outbound.text);
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.role !== "user") continue;
    return normalizeMessageText(block.text) === want;
  }
  return false;
}

function absorbBlocks(
  prev: ChatBlock[],
  incoming: ChatBlock[],
  mode: "append" | "prepend" = "append",
): ChatBlock[] {
  if (incoming.length === 0) return prev;

  if (mode === "prepend") {
    const prevKeys = new Set(
      prev.map((b) => blockKey(b)).filter((k): k is string => k != null),
    );
    const uniqueIncoming: ChatBlock[] = [];
    for (const block of incoming) {
      const key = blockKey(block);
      if (key && prevKeys.has(key)) continue;
      uniqueIncoming.push(block);
    }
    if (uniqueIncoming.length === 0) return prev;
    return [...uniqueIncoming, ...prev];
  }

  let changed = false;
  const next = [...prev];
  for (const block of incoming) {
    const key = blockKey(block);
    const at = key ? next.findIndex((b) => blockKey(b) === key) : -1;
    if (at < 0) {
      if (!key && tailHasEquivalent(next, block)) continue;
      next.push(block);
      changed = true;
      continue;
    }
    const kept = next[at];
    const merged = {
      ...kept,
      ...block,
      text: block.text || kept.text,
      failed: Boolean(block.failed || kept.failed),
    };
    if (
      merged.text !== kept.text ||
      merged.failed !== kept.failed ||
      merged.kind !== kept.kind ||
      merged.role !== kept.role
    ) {
      next[at] = merged;
      changed = true;
    }
  }
  return changed ? next : prev;
}

const ChatBlockView = memo(function ChatBlockView({
  block,
  agentLabel,
}: {
  block: ChatBlock;
  agentLabel: string;
}) {
  const katexPaused = useContext(KatexPausedContext);
  const cacheKey = renderCacheKey(block);

  const [html, setHtml] = useState<string | null>(() => {
    if (block.kind === "tool") return null;
    const cachedKatex = getKatex(cacheKey);
    if (cachedKatex) return cachedKatex;
    const cachedPlain = getPlain(cacheKey);
    if (cachedPlain) return cachedPlain;
    const plain = renderMarkdownWithMath(block.text, null);
    setPlain(cacheKey, plain);
    return plain;
  });

  useEffect(() => {
    if (block.kind === "tool") {
      setHtml(null);
      return;
    }
    const key = renderCacheKey(block);
    const cachedKatex = getKatex(key);
    if (cachedKatex) {
      setHtml(cachedKatex);
      return;
    }
    const plain =
      getPlain(key) ??
      (() => {
        const rendered = renderMarkdownWithMath(block.text, null);
        setPlain(key, rendered);
        return rendered;
      })();
    setHtml(plain);

    if (katexPaused) return;

    let cancelled = false;
    const paint = () => {
      if (cancelled) return;
      const hit = getKatex(key);
      if (hit) {
        setHtml(hit);
        return;
      }
      const withMath = renderMarkdownWithMath(block.text, window.katex);
      setKatex(key, withMath, plain);
      setHtml(withMath);
    };
    let idleId: number | undefined;
    let timeoutId: number | undefined;
    const ric = window.requestIdleCallback;
    if (typeof ric === "function") {
      idleId = ric(paint, { timeout: KATEX_IDLE_TIMEOUT_MS });
    } else {
      timeoutId = window.setTimeout(paint, KATEX_IDLE_TIMEOUT_MS);
    }
    return () => {
      cancelled = true;
      if (idleId != null && typeof window.cancelIdleCallback === "function") {
        window.cancelIdleCallback(idleId);
      }
      if (timeoutId != null) window.clearTimeout(timeoutId);
    };
  }, [block, katexPaused]);

  if (block.kind === "tool") {
    return (
      <div className={`chat-tool${block.failed ? " chat-tool--failed" : ""}`}>
        <span className="chat-tool-mark" aria-hidden>
          {block.failed ? "✕" : "·"}
        </span>
        <span className="chat-tool-text">{block.text || "working"}</span>
      </div>
    );
  }
  const mine = block.role === "user";
  const pendingLabel =
    block.pending === "queued" || block.pending === "sending"
      ? "Sending…"
      : block.pending === "failed"
        ? block.pendingError || "Failed to send"
        : null;
  return (
    <div
      className={`chat-msg ${mine ? "chat-msg--you" : "chat-msg--agent"}${
        block.kind === "thinking" ? " chat-msg--thinking" : ""
      }${block.pending ? " chat-msg--pending" : ""}${
        block.pending === "failed" ? " chat-msg--pending-failed" : ""
      }`}
    >
      <div className="chat-who">{mine ? "you" : agentLabel}</div>
      <div className="chat-body" dangerouslySetInnerHTML={{ __html: html || "" }} />
      {pendingLabel ? (
        <div className="chat-pending-label" role="status">
          {pendingLabel}
        </div>
      ) : null}
    </div>
  );
});

function ChatCompose({
  host,
  task,
  running,
  onEnqueue,
  onComposeFocusChange,
  outboundBusy,
}: {
  host: string;
  task: string;
  running: boolean;
  onEnqueue: (text: string) => void;
  onComposeFocusChange: (focused: boolean) => void;
  outboundBusy: boolean;
}) {
  const [draft, setDraft] = useState("");

  useEffect(() => {
    setDraft("");
  }, [host, task]);

  function onSend() {
    const text = draft;
    if (!text.trim() || !running) return;
    setDraft("");
    onEnqueue(text);
  }

  return (
    <div className="chat-compose">
      {outboundBusy ? (
        <div className="chat-status" role="status">
          Sending…
        </div>
      ) : null}
      <div className="chat-compose-row">
        <textarea
          className="chat-text"
          value={draft}
          disabled={!running}
          placeholder={
            running ? "Message the agent…" : "Session is offline — history only"
          }
          rows={3}
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          autoComplete="off"
          onFocus={() => onComposeFocusChange(true)}
          onBlur={() => onComposeFocusChange(false)}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              onSend();
            }
          }}
        />
        <button
          type="button"
          className="btn btn-primary"
          disabled={!running || !draft.trim()}
          onClick={onSend}
        >
          Send
        </button>
      </div>
    </div>
  );
}

export type ChatLogHandle = {
  scrollToBottom: () => void;
};

function blockRenderKey(block: ChatBlock, index: number): string {
  return block.id
    ? `id:${block.id}`
    : `i:${index}:${block.kind}:${block.ts ?? ""}`;
}

function ChatLog({
  blocks,
  agentLabel,
  empty,
  logRef,
  onScroll,
  apiRef,
  loadEarlier,
  hasEarlier,
  loadingEarlier,
}: {
  blocks: ChatBlock[];
  agentLabel: string;
  empty: boolean;
  logRef: RefObject<HTMLDivElement | null>;
  onScroll: () => void;
  apiRef: RefObject<ChatLogHandle | null>;
  loadEarlier: (() => void) | null;
  hasEarlier: boolean;
  loadingEarlier: boolean;
}) {
  useImperativeHandle(
    apiRef,
    () => ({
      scrollToBottom: () => {
        const log = logRef.current;
        if (!log) return;
        const go = () => {
          log.scrollTop = log.scrollHeight;
        };
        go();
        requestAnimationFrame(() => {
          go();
          requestAnimationFrame(go);
        });
      },
    }),
    [apiRef, logRef],
  );

  useEffect(() => {
    const log = logRef.current;
    if (!log) return;
    const onDown = (e: PointerEvent) => {
      if (e.button !== 0) return;
      log.classList.add("chat-log--interact");
    };
    const onUp = () => log.classList.remove("chat-log--interact");
    log.addEventListener("pointerdown", onDown);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      log.removeEventListener("pointerdown", onDown);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      log.classList.remove("chat-log--interact");
    };
  }, [logRef]);

  return (
    <>
      {hasEarlier && loadEarlier ? (
        <div className="chat-load-earlier">
          <button
            type="button"
            className="btn btn-compact"
            disabled={loadingEarlier}
            onClick={loadEarlier}
          >
            {loadingEarlier ? "Loading…" : "Load earlier"}
          </button>
        </div>
      ) : null}
      <div className="chat-log" ref={logRef} onScroll={onScroll}>
        {empty ? <p className="muted empty-note">No conversation yet.</p> : null}
        {blocks.map((block, index) => (
          <ChatBlockView
            key={blockRenderKey(block, index)}
            block={block}
            agentLabel={agentLabel}
          />
        ))}
        <div className="chat-tail-sentinel" aria-hidden="true" />
      </div>
    </>
  );
}

function writeThroughCache(
  host: string,
  task: string,
  blocks: ChatBlock[],
  cursorJson: string,
  sessionId: string | null,
  blocked: boolean,
  hasEarlier: boolean,
  earlierUntil: number | null,
) {
  setTranscriptCache(host, task, {
    blocks,
    cursorJson,
    sessionId,
    blocked,
    hasEarlier,
    earlierUntil,
  });
}

export function ChatPanel({ host, task, provider, running, onClose }: ChatPanelProps) {
  const cached = getTranscriptCache(host, task);
  const [maximized, setMaximized] = useState(() => localStorage.getItem(CHAT_MAX_KEY) === "1");
  const [blocks, setBlocks] = useState<ChatBlock[]>(() => cached?.blocks ?? []);
  const [blocked, setBlocked] = useState(() => cached?.blocked ?? false);
  const [hasEarlier, setHasEarlier] = useState(() => cached?.hasEarlier ?? false);
  const [earlierUntil, setEarlierUntil] = useState<number | null>(
    () => cached?.earlierUntil ?? null,
  );
  const [loadingEarlier, setLoadingEarlier] = useState(false);
  const [status, setStatus] = useState(() => (cached ? "Updating…" : ""));
  const [error, setError] = useState<string | null>(null);
  const [katexPaused, setKatexPaused] = useState(false);
  type Outbound = {
    id: string;
    text: string;
    status: "queued" | "sending" | "sent" | "failed";
    error?: string;
    sentAtBlockCount?: number | null;
  };
  const [outbound, setOutbound] = useState<Outbound[]>([]);
  const outboundRef = useRef<Outbound[]>([]);
  outboundRef.current = outbound;
  const blocksRef = useRef(blocks);
  blocksRef.current = blocks;
  const drainBusyRef = useRef(false);
  const busyRef = useRef(false);
  const logRef = useRef<HTMLDivElement | null>(null);
  const logApiRef = useRef<ChatLogHandle | null>(null);
  const stickBottomRef = useRef(true);
  const [showJump, setShowJump] = useState(false);
  const [bindOpen, setBindOpen] = useState(false);
  const [bindings, setBindings] = useState<BindingsDoc | null>(null);
  const [bindBusy, setBindBusy] = useState(false);
  const cursorRef = useRef(cached?.cursorJson ?? "");
  const sessionRef = useRef<string | null>(cached?.sessionId ?? null);
  const pollTimerRef = useRef<number | null>(null);
  const streamOkRef = useRef(false);
  const prevBlockCountRef = useRef(cached?.blocks.length ?? 0);
  const hasEarlierRef = useRef(hasEarlier);
  const earlierUntilRef = useRef(earlierUntil);
  hasEarlierRef.current = hasEarlier;
  earlierUntilRef.current = earlierUntil;

  useEffect(() => {
    document.body.classList.add("chat-open");
    return () => document.body.classList.remove("chat-open");
  }, []);

  function stopPolling() {
    if (pollTimerRef.current != null) {
      window.clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }

  function startPollingFallback(tick: () => void) {
    stopPolling();
    pollTimerRef.current = window.setInterval(() => {
      if (!stickBottomRef.current) return;
      tick();
    }, CHAT_POLL_FALLBACK_MS);
  }

  const scrollToBottom = useCallback(() => {
    logApiRef.current?.scrollToBottom();
  }, []);

  const persistFromState = useCallback(
    (nextBlocks: ChatBlock[], nextBlocked: boolean) => {
      writeThroughCache(
        host,
        task,
        nextBlocks,
        cursorRef.current,
        sessionRef.current,
        nextBlocked,
        hasEarlierRef.current,
        earlierUntilRef.current,
      );
    },
    [host, task],
  );

  const applyDoc = useCallback(
    (
      doc: TranscriptDoc,
      opts?: { prepend?: boolean; tailBootstrap?: boolean },
    ) => {
      const shouldReset = Boolean(
        !opts?.prepend &&
          (doc.reset ||
            (sessionRef.current &&
              doc.session_id &&
              doc.session_id !== sessionRef.current)),
      );
      if (doc.session_id) sessionRef.current = doc.session_id;
      const incoming = doc.blocks || [];

      if (opts?.tailBootstrap) {
        setHasEarlier(Boolean(doc.has_earlier));
        hasEarlierRef.current = Boolean(doc.has_earlier);
        const until =
          doc.earlier_until != null ? Number(doc.earlier_until) : null;
        setEarlierUntil(until);
        earlierUntilRef.current = until;
      }
      if (opts?.prepend) {
        setHasEarlier(false);
        hasEarlierRef.current = false;
        setEarlierUntil(null);
        earlierUntilRef.current = null;
      }

      if (!opts?.prepend && doc.cursor) {
        cursorRef.current = JSON.stringify(doc.cursor);
      }

      if (shouldReset || incoming.length > 0 || opts?.prepend) {
        startTransition(() => {
          setBlocks((prev) => {
            const next = absorbBlocks(
              shouldReset ? [] : prev,
              incoming,
              opts?.prepend ? "prepend" : "append",
            );
            writeThroughCache(
              host,
              task,
              next,
              cursorRef.current,
              sessionRef.current,
              Boolean(doc.blocked_on_prompt),
              hasEarlierRef.current,
              earlierUntilRef.current,
            );
            return next;
          });
        });
      } else {
        setBlocks((prev) => {
          persistFromState(prev, Boolean(doc.blocked_on_prompt));
          return prev;
        });
      }

      setBlocked((was) => {
        const next = Boolean(doc.blocked_on_prompt);
        return was === next ? was : next;
      });
      setStatus((s) => (s ? "" : s));
      setError((e) => (e ? null : e));
    },
    [host, task, persistFromState],
  );

  const tryStartStream = useCallback(async () => {
    try {
      await invoke("start_transcript_stream_cmd", {
        host,
        task,
        after: cursorRef.current || null,
        tail: cursorRef.current ? null : CHAT_TAIL,
      });
      streamOkRef.current = true;
      stopPolling();
    } catch {
      streamOkRef.current = false;
    }
  }, [host, task]);

  const fetchDoc = useCallback(
    async (
      quiet: boolean,
      opts?: { tail?: number; untilOffset?: number; prepend?: boolean },
    ) => {
      if (busyRef.current) return;
      busyRef.current = true;
      if (!quiet) {
        setStatus(opts?.prepend ? "Loading earlier…" : "Reading…");
        setError(null);
      }
      try {
        const useSpecial = opts?.tail != null || opts?.untilOffset != null;
        const after = useSpecial ? null : cursorRef.current || null;
        const doc = await invoke<TranscriptDoc>("fetch_transcript", {
          host,
          task,
          after,
          tail: opts?.tail ?? null,
          untilOffset: opts?.untilOffset ?? null,
        });
        applyDoc(doc, {
          prepend: opts?.prepend,
          tailBootstrap: opts?.tail != null,
        });
        if (!streamOkRef.current) {
          startPollingFallback(() => void fetchDoc(true));
          void tryStartStream();
        }
      } catch (e) {
        setError(String(e));
        setStatus("");
        if (!streamOkRef.current) {
          startPollingFallback(() => void fetchDoc(true));
        }
      } finally {
        busyRef.current = false;
        setLoadingEarlier(false);
      }
    },
    [host, task, applyDoc, tryStartStream],
  );

  const openBindPicker = useCallback(async () => {
    setBindOpen(true);
    setBindBusy(true);
    setError(null);
    try {
      const doc = await invoke<BindingsDoc>("list_bindings_cmd", { host, task });
      setBindings(doc);
    } catch (e) {
      setError(String(e));
      setBindOpen(false);
    } finally {
      setBindBusy(false);
    }
  }, [host, task]);

  const applyBinding = useCallback(
    async (candidate: BindingCandidate) => {
      if (candidate.warning) {
        const ok = window.confirm(
          `Bind ${task} to ${candidate.session_id}?\n${candidate.warning}`,
        );
        if (!ok) return;
      }
      setBindBusy(true);
      setError(null);
      try {
        const doc = await invoke<BindingsDoc>("bind_session_cmd", {
          host,
          task,
          sessionId: candidate.session_id,
        });
        setBindings(doc);
        setBindOpen(false);
        cursorRef.current = "";
        sessionRef.current = null;
        setBlocks([]);
        void invoke("stop_transcript_stream_cmd");
        streamOkRef.current = false;
        void fetchDoc(false, { tail: CHAT_TAIL });
      } catch (e) {
        setError(String(e));
      } finally {
        setBindBusy(false);
      }
    },
    [host, task, fetchDoc],
  );

  useEffect(() => {
    let unlistenTick: UnlistenFn | undefined;
    let unlistenErr: UnlistenFn | undefined;
    let cancelled = false;

    const setup = async () => {
      unlistenTick = await listen<TranscriptDoc>("transcript-tick", (ev) => {
        applyDoc(ev.payload);
      });
      unlistenErr = await listen<string>("transcript-stream-error", () => {
        streamOkRef.current = false;
        startPollingFallback(() => void fetchDoc(true));
      });
      if (cancelled) return;

      const warm = getTranscriptCache(host, task);
      setOutbound([]);
      outboundRef.current = [];
      drainBusyRef.current = false;
      if (warm) {
        setBlocks(warm.blocks);
        cursorRef.current = warm.cursorJson;
        sessionRef.current = warm.sessionId;
        setBlocked(warm.blocked);
        setHasEarlier(warm.hasEarlier);
        setEarlierUntil(warm.earlierUntil);
        hasEarlierRef.current = warm.hasEarlier;
        earlierUntilRef.current = warm.earlierUntil;
        prevBlockCountRef.current = warm.blocks.length;
        setStatus("Updating…");
        setError(null);
        stickBottomRef.current = true;
        setShowJump(false);
        void fetchDoc(true);
      } else {
        setBlocks([]);
        cursorRef.current = "";
        sessionRef.current = null;
        setBlocked(false);
        setHasEarlier(false);
        setEarlierUntil(null);
        hasEarlierRef.current = false;
        earlierUntilRef.current = null;
        setError(null);
        setStatus("");
        stickBottomRef.current = true;
        setShowJump(false);
        prevBlockCountRef.current = 0;
        void fetchDoc(false, { tail: CHAT_TAIL });
      }

      await tryStartStream();
      if (!streamOkRef.current && pollTimerRef.current == null) {
        startPollingFallback(() => void fetchDoc(true));
      }
    };

    void setup();
    return () => {
      cancelled = true;
      void invoke("stop_transcript_stream_cmd");
      unlistenTick?.();
      unlistenErr?.();
      stopPolling();
      streamOkRef.current = false;
    };
  }, [host, task, fetchDoc, applyDoc, tryStartStream]);

  useLayoutEffect(() => {
    const count = blocks.length + outbound.length;
    if (!stickBottomRef.current || count === 0) {
      prevBlockCountRef.current = count;
      return;
    }
    const grew = count !== prevBlockCountRef.current;
    prevBlockCountRef.current = count;
    if (!grew && count > 0) return;
    scrollToBottom();
  }, [blocks.length, outbound.length, scrollToBottom]);

  useEffect(() => {
    const log = logRef.current;
    if (!log) return;
    const ro = new ResizeObserver(() => {
      if (stickBottomRef.current) scrollToBottom();
    });
    ro.observe(log);
    return () => ro.disconnect();
  }, [scrollToBottom]);

  useEffect(() => {
    const log = logRef.current;
    if (!log) return;
    const tail = log.querySelector(".chat-tail-sentinel");
    if (!tail) return;
    const io = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (!entry) return;
        const atBottom = entry.isIntersecting;
        stickBottomRef.current = atBottom;
        setShowJump((was) => {
          const next = !atBottom;
          return was === next ? was : next;
        });
      },
      { root: log, threshold: 0, rootMargin: "0px 0px 80px 0px" },
    );
    io.observe(tail);
    return () => io.disconnect();
  }, [blocks.length, outbound.length]);

  const onLogScroll = useCallback(() => {
    const log = logRef.current;
    if (!log) return;
    const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 80;
    stickBottomRef.current = atBottom;
    setShowJump((was) => {
      const next = !atBottom;
      return was === next ? was : next;
    });
  }, []);

  const jumpToLatest = useCallback(() => {
    stickBottomRef.current = true;
    setShowJump(false);
    scrollToBottom();
  }, [scrollToBottom]);

  const pumpOutbound = useCallback(async () => {
    if (drainBusyRef.current) return;
    drainBusyRef.current = true;
    try {
      while (true) {
        const next = outboundRef.current.find((o) => o.status === "queued");
        if (!next) break;
        setOutbound((list) => {
          const updated = list.map((o) =>
            o.id === next.id ? { ...o, status: "sending" as const } : o,
          );
          outboundRef.current = updated;
          return updated;
        });
        try {
          await invoke<string>("say_to_session", {
            host,
            task,
            text: next.text,
          });
          setOutbound((list) => {
            const updated = list.map((o) =>
              o.id === next.id
                ? {
                    ...o,
                    status: "sent" as const,
                    sentAtBlockCount: blocksRef.current.length,
                  }
                : o,
            );
            outboundRef.current = updated;
            return updated;
          });
          void fetchDoc(true);
          if (!streamOkRef.current) {
            window.setTimeout(() => void fetchDoc(true), 500);
          }
        } catch (e) {
          setOutbound((list) => {
            const updated = list.map((o) =>
              o.id === next.id
                ? { ...o, status: "failed" as const, error: String(e) }
                : o,
            );
            outboundRef.current = updated;
            return updated;
          });
          break;
        }
      }
    } finally {
      drainBusyRef.current = false;
      if (outboundRef.current.some((o) => o.status === "queued")) {
        void pumpOutbound();
      }
    }
  }, [host, task, fetchDoc]);

  const enqueueSend = useCallback(
    (text: string) => {
      const id = `pending:${Date.now()}:${Math.random().toString(36).slice(2, 8)}`;
      const item: Outbound = { id, text, status: "queued" };
      setOutbound((list) => {
        const updated = [...list, item];
        outboundRef.current = updated;
        return updated;
      });
      stickBottomRef.current = true;
      setShowJump(false);
      void pumpOutbound();
    },
    [pumpOutbound],
  );

  const requestClose = useCallback(() => {
    if (outbound.some((o) => o.status === "queued" || o.status === "sending")) {
      if (!window.confirm("A message is still sending. Close anyway?")) return;
    }
    onClose();
  }, [onClose, outbound]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (bindOpen) {
          setBindOpen(false);
          return;
        }
        requestClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [requestClose, bindOpen]);

  const onLoadEarlier = useCallback(() => {
    const until = earlierUntilRef.current;
    if (until == null || loadingEarlier) return;
    setLoadingEarlier(true);
    void fetchDoc(false, { untilOffset: until, prepend: true });
  }, [fetchDoc, loadingEarlier]);

  const logBlocks = useMemo((): ChatBlock[] => {
    if (outbound.length === 0) return blocks;
    const pending = outbound.filter(
      (o) =>
        o.status === "failed" || !outboundAlreadyInBlocks(blocks, o),
    );
    if (pending.length === 0) return blocks;
    return [
      ...blocks,
      ...pending.map(
        (o): ChatBlock => ({
          kind: "text",
          role: "user",
          text: o.text,
          id: o.id,
          pending:
            o.status === "queued" || o.status === "sending" || o.status === "failed"
              ? o.status
              : undefined,
          pendingError: o.error,
          failed: o.status === "failed",
        }),
      ),
    ];
  }, [blocks, outbound]);

  useEffect(() => {
    setOutbound((list) => {
      const next = list.filter(
        (o) => o.status === "failed" || !outboundAlreadyInBlocks(blocks, o),
      );
      if (next.length === list.length) return list;
      outboundRef.current = next;
      return next;
    });
  }, [blocks]);

  const outboundBusy = outbound.some(
    (o) => o.status === "queued" || o.status === "sending",
  );

  const agentLabel = task || provider || "agent";

  function toggleMaximized() {
    setMaximized((prev) => {
      const next = !prev;
      localStorage.setItem(CHAT_MAX_KEY, next ? "1" : "0");
      return next;
    });
  }

  return (
    <KatexPausedContext.Provider value={katexPaused}>
      <div
        className={`modal-backdrop${maximized ? " modal-backdrop--chat-max" : ""}`}
        role="presentation"
        onClick={requestClose}
      >
        <div
          className={`modal chat-modal${maximized ? " chat-modal--max" : ""}`}
          role="dialog"
          aria-modal="true"
          aria-labelledby="chat-dialog-title"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="chat-shell">
            <div className="chat-head">
              <div className="chat-title-group">
                <h2 id="chat-dialog-title">{task}</h2>
                <span className="muted mono">
                  {host}
                  {provider ? ` · ${provider}` : ""}
                  {!running ? " · exited" : ""}
                </span>
              </div>
              <div className="chat-head-actions">
                <button
                  type="button"
                  className="btn btn-compact"
                  aria-pressed={bindOpen}
                  title="Choose which Claude/Codex thread this task reads"
                  onClick={() => {
                    if (bindOpen) setBindOpen(false);
                    else void openBindPicker();
                  }}
                >
                  Bind
                </button>
                <button
                  type="button"
                  className="btn btn-compact"
                  onClick={() => void fetchDoc(false)}
                >
                  Refresh
                </button>
                <button
                  type="button"
                  className="btn btn-compact"
                  title={maximized ? "Unmaximize chat" : "Maximize chat"}
                  aria-pressed={maximized}
                  onClick={toggleMaximized}
                >
                  {maximized ? "Unmaximize" : "Maximize"}
                </button>
                <button type="button" className="btn btn-compact" onClick={requestClose}>
                  Close
                </button>
              </div>
            </div>

            {bindOpen ? (
              <div className="chat-bind" role="region" aria-label="Transcript binding">
                <p className="chat-bind-lead muted">
                  {bindBusy && !bindings
                    ? "Listing threads…"
                    : "Pick the thread hive should read. The highlighted row is the obvious default."}
                </p>
                <ul className="chat-bind-list">
                  {(bindings?.candidates ?? []).map((c) => (
                    <li key={c.session_id}>
                      <button
                        type="button"
                        className={`chat-bind-item${c.suggested ? " chat-bind-item--suggested" : ""}${c.current ? " chat-bind-item--current" : ""}`}
                        disabled={bindBusy}
                        onClick={() => void applyBinding(c)}
                      >
                        <span className="mono">{c.session_id}</span>
                        <span className="chat-bind-meta">
                          {c.thread_name || "—"}
                          {c.updated_at ? ` · ${c.updated_at}` : ""}
                          {c.live ? " · live" : ""}
                          {c.current ? " · current" : ""}
                          {c.suggested ? " · suggested" : ""}
                        </span>
                        {c.warning ? (
                          <span className="chat-bind-warn">{c.warning}</span>
                        ) : null}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            {blocked ? (
              <div className="chat-blocked" role="status">
                <span>
                  Waiting on a permission prompt — attach in a terminal to approve.
                </span>
              </div>
            ) : null}

            {(status || error) && (
              <div className={`chat-status${error ? " chat-status--error" : ""}`}>
                {error || status}
              </div>
            )}

            <div className="chat-log-wrap">
              <ChatLog
                blocks={logBlocks}
                agentLabel={agentLabel}
                empty={logBlocks.length === 0 && !error}
                logRef={logRef}
                apiRef={logApiRef}
                onScroll={onLogScroll}
                hasEarlier={hasEarlier}
                loadingEarlier={loadingEarlier}
                loadEarlier={hasEarlier ? onLoadEarlier : null}
              />
              {showJump ? (
                <button
                  type="button"
                  className="chat-jump"
                  onClick={jumpToLatest}
                >
                  Latest ↓
                </button>
              ) : null}
            </div>

            <ChatCompose
              host={host}
              task={task}
              running={running}
              onEnqueue={enqueueSend}
              outboundBusy={outboundBusy}
              onComposeFocusChange={setKatexPaused}
            />
          </div>
        </div>
      </div>
    </KatexPausedContext.Provider>
  );
}
