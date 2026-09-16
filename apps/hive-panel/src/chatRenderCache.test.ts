import { describe, expect, it, beforeEach } from "vitest";
import {
  getPlain,
  renderCacheKey,
  setPlain,
  clearRenderCacheForTests,
} from "./chatRenderCache";

describe("chatRenderCache", () => {
  beforeEach(() => {
    clearRenderCacheForTests();
  });

  it("uses stable id keys", () => {
    expect(
      renderCacheKey({
        id: "abc",
        kind: "text",
        role: "user",
        text: "x",
      }),
    ).toBe("id:abc");
  });

  it("hashes content when id missing", () => {
    const a = renderCacheKey({
      kind: "text",
      role: "user",
      text: "hello",
    });
    const b = renderCacheKey({
      kind: "text",
      role: "user",
      text: "hello",
    });
    expect(a).toBe(b);
    expect(a.startsWith("h:")).toBe(true);
  });

  it("stores and retrieves plain html", () => {
    const key = renderCacheKey({
      kind: "text",
      role: "assistant",
      text: "math",
    });
    setPlain(key, "<p>math</p>");
    expect(getPlain(key)).toBe("<p>math</p>");
  });
});
