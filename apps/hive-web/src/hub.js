const DEFAULT_HUB_PORT = 8787;

let hubBase = null;

export function configJsonUrl(base = import.meta.env.BASE_URL) {
  const root = base.endsWith("/") ? base : `${base}/`;
  return new URL("config.json", new URL(root, "http://local")).pathname;
}

export function normalizeHubUrl(raw) {
  const trimmed = String(raw || "").trim();
  if (!trimmed) return "";
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed.replace(/\/+$/, "");
  }
  if (trimmed.includes(":")) {
    return `http://${trimmed}`;
  }
  return `http://${trimmed}:${DEFAULT_HUB_PORT}`;
}

/** Hub URL on the host that served this page (production: nginx on the hub). */
export function sameOriginHub(hostname) {
  const name = String(hostname || "").trim();
  return name ? `http://${name}:${DEFAULT_HUB_PORT}` : "";
}

export async function resolveHubUrl() {
  if (hubBase) return hubBase;
  const fromEnv = normalizeHubUrl(import.meta.env.VITE_HUB_URL);
  if (fromEnv) {
    hubBase = fromEnv;
    return hubBase;
  }
  try {
    const res = await fetch(configJsonUrl(), { cache: "no-store" });
    if (res.ok) {
      const doc = await res.json();
      const fromConfig = normalizeHubUrl(doc.hub);
      if (fromConfig) {
        hubBase = fromConfig;
        return hubBase;
      }
    }
  } catch (_) {
    /* fall through */
  }
  const pageHost = typeof window !== "undefined" ? window.location.hostname : "";
  hubBase = sameOriginHub(pageHost) || `http://localhost:${DEFAULT_HUB_PORT}`;
  return hubBase;
}

export function encodePathSegment(value) {
  return encodeURIComponent(String(value));
}

export function hostUrl(hub, host, rest) {
  const path = rest.startsWith("/") ? rest : `/${rest}`;
  return `${hub}/v1/hosts/${encodePathSegment(host)}${path}`;
}

export function fleetUrl(hub) {
  return `${hub}/v1/fleet`;
}

export function fsUrl(hub, host, path = "") {
  const base = hostUrl(hub, host, "/v1/fs");
  if (!path) return base;
  return `${base}?path=${encodeURIComponent(path)}`;
}

export function sessionsUrl(hub, host) {
  return hostUrl(hub, host, "/v1/sessions");
}

export function sessionUrl(hub, host, task, verb, query = {}) {
  const base = hostUrl(hub, host, `/v1/sessions/${encodePathSegment(task)}/${verb}`);
  const qs = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value != null && value !== "") qs.set(key, String(value));
  }
  const q = qs.toString();
  return q ? `${base}?${q}` : base;
}

export function transcriptStreamUrl(hub, host, task, after, tail) {
  const query = {};
  if (after) query.after = after;
  if (tail != null) query.tail = String(tail);
  return sessionUrl(hub, host, task, "transcript/stream", query);
}

function errorMessage(doc, statusText) {
  if (!doc || typeof doc !== "object") return statusText;
  if (typeof doc.error === "string" && doc.error) return doc.error;
  if (typeof doc.output === "string" && doc.output) return doc.output;
  return statusText;
}

export async function hubFetch(pathOrUrl, options = {}) {
  const hub = await resolveHubUrl();
  const url = pathOrUrl.startsWith("http") ? pathOrUrl : `${hub}${pathOrUrl}`;
  const res = await fetch(url, options);
  const text = await res.text();
  let doc = null;
  try {
    doc = text ? JSON.parse(text) : null;
  } catch (_) {
    throw new Error(text.slice(0, 300) || res.statusText);
  }
  if (!res.ok) throw new Error(errorMessage(doc, res.statusText));
  return doc;
}

/** Session-scoped API helper for chat.js (path relative to host session). */
export function makeSessionApi(host, task) {
  return async (verb, options = {}, query = {}) => {
    const hub = await resolveHubUrl();
    const url = sessionUrl(hub, host, task, verb, query);
    return hubFetch(url, options);
  };
}

export async function api(path, options = {}) {
  const hub = await resolveHubUrl();
  return hubFetch(`${hub}${path}`, options);
}

export async function listRemoteDirs(host, path = "") {
  const hub = await resolveHubUrl();
  return hubFetch(fsUrl(hub, host, path));
}

export async function spawnSession(host, req) {
  const hub = await resolveHubUrl();
  const doc = await hubFetch(sessionsUrl(hub, host), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  });
  if (doc && typeof doc.output === "string" && doc.output) return doc.output;
  return "Spawned.";
}
