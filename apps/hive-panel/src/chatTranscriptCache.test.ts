import { describe, expect, it, beforeEach } from "vitest";
import {
  getTranscriptCache,
  setTranscriptCache,
  clearTranscriptCacheForTests,
} from "./chatTranscriptCache";

describe("chatTranscriptCache", () => {
  beforeEach(() => {
    clearTranscriptCacheForTests();
  });

  it("round-trips session transcript", () => {
    setTranscriptCache("box", "demo", {
      blocks: [{ kind: "text", role: "user", text: "hi" }],
      cursorJson: '{"offset":1}',
      sessionId: "s1",
      blocked: false,
      hasEarlier: true,
      earlierUntil: 42,
    });
    const hit = getTranscriptCache("box", "demo");
    expect(hit?.blocks[0]?.text).toBe("hi");
    expect(hit?.cursorJson).toBe('{"offset":1}');
    expect(hit?.hasEarlier).toBe(true);
    expect(hit?.earlierUntil).toBe(42);
  });

  it("evicts oldest when over MAX_SESSIONS", () => {
    for (let i = 0; i < 21; i++) {
      setTranscriptCache("box", `task-${i}`, {
        blocks: [],
        cursorJson: "",
        sessionId: null,
        blocked: false,
        hasEarlier: false,
        earlierUntil: null,
      });
    }
    expect(getTranscriptCache("box", "task-0")).toBeUndefined();
    expect(getTranscriptCache("box", "task-20")).toBeDefined();
  });
});
