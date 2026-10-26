// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { BRAND_MARKS, serverMark } from "../src/index.js";


describe("brand marks", () => {
  it("every mark is simple-icons' path at a pinned release, under CC0", () => {
    for (const m of BRAND_MARKS) {
      expect(m.source, m.id).toMatch(/^https:\/\/github\.com\/simple-icons\/simple-icons\/blob\/16\.30\.0\/icons\/[a-z]+\.svg$/);
      expect(m.license, m.id).toBe("CC0-1.0");
    }
  });

  it("a server reached over an address takes the mark of the company that serves it, by the host or a domain under it", () => {
    expect(serverMark({ kind: "http", host: "mcp.linear.app" })?.id).toBe("linear");
    expect(serverMark({ kind: "http", host: "MCP.Linear.App" })?.id).toBe("linear");
    expect(serverMark({ kind: "http", host: "api.githubcopilot.com" })?.id).toBe("github");
    expect(serverMark({ kind: "http", host: "bindings.mcp.cloudflare.com" })?.id).toBe("cloudflare");
    expect(serverMark({ kind: "http", host: "mcp.linear.app:8443" })?.id).toBe("linear");
    expect(serverMark({ kind: "http", host: "mcp.linear.app." })?.id).toBe("linear");
    expect(serverMark({ kind: "http", host: "notlinear.app" })).toBeUndefined();
    expect(serverMark({ kind: "http", host: "github.com.evil.io" })).toBeUndefined();
    expect(serverMark({ kind: "http", host: "mcp.example.com" })).toBeUndefined();
  });

  it("a command server takes the mark of the package it runs, or of an address on its line", () => {
    expect(serverMark({ kind: "stdio", line: "npx -y @modelcontextprotocol/server-github" })?.id).toBe("github");
    expect(serverMark({ kind: "stdio", line: "docker run -i --rm -e GITHUB_PERSONAL_ACCESS_TOKEN ghcr.io/github/github-mcp-server" })?.id).toBe("github");
    expect(serverMark({ kind: "stdio", line: "npx -y @cloudflare/mcp-server-cloudflare@0.4.1 run" })?.id).toBe("cloudflare");
    expect(serverMark({ kind: "stdio", line: "npx -y mcp-remote https://mcp.linear.app/sse" })?.id).toBe("linear");
    expect(serverMark({ kind: "stdio", line: "npx -y mcp-remote HTTPS://mcp.linear.app/sse" })?.id).toBe("linear");
    expect(serverMark({ kind: "stdio", line: "node ./my-github-helper.js" })).toBeUndefined();
    expect(serverMark({ kind: "stdio", line: "" })).toBeUndefined();
  });
});
