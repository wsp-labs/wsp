// SPDX-License-Identifier: AGPL-3.0-only
// The update download in the app's main process on a network where each
// address of GitHub takes over 250 ms to connect, node's own window for one.
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer, type Server } from "node:http";
import net, { type AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { connectPatiently, releaseFetch } from "@wsp/host";
import { RELEASE_API_ENV } from "@wsp/protocol";
import { slowRoad } from "../../../packages/host/test/held-connect.js";
import { getBundle } from "../src/get-bundle.js";

const BYTES = Buffer.from("a disk image, as far as this test is concerned");
const ASSET = "wsp-classic-0.3.0-mac.dmg";
const realAttemptMs = net.getDefaultAutoSelectFamilyAttemptTimeout();
let road: (() => void) | undefined;
let server: Server | undefined;
let dir: string | undefined;

afterEach(async () => {
  road?.();
  road = undefined;
  net.setDefaultAutoSelectFamilyAttemptTimeout(realAttemptMs);
  await new Promise<void>(resolve => (server === undefined ? resolve() : server.close(() => resolve())));
  server = undefined;
  if (dir !== undefined) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

/** The fake GitHub on loopback: the release's answer and its download. */
async function fakeGithub(): Promise<number> {
  server = createServer((req, res) => {
    if (req.url?.includes("/releases/tags/")) {
      const digest = `sha256:${createHash("sha256").update(BYTES).digest("hex")}`;
      return void res.end(JSON.stringify({ tag_name: "v0.3.0", assets: [{ name: ASSET, size: BYTES.length, digest }] }));
    }
    res.end(BYTES);
  });
  await new Promise<void>(resolve => server!.listen(0, "127.0.0.1", resolve));
  return (server.address() as AddressInfo).port;
}

describe("the app's update download where each address takes 600 ms to connect", () => {
  it("downloads when GitHub's first address answers late and its second never does", { timeout: 20_000 }, async () => {
    const port = await fakeGithub();
    road = slowRoad([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }], { "127.0.0.1": 600, "::1": Infinity });
    connectPatiently();
    dir = mkdtempSync(join(tmpdir(), "wsp-update-connect-"));
    const env = { [RELEASE_API_ENV]: `http://github.acme.test:${port}` };
    const got = await getBundle("0.3.0", { platform: "darwin", dir, env, userAgent: "wsp/0.2.0", fetch: releaseFetch({ connectMs: 3_000, backoffMs: 50 }) });
    expect(got).toEqual({ ok: true, file: join(dir, ASSET), sum: createHash("sha256").update(BYTES).digest("hex") });
    expect(readFileSync(join(dir, ASSET))).toEqual(BYTES);
  });

  it("main gives each address that window before anything else runs", () => {
    const main = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8");
    const firstStatement = main.split("\n").find(line => line !== "" && !/^(\/\/|import |\s|}|\/\*\*| \*)/.test(line));
    expect(firstStatement).toBe("connectPatiently();");
  });
});
