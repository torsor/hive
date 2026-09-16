import { describe, expect, it } from "vitest";
import { absorbBlocks, mergeOutboundBlocks } from "../src/chat.js";

describe("absorbBlocks", () => {
  it("skips duplicate id-less tail blocks", () => {
    const existing = [
      { kind: "text", role: "user", text: "hello?" },
    ];
    const changed = absorbBlocks(existing, [
      { kind: "text", role: "user", text: "hello?" },
    ]);
    expect(changed).toBe(0);
    expect(existing).toHaveLength(1);
  });

  it("still appends distinct user messages", () => {
    const existing = [
      { kind: "text", role: "user", text: "first" },
    ];
    const changed = absorbBlocks(existing, [
      { kind: "text", role: "user", text: "second" },
    ]);
    expect(changed).toBe(1);
    expect(existing).toHaveLength(2);
  });
});

describe("mergeOutboundBlocks", () => {
  it("appends pending outbound bubble before transcript echo", () => {
    const blocks = [{ kind: "text", role: "agent", text: "hi" }];
    const merged = mergeOutboundBlocks(blocks, [
      { text: "hello?", status: "sending" },
    ]);
    expect(merged).toHaveLength(2);
    expect(merged[1].text).toBe("hello?");
    expect(merged[1].pending).toBe("sending");
  });

  it("keeps sent outbound visible until a new transcript user line arrives", () => {
    const blocks = [
      { kind: "text", role: "agent", text: "hi" },
      { kind: "text", role: "user", text: "older" },
    ];
    const merged = mergeOutboundBlocks(blocks, [
      { text: "hello?", status: "sent", sentAtBlockCount: 2 },
    ]);
    expect(merged).toHaveLength(3);
    expect(merged[2].text).toBe("hello?");
  });

  it("drops sent outbound once transcript grows with the echoed user line", () => {
    const blocks = [
      { kind: "text", role: "agent", text: "hi" },
      { kind: "text", role: "user", text: "hello?" },
    ];
    const merged = mergeOutboundBlocks(blocks, [
      { text: "hello?", status: "sent", sentAtBlockCount: 1 },
    ]);
    expect(merged).toHaveLength(2);
  });

  it("matches echoed user lines with trimmed outbound text", () => {
    const blocks = [
      { kind: "text", role: "agent", text: "hi" },
      { kind: "text", role: "user", text: "hello?\n" },
    ];
    const merged = mergeOutboundBlocks(blocks, [
      { text: "hello?", status: "sent", sentAtBlockCount: 1 },
    ]);
    expect(merged).toHaveLength(2);
  });

  it("drops outbound once transcript contains the user line", () => {
    const blocks = [
      { kind: "text", role: "agent", text: "hi" },
      { kind: "text", role: "user", text: "hello?" },
    ];
    const merged = mergeOutboundBlocks(blocks, [
      { text: "hello?", status: "sending" },
    ]);
    expect(merged).toHaveLength(2);
  });
});
