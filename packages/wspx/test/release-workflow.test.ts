// SPDX-License-Identifier: AGPL-3.0-only
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { ASSET_KINDS, DAEMON_TARGETS, daemonArtifactName, workspaceAsset } from "@wsp/host";
import { bundleEnv, bundleNames, CLASSIC_STABLE_NAMES, classicNames, STABLE_NAMES } from "../../protocol/src/bundles.mjs";
import { CLASSIC_APP_MB, sizeOnDiskMb, sizeVerdict } from "../scripts/bundle-size.mjs";
import { writeStub } from "../../protocol/test/stub-script.js";

const repo = fileURLToPath(new URL("../../..", import.meta.url));
const workflow = readFileSync(join(repo, ".github", "workflows", "release.yml"), "utf8");
const desktopPackage = JSON.parse(readFileSync(join(repo, "apps", "desktop", "package.json"), "utf8")) as { scripts: Record<string, string>; devDependencies: Record<string, string> };
const desktopScripts = desktopPackage.scripts;
const builderConfig = readFileSync(join(repo, "apps", "desktop", "electron-builder.yml"), "utf8");
const jobAt = (id: string) => workflow.indexOf(`\n  ${id}:\n`);
/** The GPUI app's jobs, the main download, then the classic app's, the Electron app the release lists second. */
const macJob = workflow.slice(jobAt("mac"), jobAt("linux"));
const linuxJob = workflow.slice(jobAt("linux"), jobAt("mac-classic"));
const macClassicJob = workflow.slice(jobAt("mac-classic"), jobAt("linux-classic"));
const linuxClassicJob = workflow.slice(jobAt("linux-classic"), jobAt("npm"));
const bundleJobs = [macJob, linuxJob, macClassicJob, linuxClassicJob];
const npmJob = workflow.slice(workflow.indexOf("\n  npm:\n"), workflow.indexOf("\n  publish:\n"));
const publishJob = workflow.slice(workflow.indexOf("\n  publish:\n"));
const daemonJob = workflow.slice(workflow.indexOf("\n  daemon:\n"), workflow.indexOf("\n  draft:\n"));
/** The names bundle-env.mjs writes into a job's environment, which is where every job reads them from. */
const envNames = bundleEnv("0.1.5")
  .trim()
  .split("\n")
  .map(line => line.slice(0, line.indexOf("=")));
/** What signs and notarizes the mac bundles, by the names electron-builder reads them under. */
const SIGNING_SECRETS = ["CSC_LINK", "CSC_KEY_PASSWORD", "APPLE_ID", "APPLE_APP_SPECIFIC_PASSWORD", "APPLE_TEAM_ID"];
const made: string[] = [];

afterEach(() => {
  for (const dir of made.splice(0)) rmSync(dir, { recursive: true, force: true });
});

/** The shell one step of a job runs, taken out of the block the runner hands to bash. */
function stepScript(job: string, name: string): string {
  const at = job.indexOf(`- name: ${name}\n`);
  expect(at, name).toBeGreaterThan(-1);
  const body = job.slice(job.indexOf("run: |\n", at) + "run: |\n".length).split("\n");
  const lines: string[] = [];
  for (const line of body) {
    if (line.trim() !== "" && !line.startsWith(" ".repeat(10))) break;
    lines.push(line.slice(10));
  }
  return `${lines.join("\n").trimEnd()}\n`;
}

/** A gh of its own answering the read of the release served as latest, and recording every `gh release` call so a
 * case reads whether the draft was ever flipped. */
function stubGh(answer: string): string {
  return `#!/bin/sh\ncase "$1 $2" in\n  "api repos/wsp-labs/wsp/releases/latest") ${answer} ;;\nesac\ncase "$1" in release) echo "$*" >> edits ;; esac\n`;
}

/** That step as the runner runs it, `bash -e` with that gh first on PATH, in a checkout of its own carrying the
 * scripts the step calls, so nothing is written into this one. They are copied and not linked: node reads a main
 * module through its real path, and a script reached by a link would not know it was run as one. */
function runStep(script: string, gh: string): { status: number; said: string; edits: string } {
  const dir = mkdtempSync(join(tmpdir(), "wsp-publish-"));
  made.push(dir);
  writeStub(join(dir, "gh"), gh);
  cpSync(join(repo, "packages", "wspx", "scripts"), join(dir, "packages", "wspx", "scripts"), { recursive: true });
  mkdirSync(join(dir, "packages", "protocol", "src"), { recursive: true });
  for (const shared of ["semver.mjs", "bundles.mjs"]) cpSync(join(repo, "packages", "protocol", "src", shared), join(dir, "packages", "protocol", "src", shared));
  writeFileSync(join(dir, "step.sh"), script);
  const ran = spawnSync("bash", ["-e", "step.sh"], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, PATH: `${dir}:${process.env["PATH"] ?? ""}`, GITHUB_REF_NAME: "v0.3.0", GITHUB_REPOSITORY: "wsp-labs/wsp" },
  });
  const edits = join(dir, "edits");
  return { status: ran.status ?? -1, said: `${ran.stdout}${ran.stderr}`, edits: existsSync(edits) ? readFileSync(edits, "utf8") : "" };
}

describe("the release workflow", () => {
  it("runs on a pushed release tag, and by hand as a dry run with no box to untick", () => {
    expect(workflow).toContain('on:\n  push:\n    tags:\n      - "v*"\n');
    expect(workflow).not.toContain("branches:");
    expect(workflow).not.toContain("pull_request:");
    expect(workflow).not.toContain("schedule:");
    // A run started by hand publishes nothing whatever ref it runs on, so there is no input to read.
    expect(workflow).toContain("workflow_dispatch:\n");
    expect(workflow).not.toContain("dry_run");
    expect(workflow).not.toContain("inputs.");
    expect(workflow).toContain("if: ${{ github.event_name == 'push' && github.ref_type == 'tag' }}");
    expect(npmJob).toContain("name: Publish\n        if: ${{ github.event_name == 'push' }}");
    expect(npmJob).toContain("needs.daemon.result == 'success' && (github.event_name != 'push' || needs.draft.result == 'success')");
    // npm publishes nothing for a release an app job failed, which stays a draft.
    expect(npmJob).toContain("needs: [draft, daemon, mac, linux, mac-classic, linux-classic]\n");
    expect(npmJob).toContain("if: ${{ !cancelled() && !contains(needs.*.result, 'failure') && ");
    // The mac and linux jobs build and check their bundles on a run by hand too, and only a pushed tag attaches them.
    for (const job of bundleJobs) {
      expect(job).toContain("needs.daemon.result == 'success' && (github.event_name != 'push' || needs.draft.result == 'success')");
      expect(job).toContain("name: Attach them to the draft\n        if: ${{ github.event_name == 'push' }}");
    }
  });

  it("refuses a tag off main's own line before it drafts anything", () => {
    const draftJob = workflow.slice(workflow.indexOf("\n  draft:\n"), workflow.indexOf("\n  mac:\n"));
    expect(draftJob).toContain("git fetch --no-tags origin +refs/heads/main:refs/remotes/origin/main");
    expect(draftJob).toContain('node packages/wspx/scripts/tag-version.mjs "$GITHUB_REF_NAME"');
    // Every job that drafts, builds, publishes or flips waits on that check.
    for (const job of bundleJobs) expect(job).toContain("needs: [draft, daemon]");
    expect(publishJob).toContain("needs: [draft, mac, linux, mac-classic, linux-classic, npm]");
  });

  it("attaches the binaries and decides latest from the job that flips the draft, not from the one that publishes", () => {
    // The job that can mint the identity token writes nothing here, so the attach moved to the job that flips.
    expect(npmJob).not.toContain("gh release upload");
    expect(publishJob).toContain("name: Attach the binaries to the draft");
    expect(publishJob).toContain('gh release upload "$GITHUB_REF_NAME" daemon-bins/*/wsp-daemon-* --clobber');
    expect(publishJob).toContain("pattern: wsp-daemon-*");
    expect(publishJob).toContain("- uses: actions/checkout@");
    expect(publishJob).toContain('gh api "repos/$GITHUB_REPOSITORY/releases/latest" --jq .tag_name');
    expect(publishJob).toContain('node packages/wspx/scripts/latest-flag.mjs "$GITHUB_REF_NAME" "$current"');
    expect(publishJob).toContain('gh release edit "$GITHUB_REF_NAME" --draft=false $flag');
    expect(publishJob).not.toContain("--latest");
  });

  it("reads no release served as latest off the 404 and not off an empty answer, and publishes as latest", () => {
    // gh writes the error body to stdout and exits 1, so an empty answer is never what a repository with no latest
    // release looks like.
    const notFound = `printf '{"message":"Not Found"}\\n'; echo 'gh: Not Found (HTTP 404)' >&2; exit 1`;
    const ran = runStep(stepScript(publishJob, "Turn the draft into a release, latest where its version says so"), stubGh(notFound));
    expect(ran.status, ran.said).toBe(0);
    expect(ran.said).toContain("(none): --latest");
    expect(ran.edits.trim()).toBe("release edit v0.3.0 --draft=false --latest");
  });

  it("fails on any other failure of that read and leaves the release a draft", () => {
    const badGateway = `printf '{"message":"Bad gateway"}\\n'; echo 'gh: Bad gateway (HTTP 502)' >&2; exit 1`;
    const ran = runStep(stepScript(publishJob, "Turn the draft into a release, latest where its version says so"), stubGh(badGateway));
    expect(ran.status).not.toBe(0);
    expect(ran.said).toContain("HTTP 502");
    // The step stops on the read that failed and says so, rather than handing the error body on and failing later
    // under a sentence about a tag nobody named.
    expect(ran.said).not.toContain("not a release tag");
    expect(ran.edits).toBe("");
  });

  it("reads the tag it is served and hands it to the flag script", () => {
    const ran = runStep(stepScript(publishJob, "Turn the draft into a release, latest where its version says so"), stubGh("echo v0.2.0"));
    expect(ran.status, ran.said).toBe(0);
    expect(ran.edits.trim()).toBe("release edit v0.3.0 --draft=false --latest");
    expect(ran.said).toContain("v0.3.0 against the release served as latest (v0.2.0): --latest");
  });

  it("builds one static daemon per target the host names, uploads each under the artifact name the host spells, and places all of them before every build", () => {
    // Each Linux binary is built natively on its own chip's runner, so the static link needs no cross toolchain.
    const runners: Record<string, string> = { "x86_64-unknown-linux-musl": "ubuntu-24.04", "aarch64-unknown-linux-musl": "ubuntu-24.04-arm", "aarch64-apple-darwin": "macos-14", "x86_64-apple-darwin": "macos-14" };
    for (const target of DAEMON_TARGETS) {
      const row = daemonJob.slice(daemonJob.indexOf(`- target: ${target.triple}\n`));
      expect(row.length).toBeGreaterThan(0);
      expect(/os: (\S+)/.exec(row)![1]).toBe(runners[target.triple]);
    }
    expect(daemonJob).toContain(`name: ${daemonArtifactName("${{ matrix.target }}")}`);
    expect(daemonJob).toContain("./scripts/libseccomp-archive.sh musl");
    expect(daemonJob).toContain('LIBSECCOMP_LIB_PATH="$PWD/target/libseccomp/musl" cargo build --locked --release --target "$TARGET" -p wsp-daemon-bin');
    expect(daemonJob).not.toContain("zig");
    // Every job that builds the command or the app reads the binaries back first, through the one script.
    for (const job of [...bundleJobs, npmJob]) {
      expect(job).toContain("pattern: wsp-daemon-*");
      expect(job).toContain("node packages/wspx/scripts/daemon-binary.mjs --from-artifacts daemon-bins");
      expect(job).toMatch(/needs: \[draft, daemon[\],]/);
    }
    expect(npmJob).toContain("node packages/wspx/scripts/daemon-binary.mjs --check");
    // The release is the one place the daemon binaries are required of a build: each job that builds sets the
    // variable the stage reads, so a missing binary fails there and a checkout without one still builds.
    for (const job of [...bundleJobs, npmJob]) expect(job).toContain('WSP_REQUIRE_DAEMON: "1"');
    expect(workflow.split('WSP_REQUIRE_DAEMON: "1"').length - 1).toBe(5);
    expect(existsSync(join(repo, "packages/wspx/scripts/daemon-binary.mjs"))).toBe(true);
  });

  it("builds everything it ships public and reads each bundle for a cloud provider before it leaves the runner", () => {
    const roads = [
      // The GPUI apps are read whole, their own binary with them; the AppImage unpacked, since its squashfs is compressed.
      { job: macJob, build: "- name: Build the app\n", check: "node packages/wspx/scripts/public-check.mjs apps/desktop-gpui/dist/wsp.app\n", ships: "gh release upload" },
      { job: linuxJob, build: "- name: Build the bundle\n", check: '"$GITHUB_WORKSPACE"/apps/desktop-gpui/dist/*.AppImage --appimage-extract >/dev/null\n          node "$GITHUB_WORKSPACE"/packages/wspx/scripts/public-check.mjs squashfs-root\n', ships: "gh release upload" },
      { job: macClassicJob, build: "- name: Build the bundles\n", check: "node packages/wspx/scripts/public-check.mjs apps/desktop/build/app\n", ships: "gh release upload" },
      { job: linuxClassicJob, build: "- name: Build the bundle\n", check: "node packages/wspx/scripts/public-check.mjs apps/desktop/build/app\n", ships: "gh release upload" },
      { job: npmJob, build: "- name: Build, with every daemon binary required\n", check: 'node packages/wspx/scripts/public-check.mjs "$RUNNER_TEMP"/wsp-labs-wsp-*.tgz\n', ships: "npm publish" },
    ];
    for (const { job, build, check, ships } of roads) {
      const at = job.indexOf(build);
      expect(at, build).toBeGreaterThan(-1);
      expect(job.slice(at, job.indexOf("- name:", at + 1))).toContain('PUBLIC_BUILD: "1"');
      expect(job.indexOf(check)).toBeGreaterThan(at);
      expect(job.indexOf(check)).toBeLessThan(job.indexOf(ships));
    }
    // The daemons are built public too, and read where they ship: in each app's folder and in the tarball.
    const daemonBuild = daemonJob.indexOf("- name: Build\n");
    expect(daemonJob.slice(daemonBuild, daemonJob.indexOf("- name:", daemonBuild + 1))).toContain('PUBLIC_BUILD: "1"');
    expect(workflow.split('PUBLIC_BUILD: "1"').length - 1).toBe(roads.length + 1);
  });

  it("publishes the command line package from the runner through trusted publishing, and holds no npm token", () => {
    expect(workflow).toContain("npm publish --provenance --access public");
    expect(workflow).toContain("id-token: write");
    expect(workflow).not.toContain("NPM_TOKEN");
    // setup-node with a registry-url writes an .npmrc that reads NODE_AUTH_TOKEN, so that name is the other road a
    // credential could arrive by and neither is allowed to appear.
    expect(workflow).not.toContain("NODE_AUTH_TOKEN");
    expect(workflow).not.toContain("pnpm release");
    // npm's own latest dist-tag would move under the identity token, so the version it installs is exact.
    expect(npmJob).toMatch(/npm install -g npm@\d+\.\d+\.\d+\n/);
    expect(workflow).toContain("--draft");
    expect(workflow).toContain("--draft=false");
  });

  it("asks the scripts in the repo for the version, the notes and every asset name", () => {
    for (const script of ["packages/wspx/scripts/tag-version.mjs", "packages/wspx/scripts/release-notes.mjs", "packages/wspx/scripts/bundle-env.mjs", "packages/wspx/scripts/latest-flag.mjs"]) {
      expect(workflow).toContain(`node ${script}`);
      expect(existsSync(join(repo, script))).toBe(true);
    }
  });

  it("builds the classic app on each platform through the script apps/desktop owns", () => {
    expect(macClassicJob).toContain("pnpm --filter @wsp/desktop run build:mac");
    expect(linuxClassicJob).toContain("pnpm --filter @wsp/desktop run build:linux");
    for (const job of [macJob, linuxJob]) expect(job).not.toContain("@wsp/desktop run build:");
    const check = macClassicJob.indexOf("- name: Check the packaged trees\n");
    const smoke = macClassicJob.indexOf("- name: Drive the packaged app with the smoke\n");
    expect(check).toBeGreaterThan(-1);
    expect(smoke).toBeGreaterThan(check);
    expect(macClassicJob.indexOf("- name:", check + 1)).toBe(smoke);
    expect(macClassicJob.slice(smoke, macClassicJob.indexOf("- name:", smoke + 1))).toContain("run: pnpm --filter @wsp/desktop smoke\n");
    expect(workflow).not.toContain("WSP_DESKTOP_SCREEN");
    expect(workflow).not.toContain("merge gate");
    // ci runs the signing check word for word on every land run, so a root config that breaks it from apps/desktop
    // fails a landing and not a tag.
    const signing = "run: pnpm --filter @wsp/desktop exec vitest run --minWorkers=1 --maxWorkers=1 test/signing.test.ts\n";
    expect(macClassicJob.slice(check, smoke)).toContain(signing);
    expect(readFileSync(join(repo, ".github", "workflows", "ci.yml"), "utf8")).toContain(signing);
    expect(workflow).not.toContain("pty-native");
    expect(desktopScripts["build:mac"]).toContain("--mac");
    expect(desktopScripts["build:linux"]).toContain("--linux");
    // The plain build packages the machine it runs on and the workflow's two jobs are what make both platforms.
    expect(desktopScripts["build"]).toBe("pnpm run build:deps && pnpm run build:app && electron-builder --config electron-builder.yml --publish never");
  });

  it("starts each AppImage it uploads once, after the rename and before the upload, on a virtual display", () => {
    const roads = [
      { job: linuxJob, step: "- name: Start the AppImage once and wait for its window\n", run: 'run: xvfb-run --auto-servernum apps/desktop-gpui/scripts/appimage-launch.sh "$APPIMAGE"\n' },
      { job: linuxClassicJob, step: "- name: Start the AppImage once and wait for its window's page\n", run: 'run: xvfb-run --auto-servernum node apps/desktop/scripts/appimage-launch.mjs "$CLASSIC_APPIMAGE"\n' },
    ];
    for (const { job, step, run } of roads) {
      const rename = job.indexOf("- name: Name the AppImage after the release");
      const launch = job.indexOf(step);
      const upload = job.indexOf("- name: Attach them to the draft\n");
      expect(rename).toBeGreaterThan(-1);
      expect(job.indexOf("- name:", rename + 1)).toBe(launch);
      expect(job.indexOf("- name:", launch + 1)).toBe(upload);
      expect(job.slice(launch, upload)).toContain(run);
    }
    const checked = spawnSync(process.execPath, ["--check", join(repo, "apps", "desktop", "scripts", "appimage-launch.mjs")], { encoding: "utf8" });
    expect(checked.status, checked.stderr).toBe(0);
    const parsed = spawnSync("bash", ["-n", join(repo, "apps", "desktop-gpui", "scripts", "appimage-launch.sh")], { encoding: "utf8" });
    expect(parsed.status, parsed.stderr).toBe(0);
    // The GPUI AppImage runs unpacked, as the classic one does, rather than mounted, which needs a FUSE the runner may lack.
    expect(readFileSync(join(repo, "apps", "desktop-gpui", "scripts", "appimage-launch.sh"), "utf8")).toMatch(/^setsid env -i (?:.|\\\n)*? APPIMAGE_EXTRACT_AND_RUN=1 /m);
  });

  it("builds every package the app's staging copies from in build:deps, the only build the release runs first", () => {
    expect(desktopScripts["build:deps"]).toBe('pnpm --filter "@wsp/desktop^..." build');
    for (const kind of ASSET_KINDS) {
      let dir = workspaceAsset(kind);
      while (!existsSync(join(dir, "package.json"))) {
        if (dirname(dir) === dir) throw new Error(`staged asset ${kind} sits in no package`);
        dir = dirname(dir);
      }
      const { name } = JSON.parse(readFileSync(join(dir, "package.json"), "utf8")) as { name: string };
      expect(desktopPackage.devDependencies[name], `${kind} asset from ${name}`).toBe("workspace:*");
    }
  });

  it("makes one mac bundle for both chips, so no download depends on reading the chip", () => {
    expect(desktopScripts["build:mac"]).not.toMatch(/--(arm64|x64)\b/);
    expect(/^mac:\n((?: .*\n)+)/m.exec(builderConfig)?.[1]).toContain("arch: [universal]");
    for (const job of [macJob, macClassicJob]) {
      expect(job).not.toContain("mac-arm64");
      expect(job).not.toContain("dist/mac");
    }
    expect(macJob).toContain("run: apps/desktop-gpui/scripts/package.sh --target mac\n");
    expect(readFileSync(join(repo, "apps", "desktop-gpui", "scripts", "package.sh"), "utf8")).toContain('run lipo -create -output "$app/Contents/MacOS/$bin"');
  });

  it("builds the AppImage on the static runtime, which opens where libfuse.so.2 is not installed", () => {
    expect(builderConfig).toMatch(/^toolsets:\n {2}appimage: "(?!0\.0\.0")[\d.]+"$/m);
  });

  it("hands the signing secrets by name to the step that builds, tells the step that checks whether one signed, and no value anywhere", () => {
    const secretOf = (name: string) => `${name}: \${{ secrets.${name} }}`;
    for (const name of SIGNING_SECRETS) {
      const assignments = workflow.split("\n").filter(line => line.includes(`${name}:`)).map(line => line.trim());
      expect(new Set(assignments)).toEqual(new Set([secretOf(name)]));
      expect(assignments).toHaveLength(2);
    }
    for (const { job, building, checking } of [
      { job: macJob, building: "name: Build the app", checking: "name: Check the signature" },
      { job: macClassicJob, building: "name: Build the bundles", checking: "name: Check the packaged trees" },
    ]) {
      const [header, ...steps] = job.split("\n      - ");
      const build = steps.find(step => step.startsWith(building));
      const check = steps.find(step => step.startsWith(checking));
      for (const name of SIGNING_SECRETS) expect(build).toContain(secretOf(name));
      // The check step reads whether an identity signed and never the certificate itself, so no test's environment
      // carries its bytes.
      expect(check).toContain("WSP_SIGNED: ${{ secrets.CSC_LINK != '' && '1' || '' }}");
      for (const name of SIGNING_SECRETS) expect(check).not.toContain(secretOf(name));
      expect(check).not.toMatch(/secrets\.(CSC_KEY|APPLE)/);
      for (const part of [header, ...steps.filter(step => step !== build && step !== check)]) expect(part).not.toMatch(/secrets\.(CSC|APPLE)/);
    }
    for (const job of [linuxJob, linuxClassicJob]) expect(job).not.toMatch(/secrets\.(CSC|APPLE)/);
    // The test that reads whether an identity signed reads the name this step sets, so the two move as one.
    const signing = readFileSync(join(repo, "apps", "desktop", "test", "signing.test.ts"), "utf8");
    expect(signing).toContain("WSP_SIGNED");
    expect(signing).not.toContain("CSC_LINK");
    expect(workflow).not.toContain("Developer ID Application:");
  });

  it("unsets an empty CSC_LINK before the build, which would take it for a certificate", () => {
    expect(workflow).toContain('if [ -z "$CSC_LINK" ]; then unset CSC_LINK; fi\n          pnpm --filter @wsp/desktop run build:mac');
  });

  it("drafts the notes without the right-click Open paragraph once the certificate signs the bundles", () => {
    expect(workflow).toContain(`release-notes.mjs "$GITHUB_REF_NAME" \${{ secrets.CSC_LINK != '' && '--signed' || '' }} > notes.md`);
  });

  it("takes every asset name from the one formatter and spells none of its own", () => {
    for (const job of bundleJobs) expect(job).toContain('node packages/wspx/scripts/bundle-env.mjs "$VERSION" >> "$GITHUB_ENV"');
    const names = [bundleNames("$VERSION"), classicNames("$VERSION"), STABLE_NAMES, CLASSIC_STABLE_NAMES].flatMap(set => Object.values(set));
    for (const name of names) expect(workflow).not.toContain(name);
    expect(workflow).not.toMatch(/wsp-[^"\s]*\.(dmg|AppImage|zip)/);
    for (const name of envNames) expect(workflow).toContain(`$${name}`);
  });

  it("uploads each bundle under the release's own name and under the name a link can hold, the GPUI app's under today's names", () => {
    for (const [job, dist, prefix] of [
      [macJob, "apps/desktop-gpui/dist", ""],
      [macClassicJob, "apps/desktop/dist", "CLASSIC_"],
    ] as const) {
      expect(job).toContain(`gh release upload "$GITHUB_REF_NAME" "$${prefix}MAC_DMG" "$${prefix}MAC_STABLE" "$${prefix}MAC_ZIP" --clobber`);
      expect(job).toContain(`set -- ${dist}/*.dmg\n`);
      expect(job).toContain(`mv "$1" "$${prefix}MAC_DMG"\n          cp "$${prefix}MAC_DMG" "$${prefix}MAC_STABLE"`);
      expect(job).toContain(`set -- ${dist}/*.zip\n`);
      expect(job).toContain(`mv "$1" "$${prefix}MAC_ZIP"`);
    }
    for (const [job, dist, prefix] of [
      [linuxJob, "apps/desktop-gpui/dist", ""],
      [linuxClassicJob, "apps/desktop/dist", "CLASSIC_"],
    ] as const) {
      expect(job).toContain(`gh release upload "$GITHUB_REF_NAME" "$${prefix}APPIMAGE" "$${prefix}APPIMAGE_STABLE" --clobber`);
      expect(job).toContain(`set -- ${dist}/*.AppImage\n`);
      expect(job).toContain(`cp "$${prefix}APPIMAGE" "$${prefix}APPIMAGE_STABLE"`);
    }
    expect(workflow.split("gh release upload").length - 1).toBe(5);
  });

  it("lists the GPUI app first and the classic app second, its jobs before the classic app's", () => {
    const order = ["daemon", "draft", "mac", "linux", "mac-classic", "linux-classic", "npm", "publish"].map(jobAt);
    for (const at of order) expect(at).toBeGreaterThan(-1);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(macJob).toContain("name: macOS app\n");
    expect(macClassicJob).toContain("name: macOS disk image, the classic app\n");
    expect(linuxClassicJob).toContain("name: Linux AppImage, the classic app\n");
    // The GPUI jobs build out of apps/desktop-gpui alone and the classic jobs out of apps/desktop alone.
    for (const job of [macJob, linuxJob]) expect(job).not.toContain("apps/desktop/dist");
    for (const job of [macClassicJob, linuxClassicJob]) expect(job).not.toContain("apps/desktop-gpui");
    expect(linuxJob).toContain("run: apps/desktop-gpui/scripts/package.sh --target linux\n");
  });

  it("refuses a GPUI Mac app that is not smaller on disk than the classic app, after the build and before anything is named or attached", () => {
    const build = macJob.indexOf("- name: Build the app\n");
    const size = macJob.indexOf("- name: The app is smaller on disk than the classic app\n");
    const rename = macJob.indexOf("- name: Name the disk image after the release");
    expect(build).toBeGreaterThan(-1);
    expect(size).toBeGreaterThan(build);
    expect(rename).toBeGreaterThan(size);
    expect(macJob.slice(size, rename)).toContain("run: node packages/wspx/scripts/bundle-size.mjs apps/desktop-gpui/dist/wsp.app\n");
    // Today's 0.3.8 wsp.app, of which Electron's framework is 451 MB.
    expect(CLASSIC_APP_MB).toBe(515);
    expect(sizeVerdict(514.9).ok).toBe(true);
    expect(sizeVerdict(515)).toEqual({ ok: false, line: "refused: 515.0 MB on disk, against the classic app's 515 MB; the main download may not grow" });
    // Counted as du counts it, every file's blocks once and no link followed.
    const app = mkdtempSync(join(tmpdir(), "wsp-bundle-size-"));
    made.push(app);
    mkdirSync(join(app, "Contents", "MacOS"), { recursive: true });
    writeFileSync(join(app, "Contents", "MacOS", "wsp"), Buffer.alloc(3 * 1024 * 1024, 1));
    symlinkSync("/usr", join(app, "Contents", "elsewhere"));
    const mb = sizeOnDiskMb(app);
    expect(mb).toBeGreaterThanOrEqual(3);
    expect(mb).toBeLessThan(3.2);
    const script = join(repo, "packages", "wspx", "scripts", "bundle-size.mjs");
    const passed = spawnSync(process.execPath, [script, app], { encoding: "utf8" });
    expect(passed.status, passed.stderr).toBe(0);
    expect(passed.stdout).toMatch(/^ok {4}3\.\d MB on disk, against the classic app's 515 MB\n$/);
    const missing = spawnSync(process.execPath, [script, join(app, "absent")], { encoding: "utf8" });
    expect(missing.status).toBe(1);
  });

  it("never lets the packager upload a release of its own", () => {
    for (const name of ["build:mac", "build:linux"]) expect(desktopScripts[name]).toContain("--publish never");
  });
});
