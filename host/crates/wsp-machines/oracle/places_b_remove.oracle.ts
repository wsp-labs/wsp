// SPDX-License-Identifier: AGPL-3.0-only
// The TS sentences, scripts and digests the places/manage remove unit's Rust tests compare with, written as
// tests/fixtures/oracle/places_b_remove.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 places_b_remove.oracle.ts
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import * as p from "@wsp/protocol";
import { CODEX_TOML, MCP_SERVERS_JSON } from "@wsp/catalog";

const t = await import("../../../../packages/runtime/src/places/types.ts");
const h = await import("../../../../packages/runtime/src/places/helpers.ts");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/places_b_remove.json", import.meta.url));
const sha = (entry: string): string => createHash("sha256").update(entry).digest("hex");

it("writes the places b remove fixture", () => {
  const ssh = "maya@10.0.0.5";
  const name = "old-macbook";
  const said = "vps ran the leave and did not finish it: Failed to stop: Unit is masked.";
  const away = p.placeLoginElsewhere(ssh, name);
  const awayOther = p.placeLoginElsewhere(ssh, name, "vps");
  const halves = (r: { happened: string; fix: string }) => ({ happened: r.happened, fix: r.fix, said: p.refusalLine(r.happened, r.fix) });
  const refusal = (r: { said: string; fix: string }) => ({ said: p.refusalLine(r.said, r.fix), fix: r.fix });
  const config = "/home/maya/.codex/config.toml";
  const theirs = ['[projects."/root/repo"]', 'trust_level = "trusted"', ""];
  const codexText = [...theirs, "[mcp_servers.context7]", 'command = "npx"', ""].join("\n");
  const claudeText = JSON.stringify({ mcpServers: { gsc: { command: "npx", args: ["gsc-mcp"] } } });
  const codexOnly = ["[mcp_servers.context7]", 'command = "npx"', ""].join("\n");
  const words = {
    leaveLine: p.PLACE_LEAVE_LINE,
    away,
    awayOther,
    elsewhereRemoved: p.placeLoginElsewhereRemovedLine(ssh, name),
    elsewhereRemovedOther: p.placeLoginElsewhereRemovedLine(ssh, name, "vps"),
    elsewhereSweptOverLink: t.placeElsewhereSweptOverLinkLine(name, away),
    elsewhereSweptOverLinkOther: t.placeElsewhereSweptOverLinkLine(name, awayOther),
    sweptOverLink: t.placeSweptOverLinkLine(name, ssh),
    sweptOverLinkSaid: t.placeSweptOverLinkLine(name, ssh, said),
    sweptOverSsh: t.placeSweptOverSshLine(name),
    loginRoadSaid: t.placeLoginRoadLine(name, ssh, said),
    loginRoadNone: t.placeLoginRoadLine(name, ssh),
    stillInstalled: p.placeStillInstalledLine(name),
    forgotten: p.placeForgottenLine(name, false),
    forgottenForks: p.placeForgottenLine(name, true),
    pluginOff: p.pluginOffLine("superpowers@market"),
    pluginsKept: p.pluginsKeptLine(name, ["stuck@market"]),
    pluginsKeptTwo: p.pluginsKeptLine(name, ["lint", "stuck"]),
    pluginsKeptStuck: p.pluginsKeptLine(name, ["stuck"]),
    unsavedRefusal: refusal(p.placeUnsavedRefusal(name, ["site has 2 commits no remote has", "/wsp/projects/old/checkout has 1 commit no remote has"])),
    awayRefusal: refusal(p.placeAwayRefusal(name, p.absentComputer(name, null).said)),
    forgetAnswersRefusal: refusal(p.placeForgetAnswersRefusal(name)),
    uncheckedSudo: halves(p.placeLoginUncheckedRefusal(ssh, name, "sudo: a password is required")),
    uncheckedTimeout: halves(p.placeLoginUncheckedRefusal(ssh, name, "the read timed out after 15s")),
    uncheckedNoHome: halves(p.placeLoginUncheckedRefusal(ssh, name, `${name} never said where its home is`)),
    uncheckedNoRoad: halves(p.placeLoginUncheckedRefusal(ssh, name, "this host holds no ssh road to read it over")),
    uncheckedExit: halves(p.placeLoginUncheckedRefusal(ssh, name, "the read exited 3")),
    otherRefusal: halves(p.placeLoginOtherRefusal(ssh, { id: "p_0000000000000001", name }, undefined)),
    otherRefusalNamed: halves(p.placeLoginOtherRefusal(ssh, { id: "p_0000000000000001", name }, "vps")),
    noSuchPlace: p.noSuchPlaceRefusal("p_nope", [name]),
    syncing: p.placeSyncingLine(name),
    unpushedUnread: p.unpushedUnreadLine("/wsp/projects", "the listing exited 2"),
    unpushedUnreadStderr: p.unpushedUnreadLine("/wsp/projects", "Permission denied"),
    updateSlow60: t.placeUpdateSlowLine(name, 60),
    updateSlow1: t.placeUpdateSlowLine(name, 1),
    updateWaitMs: h.UPDATE_WAIT_MS,
    noPlaceUpdater: t.NO_PLACE_UPDATER,
    serversUnread: p.serversUnreadLine(config, p.CONFIG_NOT_PLAIN),
    behindWord: p.placeDaemonBehind({ daemonVersion: p.DAEMON_VERSION - 1 } as never)!,
    daemonVersion: p.DAEMON_VERSION,
    unsavedOld: p.unpushedLine("/wsp/projects/old/checkout", { branch: "", ahead: 2, behind: 0, changed: 1, readAt: 0 }),
    unreadFresh: p.unpushedUnreadLine("/wsp/projects/fresh/checkout", "git failed"),
    unreadBoom: p.unpushedUnreadLine("/wsp/projects", "boom"),
    blocked: p.placeCannotBootLine("srv", "this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1"),
  };
  const scripts = {
    held: p.heldPlaceScript("/home/maya"),
    cgroupsEnd: p.threadCgroupsEndScript(),
    placeFile: "/home/maya/.wsp/place.json",
  };
  const unmerge = {
    codexText,
    codexConfig: config,
    codexDigest: sha(CODEX_TOML.entryOf(codexText, "context7")!),
    claudeText,
    claudeDigest: sha(MCP_SERVERS_JSON.entryOf(claudeText, "gsc")!),
    codexOnlyText: codexOnly,
    codexOnlyDigest: sha(CODEX_TOML.entryOf(codexOnly, "context7")!),
  };
  writeFileSync(out, `${JSON.stringify({ words, scripts, unmerge }, null, 2)}\n`);
});
