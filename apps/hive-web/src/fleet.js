/** Flatten hub fleet JSON for phone cards (mirrors hive-panel fleet.ts). */

export function flattenFleet(fleet) {
  const rows = [];
  for (const h of fleet?.hosts || []) {
    if (h.error && (!h.sessions || h.sessions.length === 0)) {
      rows.push({
        host: h.host,
        state: "unreachable",
        task: "—",
        provider: "",
        error: h.error,
        _pseudo: true,
      });
      continue;
    }
    if (!h.sessions || h.sessions.length === 0) {
      rows.push({
        host: h.host,
        state: "idle",
        task: "—",
        provider: "",
        error: null,
        _pseudo: true,
      });
      continue;
    }
    for (const s of h.sessions) {
      rows.push({
        host: s.host || h.host,
        task: s.task,
        state: s.state,
        provider: (s.provider || "").trim() || "claude",
        dir: s.dir || "",
        started: s.started || "",
        last: s.last ?? null,
        error: h.error ?? null,
        starred: Boolean(s.starred),
        _pseudo: false,
      });
    }
  }
  return rows;
}

export function chatCapable(provider) {
  return provider === "claude" || provider === "codex";
}

/** True when the fleet row allows say/chat compose. */
export function composeEnabledForState(state) {
  return String(state || "").trim() === "running";
}
