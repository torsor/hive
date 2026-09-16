import type { ChatBlock } from "./types";

export type CachedTranscript = {
  blocks: ChatBlock[];
  cursorJson: string;
  sessionId: string | null;
  blocked: boolean;
  hasEarlier: boolean;
  earlierUntil: number | null;
  savedAt: number;
};

const MAX_SESSIONS = 20;
const store = new Map<string, CachedTranscript>();

function cacheKey(host: string, task: string): string {
  return `${host}\0${task}`;
}

export function getTranscriptCache(
  host: string,
  task: string,
): CachedTranscript | undefined {
  const key = cacheKey(host, task);
  const hit = store.get(key);
  if (!hit) return undefined;
  store.delete(key);
  store.set(key, hit);
  return hit;
}

export function setTranscriptCache(
  host: string,
  task: string,
  value: Omit<CachedTranscript, "savedAt">,
): void {
  const key = cacheKey(host, task);
  store.delete(key);
  store.set(key, { ...value, savedAt: Date.now() });
  while (store.size > MAX_SESSIONS) {
    const oldest = store.keys().next().value;
    if (oldest == null) break;
    store.delete(oldest);
  }
}

/** Test-only: clear module cache between cases. */
export function clearTranscriptCacheForTests(): void {
  store.clear();
}
