import { describe, expect, it } from "vitest";
import {
  configJsonUrl,
  fleetUrl,
  hostUrl,
  normalizeHubUrl,
  sameOriginHub,
  sessionUrl,
  transcriptStreamUrl,
} from "../src/hub.js";

describe("configJsonUrl", () => {
  it("joins base path for subpath deploy", () => {
    expect(configJsonUrl("/hive/")).toBe("/hive/config.json");
  });

  it("works at site root", () => {
    expect(configJsonUrl("/")).toBe("/config.json");
  });
});

describe("normalizeHubUrl", () => {
  it("adds scheme and port for hostname", () => {
    expect(normalizeHubUrl("hub")).toBe("http://hub:8787");
  });

  it("preserves full URL", () => {
    expect(normalizeHubUrl("http://box:9999/")).toBe("http://box:9999");
  });
});

describe("hub paths", () => {
  const hub = "http://hub:8787";

  it("builds fleet url", () => {
    expect(fleetUrl(hub)).toBe("http://hub:8787/v1/fleet");
  });

  it("proxies session attach", () => {
    expect(hostUrl(hub, "box-a", "/v1/sessions/demo/attach")).toBe(
      "http://hub:8787/v1/hosts/box-a/v1/sessions/demo/attach",
    );
  });

  it("encodes host and task segments", () => {
    expect(sessionUrl(hub, "my box", "my task", "say")).toBe(
      "http://hub:8787/v1/hosts/my%20box/v1/sessions/my%20task/say",
    );
  });

  it("builds transcript stream query", () => {
    const url = transcriptStreamUrl(hub, "box", "task", '{"offset":1}', null);
    expect(url).toContain("/transcript/stream");
    expect(url).toContain("after=");
  });
});

describe("sameOriginHub", () => {
  it("points at the hub port on the page host", () => {
    expect(sameOriginHub("hub")).toBe("http://hub:8787");
  });

  it("is empty without a hostname", () => {
    expect(sameOriginHub("")).toBe("");
  });
});
