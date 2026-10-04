import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LocalAgent } from "./types";

type LocalAgentsProps = {
  onFooter: (msg: string, tone?: "success" | "error") => void;
};

const DEFAULT_GROUP_TAG = "my-team";

export function LocalAgents({ onFooter }: LocalAgentsProps) {
  const [agents, setAgents] = useState<LocalAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [tagFilter, setTagFilter] = useState(DEFAULT_GROUP_TAG);
  const [formOpen, setFormOpen] = useState(false);
  const [form, setForm] = useState({
    id: "",
    title: "",
    cwd: "",
    resume: "cursor .",
    agentmsg: "",
    provider: "cursor",
    tags: "laptop,my-team",
    notes: "",
  });

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const list = await invoke<LocalAgent[]>("list_local_agents_cmd");
      setAgents(list);
    } catch (e) {
      onFooter(String(e), "error");
    } finally {
      setLoading(false);
    }
  }, [onFooter]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const filtered = useMemo(() => {
    const t = tagFilter.trim().toLowerCase();
    if (!t) return agents;
    return agents.filter((a) => a.tags?.some((x) => x === t));
  }, [agents, tagFilter]);

  async function onRegister(e: React.FormEvent) {
    e.preventDefault();
    try {
      await invoke("register_local_agent_cmd", {
        id: form.id.trim(),
        title: form.title.trim(),
        cwd: form.cwd.trim(),
        resume: form.resume.trim(),
        notes: form.notes.trim() || null,
        agentmsg: form.agentmsg.trim() || null,
        provider: form.provider.trim() || null,
        tags: form.tags
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
      });
      setFormOpen(false);
      onFooter(`Registered ${form.id}`, "success");
      await refresh();
    } catch (err) {
      onFooter(String(err), "error");
    }
  }

  async function onOpen(id: string) {
    try {
      const msg = await invoke<string>("open_local_agent_cmd", { id });
      onFooter(msg, "success");
    } catch (err) {
      onFooter(String(err), "error");
    }
  }

  async function onOpenGroup() {
    const tag = tagFilter.trim();
    if (!tag) {
      onFooter("Enter a tag to open group", "error");
      return;
    }
    try {
      const msg = await invoke<string>("open_local_group_cmd", {
        tags: [tag],
        matchAll: false,
      });
      onFooter(msg, "success");
    } catch (err) {
      onFooter(String(err), "error");
    }
  }

  async function onRemove(id: string) {
    if (!window.confirm(`Remove local agent ${id}?`)) return;
    try {
      await invoke("remove_local_agent_cmd", { id });
      onFooter(`Removed ${id}`, "success");
      await refresh();
    } catch (err) {
      onFooter(String(err), "error");
    }
  }

  return (
    <section className="local-agents" aria-label="Local agents">
      <div className="local-agents-header">
        <h2>Local agents</h2>
        <p className="muted local-agents-hint">
          Laptop bookmarks (~/.hive/local-agents.json). Cursor: opens repo; pick chat in GUI.
        </p>
        <div className="local-agents-toolbar">
          <label className="local-tag-filter">
            <span className="filter-group-label">Group tag</span>
            <input
              className="tag-edit-input mono"
              value={tagFilter}
              onChange={(e) => setTagFilter(e.target.value)}
              spellCheck={false}
            />
          </label>
          <button type="button" className="btn btn-compact" onClick={() => void refresh()}>
            Refresh
          </button>
          <button type="button" className="btn btn-compact btn-primary" onClick={() => void onOpenGroup()}>
            Open group
          </button>
          <button type="button" className="btn btn-compact" onClick={() => setFormOpen((o) => !o)}>
            {formOpen ? "Cancel" : "Register"}
          </button>
        </div>
      </div>

      {formOpen ? (
        <form className="local-agent-form" onSubmit={(e) => void onRegister(e)}>
          <div className="local-form-grid">
            <label>
              id
              <input
                className="mono"
                required
                value={form.id}
                onChange={(e) => setForm((f) => ({ ...f, id: e.target.value }))}
              />
            </label>
            <label>
              title
              <input
                required
                value={form.title}
                onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
              />
            </label>
            <label className="local-form-wide">
              cwd
              <input
                className="mono"
                required
                value={form.cwd}
                onChange={(e) => setForm((f) => ({ ...f, cwd: e.target.value }))}
              />
            </label>
            <label className="local-form-wide">
              resume
              <input
                className="mono"
                required
                value={form.resume}
                onChange={(e) => setForm((f) => ({ ...f, resume: e.target.value }))}
              />
            </label>
            <label>
              tags
              <input
                className="mono"
                value={form.tags}
                onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))}
              />
            </label>
            <label>
              agentmsg
              <input
                className="mono"
                value={form.agentmsg}
                onChange={(e) => setForm((f) => ({ ...f, agentmsg: e.target.value }))}
              />
            </label>
            <label className="local-form-wide">
              notes
              <input value={form.notes} onChange={(e) => setForm((f) => ({ ...f, notes: e.target.value }))} />
            </label>
          </div>
          <button type="submit" className="btn btn-primary">
            Save (register)
          </button>
        </form>
      ) : null}

      {loading ? (
        <p className="muted empty-state">Loading local agents…</p>
      ) : filtered.length === 0 ? (
        <p className="muted empty-state">
          No local agents{tagFilter.trim() ? ` with tag “${tagFilter.trim()}”` : ""}. Use Register or{" "}
          <code className="mono">hive local register</code>.
        </p>
      ) : (
        <table className="t local-agents-table">
          <thead>
            <tr>
              <th>id</th>
              <th>title</th>
              <th>tags</th>
              <th>actions</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((a) => (
              <tr key={a.id}>
                <td className="mono">{a.id}</td>
                <td>
                  <div>{a.title}</div>
                  {a.notes ? <div className="muted local-notes">{a.notes}</div> : null}
                </td>
                <td className="mono">{a.tags?.length ? a.tags.join(", ") : "—"}</td>
                <td>
                  <div className="actions-cell">
                    <button type="button" className="btn btn-compact btn-primary" onClick={() => void onOpen(a.id)}>
                      Open
                    </button>
                    <button type="button" className="btn btn-compact btn-danger" onClick={() => void onRemove(a.id)}>
                      Remove
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
