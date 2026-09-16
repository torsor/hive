import { describe, expect, it } from "vitest";
import { formatCommand, shellQuote, sshShellAtArgv, sshShellAtCommand } from "../src/paths.js";

describe("shellQuote", () => {
  it("quotes spaces and escapes single quotes", () => {
    expect(shellQuote("/home/me/my project")).toBe("'/home/me/my project'");
    expect(shellQuote("it's")).toBe("'it'\\''s'");
  });
});

describe("sshShellAtCommand", () => {
  it("matches hive-panel argv shape", () => {
    expect(sshShellAtArgv("box-a", "/srv/lab/hive")).toEqual([
      "ssh",
      "box-a",
      "-t",
      "cd '/srv/lab/hive' && exec ${SHELL:-/bin/zsh} -l",
    ]);
  });

  it("formats a pasteable command line", () => {
    expect(sshShellAtCommand("hub", "/home/me/proj")).toBe(
      "ssh 'hub' -t 'cd '\\''/home/me/proj'\\'' && exec ${SHELL:-/bin/zsh} -l'",
    );
  });

  it("handles empty dir", () => {
    expect(formatCommand(sshShellAtArgv("box", ""))).toContain("exec ${SHELL:-/bin/zsh} -l");
  });
});
