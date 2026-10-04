import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import skyHiveGlyph from "./assets/sky-hive-glyph.png";
import { ChatPanel } from "./ChatPanel";
import { flattenFleet } from "./fleet";
import {
  parseFleetView,
  sessionActions,
  showInTiles,
  type FleetView,
  type SessionAction,
} from "./fleetView";
import { SpawnDialog } from "./SpawnDialog";
import { SessionTags } from "./SessionTags";
import { tagsAfterAdd, tagsAfterRemove } from "./sessionTagUtils";
import type { ConfigView, Filters, FlatSessionRow, Fleet, Theme } from "./types";
import "./styles.css";

const APP_LABEL = "hive";

const THEMES = [
  "thing",
  "thing-light",
  "torsor",
  "torsor-dark",
  "torsor-b",
  "torsor-b-dark",
] as const satisfies readonly Theme[];

const THEME_KEY = "hivePanelTheme";
const FONT_KEY = "hivePanelFontStep";
const FILTERS_KEY = "hivePanelFilters";
const FILTERS_OPEN_KEY = "hivePanelFiltersOpen";
const VIEW_KEY = "hivePanelFleetView";
const FONT_STEPS = [0.8125, 0.875, 0.9375, 0.96875, 1, 1.0625, 1.125, 1.1875, 1.25];

function clampFontStep(step: number) {
  return Math.max(-4, Math.min(4, step));
}

function isTheme(value: string | null): value is Theme {
  return THEMES.includes(value as Theme);
}

function loadTheme(): Theme {
  const raw = localStorage.getItem(THEME_KEY);
  return isTheme(raw) ? raw : "torsor";
}

function loadFilters(): Filters {
  try {
    const raw = localStorage.getItem(FILTERS_KEY);
    if (!raw) {
      return {
        states: [],
        hosts: [],
        providers: [],
        tags: [],
        starredOnly: false,
        showHidden: false,
      };
    }
    const parsed = JSON.parse(raw) as Partial<Filters>;
    return {
      states: Array.isArray(parsed.states) ? parsed.states.map(String) : [],
      hosts: Array.isArray(parsed.hosts) ? parsed.hosts.map(String) : [],
      providers: Array.isArray(parsed.providers) ? parsed.providers.map(String) : [],
      tags: Array.isArray(parsed.tags) ? parsed.tags.map(String) : [],
      starredOnly: parsed.starredOnly === true,
      showHidden: parsed.showHidden === true,
    };
  } catch {
    return {
      states: [],
      hosts: [],
      providers: [],
      tags: [],
      starredOnly: false,
      showHidden: false,
    };
  }
}

function loadFiltersOpen(): boolean {
  const raw = localStorage.getItem(FILTERS_OPEN_KEY);
  if (raw === null) return true;
  return raw !== "0";
}

function loadFleetView(): FleetView {
  return parseFleetView(localStorage.getItem(VIEW_KEY));
}

function whereAge(row: FlatSessionRow): string {
  const bits = [row.dir, row.started].filter(Boolean);
  return bits.join(" · ");
}

function formatHostError(error: string | null): string {
  if (!error) return "—";
  const urlMatch = error.match(/url \(([^)]+)\)/);
  if (urlMatch) {
    return `Cannot reach ${urlMatch[1]}`;
  }
  return error.length > 96 ? `${error.slice(0, 93)}…` : error;
}

function rowProvider(row: FlatSessionRow): string {
  return row.provider?.trim() || "claude";
}

function rowStateKey(row: FlatSessionRow): string {
  return row.error && !row.actionable ? "unreachable" : row.state;
}

function uniqueSorted(values: string[]): string[] {
  return [...new Set(values)].sort((a, b) => a.localeCompare(b));
}

function toggleInList(list: string[], value: string): string[] {
  return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}

function rowTags(row: FlatSessionRow): string[] {
  return row.tags ?? [];
}

function matchesFilters(row: FlatSessionRow, filters: Filters): boolean {
  if (!filters.showHidden && rowTags(row).includes("hidden")) {
    return false;
  }
  if (filters.states.length > 0 && !filters.states.includes(rowStateKey(row))) {
    return false;
  }
  if (filters.hosts.length > 0 && !filters.hosts.includes(row.host)) {
    return false;
  }
  if (filters.providers.length > 0 && !filters.providers.includes(rowProvider(row))) {
    return false;
  }
  if (filters.tags.length > 0 && !filters.tags.some((t) => rowTags(row).includes(t))) {
    return false;
  }
  if (filters.starredOnly && !row.starred) {
    return false;
  }
  return true;
}

function filterSummary(filters: Filters): string {
  const parts: string[] = [];
  if (filters.states.length) parts.push(filters.states.join(", "));
  if (filters.hosts.length) parts.push(filters.hosts.join(", "));
  if (filters.providers.length) parts.push(filters.providers.join(", "));
  if (filters.tags.length) parts.push(`tags: ${filters.tags.join(", ")}`);
  if (filters.starredOnly) parts.push("starred");
  if (filters.showHidden) parts.push("incl. hidden");
  return parts.length ? `Filters: ${parts.join(" · ")}` : "Filters: all";
}

function stateBadgeClass(state: string, error: string | null) {
  if (error && state === "unreachable") return "badge badge-unreachable";
  if (state === "running") return "badge badge-running";
  if (state === "exited") return "badge badge-exited";
  return "badge badge-idle";
}

function formatRefreshTime(d: Date) {
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function FilterChips({
  label,
  options,
  selected,
  onToggle,
}: {
  label: string;
  options: string[];
  selected: string[];
  onToggle: (value: string) => void;
}) {
  if (options.length === 0) return null;
  return (
    <div className="filter-group">
      <span className="filter-group-label">{label}</span>
      <div className="tag-bar">
        {options.map((opt) => (
          <button
            key={opt}
            type="button"
            className={`tag-chip${selected.includes(opt) ? " active" : ""}`}
            aria-pressed={selected.includes(opt)}
            onClick={() => onToggle(opt)}
          >
            {opt}
          </button>
        ))}
      </div>
    </div>
  );
}

const ACTION_META: Record<
  SessionAction,
  { label: string; title?: string; primary?: boolean }
> = {
  chat: { label: "Chat", title: "Formatted chat (transcript + say)" },
  attach: { label: "Attach", primary: true },
  resume: {
    label: "Resume",
    title: "Resume this stopped session on the box, then attach",
    primary: true,
  },
  ssh: {
    label: "ssh",
    title: "Open SSH shell in the project directory (not tmux attach)",
  },
  "copy-ssh": {
    label: "copy ssh",
    title: "Copy SSH command for Termius or another terminal",
  },
  stop: { label: "Stop", title: "Take offline — stop it but keep it resumable" },
  restart: {
    label: "Restart",
    title: "Stop if needed, then resume on the box — does not open a terminal",
  },
  kill: { label: "Kill" },
};

function TaskIdentity({
  row,
  tagSuggestions,
  onToggleStar,
  onTagAdd,
  onTagRemove,
}: {
  row: FlatSessionRow;
  tagSuggestions: string[];
  onToggleStar: (row: FlatSessionRow) => void;
  onTagAdd: (row: FlatSessionRow, tag: string) => Promise<void>;
  onTagRemove: (row: FlatSessionRow, tag: string) => Promise<void>;
}) {
  return (
    <div className="task-cell">
      {row.actionable ? (
        <button
          type="button"
          className={`btn btn-compact star-btn${row.starred ? " starred" : ""}`}
          title={row.starred ? "Unstar session" : "Star session"}
          aria-pressed={row.starred}
          onClick={() => void onToggleStar(row)}
        >
          {row.starred ? "★" : "☆"}
        </button>
      ) : null}
      <span className="mono">{row.task}</span>
      {row.provider ? (
        <span className="provider-badge mono" title={`provider: ${row.provider}`}>
          {row.provider}
        </span>
      ) : null}
      <SessionTags
        row={row}
        suggested={tagSuggestions}
        onTagAdd={onTagAdd}
        onTagRemove={onTagRemove}
      />
    </div>
  );
}

function SessionActions({
  row,
  onChat,
  onAttach,
  onSsh,
  onCopySsh,
  onStop,
  onRestart,
  onKill,
}: {
  row: FlatSessionRow;
  onChat: (row: FlatSessionRow) => void;
  onAttach: (host: string, task: string, resume?: boolean) => void;
  onSsh: (row: FlatSessionRow) => void;
  onCopySsh: (row: FlatSessionRow) => void;
  onStop: (host: string, task: string) => void;
  onRestart: (host: string, task: string) => void;
  onKill: (host: string, task: string) => void;
}) {
  const actions = sessionActions(row);
  if (actions.length === 0) return null;
  return (
    <div className="actions-cell">
      {actions.map((action) => {
        const meta = ACTION_META[action];
        return (
          <button
            key={action}
            type="button"
            className={`btn btn-compact${meta.primary ? " btn-primary" : ""}`}
            title={meta.title}
            onClick={() => {
              if (action === "chat") onChat(row);
              else if (action === "attach") void onAttach(row.host, row.task);
              else if (action === "resume") void onAttach(row.host, row.task, true);
              else if (action === "ssh") void onSsh(row);
              else if (action === "copy-ssh") void onCopySsh(row);
              else if (action === "stop") void onStop(row.host, row.task);
              else if (action === "restart") void onRestart(row.host, row.task);
              else void onKill(row.host, row.task);
            }}
          >
            {meta.label}
          </button>
        );
      })}
    </div>
  );
}

function applyFleet(fleet: Fleet, setRows: (rows: FlatSessionRow[]) => void) {
  setRows(flattenFleet(fleet));
}

function App() {
  const [theme, setTheme] = useState<Theme>(loadTheme);
  const [fontStep, setFontStep] = useState(() => Number(localStorage.getItem(FONT_KEY) || "0"));
  const [rows, setRows] = useState<FlatSessionRow[]>([]);
  const [fleetLoaded, setFleetLoaded] = useState(false);
  const [filters, setFilters] = useState<Filters>(loadFilters);
  const [filtersOpen, setFiltersOpen] = useState(loadFiltersOpen);
  const [view, setView] = useState<FleetView>(loadFleetView);
  const [config, setConfig] = useState<ConfigView | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [footer, setFooter] = useState("Connecting to hub…");
  const [footerTone, setFooterTone] = useState<"default" | "success" | "error">("default");
  const [spawnOpen, setSpawnOpen] = useState(false);
  const [chatTarget, setChatTarget] = useState<{
    host: string;
    task: string;
    provider: string;
  } | null>(null);
  const footerToneTimeoutRef = useRef<number | null>(null);

  const chatRunning = Boolean(
    chatTarget &&
      rows.some(
        (r) =>
          r.host === chatTarget.host &&
          r.task === chatTarget.task &&
          r.state === "running",
      ),
  );

  useEffect(() => {
    document.documentElement.dataset.browseTheme = theme;
    localStorage.setItem(THEME_KEY, theme);
  }, [theme]);

  useEffect(() => {
    const step = clampFontStep(fontStep);
    document.documentElement.style.setProperty(
      "--browse-root-font",
      `${FONT_STEPS[step + 4]}rem`,
    );
    localStorage.setItem(FONT_KEY, String(step));
  }, [fontStep]);

  useEffect(() => {
    localStorage.setItem(FILTERS_KEY, JSON.stringify(filters));
  }, [filters]);

  useEffect(() => {
    localStorage.setItem(FILTERS_OPEN_KEY, filtersOpen ? "1" : "0");
  }, [filtersOpen]);

  useEffect(() => {
    localStorage.setItem(VIEW_KEY, view);
  }, [view]);

  function scheduleFooterToneReset() {
    if (footerToneTimeoutRef.current != null) {
      window.clearTimeout(footerToneTimeoutRef.current);
    }
    footerToneTimeoutRef.current = window.setTimeout(() => {
      footerToneTimeoutRef.current = null;
      setFooterTone("default");
    }, 5000);
  }

  const paintFleet = useCallback((fleet: Fleet) => {
    applyFleet(fleet, setRows);
    setFleetLoaded(true);
    setLastRefresh(new Date());
    const hosts = fleet.hosts.length;
    const sessions = fleet.hosts.reduce((n, h) => n + (h.sessions?.length || 0), 0);
    setFooterTone("default");
    setFooter(`${hosts} host${hosts === 1 ? "" : "s"} · ${sessions} session${sessions === 1 ? "" : "s"}`);
    setError(null);
  }, []);

  const refresh = useCallback(async (background = false) => {
    if (!background) setRefreshing(true);
    try {
      const [fleet, cfg] = await Promise.all([
        invoke<Fleet>(background ? "fetch_fleet_cmd" : "refresh_fleet_cmd"),
        invoke<ConfigView>("get_config"),
      ]);
      paintFleet(fleet);
      setConfig(cfg);
    } catch (e) {
      const msg = String(e);
      setError(msg);
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(msg);
    } finally {
      setRefreshing(false);
    }
  }, [paintFleet]);

  useEffect(() => {
    void refresh(true);
    let unlistenFleet: (() => void) | undefined;
    let unlistenErr: (() => void) | undefined;
    void listen<Fleet>("fleet", (ev) => {
      paintFleet(ev.payload);
    }).then((fn) => {
      unlistenFleet = fn;
    });
    void listen<string>("fleet-error", (ev) => {
      setError(String(ev.payload));
    }).then((fn) => {
      unlistenErr = fn;
    });
    return () => {
      unlistenFleet?.();
      unlistenErr?.();
    };
  }, [paintFleet, refresh]);

  const stateOptions = useMemo(() => {
    const base = ["running", "exited"];
    if (rows.some((r) => r.state === "idle")) base.push("idle");
    if (rows.some((r) => r.error && !r.actionable)) base.push("unreachable");
    return base;
  }, [rows]);

  const hostOptions = useMemo(
    () => uniqueSorted(rows.map((r) => r.host).filter(Boolean)),
    [rows],
  );

  const providerOptions = useMemo(
    () => uniqueSorted(rows.filter((r) => r.actionable).map(rowProvider)),
    [rows],
  );

  const tagOptions = useMemo(
    () => uniqueSorted(rows.filter((r) => r.actionable).flatMap((r) => rowTags(r))),
    [rows],
  );

  const filteredRows = useMemo(
    () => rows.filter((row) => matchesFilters(row, filters)),
    [rows, filters],
  );

  const tileRows = useMemo(() => filteredRows.filter(showInTiles), [filteredRows]);

  async function confirmKill(host: string, task: string) {
    try {
      return await ask(
        `Kill session "${task}" on ${host}?\n\nThis stops the tmux session and removes its metadata.`,
        { title: "Kill session", kind: "warning" },
      );
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(`Confirm dialog failed: ${String(e)}`);
      return false;
    }
  }

  function openChat(row: FlatSessionRow) {
    setChatTarget({
      host: row.host,
      task: row.task,
      provider: rowProvider(row),
    });
  }

  async function onKill(host: string, task: string) {
    if (!(await confirmKill(host, task))) return;
    setFooter(`Killing ${host}/${task}…`);
    try {
      const msg = await invoke<string>("kill_session", { host, task });
      setFooterTone("success");
      scheduleFooterToneReset();
      setFooter(msg);
      void refresh(false);
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(String(e));
    }
  }

  async function onStop(host: string, task: string) {
    setFooter(`Stopping ${host}/${task}…`);
    try {
      const msg = await invoke<string>("stop_session", { host, task });
      setFooterTone("success");
      scheduleFooterToneReset();
      setFooter(msg);
      void refresh(false);
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(String(e));
    }
  }

  async function onToggleStar(row: FlatSessionRow) {
    const next = !row.starred;
    const op = next ? "star" : "unstar";
    setRows((rs) =>
      rs.map((r) =>
        r.host === row.host && r.task === row.task ? { ...r, starred: next } : r,
      ),
    );
    try {
      await invoke("label_session", { host: row.host, task: row.task, op });
      setFooterTone("success");
      setFooter(`${op}red ${row.task} on ${row.host}`);
      scheduleFooterToneReset();
    } catch (e) {
      setRows((rs) =>
        rs.map((r) =>
          r.host === row.host && r.task === row.task ? { ...r, starred: row.starred } : r,
        ),
      );
      setFooterTone("error");
      setFooter(String(e));
      scheduleFooterToneReset();
    }
  }

  async function onTagAdd(row: FlatSessionRow, tag: string) {
    const prev = row.tags ?? [];
    const next = tagsAfterAdd(prev, tag);
    setRows((rs) =>
      rs.map((r) =>
        r.host === row.host && r.task === row.task ? { ...r, tags: next } : r,
      ),
    );
    try {
      await invoke("label_session", {
        host: row.host,
        task: row.task,
        op: "tag-add",
        tag,
      });
      setFooterTone("success");
      setFooter(`tag +${tag} on ${row.host}/${row.task}`);
      scheduleFooterToneReset();
    } catch (e) {
      setRows((rs) =>
        rs.map((r) =>
          r.host === row.host && r.task === row.task ? { ...r, tags: prev } : r,
        ),
      );
      setFooterTone("error");
      setFooter(String(e));
      scheduleFooterToneReset();
      throw e;
    }
  }

  async function onTagRemove(row: FlatSessionRow, tag: string) {
    const prev = row.tags ?? [];
    const next = tagsAfterRemove(prev, tag);
    setRows((rs) =>
      rs.map((r) =>
        r.host === row.host && r.task === row.task ? { ...r, tags: next } : r,
      ),
    );
    try {
      await invoke("label_session", {
        host: row.host,
        task: row.task,
        op: "tag-remove",
        tag,
      });
      setFooterTone("success");
      setFooter(`tag −${tag} on ${row.host}/${row.task}`);
      scheduleFooterToneReset();
    } catch (e) {
      setRows((rs) =>
        rs.map((r) =>
          r.host === row.host && r.task === row.task ? { ...r, tags: prev } : r,
        ),
      );
      setFooterTone("error");
      setFooter(String(e));
      scheduleFooterToneReset();
      throw e;
    }
  }

  async function onRestart(host: string, task: string) {
    setFooter(`Restarting ${host}/${task}…`);
    try {
      const msg = await invoke<string>("restart_session", { host, task });
      setFooterTone("success");
      scheduleFooterToneReset();
      setFooter(msg);
      void refresh(false);
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(String(e));
    }
  }

  async function onSsh(row: FlatSessionRow) {
    const dir = row.dir?.trim() || "";
    if (!dir) {
      setFooterTone("error");
      setFooter("No project directory recorded for this session.");
      scheduleFooterToneReset();
      return;
    }
    setFooter(`Opening SSH shell on ${row.host}…`);
    try {
      const msg = await invoke<string>("open_shell_at_session", {
        host: row.host,
        task: row.task,
        dir,
      });
      setFooterTone("success");
      scheduleFooterToneReset();
      setFooter(msg);
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(String(e));
    }
  }

  async function onCopySsh(row: FlatSessionRow) {
    const dir = row.dir?.trim() || "";
    if (!dir) {
      setFooterTone("error");
      setFooter("No project directory recorded for this session.");
      scheduleFooterToneReset();
      return;
    }
    try {
      const cmd = await invoke<string>("ssh_shell_command", { host: row.host, dir });
      await navigator.clipboard.writeText(cmd);
      setFooterTone("success");
      scheduleFooterToneReset();
      setFooter(`Copied SSH command for ${row.host}`);
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(String(e));
    }
  }

  async function onAttach(host: string, task: string, resume = false) {
    setFooter(
      resume
        ? `Resuming & attaching ${host}/${task}…`
        : `Opening terminal for ${host}/${task}…`,
    );
    try {
      const msg = await invoke<string>("attach_session", { host, task, resume });
      setFooterTone("success");
      scheduleFooterToneReset();
      setFooter(msg);
    } catch (e) {
      setFooterTone("error");
      scheduleFooterToneReset();
      setFooter(String(e));
    }
  }

  function onSpawned(message: string) {
    setFooterTone("success");
    scheduleFooterToneReset();
    setFooter(message);
    void refresh(false);
  }

  function closeApp() {
    void invoke("close_app");
  }

  const emptyMessage = !fleetLoaded
    ? "No rows yet. Set ~/.hive/client.toml hub=… and wait for hive-hub."
    : "No sessions match filters.";

  return (
    <>
      <span className="app-label-badge" aria-hidden>
        {APP_LABEL}
      </span>
      <div className="app-chrome">
        <header className="header">
          <div className="brand">
            <img src={skyHiveGlyph} alt="" className="title-icon" />
            <div>
              <h1>hive</h1>
              <p>{config ? config.hub : "Fleet via hive-hub"}</p>
            </div>
          </div>
          <div className="header-actions">
            <select
              value={theme}
              onChange={(e) => setTheme(e.target.value as Theme)}
              aria-label="color theme"
            >
              {THEMES.map((id) => (
                <option key={id} value={id}>
                  {id}
                </option>
              ))}
            </select>
            <button
              type="button"
              className="btn btn-compact"
              onClick={() => setFontStep((s) => clampFontStep(s - 1))}
            >
              A-
            </button>
            <button
              type="button"
              className="btn btn-compact"
              onClick={() => setFontStep((s) => clampFontStep(s + 1))}
            >
              A+
            </button>
            <div className="view-toggle" role="group" aria-label="fleet view">
              <button
                type="button"
                className={`btn btn-compact${view === "list" ? " active" : ""}`}
                aria-pressed={view === "list"}
                onClick={() => setView("list")}
              >
                List
              </button>
              <button
                type="button"
                className={`btn btn-compact${view === "tiles" ? " active" : ""}`}
                aria-pressed={view === "tiles"}
                onClick={() => setView("tiles")}
              >
                Tiles
              </button>
            </div>
            <button type="button" className="btn" onClick={() => setSpawnOpen(true)}>
              Spawn
            </button>
            <button type="button" className="btn btn-primary" disabled={refreshing} onClick={() => void refresh(false)}>
              {refreshing ? "Refreshing…" : "Refresh"}
            </button>
            <button type="button" className="btn btn-danger" onClick={closeApp}>
              Close
            </button>
          </div>
        </header>

        <section className="filters" aria-label="session filters">
          <button
            type="button"
            className="filters-toggle"
            aria-expanded={filtersOpen}
            onClick={() => setFiltersOpen((o) => !o)}
          >
            <span className="filters-chevron" aria-hidden>
              {filtersOpen ? "▾" : "▸"}
            </span>
            <span className="filters-summary">{filterSummary(filters)}</span>
          </button>
          {filtersOpen ? (
            <div className="filters-body">
              <FilterChips
                label="State"
                options={stateOptions}
                selected={filters.states}
                onToggle={(value) =>
                  setFilters((f) => ({ ...f, states: toggleInList(f.states, value) }))
                }
              />
              <FilterChips
                label="Host"
                options={hostOptions}
                selected={filters.hosts}
                onToggle={(value) =>
                  setFilters((f) => ({ ...f, hosts: toggleInList(f.hosts, value) }))
                }
              />
              <FilterChips
                label="Provider"
                options={providerOptions}
                selected={filters.providers}
                onToggle={(value) =>
                  setFilters((f) => ({ ...f, providers: toggleInList(f.providers, value) }))
                }
              />
              <FilterChips
                label="Tags"
                options={tagOptions}
                selected={filters.tags}
                onToggle={(value) =>
                  setFilters((f) => ({ ...f, tags: toggleInList(f.tags, value) }))
                }
              />
              <FilterChips
                label="Starred"
                options={["starred"]}
                selected={filters.starredOnly ? ["starred"] : []}
                onToggle={() =>
                  setFilters((f) => ({ ...f, starredOnly: !f.starredOnly }))
                }
              />
              <FilterChips
                label="Hidden"
                options={["show hidden"]}
                selected={filters.showHidden ? ["show hidden"] : []}
                onToggle={() =>
                  setFilters((f) => ({ ...f, showHidden: !f.showHidden }))
                }
              />
            </div>
          ) : null}
        </section>
      </div>

      {error ? <div className="error-banner">{error}</div> : null}

      <main className="main">
        {(view === "tiles" ? tileRows : filteredRows).length === 0 && !refreshing ? (
          <p className="empty-state">{emptyMessage}</p>
        ) : view === "tiles" ? (
          <ul className="tile-grid">
            {tileRows.map((row, index) => (
              <li key={`${row.host}-${row.task}-${index}`} className="session-tile">
                <div className="tile-top">
                  <TaskIdentity
                    row={row}
                    tagSuggestions={tagOptions}
                    onToggleStar={onToggleStar}
                    onTagAdd={onTagAdd}
                    onTagRemove={onTagRemove}
                  />
                  <span className={stateBadgeClass(row.state, row.error)}>
                    {row.error && !row.actionable ? "unreachable" : row.state}
                  </span>
                </div>
                <div className="tile-meta">
                  <span className="mono">{row.host}</span>
                </div>
                <p
                  className="tile-where"
                  title={row.error && !row.actionable ? row.error : undefined}
                >
                  {row.error && !row.actionable
                    ? formatHostError(row.error)
                    : whereAge(row) || "—"}
                </p>
                {row.last && row.actionable ? (
                  <p className="tile-activity">{row.last}</p>
                ) : null}
                <SessionActions
                  row={row}
                  onChat={openChat}
                  onAttach={onAttach}
                  onSsh={onSsh}
                  onCopySsh={onCopySsh}
                  onStop={onStop}
                  onRestart={onRestart}
                  onKill={onKill}
                />
              </li>
            ))}
          </ul>
        ) : (
          <table className="t">
            <thead>
              <tr>
                <th>Host</th>
                <th>State</th>
                <th>Task</th>
                <th>Where / Age</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredRows.map((row, index) => (
                <Fragment key={`${row.host}-${row.task}-${index}`}>
                  <tr>
                    <td className="mono">{row.host}</td>
                    <td>
                      <span className={stateBadgeClass(row.state, row.error)}>
                        {row.error && !row.actionable ? "unreachable" : row.state}
                      </span>
                    </td>
                    <td>
                      <TaskIdentity
                        row={row}
                        tagSuggestions={tagOptions}
                        onToggleStar={onToggleStar}
                        onTagAdd={onTagAdd}
                        onTagRemove={onTagRemove}
                      />
                    </td>
                    <td title={row.error && !row.actionable ? row.error : undefined}>
                      {row.error && !row.actionable
                        ? formatHostError(row.error)
                        : whereAge(row) || "—"}
                    </td>
                    <td>
                      <SessionActions
                        row={row}
                        onChat={openChat}
                        onAttach={onAttach}
                        onSsh={onSsh}
                        onCopySsh={onCopySsh}
                        onStop={onStop}
                        onRestart={onRestart}
                        onKill={onKill}
                      />
                    </td>
                  </tr>
                  {row.last && row.actionable ? (
                    <tr className="activity-row">
                      <td colSpan={5}>
                        <span>{row.last}</span>
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              ))}
            </tbody>
          </table>
        )}
      </main>

      {spawnOpen ? (
        <SpawnDialog
          open={spawnOpen}
          onClose={() => setSpawnOpen(false)}
          onSpawned={onSpawned}
        />
      ) : null}

      {chatTarget ? (
        <ChatPanel
          host={chatTarget.host}
          task={chatTarget.task}
          provider={chatTarget.provider}
          running={chatRunning}
          onClose={() => setChatTarget(null)}
        />
      ) : null}

      <footer className="footer" data-footer-tone={footerTone}>
        {footerTone === "success" ? (
          <span className="footer-status-icon" aria-hidden>
            ✓
          </span>
        ) : null}
        {footerTone === "error" ? (
          <span className="footer-status-icon footer-status-icon--err" aria-hidden>
            !
          </span>
        ) : null}
        {lastRefresh ? `${formatRefreshTime(lastRefresh)} · ` : ""}
        {footer}
      </footer>
    </>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
