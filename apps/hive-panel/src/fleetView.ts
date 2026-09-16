import type { FlatSessionRow } from "./types";

export type FleetView = "list" | "tiles";

export type SessionAction =
  | "chat"
  | "attach"
  | "resume"
  | "ssh"
  | "copy-ssh"
  | "stop"
  | "restart"
  | "kill";

function hasProjectDir(row: FlatSessionRow): boolean {
  return Boolean(row.dir?.trim());
}

export function parseFleetView(value: string | null | undefined): FleetView {
  return value === "tiles" ? "tiles" : "list";
}

function chatCapable(provider: string): boolean {
  return provider === "claude" || provider === "codex";
}

function rowProvider(row: FlatSessionRow): string {
  return row.provider?.trim() || "claude";
}

export function showInTiles(row: FlatSessionRow): boolean {
  return row.actionable;
}

export function sessionActions(row: FlatSessionRow): SessionAction[] {
  if (!row.actionable) return [];
  const actions: SessionAction[] = [];
  if (chatCapable(rowProvider(row))) actions.push("chat");
  if (row.state === "running") actions.push("attach", "stop");
  if (row.state === "exited") actions.push("resume");
  if (hasProjectDir(row)) actions.push("ssh", "copy-ssh");
  if (row.state === "running" || row.state === "exited") {
    actions.push("restart", "kill");
  }
  return actions;
}
