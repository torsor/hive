import { describe, expect, it } from "vitest";
import { flattenFleet } from "./fleet";
import type { Fleet } from "./types";

describe("flattenFleet", () => {
  it("marks unreachable hosts", () => {
    const fleet: Fleet = {
      generated_at: "2026-01-01T00:00:00Z",
      hosts: [{ host: "box", error: "timeout", sessions: [] }],
    };
    const rows = flattenFleet(fleet);
    expect(rows).toHaveLength(1);
    expect(rows[0].actionable).toBe(false);
    expect(rows[0].state).toBe("unreachable");
    expect(rows[0].last).toBeNull();
    expect(rows[0].error).toBe("timeout");
  });

  it("shows idle rows for reachable hosts with no sessions", () => {
    const fleet: Fleet = {
      generated_at: "2026-01-01T00:00:00Z",
      hosts: [
        { host: "box-a", sessions: [] },
        { host: "box", error: "timeout", sessions: [] },
      ],
    };
    const rows = flattenFleet(fleet);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ host: "box-a", state: "idle", actionable: false });
    expect(rows[1]).toMatchObject({ host: "box", state: "unreachable", actionable: false });
  });

  it("flattens sessions with host fallback", () => {
    const fleet: Fleet = {
      generated_at: "2026-01-01T00:00:00Z",
      hosts: [
        {
          host: "hub",
          sessions: [
            {
              task: "demo",
              state: "running",
              provider: "claude",
              dir: "/tmp/x",
              starred: true,
            },
          ],
        },
      ],
    };
    const rows = flattenFleet(fleet);
    expect(rows).toHaveLength(1);
    expect(rows[0].host).toBe("hub");
    expect(rows[0].task).toBe("demo");
    expect(rows[0].starred).toBe(true);
  });
});
