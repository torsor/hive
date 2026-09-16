import { describe, expect, it } from "vitest";
import { basenameOf, parseExtraArgs } from "../src/spawn.js";

describe("basenameOf", () => {
  it("uses last path segment", () => {
    expect(basenameOf("/home/me/project")).toBe("project");
    expect(basenameOf("~/lab/hive")).toBe("hive");
  });

  it("handles tilde-only paths", () => {
    expect(basenameOf("~")).toBe("");
    expect(basenameOf("~/")).toBe("");
  });
});

describe("parseExtraArgs", () => {
  it("splits whitespace", () => {
    expect(parseExtraArgs("--model opus")).toEqual(["--model", "opus"]);
  });

  it("returns empty for blank input", () => {
    expect(parseExtraArgs("   ")).toEqual([]);
  });
});
