import { describe, expect, it } from "vitest";
import { parseFleetView, sessionActions, showInTiles } from "./fleetView";
import type { FlatSessionRow } from "./types";

function row(partial: Partial<FlatSessionRow> = {}): FlatSessionRow {
  return {
    host: "box-a",
    state: "running",
    task: "demo",
    provider: "claude",
    dir: "/tmp/x",
    started: "1h",
    last: "ok",
    error: null,
    actionable: true,
    starred: false,
    ...partial,
  };
}

describe("parseFleetView", () => {
  it("defaults to list when unset or unknown", () => {
    expect(parseFleetView(null)).toBe("list");
    expect(parseFleetView("")).toBe("list");
    expect(parseFleetView("grid")).toBe("list");
  });

  it("accepts list and tiles", () => {
    expect(parseFleetView("list")).toBe("list");
    expect(parseFleetView("tiles")).toBe("tiles");
  });
});

describe("sessionActions", () => {
  it("hides actions on non-actionable rows", () => {
    expect(sessionActions(row({ actionable: false }))).toEqual([]);
  });

  it("offers chat, attach, ssh, stop, restart, kill for a running claude session", () => {
    expect(sessionActions(row())).toEqual([
      "chat",
      "attach",
      "stop",
      "ssh",
      "copy-ssh",
      "restart",
      "kill",
    ]);
  });

  it("offers resume instead of attach/stop when exited", () => {
    expect(sessionActions(row({ state: "exited" }))).toEqual([
      "chat",
      "resume",
      "ssh",
      "copy-ssh",
      "restart",
      "kill",
    ]);
  });

  it("omits chat when the provider is not chat-capable", () => {
    expect(sessionActions(row({ provider: "other" }))).toEqual([
      "attach",
      "stop",
      "ssh",
      "copy-ssh",
      "restart",
      "kill",
    ]);
  });

  it("omits ssh when project dir is unknown", () => {
    expect(sessionActions(row({ dir: "" }))).toEqual([
      "chat",
      "attach",
      "stop",
      "restart",
      "kill",
    ]);
  });
});

describe("showInTiles", () => {
  it("hides idle and unreachable host placeholders", () => {
    expect(showInTiles(row({ actionable: false, state: "idle", task: "—" }))).toBe(false);
    expect(
      showInTiles(row({ actionable: false, state: "unreachable", task: "—", error: "timeout" })),
    ).toBe(false);
  });

  it("keeps running and exited sessions", () => {
    expect(showInTiles(row())).toBe(true);
    expect(showInTiles(row({ state: "exited" }))).toBe(true);
  });
});
