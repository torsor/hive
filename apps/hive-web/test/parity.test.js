import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parseSseDataPayload } from "../src/sse.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

describe("chat-render parity", () => {
  it("matches hive-panel canonical copy", () => {
    const web = readFileSync(join(root, "src/chat-render.js"));
    const panel = readFileSync(
      join(root, "..", "hive-panel", "src", "chat-render.js"),
    );
    expect(web.equals(panel)).toBe(true);
  });
});

describe("parseSseDataPayload", () => {
  it("parses tick data line", () => {
    const doc = parseSseDataPayload('event: tick\ndata: {"blocks":[]}\n\n');
    expect(doc).toEqual({ blocks: [] });
  });
});
