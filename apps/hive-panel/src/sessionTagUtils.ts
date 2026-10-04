/** Match hive-host `labels::normalize_tag` (client-side for instant validation). */

export const TAG_PRESETS = [
  "room",
  "evaluation",
  "workshop",
  "hidden",
  "research",
  "editorial",
] as const;

export function normalizeTagInput(raw: string): { ok: true; tag: string } | { ok: false; error: string } {
  const t = raw.trim().toLowerCase();
  if (!t) {
    return { ok: false, error: "Tag must not be empty" };
  }
  if (t.length > 32) {
    return { ok: false, error: "Tag too long (max 32)" };
  }
  if (!/^[a-z0-9_-]+$/.test(t)) {
    return { ok: false, error: "Use letters, digits, -, or _ only" };
  }
  return { ok: true, tag: t };
}

/** Mirror host rule: adding evaluation also adds room. */
export function tagsAfterAdd(current: string[], tag: string): string[] {
  const set = new Set(current);
  set.add(tag);
  if (tag === "evaluation") {
    set.add("room");
  }
  return [...set].sort((a, b) => a.localeCompare(b));
}

export function tagsAfterRemove(current: string[], tag: string): string[] {
  return current.filter((t) => t !== tag);
}
