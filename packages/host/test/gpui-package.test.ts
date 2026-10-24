// SPDX-License-Identifier: AGPL-3.0-only
// apps/desktop-gpui/scripts/package.sh, the GPUI app's build script, through its --dry-run: which signing and
// notarizing steps it takes for which secrets, and that no secret's value reaches its output. Every run gets an
// environment built from nothing and a PATH whose first folder holds stubs of the tools a real run calls; a dry run
// calls none of them, so the stubs only record. Apple's services are never reached.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";

const script = fileURLToPath(new URL("../../../apps/desktop-gpui/scripts/package.sh", import.meta.url));
const root = mkdtempSync(join(tmpdir(), "wsp-gpui-package-"));
const home = mkdtempSync(join(root, "home-"));
const stubs = mkdtempSync(join(root, "stubs-"));
const calls = join(root, "calls");
for (const tool of ["cargo", "rustup", "codesign", "xcrun", "hdiutil", "lipo", "security", "ditto", "osascript", "tiffutil", "plutil", "curl", "mksquashfs", "pnpm", "tar", "node"]) {
  writeStub(join(stubs, tool), `#!/bin/sh\necho "${tool} $*" >> '${calls}'\n`);
}
afterAll(() => rmSync(root, { recursive: true, force: true }));

const CERT = { CSC_LINK: "dummy-csc-link-Zq81", CSC_KEY_PASSWORD: "dummy-key-password-Zq82" };
const APPLE = { APPLE_ID: "dummy-apple-id-Zq83@example.com", APPLE_TEAM_ID: "DUMMYTEAM84", APPLE_APP_SPECIFIC_PASSWORD: "dummy-app-password-Zq85" };
const SECRETS = Object.values({ ...CERT, ...APPLE });

function dryRun(target: "mac" | "linux", secrets: Record<string, string> = {}) {
  writeFileSync(calls, "");
  const run = spawnSync(script, ["--dry-run", "--target", target], {
    env: { PATH: `${stubs}:/usr/bin:/bin`, HOME: home, ...secrets },
    encoding: "utf8",
  });
  const out = `${run.stdout}${run.stderr}`;
  expect(run.status, out).toBe(0);
  expect(readFileSync(calls, "utf8"), "a dry run called a tool it should only print").toBe("");
  for (const value of SECRETS) expect(out.includes(value), "a secret's value reached the output").toBe(false);
  return out;
}
const at = (out: string, line: RegExp) => {
  const index = out.search(line);
  expect(index, `no line matches ${line}`).toBeGreaterThanOrEqual(0);
  return index;
};

describe("package.sh --dry-run", () => {
  it("with no signing secrets signs ad hoc, and skips signing with an identity and notarizing with a line each", () => {
    const out = dryRun("mac");
    at(out, /^skip {2}signing with a Developer ID: CSC_LINK and CSC_KEY_PASSWORD are not set/m);
    at(out, /^skip {2}notarizing: /m);
    at(out, /^\+ codesign --force --options runtime --sign - 'dist\/wsp Experimental\.app'$/m);
    expect(out).not.toMatch(/--timestamp|security import|notarytool|stapler/);
  });

  it("carries one Node for both chips out of the pinned archives, and signs it and each Mac daemon before the bundle", () => {
    const out = dryRun("mac", CERT);
    for (const [pa, sum] of [
      ["darwin-arm64", "61130f394c1630d211dd50aecc4353d379480f36d3ac913cd85dbba1aed585c6"],
      ["darwin-x64", "58e99022c2ff89395576cc7fd4d98cea24bb68081475d5f88b801ee8729fb026"],
    ]) {
      at(out, new RegExp(`^\\+ curl -fsSL https://nodejs\\.org/dist/v22\\.23\\.2/node-v22\\.23\\.2-${pa}\\.tar\\.gz `, "m"));
      expect(readFileSync(script, "utf8")).toContain(`${pa}) echo ${sum} ;;`);
    }
    const build = at(out, /^\+ pnpm -C \.\.\/\.\. --filter @wsp\/desktop run build:app$/m);
    const lipo = at(out, /^\+ lipo -create -output 'dist\/wsp Experimental\.app\/Contents\/MacOS\/node' \S+node-darwin-arm64\/bin\/node \S+node-darwin-x64\/bin\/node$/m);
    expect(build).toBeLessThan(lipo);
    const node = at(out, /^\+ codesign --force --options runtime --timestamp .*--entitlements \.\.\/desktop\/build\/entitlements\.mac\.plist .*'dist\/wsp Experimental\.app\/Contents\/MacOS\/node'$/m);
    const daemon = at(out, /^\+ codesign --force --options runtime --timestamp .*'dist\/wsp Experimental\.app\/Contents\/Resources\/app\/assets\/daemon\/x86_64-apple-darwin\/wsp-daemon'$/m);
    const bundle = at(out, /^\+ codesign --force --options runtime --timestamp .*--sign .*'dist\/wsp Experimental\.app'$/m);
    expect(node).toBeLessThan(bundle);
    expect(daemon).toBeLessThan(bundle);
  });

  it("with a certificate signs the inner Mach-O and then the bundle under the hardened runtime with a timestamp", () => {
    const out = dryRun("mac", CERT);
    at(out, /^\+ security import \S+ -k \S+ -P '<hidden>'/m);
    const inner = at(out, /^\+ codesign --force --options runtime --timestamp .*--sign .*'dist\/wsp Experimental\.app\/Contents\/MacOS\/desktop-gpui'$/m);
    const bundle = at(out, /^\+ codesign --force --options runtime --timestamp .*--sign .*'dist\/wsp Experimental\.app'$/m);
    expect(inner).toBeLessThan(bundle);
    at(out, /^\+ codesign --force --timestamp .*dist\/wsp-experimental-[\d.]+\.dmg$/m);
    at(out, /^skip {2}notarizing: APPLE_ID, APPLE_TEAM_ID and APPLE_APP_SPECIFIC_PASSWORD are not all set$/m);
    expect(out).not.toMatch(/--sign -|notarytool|stapler/);
  });

  it("with the Apple account too notarizes the disk image, then staples the app and the image before zipping the app", () => {
    const out = dryRun("mac", { ...CERT, ...APPLE });
    const submit = at(out, /^\+ xcrun notarytool submit dist\/wsp-experimental-[\d.]+\.dmg --apple-id '<hidden>' --team-id '<hidden>' --password '<hidden>' --wait$/m);
    const app = at(out, /^\+ xcrun stapler staple 'dist\/wsp Experimental\.app'$/m);
    const dmg = at(out, /^\+ xcrun stapler staple dist\/wsp-experimental-[\d.]+\.dmg$/m);
    const zip = at(out, /^\+ ditto -c -k --sequesterRsrc --keepParent 'dist\/wsp Experimental\.app' dist\/wsp-experimental-[\d.]+-mac\.zip$/m);
    expect(submit).toBeLessThan(app);
    expect(app).toBeLessThan(zip);
    expect(dmg).toBeLessThan(zip);
    expect(out).not.toMatch(/^skip {2}notarizing/m);
  });

  it("on Linux builds the binary and packs the AppImage behind the pinned runtime, with no Apple tool", () => {
    const out = dryRun("linux", { ...CERT, ...APPLE });
    at(out, /^\+ cargo build --release --locked -p desktop-gpui$/m);
    at(out, /^\+ curl -fsSL https:\/\/nodejs\.org\/dist\/v22\.23\.2\/node-v22\.23\.2-linux-x64\.tar\.gz /m);
    const node = at(out, /^\+ install -m 755 \S+node-linux-x64\/bin\/node \S+\/AppDir\/usr\/bin\/node$/m);
    const squash = at(out, /^\+ mksquashfs /m);
    expect(node).toBeLessThan(squash);
    at(out, /^\+ cp -R \.\.\/desktop\/build\/app\/assets \S+\/AppDir\/usr\/lib\/wsp\/app\/assets$/m);
    at(out, /^\+ curl -fsSL https:\/\/github\.com\/AppImage\/type2-runtime\/releases\/download\/20251108\/runtime-x86_64 /m);
    at(out, /^\+ mksquashfs /m);
    at(out, /^\+ write dist\/wsp-experimental-[\d.]+-x86_64\.AppImage from /m);
    expect(out).not.toMatch(/codesign|xcrun|hdiutil/);
  });
});
