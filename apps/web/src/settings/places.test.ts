// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { DAEMON_VERSION, JOINED_COMPUTER, PLACE_BLOCKED_WORD, absentComputer, placeDaemonBehind, placeWord, type PlaceSetup, type PlaceSync, type PlaceView, type SealedImageCopy, type WorkspaceView } from "@wsp/protocol";
import { copyOn } from "./image.js";
import { NOTHING_HELD, PLACE_KIND_WORDS, hereName, outcomeWord, placeName, placeOf, placeStateCell, providerForgetSentence, removeSentence, removeTitle } from "./places.js";

const NOW = Date.parse("2026-09-12T12:00:00.000Z");
const ago = (ms: number): string => new Date(NOW - ms).toISOString();

const MAC = "zingzy's MacBook Pro";
const here: PlaceView = { id: "here", kind: "computer", name: "zingzys-macbook-pro.local", default: false, shape: { cpu: 8, memMb: 16 * 1024 }, diskFreeBytes: 210 * 1024 ** 3, engine: "none", present: true, takesForks: false };
const hetzner: PlaceView = {
  id: "p_1",
  kind: "computer",
  name: "hetzner",
  default: true,
  shape: { cpu: 2, memMb: 4 * 1024 },
  diskFreeBytes: 38 * 1024 ** 3,
  engine: "docker",
  present: true,
  takesForks: true,
  takesRuntime: true,
  joinedAt: ago(60 * 60 * 1000),
  lastSeenAt: ago(3_000),
};
const laptop: PlaceView = { ...hetzner, id: "p_2", name: "old-macbook", default: false, takesRuntime: false, shape: { cpu: 4, memMb: 8 * 1024 }, diskFreeBytes: 91 * 1024 ** 3, engine: "none", present: false, lastSeenAt: ago(2 * 60 * 60 * 1000) };
const SOLARI: PlaceView = { id: "solari", kind: "provider", name: "solari", default: false, rateUsdPerHour: 0.11, takesForks: true };
const ascii: PlaceView = { id: "box", kind: "provider", name: "box", default: false, shape: { cpu: 2, memMb: 4 * 1024 }, diskFreeBytes: 40 * 1024 ** 3, rateUsdPerHour: 0.018, takesForks: true };

describe("what the section computes beyond the table's own cells", () => {
  it("reads a provider row under the name a person knows it by", () => {
    expect(placeName(ascii)).toBe("Boat");
    expect(placeName(hetzner)).toBe("hetzner");
  });

  it("calls the computer the host runs on by the name its owner gave it, else its hostname, never this Mac", () => {
    expect(placeName({ ...here, name: "zingzys-macbook-pro.local", label: MAC })).toBe(MAC);
    expect(placeName({ ...here, name: "zingzys-macbook-pro.local" })).toBe("zingzys-macbook-pro.local");
    expect(hereName([{ ...here, label: MAC }, hetzner])).toBe(MAC);
    expect(hereName([hetzner, { ...here, label: MAC }])).toBe(MAC);
    // Until the places list holds the row there is no name, and no stand-in word is said in its place.
    expect(hereName([hetzner])).toBe("");
    expect(hereName([])).toBe("");
    expect(removeSentence(hetzner, NOTHING_HELD, "")).toBe("");
  });
});

describe("the remove sentence", () => {
  it("names what comes off a computer that holds workspaces, and what leaves the computer the host runs on", () => {
    expect(removeTitle(hetzner)).toBe("Remove hetzner?");
    expect(removeSentence(hetzner, { workspaces: [{ name: "spoo-fix", state: "Running", threads: 2 }] }, MAC, 4.2 * 1024 ** 3)).toBe(
      "wsp, its task and the copy of your image (4.2 GB) come off hetzner, which is otherwise left as it is. The task's record and 2 threads leave zingzy's MacBook Pro.",
    );
  });

  it("says workspaces and records in the plural above one", () => {
    expect(removeSentence(hetzner, { workspaces: [{ name: "a", state: "Running", threads: 2 }, { name: "b", state: "Running", threads: 1 }] }, MAC, 4.2 * 1024 ** 3)).toBe(
      "wsp, its 2 tasks and the copy of your image (4.2 GB) come off hetzner, which is otherwise left as it is. The tasks' records and 3 threads leave zingzy's MacBook Pro.",
    );
  });

  it("names the projects that leave with a computer once the host has said which", () => {
    expect(removeSentence(hetzner, { workspaces: [], projects: ["wsp-vm"] }, MAC)).toBe("wsp and the copy of your image come off hetzner, which is otherwise left as it is. Its project wsp-vm leaves zingzy's MacBook Pro.");
    expect(removeSentence(hetzner, { workspaces: [], projects: ["a", "b"] }, MAC)).toContain("Its 2 projects a, b leave zingzy's MacBook Pro.");
  });

  it("drops the second sentence for a computer that holds none", () => {
    expect(removeSentence(hetzner, NOTHING_HELD, MAC, 4.2 * 1024 ** 3)).toBe("wsp and the copy of your image (4.2 GB) come off hetzner, which is otherwise left as it is.");
  });

  it("leaves the size out where nothing has measured the image", () => {
    expect(removeSentence(hetzner, NOTHING_HELD, MAC)).toBe("wsp and the copy of your image come off hetzner, which is otherwise left as it is.");
  });

  it("says what becomes of the copy of the image, since every computer that joined runs workspaces and holds one", () => {
    // What Remove promises about four gigabytes of somebody's disk is what the sweep does: the leave takes the
    // runtime's folder the copy sits in, so the copy comes off with wsp.
    expect(removeSentence(hetzner, NOTHING_HELD, MAC, 4.2 * 1024 ** 3)).toBe("wsp and the copy of your image (4.2 GB) come off hetzner, which is otherwise left as it is.");
  });

  it("says the copy of the image stays on a computer whose leave keeps the runtime's folder, as one joined with a code does", () => {
    const studio: PlaceView = { ...hetzner, name: "studio", takesRuntime: false };
    expect(removeSentence(studio, NOTHING_HELD, MAC, 4.2 * 1024 ** 3)).toBe("wsp comes off studio, which keeps the copy of your image (4.2 GB) and is otherwise left as it is.");
    expect(removeSentence(studio, { workspaces: [{ name: "spoo-fix", state: "Running", threads: 2 }] }, MAC)).toBe(
      "wsp and its task come off studio, which keeps the copy of your image and is otherwise left as it is. The task's record and 2 threads leave zingzy's MacBook Pro.",
    );
  });

  it("says Boat's machines still standing are deleted there and its key forgotten here, its sub-threads kept to ask for it again", () => {
    expect(removeSentence(ascii, { workspaces: [{ name: "fix", state: "Running", threads: 1 }, { name: "cap", state: "Stopped", threads: 1 }, { name: "events", state: "Deleted", threads: 1 }] }, MAC)).toBe(
      "Its 2 machines are deleted at Boat and the key is forgotten on zingzy's MacBook Pro. The 3 sub-threads stay with their transcripts, and a message to one asks for Boat again.",
    );
    expect(removeSentence(ascii, { workspaces: [{ name: "events", state: "Deleted", threads: 1 }] }, MAC)).toBe("The key for Boat is forgotten on zingzy's MacBook Pro. Its sub-thread stays with its transcript, and a message to one asks for Boat again.");
  });

  it("names each image snapshot this host saved at a cloud with its size, deleted there before the key goes", () => {
    expect(removeSentence(ascii, { workspaces: [{ name: "fix", state: "Running", threads: 1 }], images: [{ id: "wsp-acme-default-v1", sizeBytes: 345_000_000 }, { id: "wsp-acme-project-api" }] }, MAC)).toBe(
      "Its machine is deleted at Boat and the key is forgotten on zingzy's MacBook Pro. Its 2 image snapshots, wsp-acme-default-v1 (329 MB) and wsp-acme-project-api, are deleted at Boat before the key goes. Its sub-thread stays with its transcript, and a message to one asks for Boat again.",
    );
    expect(removeSentence(SOLARI, { workspaces: [], images: [{ id: "snap_7", sizeBytes: 9_120_485_376 }] }, MAC)).toBe("The key for Solari is forgotten on zingzy's MacBook Pro. Its image snapshot snap_7 (8.5 GB) is deleted at Solari before the key goes.");
  });

  it("names the image snapshots a forget leaves at a cloud whose key it refused, which bill every month", () => {
    expect(providerForgetSentence(ascii, { workspaces: [], images: [{ id: "wsp-acme-default-v1", sizeBytes: 345_000_000 }] }, MAC)).toBe(
      "The key leaves zingzy's MacBook Pro. Nothing at Boat is touched, so what wsp made there may bill until Boat's own timer ends it. Its image snapshot wsp-acme-default-v1 (329 MB) stays at Boat and bills every month; add the key again and remove Boat to delete it.",
    );
  });

  it("counts no machine for a sub-thread still waiting for one", () => {
    expect(removeSentence(ascii, { workspaces: [{ name: "fix", state: "Running", threads: 1 }, { name: "cap", state: "Waiting", threads: 1 }] }, MAC)).toBe(
      "Its machine is deleted at Boat and the key is forgotten on zingzy's MacBook Pro. The 2 sub-threads stay with their transcripts, and a message to one asks for Boat again.",
    );
  });

  it("says another cloud's workspaces are deleted there and its key forgotten here, its threads kept", () => {
    const ascii = { ...SOLARI };
    expect(removeSentence(ascii, { workspaces: [{ name: "api", state: "Running", threads: 3 }, { name: "web", state: "Running", threads: 2 }] }, MAC)).toBe(
      "Its 2 tasks are deleted at Solari and the key is forgotten on zingzy's MacBook Pro. The 5 threads stay with their transcripts.",
    );
    expect(removeSentence(ascii, { workspaces: [{ name: "api", state: "Running", threads: 1 }] }, MAC)).toBe("Its task is deleted at Solari and the key is forgotten on zingzy's MacBook Pro. Its thread stays with its transcript.");
  });

  it("says nothing comes off an offline computer joined with a code, and hands over the line to run there", () => {
    // The remove sweeps over the link or over the ssh login (places/manage.ts); with neither, nothing sweeps it later.
    expect(removeSentence({ ...laptop, road: {} }, NOTHING_HELD, MAC)).toBe("old-macbook is offline, so nothing comes off it: run wsp leave on that computer once it is back.");
  });

  it("says nothing of an ssh login or a leave for a cloud that is not answering", () => {
    expect(removeSentence({ ...ascii, present: false }, NOTHING_HELD, MAC)).toBe("The key for Boat is forgotten on zingzy's MacBook Pro.");
  });

  it("says an offline computer added over ssh is swept over that login", () => {
    expect(removeSentence({ ...laptop, road: { ssh: "root@203.0.113.7" } }, NOTHING_HELD, MAC)).toBe(
      "wsp comes off old-macbook, which keeps the copy of your image and is otherwise left as it is. It is offline, so wsp logs in as root@203.0.113.7 to take itself off; if that fails too, run wsp leave on that computer.",
    );
  });
});

describe("the copy of the image a computer holds", () => {
  const copy = (place: string, version: number): SealedImageCopy => ({ place, version, snapshotId: `snap_${place}`, builtAt: ago(60_000) });

  it("reads a copy by the word it names its place with, the id or the name alike", () => {
    expect(copyOn([copy("p_1", 2)], hetzner)?.version).toBe(2);
    expect(copyOn([copy("hetzner", 2)], hetzner)?.version).toBe(2);
    expect(copyOn([copy("old-macbook", 2)], hetzner)).toBeUndefined();
  });
});

describe("which row a workspace stands on", () => {
  const on = (id: string, kind: WorkspaceView["kind"], machineId: string, place?: string): WorkspaceView => ({
    id,
    name: id,
    kind,
    machineId,
    phase: "running",
    golden: "",
    createdAt: ago(0),
    project: { id: "pr_1", name: "api", path: "/root/api", computer: place ?? "default" },
    ...(place === undefined ? {} : { place }),
  });

  it("puts a fork on a joined computer on that computer's row", () => {
    expect(placeOf([here, hetzner, ascii], on("ws_a", "cloud", "ctr_1", "p_1"))?.id).toBe("p_1");
  });

  it("puts what runs here on the first row, and a fork at the provider", () => {
    expect(placeOf([here, hetzner, ascii], on("ws_b", "local", "local"))?.id).toBe("here");
    expect(placeOf([here, hetzner, ascii], on("ws_c", "cloud", "fk_1"))?.id).toBe("box");
  });

  it("places nothing where the list holds no row for it", () => {
    expect(placeOf([here], on("ws_d", "cloud", "fk_1"))).toBeUndefined();
    expect(placeOf([here, ascii], on("ws_e", "cloud", "ctr_1", "p_gone"))).toBeUndefined();
  });

});

describe("the state cell of a list row", () => {
  const ready = { kind: "word", word: "Ready", mark: "ready" };
  it("says a computer behind this wsp's daemon as Update where the client can ask for one, else as Behind, the protocol's word on the hover", () => {
    const behind = { ...hetzner, daemonVersion: DAEMON_VERSION - 5 };
    expect(placeStateCell(behind, null, { canUpdate: true })).toEqual({ kind: "update", why: placeDaemonBehind(behind) });
    expect(placeStateCell(behind, null, { canUpdate: false })).toEqual({ kind: "word", word: "Behind", why: `daemon ${DAEMON_VERSION - 5}, host ${DAEMON_VERSION}` });
  });

  it("says Behind for a change to the computer's recipe on its way, with placeWord's sentence on the hover, before a daemon behind", () => {
    const sync: PlaceSync = { state: "behind", changes: ["ripgrep", "uv"], since: "x" };
    expect(placeWord({ ...hetzner, sync }, null)).toEqual({ word: "Behind", sentence: "waiting to put on the recipe's 2 changes: ripgrep, uv" });
    expect(placeStateCell({ ...hetzner, sync }, null, { canUpdate: true })).toEqual({ kind: "word", word: "Behind", why: "waiting to put on the recipe's 2 changes: ripgrep, uv" });
    expect(placeStateCell({ ...hetzner, sync, daemonVersion: DAEMON_VERSION - 5 }, null, { canUpdate: true })).toEqual({ kind: "word", word: "Behind", why: "waiting to put on the recipe's 2 changes: ripgrep, uv" });
    expect(placeStateCell({ ...hetzner, sync: { ...sync, state: "running" } }, null, { canUpdate: true })).toEqual({ kind: "word", word: "Behind", why: "putting on the recipe's 2 changes: ripgrep, uv", mark: "working" });
  });

  it("says a computer that is not answering first, since nothing can be put on a computer that is off", () => {
    const away = absentComputer("hetzner", 32 * 60 * 1000);
    expect(placeStateCell({ ...hetzner, daemonVersion: DAEMON_VERSION - 5, present: false }, away, { canUpdate: true })).toEqual({ kind: "word", word: "No answer", why: away.sentence, mark: "offline" });
  });

  it("says a computer that cannot run workspaces is Blocked first, its reason on the hover, before not answering and before behind", () => {
    const blocked = { ...hetzner, daemonVersion: DAEMON_VERSION - 5, blocked: "hetzner cannot run wsp workspaces: it mounts cgroup v1 at /sys/fs/cgroup" };
    expect(placeStateCell(blocked, null, { canUpdate: true })).toEqual({ kind: "word", word: "Blocked", why: blocked.blocked, mark: "failed" });
    expect(placeStateCell({ ...blocked, present: false }, absentComputer("hetzner", 32 * 60 * 1000), { canUpdate: true })).toEqual({ kind: "word", word: "Blocked", why: blocked.blocked, mark: "failed" });
  });

  it("offers Sign in where an agent there needs one, naming it, and reads Ready otherwise", () => {
    expect(placeStateCell({ ...hetzner, signIns: { claude: "signed-in", codex: "none" } }, null, { canUpdate: true })).toEqual({ kind: "sign-in", why: "needs a sign-in: Codex" });
    expect(placeStateCell({ ...hetzner, daemonVersion: DAEMON_VERSION }, null, { canUpdate: true })).toEqual(ready);
    expect(placeStateCell(hetzner, null, { canUpdate: false })).toEqual(ready);
    expect(placeStateCell(ascii, null, { canUpdate: false })).toEqual(ready);
  });

  it("reads the setup by the one rule: failed and waiting on the person before the silence, setting up after it", () => {
    const running: PlaceSetup = { state: "running", addId: "a_1", startedAt: "x", steps: [{ step: "clis", state: "running" }], waiting: [] };
    const failed: PlaceSetup = { ...running, state: "failed", steps: [], said: "the base tools did not install: curl" };
    const waiting: PlaceSetup = { ...running, state: "done", steps: [], waiting: [{ row: "signins/codex", label: "Codex", expiresAt: "x", state: "waiting" }] };
    expect(placeStateCell({ ...hetzner, setup: running, daemonVersion: 1 }, null, { canUpdate: true })).toEqual({ kind: "word", word: "Setting up", why: "setting up the CLIs", mark: "working" });
    expect(placeStateCell({ ...hetzner, setup: failed }, null, { canUpdate: true })).toEqual({ kind: "word", word: "Setup failed", why: "the base tools did not install: curl", mark: "failed" });
    expect(placeStateCell({ ...laptop, setup: waiting }, absentComputer("old-macbook", null), { canUpdate: true })).toEqual({ kind: "word", word: "Needs you", why: "Codex waits on you to sign in", mark: "needs-you" });
    expect(placeStateCell({ ...laptop, setup: running }, absentComputer("old-macbook", null), { canUpdate: true })).toMatchObject({ word: "No answer" });
  });
});

describe("the word for a row's kind", () => {
  it("names a cloud row cloud where a row's kind is read in a sentence, and a computer of the person's own by what it is", () => {
    expect(PLACE_KIND_WORDS.provider).toBe("cloud");
    expect(PLACE_KIND_WORDS.computer).toBe(JOINED_COMPUTER);
  });

  it("shows a present row's own note, which is where a tool answered from outside the directories its road links into", () => {
    const note = "node answers from /usr/bin/node, outside where its own installer puts it (/usr/local/bin)";
    expect(outcomeWord({ id: "tools/brew/node", label: "node", outcome: "present", note })).toBe(`already there: ${note}`);
    // A present row with nothing to add is the word alone, and an installed row's note is the road's own: the row
    // already says it installed, so the landing's "already on the machine" never reaches a line here.
    expect(outcomeWord({ id: "tools/brew/gh", label: "gh", outcome: "present" })).toBe("already there");
    expect(outcomeWord({ id: "tools/brew/gh", label: "gh", outcome: "installed", note: "already on the machine" })).toBe("installed");
  });

});
