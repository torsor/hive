import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DirListing } from "./types";

type SpawnDialogProps = {
  open: boolean;
  onClose: () => void;
  onSpawned: (message: string) => void;
};

function basenameOf(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const parts = trimmed.split("/");
  const last = parts[parts.length - 1] || "";
  if (last === "~" || last === "") return "";
  if (last.startsWith("~")) return last.slice(1) || "";
  return last;
}

function parentPath(cwd: string): string | null {
  if (!cwd || cwd === "/") return null;
  const trimmed = cwd.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  if (idx <= 0) return "/";
  return trimmed.slice(0, idx) || "/";
}

function joinPath(cwd: string, name: string): string {
  if (cwd === "/") return `/${name}`;
  return `${cwd.replace(/\/+$/, "")}/${name}`;
}

type ProviderId = "claude" | "codex";

export function SpawnDialog({ open, onClose, onSpawned }: SpawnDialogProps) {
  const [hosts, setHosts] = useState<string[]>([]);
  const [host, setHost] = useState("");
  const [root, setRoot] = useState("");
  const [task, setTask] = useState("");
  const [taskTouched, setTaskTouched] = useState(false);
  const [provider, setProvider] = useState<ProviderId>("claude");
  const [auto, setAuto] = useState(false);
  const [extraOpen, setExtraOpen] = useState(false);
  const [extraArgs, setExtraArgs] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [spawning, setSpawning] = useState(false);
  const [loadingHosts, setLoadingHosts] = useState(false);
  const claudeOnly = provider === "claude";

  const [browseOpen, setBrowseOpen] = useState(false);
  const [browsePath, setBrowsePath] = useState("");
  const [listing, setListing] = useState<DirListing | null>(null);
  const [browseLoading, setBrowseLoading] = useState(false);
  const [browseError, setBrowseError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setSpawning(false);
    setBrowseOpen(false);
    setLoadingHosts(true);
    void (async () => {
      try {
        const list = await invoke<string[]>("list_hosts_cmd");
        setHosts(list);
        setHost((prev) => (prev && list.includes(prev) ? prev : list[0] ?? ""));
      } catch (e) {
        setError(String(e));
        setHosts([]);
      } finally {
        setLoadingHosts(false);
      }
    })();
  }, [open]);

  function onRootChange(value: string) {
    setRoot(value);
    if (!taskTouched) {
      setTask(basenameOf(value));
    }
  }

  const loadBrowse = useCallback(
    async (path: string) => {
      if (!host) {
        setBrowseError("Select a host first");
        return;
      }
      setBrowseLoading(true);
      setBrowseError(null);
      try {
        const result = await invoke<DirListing>("list_remote_dirs_cmd", {
          host,
          path,
        });
        setListing(result);
        setBrowsePath(result.path);
      } catch (e) {
        setBrowseError(String(e));
        setListing(null);
      } finally {
        setBrowseLoading(false);
      }
    },
    [host],
  );

  function openBrowse() {
    setBrowseOpen(true);
    setBrowseError(null);
    void loadBrowse("");
  }

  function selectBrowsePath() {
    if (!listing) return;
    onRootChange(listing.path);
    setBrowseOpen(false);
  }

  async function onSpawn() {
    const h = host.trim();
    const d = root.trim();
    const t = task.trim();
    if (!h) {
      setError("Select a host");
      return;
    }
    if (!d) {
      setError("Enter a project root");
      return;
    }
    if (!t) {
      setError("Enter a task name");
      return;
    }
    setSpawning(true);
    setError(null);
    try {
      const msg = await invoke<string>("spawn_session_cmd", {
        args: {
          host: h,
          task: t,
          dir: d,
          provider,
          auto: claudeOnly ? auto : false,
          extra_args: extraArgs.trim(),
        },
      });
      onSpawned(msg);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSpawning(false);
    }
  }

  if (!open) return null;

  const parent = listing ? parentPath(listing.path) : null;
  const dirs = listing?.entries.filter((e) => e.is_dir) ?? [];

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="spawn-dialog-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h2 id="spawn-dialog-title">Spawn session</h2>
          <button type="button" className="btn btn-compact" onClick={onClose}>
            Cancel
          </button>
        </div>

        {browseOpen ? (
          <div className="browse-panel">
            <div className="browse-toolbar">
              <button
                type="button"
                className="btn btn-compact"
                disabled={!parent || browseLoading}
                onClick={() => void loadBrowse(parent!)}
              >
                Up
              </button>
              <span className="browse-cwd mono" title={browsePath}>
                {browsePath}
              </span>
              <button
                type="button"
                className="btn btn-compact"
                onClick={() => setBrowseOpen(false)}
              >
                Back
              </button>
            </div>
            {browseError ? <p className="form-error">{browseError}</p> : null}
            {browseLoading ? <p className="muted">Loading…</p> : null}
            {!browseLoading && listing ? (
              <ul className="browse-list">
                {dirs.length === 0 ? (
                  <li className="muted">No subdirectories</li>
                ) : (
                  dirs.map((entry) => (
                    <li key={entry.name}>
                      <button
                        type="button"
                        className="browse-entry"
                        onClick={() => void loadBrowse(joinPath(listing.path, entry.name))}
                      >
                        {entry.name}/
                      </button>
                    </li>
                  ))
                )}
              </ul>
            ) : null}
            <div className="modal-actions">
              <button
                type="button"
                className="btn btn-primary"
                disabled={!listing || browseLoading}
                onClick={selectBrowsePath}
              >
                Select
              </button>
            </div>
          </div>
        ) : (
          <div className="spawn-form">
            <label className="form-field">
              <span>Host</span>
              <select
                value={host}
                disabled={loadingHosts || hosts.length === 0}
                onChange={(e) => setHost(e.target.value)}
              >
                {hosts.length === 0 ? (
                  <option value="">No hosts</option>
                ) : (
                  hosts.map((h) => (
                    <option key={h} value={h}>
                      {h}
                    </option>
                  ))
                )}
              </select>
            </label>

            <label className="form-field">
              <span>Root</span>
              <div className="root-row">
                <input
                  type="text"
                  className="mono"
                  value={root}
                  placeholder="~/project"
                  onChange={(e) => onRootChange(e.target.value)}
                />
                <button
                  type="button"
                  className="btn"
                  disabled={!host}
                  onClick={openBrowse}
                >
                  Browse…
                </button>
              </div>
            </label>

            <label className="form-field">
              <span>Task</span>
              <input
                type="text"
                className="mono"
                value={task}
                placeholder="task-name"
                onChange={(e) => {
                  setTaskTouched(true);
                  setTask(e.target.value);
                }}
              />
            </label>

            <label className="form-field">
              <span>Provider</span>
              <select
                value={provider}
                onChange={(e) => {
                  const next = e.target.value as ProviderId;
                  setProvider(next);
                  if (next === "codex") setAuto(false);
                }}
              >
                <option value="claude">Claude</option>
                <option value="codex">Codex</option>
              </select>
            </label>

            <div className="form-checks">
              <label className="form-check">
                <input
                  type="checkbox"
                  checked={auto}
                  disabled={!claudeOnly}
                  onChange={(e) => setAuto(e.target.checked)}
                />
                <span>Auto</span>
                <span className="muted form-hint">
                  {claudeOnly ? "--permission-mode acceptEdits" : "Claude only"}
                </span>
              </label>
            </div>

            <div className="form-extra">
              <button
                type="button"
                className="btn btn-compact"
                onClick={() => setExtraOpen((o) => !o)}
              >
                {extraOpen ? "Hide extra args" : "Extra args…"}
              </button>
              {extraOpen ? (
                <input
                  type="text"
                  className="mono"
                  value={extraArgs}
                  placeholder="e.g. --model opus"
                  onChange={(e) => setExtraArgs(e.target.value)}
                />
              ) : null}
            </div>

            {error ? <p className="form-error">{error}</p> : null}

            <div className="modal-actions">
              <button type="button" className="btn" onClick={onClose}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={spawning || loadingHosts}
                onClick={() => void onSpawn()}
              >
                {spawning ? "Spawning…" : "Spawn"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
