import { describe, expect, it } from "vitest";
import { chatCapable, composeEnabledForState, flattenFleet } from "../src/fleet.js";

describe("flattenFleet", () => {
  it("marks unreachable hosts", () => {
    const rows = flattenFleet({
      hosts: [{ host: "box-b", error: "timeout", sessions: [] }],
    });
    expect(rows).toHaveLength(1);
    expect(rows[0].state).toBe("unreachable");
    expect(rows[0]._pseudo).toBe(true);
  });

  it("marks idle hosts", () => {
    const rows = flattenFleet({
      hosts: [{ host: "box-a", sessions: [] }],
    });
    expect(rows[0].state).toBe("idle");
  });

  it("flattens sessions with default provider", () => {
    const rows = flattenFleet({
      hosts: [
        {
          host: "box-a",
          sessions: [{ task: "demo", state: "running" }],
        },
      ],
    });
    expect(rows[0].provider).toBe("claude");
    expect(rows[0]._pseudo).toBe(false);
  });
});

describe("chatCapable", () => {
  it("allows claude and codex", () => {
    expect(chatCapable("claude")).toBe(true);
    expect(chatCapable("codex")).toBe(true);
    expect(chatCapable("grok")).toBe(false);
  });
});

describe("composeEnabledForState", () => {
  it("allows running only", () => {
    expect(composeEnabledForState("running")).toBe(true);
    expect(composeEnabledForState("exited")).toBe(false);
    expect(composeEnabledForState("")).toBe(false);
    expect(composeEnabledForState(" running ")).toBe(true);
  });
});
