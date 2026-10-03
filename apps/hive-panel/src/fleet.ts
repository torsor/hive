import type { Fleet, FlatSessionRow } from "./types";

export function flattenFleet(fleet: Fleet): FlatSessionRow[] {
  const rows: FlatSessionRow[] = [];
  for (const h of fleet.hosts) {
    if (h.error && (!h.sessions || h.sessions.length === 0)) {
      rows.push({
        host: h.host,
        state: "unreachable",
        task: "—",
        provider: "",
        dir: "",
        started: "",
        last: null,
        error: h.error,
        actionable: false,
        starred: false,
        tags: [],
      });
      continue;
    }
    if (!h.sessions || h.sessions.length === 0) {
      rows.push({
        host: h.host,
        state: "idle",
        task: "—",
        provider: "",
        dir: "",
        started: "",
        last: null,
        error: null,
        actionable: false,
        starred: false,
        tags: [],
      });
      continue;
    }
    for (const s of h.sessions || []) {
      rows.push({
        host: s.host || h.host,
        state: s.state,
        task: s.task,
        provider: s.provider?.trim() || "claude",
        dir: s.dir || "",
        started: s.started || "",
        last: s.last ?? null,
        error: h.error ?? null,
        actionable: true,
        starred: Boolean(s.starred),
        tags: Array.isArray(s.tags) ? s.tags.map(String) : [],
      });
    }
  }
  return rows;
}
