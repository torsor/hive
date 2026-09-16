/** Module-level HTML cache so virtualizer remounts skip markdown/KaTeX. */

type Entry = {
  plain: string;
  katex?: string;
};

const MAX_ENTRIES = 500;
const store = new Map<string, Entry>();

function hashText(s: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(36);
}

export function renderCacheKey(block: {
  id?: string | null;
  kind: string;
  role: string;
  text: string;
}): string {
  if (block.id) return `id:${block.id}`;
  return `h:${hashText(`${block.kind}\0${block.role}\0${block.text}`)}`;
}

function touch(key: string, entry: Entry): void {
  store.delete(key);
  store.set(key, entry);
  while (store.size > MAX_ENTRIES) {
    const oldest = store.keys().next().value;
    if (oldest == null) break;
    store.delete(oldest);
  }
}

export function getPlain(key: string): string | undefined {
  const e = store.get(key);
  if (!e) return undefined;
  touch(key, e);
  return e.plain;
}

export function getKatex(key: string): string | undefined {
  const e = store.get(key);
  if (!e?.katex) return undefined;
  touch(key, e);
  return e.katex;
}

export function setPlain(key: string, plain: string): void {
  const prev = store.get(key);
  touch(key, { plain, katex: prev?.katex });
}

export function setKatex(key: string, katex: string, plain?: string): void {
  const prev = store.get(key);
  touch(key, { plain: plain ?? prev?.plain ?? "", katex });
}

/** Test-only: clear module cache between cases. */
export function clearRenderCacheForTests(): void {
  store.clear();
}
