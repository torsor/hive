import { useEffect, useId, useRef, useState } from "react";
import type { FlatSessionRow } from "./types";
import { normalizeTagInput, TAG_PRESETS } from "./sessionTagUtils";

type SessionTagsProps = {
  row: FlatSessionRow;
  suggested: string[];
  onTagAdd: (row: FlatSessionRow, tag: string) => Promise<void>;
  onTagRemove: (row: FlatSessionRow, tag: string) => Promise<void>;
};

export function SessionTags({ row, suggested, onTagAdd, onTagRemove }: SessionTagsProps) {
  const tags = row.tags ?? [];
  const editable = row.actionable;
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const [draftError, setDraftError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const inputId = useId();

  useEffect(() => {
    if (!open) return;
    function onDocClick(e: MouseEvent) {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setOpen(false);
        setDraft("");
        setDraftError(null);
      }
    }
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  async function addTag(raw: string) {
    const parsed = normalizeTagInput(raw);
    if (!parsed.ok) {
      setDraftError(parsed.error);
      return;
    }
    if (tags.includes(parsed.tag)) {
      setDraft("");
      setDraftError(null);
      return;
    }
    setBusy(true);
    setDraftError(null);
    try {
      await onTagAdd(row, parsed.tag);
      setDraft("");
      setOpen(false);
    } catch (e) {
      setDraftError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function removeTag(tag: string) {
    setBusy(true);
    setDraftError(null);
    try {
      await onTagRemove(row, tag);
    } catch (e) {
      setDraftError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const presetOptions = [...new Set([...TAG_PRESETS, ...suggested])].sort((a, b) =>
    a.localeCompare(b),
  );
  const addSuggestions = presetOptions.filter((t) => !tags.includes(t));

  return (
    <div className="session-tags" ref={wrapRef}>
      {tags.map((t) => (
        <span
          key={t}
          className={`tag-badge mono${t === "hidden" ? " tag-badge-hidden" : ""}${
            editable ? " tag-badge-editable" : ""
          }`}
          title={editable ? `Remove tag ${t}` : `tag: ${t}`}
        >
          {editable ? (
            <button
              type="button"
              className="tag-badge-btn"
              disabled={busy}
              onClick={() => void removeTag(t)}
            >
              {t}
              <span className="tag-badge-x" aria-hidden>
                ×
              </span>
            </button>
          ) : (
            t
          )}
        </span>
      ))}
      {editable ? (
        <div className="tag-edit-anchor">
          <button
            type="button"
            className="btn btn-compact tag-edit-toggle"
            aria-expanded={open}
            aria-haspopup="dialog"
            disabled={busy}
            title="Add or edit tags"
            onClick={() => setOpen((o) => !o)}
          >
            + tag
          </button>
          {open ? (
            <div className="tag-edit-popover" role="dialog" aria-labelledby={inputId}>
              <label className="tag-edit-label" htmlFor={inputId}>
                New tag
              </label>
              <div className="tag-add-row">
                <input
                  id={inputId}
                  type="text"
                  className="tag-edit-input mono"
                  value={draft}
                  disabled={busy}
                  placeholder="e.g. room"
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(e) => {
                    setDraft(e.target.value);
                    setDraftError(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      void addTag(draft);
                    }
                    if (e.key === "Escape") {
                      setOpen(false);
                      setDraft("");
                      setDraftError(null);
                    }
                  }}
                />
                <button
                  type="button"
                  className="btn btn-compact"
                  disabled={busy || !draft.trim()}
                  onClick={() => void addTag(draft)}
                >
                  Add
                </button>
              </div>
              {draftError ? <p className="tag-edit-error">{draftError}</p> : null}
              {addSuggestions.length > 0 ? (
                <div className="tag-suggest-bar">
                  {addSuggestions.map((t) => (
                    <button
                      key={t}
                      type="button"
                      className="tag-chip"
                      disabled={busy}
                      onClick={() => void addTag(t)}
                    >
                      {t}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
