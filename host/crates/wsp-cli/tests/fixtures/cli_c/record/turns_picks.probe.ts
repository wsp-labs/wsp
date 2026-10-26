// turns_picks: turns-help.ts lines 1-237 and 421-519 at 65f6b89c7: what a start carries. Each case calls one helper of the TS
// against a scripted host and writes down every frame sent, what it returned and what it threw.
//
// Files a case needs are written into its folder before the call: `files` as text (the recorder's own), and from this
// probe's own `args.filesBase64` (name to base64: the image heads are not text) and `args.sizes` (name to a length the file
// is cut or grown to, sparse: the caps are 10 MiB). `args.summary` answers the attachments as { mediaType, name, length },
// since a file at its cap is 14 MB of base64. The Rust drivers do the same from the same arguments.
import { it } from "vitest";
import { closeSync, mkdirSync, openSync, truncateSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { record, type Case, type Obj, type Run } from "./lib.ts";
import { BOTH, CLAUDE_ONLY, CODEX_DEFAULT, FAST_CLAUDE, NONE, NO_ACCESS, NO_DEFAULT, OPEN_VALUES, PREFERENCES, PREFERENCES_NONE, TABLE_CLAUDE, agentIs, defaultsOf, frame, project, refused, session, sessions, thread, workspace, wsOne } from "./turns_views.ts";

process.env["HOME"] = "/root";

const PNG = "iVBORw0KGgo=";
const JPEG = "/9j/";
const hex = (s: string): string => Buffer.from(s.replace(/ /g, ""), "hex").toString("base64");
const MIB = 1024 * 1024;

const W1 = workspace("ws-1", "attic-work", "cloud", project("p-1", "wsp", "here", "/root/wsp"));
const THREADS = sessions(
  session("s-1", "ws-1", "thread-7f3a9c21e4b5", "claude", { claudeSessionId: "cs-1", prompt: "fix the build" }),
  session("s-2", "ws-1", "thread-7f3a9c21e4b5", "claude", { claudeSessionId: "cs-1" }),
  session("s-3", "ws-2", "thread-7f3b00000000", "codex"),
  session("s-4", "ws-2", "thread-aaaa11112222", "codex"),
  session("s-5", "ws-1", undefined, "claude"),
);
const GOT = wsOne(W1);
const forkScript = { replies: { "sessions.list": THREADS, "workspaces.get": GOT, "harnesses.list": BOTH } };

const cases: Case[] = [];
const add = (c: Case): void => void cases.push(c);

// ---- picksOf
const picksOf = (case_: string, picks: Obj): void => add({ fn: "picksOf", case: case_, args: { picks } });
picksOf("nothing named", {});
picksOf("a model", { model: "claude-opus-5-5" });
picksOf("an effort", { effort: "high" });
picksOf("an empty model is a model", { model: "" });
picksOf("an empty effort is an effort", { effort: "" });
for (const access of ["ask", "auto-edit", "full", "plan"]) picksOf(`access ${access}`, { access });
picksOf("fast", { fast: true });
picksOf("fast false is left out", { fast: false });
picksOf("all four, in the order model effort access fast", { fast: true, access: "full", effort: "max", model: "m" });
picksOf("an access that is no word of wsp's", { access: "bogus" });
picksOf("an access in the harness's own spelling", { access: "bypassPermissions" });
picksOf("an empty access", { access: "" });
picksOf("an access in capitals", { access: "FULL" });
picksOf("a bad access beside a good model", { model: "m", access: "yolo" });

// ---- pickFlags
const flags = (case_: string, f: Obj): void => add({ fn: "pickFlags", case: case_, args: { flags: f } });
flags("no flags", {});
flags("model effort access", { model: "m", effort: "e", access: "full" });
flags("typed in the other order, read in the table's", { access: "full", effort: "e", model: "m" });
flags("an empty model is kept", { model: "" });
flags("a model flag given as a switch is no value", { model: true });
flags("a model flag given as a list is no value", { model: ["a", "b"] });
flags("fast", { fast: true });
flags("fast typed first still comes last", { fast: true, model: "m" });
flags("fast false is dropped", { fast: false });
flags("fast as a word is dropped", { fast: "true" });
flags("fast as a list is dropped", { fast: ["x"] });
flags("other flags are not picks", { agent: "codex", json: true, detach: true });
flags("everything and an unrelated flag", { json: true, model: "m", effort: "e", access: "plan", fast: true });

// ---- filesFrom
const files = (case_: string, args: Obj, extra: Partial<Case> = {}): void => add({ fn: "filesFrom", case: case_, args, ...extra });
files("none", { paths: [] });
files("none, elsewhere", { paths: [], elsewhere: true });
files("one text file travels untyped under its own name", { paths: ["<cwd>/a.txt"] }, { files: { "a.txt": "hello" } });
files("two files keep the order named", { paths: ["<cwd>/b.txt", "<cwd>/a.txt"] }, { files: { "a.txt": "hello", "b.txt": "world!" } });
files("the same file twice is two attachments", { paths: ["<cwd>/a.txt", "<cwd>/a.txt"] }, { files: { "a.txt": "hello" } });
files("a name with spaces and accents", { paths: ["<cwd>/résumé 1.txt"] }, { files: { "résumé 1.txt": "cv" } });
files("a path with dot dot is the file it reaches", { paths: ["<cwd>/sub/../a.txt"] }, { files: { "a.txt": "hello", "sub/x.txt": "x" } });
files("a path with a doubled slash", { paths: ["<cwd>//a.txt"] }, { files: { "a.txt": "hello" } });
files("a one byte file", { paths: ["<cwd>/one.txt"] }, { files: { "one.txt": "x" } });
files("an empty file is refused by name", { paths: ["<cwd>/empty.txt"] }, { files: { "empty.txt": "" } });
files("a file that is not there", { paths: ["<cwd>/missing.txt"] });
files("a relative path that is not there", { paths: ["no-such-file-here.txt"] });
files("a folder is no file", { paths: ["<cwd>/sub"] }, { files: { "sub/x.txt": "x" } });
files("the case folder itself is no file", { paths: ["<cwd>"] });
files("the first missing file is the one named", { paths: ["<cwd>/a.txt", "<cwd>/gone.txt", "<cwd>/also-gone.txt"] }, { files: { "a.txt": "hello" } });
files("a missing file beside an empty one: the missing one first", { paths: ["<cwd>/empty.txt", "<cwd>/gone.txt"] }, { files: { "empty.txt": "" } });
files("elsewhere refuses before any path is touched", { paths: ["<cwd>/a.txt"], elsewhere: true }, { files: { "a.txt": "hello" } });
files("elsewhere refuses a path that is not there in the same words", { paths: ["<cwd>/missing.txt"], elsewhere: true });
files("elsewhere refuses a folder in the same words", { paths: ["<cwd>"], elsewhere: true });
files("elsewhere false reads", { paths: ["<cwd>/a.txt"], elsewhere: false }, { files: { "a.txt": "hello" } });
for (const n of [1, 4, 5]) {
  files(`${n} files fit`, { paths: Array.from({ length: n }, (_, i) => `<cwd>/f${i}.txt`), summary: true }, { files: Object.fromEntries(Array.from({ length: n }, (_, i) => [`f${i}.txt`, `f${i}`])) });
}
files("six files are one too many", { paths: Array.from({ length: 6 }, (_, i) => `<cwd>/f${i}.txt`) }, { files: Object.fromEntries(Array.from({ length: 6 }, (_, i) => [`f${i}.txt`, `f${i}`])) });
files("seven files say seven", { paths: Array.from({ length: 7 }, (_, i) => `<cwd>/f${i}.txt`) }, { files: Object.fromEntries(Array.from({ length: 7 }, (_, i) => [`f${i}.txt`, `f${i}`])) });
files("six files with one empty: the count is the first thing wrong", { paths: Array.from({ length: 6 }, (_, i) => `<cwd>/f${i}.txt`) }, { files: Object.fromEntries(Array.from({ length: 6 }, (_, i) => [`f${i}.txt`, i === 2 ? "" : `f${i}`])) });
files("six paths with one missing: the missing file is refused before the count", { paths: Array.from({ length: 6 }, (_, i) => `<cwd>/f${i}.txt`) }, { files: Object.fromEntries(Array.from({ length: 5 }, (_, i) => [`f${i}.txt`, `f${i}`])) });
files("two empty files: the first is the one named", { paths: ["<cwd>/e2.txt", "<cwd>/e1.txt"] }, { files: { "e1.txt": "", "e2.txt": "" } });
files("a file at the cap of 10 MiB travels", { paths: ["<cwd>/big.bin"], sizes: { "big.bin": 10 * MIB }, summary: true }, { files: { "big.bin": "x" } });
files("a file one byte under the cap travels", { paths: ["<cwd>/big.bin"], sizes: { "big.bin": 10 * MIB - 1 }, summary: true }, { files: { "big.bin": "x" } });
files("a file one byte over the cap is refused with its weight", { paths: ["<cwd>/big.bin"], sizes: { "big.bin": 10 * MIB + 1 } }, { files: { "big.bin": "x" } });
files("a file far over the cap", { paths: ["<cwd>/video.mov"], sizes: { "video.mov": 300 * MIB } }, { files: { "video.mov": "x" } });
files("a file of 1.5 MiB over the cap", { paths: ["<cwd>/big.bin"], sizes: { "big.bin": 10 * MIB + MIB / 2 } }, { files: { "big.bin": "x" } });
files("the over-cap file is refused before the empty one behind it", { paths: ["<cwd>/big.bin", "<cwd>/empty.txt"], sizes: { "big.bin": 10 * MIB + 1 } }, { files: { "big.bin": "x", "empty.txt": "" } });
files("the empty file is refused before the over-cap one behind it", { paths: ["<cwd>/empty.txt", "<cwd>/big.bin"], sizes: { "big.bin": 10 * MIB + 1 } }, { files: { "big.bin": "x", "empty.txt": "" } });
files("a png travels as an image whatever it is called", { paths: ["<cwd>/shot.txt"], filesBase64: { "shot.txt": PNG } });
files("a png that is eight bytes and no more", { paths: ["<cwd>/shot.png"], filesBase64: { "shot.png": PNG } });
files("a png with a body", { paths: ["<cwd>/shot.png"], filesBase64: { "shot.png": hex("89504e470d0a1a0a 0000000d 49484452 00000001") } });
files("a png cut at seven bytes is a plain file", { paths: ["<cwd>/shot.png"], filesBase64: { "shot.png": hex("89504e470d0a1a") } });
files("a png whose eighth byte is wrong is a plain file", { paths: ["<cwd>/shot.png"], filesBase64: { "shot.png": hex("89504e470d0a1a0b 00") } });
files("a jpeg of three bytes", { paths: ["<cwd>/p.jpg"], filesBase64: { "p.jpg": JPEG } });
files("a jpeg with a body", { paths: ["<cwd>/p.jpg"], filesBase64: { "p.jpg": hex("ffd8ffe000104a46") } });
files("a jpeg cut at two bytes is a plain file", { paths: ["<cwd>/p.jpg"], filesBase64: { "p.jpg": hex("ffd8") } });
files("a gif is told by its first four bytes, as text too", { paths: ["<cwd>/fake.txt"] }, { files: { "fake.txt": "GIF89a..." } });
files("a gif of four bytes", { paths: ["<cwd>/g.gif"] }, { files: { "g.gif": "GIF8" } });
files("a gif cut at three bytes is a plain file", { paths: ["<cwd>/g.gif"] }, { files: { "g.gif": "GIF" } });
files("a lowercase gif word is a plain file", { paths: ["<cwd>/g.gif"] }, { files: { "g.gif": "gif8" } });
files("a webp is told by its twelve byte head", { paths: ["<cwd>/w.webp"] }, { files: { "w.webp": "RIFF\u0000\u0000\u0000\u0000WEBP" } });
files("a webp with a body", { paths: ["<cwd>/w.webp"] }, { files: { "w.webp": "RIFFabcdWEBPVP8 data" } });
files("a webp cut at eleven bytes is a plain file", { paths: ["<cwd>/w.webp"] }, { files: { "w.webp": "RIFFabcdWEB" } });
files("a riff file that is not webp is a plain file", { paths: ["<cwd>/w.wav"] }, { files: { "w.wav": "RIFFabcdWAVEfmt " } });
files("riff alone is a plain file", { paths: ["<cwd>/w.bin"] }, { files: { "w.bin": "RIFF" } });
files("an image and a file keep the order named", { paths: ["<cwd>/a.txt", "<cwd>/shot.png", "<cwd>/p.jpg"], filesBase64: { "shot.png": PNG, "p.jpg": JPEG } }, { files: { "a.txt": "hello" } });
files("an image at the cap travels", { paths: ["<cwd>/shot.png"], filesBase64: { "shot.png": PNG }, sizes: { "shot.png": 10 * MIB }, summary: true });
files("an image one byte over the cap is refused as an image", { paths: ["<cwd>/shot.png"], filesBase64: { "shot.png": PNG }, sizes: { "shot.png": 10 * MIB + 1 } });
files("a file named like an image but over the cap is refused as a file", { paths: ["<cwd>/shot.png"], sizes: { "shot.png": 10 * MIB + 1 } }, { files: { "shot.png": "not a png" } });
files("an empty png name is an empty file", { paths: ["<cwd>/shot.png"] }, { files: { "shot.png": "" } });

// ---- checkedStart
const start = (case_: string, args: Obj, script: Obj = {}, extra: Partial<Case> = {}): void => add({ fn: "checkedStart", case: case_, args, script, ...extra });
const lists = (h = BOTH): Obj => ({ replies: { "harnesses.list": h } });
const withDefaults = (map: Obj, h = BOTH): Obj => ({ replies: { "harnesses.list": h, "projects.defaults": defaultsOf(map) } });

start("an empty task is refused before the host is asked", { task: "" });
start("a task of spaces and newlines is empty", { task: "  \n\t " }, lists());
start("an empty task on a workspace asks nothing either", { task: "", workspaceId: "ws-1", picks: { model: "x" } }, lists());
start("a task of one letter", { task: "a" }, lists());
start("on this computer the picks are left to the start", { task: "go", picks: { model: "no-such-model", effort: "no-such", access: "full", fast: true } }, lists());
start("on this computer even a bad access word is left to the start", { task: "go", picks: { access: "bogus" } }, lists());
start("on this computer a named agent in the list passes", { task: "go", harness: "codex", picks: { model: "zzz" } }, lists());
start("on this computer a named agent not in the list is refused with the list", { task: "go", harness: "gemini" }, lists());
start("a named agent beside none at all says none", { task: "go", harness: "gemini" }, lists(NONE));
start("a named agent beside one", { task: "go", harness: "gemini" }, lists(CLAUDE_ONLY));
start("an agent name is exact", { task: "go", harness: "Claude" }, lists());
start("an empty agent name names no agent", { task: "go", harness: "" }, lists());
start("no agent named takes the table marked default and passes", { task: "go" }, lists());
start("no agents at all passes on this computer", { task: "go" }, lists(NONE));
start("the host refusing the lists is the answer", { task: "go" }, { replies: { "harnesses.list": refused("harnesses.list is not a thread's to ask for", "usage") } });
start("a workspace's own lists are asked for by its id", { task: "go", workspaceId: "ws-1" }, lists());
start("a workspace and a good model, effort and access", { task: "go", workspaceId: "ws-1", picks: { model: "claude-opus-5-5", effort: "max", access: "full" } }, lists());
start("a workspace and a model the list lacks", { task: "go", workspaceId: "ws-1", picks: { model: "no-such-model" } }, lists());
start("a workspace and an effort the list lacks", { task: "go", workspaceId: "ws-1", picks: { effort: "no-such-effort" } }, lists());
start("an effort the named model does not take", { task: "go", workspaceId: "ws-1", picks: { model: "claude-opus-5-5", effort: "low" } }, lists());
start("an effort of a model that takes none", { task: "go", workspaceId: "ws-1", picks: { model: "claude-haiku", effort: "high" } }, lists());
start("fast on a model with no fast mode", { task: "go", workspaceId: "ws-1", picks: { model: "claude-sonnet-5", fast: true } }, lists());
start("fast on a model with a fast mode", { task: "go", workspaceId: "ws-1", picks: { model: "claude-sonnet-5", fast: true } }, lists(FAST_CLAUDE));
start("fast with no model is read against the default model", { task: "go", workspaceId: "ws-1", picks: { fast: true } }, lists(FAST_CLAUDE));
start("an access the agent maps", { task: "go", workspaceId: "ws-1", picks: { access: "auto-edit" } }, lists());
start("an access the agent maps to no mode", { task: "go", workspaceId: "ws-1", picks: { access: "plan" } }, lists());
start("an access the agent maps to no mode, ask", { task: "go", workspaceId: "ws-1", picks: { access: "ask" } }, lists());
start("an agent that maps no access at all", { task: "go", workspaceId: "ws-1", picks: { access: "full" } }, lists(NO_ACCESS));
start("a word that is no access word is refused before the lists are read against", { task: "go", workspaceId: "ws-1", picks: { access: "bogus" } }, lists());
start("a bad access word beside a bad model: the access word first", { task: "go", workspaceId: "ws-1", picks: { model: "no-such-model", access: "bogus" } }, lists());
start("a bad model beside a bad access mode: the model first", { task: "go", workspaceId: "ws-1", picks: { model: "no-such-model", access: "plan" } }, lists());
start("lists that name no models take any model", { task: "go", workspaceId: "ws-1", picks: { model: "anything", effort: "anything" } }, lists(OPEN_VALUES));
start("a table row says whose list it quotes", { task: "go", workspaceId: "ws-1", picks: { model: "no-such-model" } }, lists(TABLE_CLAUDE));
start("a table row's model that takes no effort says the table said so", { task: "go", workspaceId: "ws-1", picks: { model: "claude-haiku", effort: "high" } }, lists(TABLE_CLAUDE));
start("a table row's effort off the list", { task: "go", workspaceId: "ws-1", picks: { effort: "nope" } }, lists(TABLE_CLAUDE));
start("a workspace with no agent named reads the default's table", { task: "go", workspaceId: "ws-1", picks: { model: "gpt-5" } }, lists(CODEX_DEFAULT));
start("a workspace with no row marked default reads no table: every pick passes", { task: "go", workspaceId: "ws-1", picks: { model: "anything", access: "plan", fast: true } }, lists(NO_DEFAULT));
start("a workspace with no agents reads no table", { task: "go", workspaceId: "ws-1", picks: { model: "anything" } }, lists(NONE));
start("a workspace and a named agent reads that agent's table", { task: "go", harness: "codex", workspaceId: "ws-1", picks: { model: "gpt-5" } }, lists());
start("a workspace and a named agent: its model not the default's", { task: "go", harness: "codex", workspaceId: "ws-1", picks: { model: "claude-opus-5-5" } }, lists());
start("an agent off on the workspace's computer is the start's to refuse when the host runs it", { task: "go", harness: "codex", workspaceId: "ws-1", picks: { model: "zzz" } }, {
  replies: { "harnesses.list #1": frame({ harnesses: [] }), "harnesses.list #2": BOTH },
});
start("an agent no list names, with the host's list read again", { task: "go", harness: "gemini", workspaceId: "ws-1" }, {
  replies: { "harnesses.list #1": CLAUDE_ONLY, "harnesses.list #2": BOTH },
});
start("an agent named only by the workspace's lists is no adapter when the host lacks it", { task: "go", harness: "codex", workspaceId: "ws-1" }, {
  replies: { "harnesses.list #1": CLAUDE_ONLY, "harnesses.list #2": CLAUDE_ONLY },
});
start("an agent the host has none of says none", { task: "go", harness: "codex", workspaceId: "ws-1" }, { replies: { "harnesses.list #1": NONE, "harnesses.list #2": NONE } });
start("the second read refused is the answer", { task: "go", harness: "codex", workspaceId: "ws-1" }, {
  replies: { "harnesses.list #1": NONE, "harnesses.list #2": refused("no harness list", "provider") },
});
start("a named agent the workspace's table has passes picks checked against it", { task: "go", harness: "claude", workspaceId: "ws-1", picks: { effort: "max" } }, lists());
start("a project's fork with no agent named reads the project's own agent", { task: "go", forkOf: "p-2", picks: { model: "gpt-5" } }, withDefaults({ "p-2": agentIs("codex") }));
start("a project's fork with its agent's model refused", { task: "go", forkOf: "p-2", picks: { model: "claude-opus-5-5" } }, withDefaults({ "p-2": agentIs("codex") }));
start("a project's fork whose default agent is claude", { task: "go", forkOf: "p-2", picks: { model: "claude-opus-5-5", effort: "high" } }, withDefaults({ "p-2": agentIs("claude", "project") }));
start("a project's fork with no defaults for it falls to the marked default table", { task: "go", forkOf: "p-9", picks: { model: "claude-opus-5-5" } }, withDefaults({ "p-2": agentIs("codex") }));
start("a project's fork with no defaults at all", { task: "go", forkOf: "p-2", picks: { model: "nope" } }, withDefaults({}));
start("a project's fork named for an agent no list carries reads no table", { task: "go", forkOf: "p-2", picks: { model: "anything", access: "plan" } }, withDefaults({ "p-2": agentIs("gemini") }));
start("a project's fork with an agent named never reads the defaults", { task: "go", forkOf: "p-2", harness: "claude", picks: { model: "claude-opus-5-5" } }, lists());
start("a project's fork with a bad agent named", { task: "go", forkOf: "p-2", harness: "gemini" }, lists());
start("a project's fork with a bad access word", { task: "go", forkOf: "p-2", harness: "claude", picks: { access: "bogus" } }, lists());
start("a project's fork with an access the agent maps to no mode", { task: "go", forkOf: "p-2", harness: "claude", picks: { access: "plan" } }, lists());
start("a project's fork with every pick good", { task: "go", forkOf: "p-2", harness: "claude", picks: { model: "claude-opus-5-5", effort: "max", access: "full" } }, lists());
start("a project's fork with nothing to pick", { task: "go", forkOf: "p-2", harness: "claude" }, lists());
start("a project's fork and a workspace both: the lists of the workspace, the defaults of the project", { task: "go", forkOf: "p-2", workspaceId: "ws-1", picks: { model: "gpt-5" } }, withDefaults({ "p-2": agentIs("codex") }));
start("the defaults read refused is the answer", { task: "go", forkOf: "p-2" }, { replies: { "harnesses.list": BOTH, "projects.defaults": refused("projects.defaults is not a thread's to ask for", "usage") } });
start("a project's fork with the lists naming no agents", { task: "go", forkOf: "p-2", picks: { model: "x" } }, withDefaults({ "p-2": agentIs("claude") }, NONE));
start("an empty task beside a fork asks nothing", { task: " ", forkOf: "p-2" }, withDefaults({ "p-2": agentIs("claude") }));

// ---- projectDefaultsOf
const defaults = (case_: string, script: Obj): void => add({ fn: "projectDefaultsOf", case: case_, script });
defaults("none", { replies: { "projects.defaults": defaultsOf({}) } });
defaults("one project, the agent alone", { replies: { "projects.defaults": defaultsOf({ "p-1": agentIs("claude") }) } });
defaults("the order of the projects is the host's", { replies: { "projects.defaults": defaultsOf({ "p-9": agentIs("codex"), "p-1": agentIs("claude", "project"), "p-5": agentIs("claude", "named") }) } });
defaults("every field, written in the host's own order, comes out in the schema's", {
  replies: {
    "projects.defaults": defaultsOf({
      "p-1": { access: { from: "default", mode: "bypassPermissions", value: "full" }, effort: { from: "catalog", value: "high" }, model: { from: "project", value: "claude-opus-5-5" }, agent: { from: "named", value: "claude" } },
    }),
  },
});
defaults("each place a pick comes from", {
  replies: { "projects.defaults": defaultsOf({ "p-1": { agent: { value: "a", from: "named" }, model: { value: "m", from: "project" }, effort: { value: "e", from: "default" }, access: { value: "plan", mode: "plan", from: "catalog" } } }) },
});
defaults("a key the schema does not know is dropped", { replies: { "projects.defaults": defaultsOf({ "p-1": { agent: { value: "claude", from: "default", extra: 1 }, other: true } }) } });
defaults("the host refusing", { replies: { "projects.defaults": refused("projects.defaults is not a thread's to ask for", "usage") } });
defaults("an op the host has no record of", {});

// ---- preferencesOf
const prefs = (case_: string, script: Obj): void => add({ fn: "preferencesOf", case: case_, script });
prefs("with the agents' lists shaped", { replies: { "preferences.get": PREFERENCES } });
prefs("with none standing", { replies: { "preferences.get": PREFERENCES_NONE } });
prefs("the host refusing", { replies: { "preferences.get": refused("preferences.get is not a thread's to ask for", "usage") } });
prefs("an op the host has no record of", {});

// ---- worktreeFor
const tree = (case_: string, args: Obj, script: Obj): void => add({ fn: "worktreeFor", case: case_, args, script });
tree("made", { project: "proj-1", branch: "feat/x" }, { replies: { "worktree.make": '{"id":1,"ok":true,"made":true,"path":"/Users/me/.wsp/worktrees/proj-1/feat-x-","extra":1,"branch":"feat/x"}' } });
tree("held elsewhere already", { project: "proj-1", branch: "main" }, { replies: { "worktree.make": '{"id":1,"ok":true,"path":"/Users/me/alpha","branch":"main","made":false}' } });
tree("the schema's order, whatever the host's", { project: "p", branch: "b" }, { replies: { "worktree.make": frame({ made: true, branch: "b", path: "/w/b" }) } });
tree("a branch named with a space and an accent", { project: "p", branch: "feat/é x" }, { replies: { "worktree.make": frame({ path: "/w/é", branch: "feat/é x", made: true }) } });
tree("an empty branch is sent as it is", { project: "p", branch: "" }, { replies: { "worktree.make": frame({ path: "/w", branch: "", made: false }) } });
tree("refused", { project: "proj-1", branch: "-x" }, { replies: { "worktree.make": refused("-x is not a name git takes for a branch", "usage") } });
tree("a project the host does not know", { project: "nope", branch: "x" }, { replies: { "worktree.make": refused('no project "nope"; this host holds wsp, site', "not-found") } });

// ---- openingOf
const O = (case_: string, args: Obj, extra: Partial<Case> = {}): void => add({ fn: "openingOf", case: case_, args, ...extra });
const ws = { id: "ws-1" };
const here = (h: Obj = {}): Obj => ({ here: h });
const ALL = { harness: "codex", cwd: "/work/lab", notify: ["me", "thread-7f3a9c21e4b5"], title: "the title", replaces: "thread-aaaa11112222", files: ["<cwd>/a.txt"], model: "m", effort: "e", access: "full", fast: true };
O("a workspace, nothing else", { at: ws, prompt: "build it", opts: {} });
O("a workspace with no options argument", { at: ws, prompt: "build it" });
O("a workspace and an empty prompt is sent as it is", { at: ws, prompt: "", opts: {} });
O("a workspace with a prompt of newlines and quotes", { at: ws, prompt: 'say "hi"\nthen\tgo', opts: {} });
O("a workspace with every option, in the frame's order", { at: ws, prompt: "build it", opts: ALL }, { files: { "a.txt": "hello" } });
O("a workspace with every option and a turn token", { at: ws, prompt: "build it", opts: ALL }, { env: { WSP_TURN: "tok-1" }, files: { "a.txt": "hello" } });
O("a workspace, the turn token alone", { at: ws, prompt: "go", opts: {} }, { env: { WSP_TURN: "tok-1" } });
O("an empty turn token is no token", { at: ws, prompt: "go", opts: {} }, { env: { WSP_TURN: "" } });
O("the host token is not the turn token", { at: ws, prompt: "go", opts: {} }, { env: { WSP_HOST_TOKEN: "host-1" } });
O("other variables are not read", { at: ws, prompt: "go", opts: {} }, { env: { HOME: "/root", PATH: "/bin", WSP_TURN_X: "no" } });
O("a workspace and a harness", { at: ws, prompt: "go", opts: { harness: "codex" } });
O("an empty harness is a harness", { at: ws, prompt: "go", opts: { harness: "" } });
O("a workspace and a folder", { at: ws, prompt: "go", opts: { cwd: "/work/lab" } });
O("a workspace and a relative folder is refused", { at: ws, prompt: "go", opts: { cwd: "lab" } });
O("an empty folder is no absolute folder", { at: ws, prompt: "go", opts: { cwd: "" } });
O("a folder with a trailing slash is kept as it is", { at: ws, prompt: "go", opts: { cwd: "/work/lab/" } });
O("a notify list of one", { at: ws, prompt: "go", opts: { notify: ["me"] } });
O("an empty notify list is sent as an empty list", { at: ws, prompt: "go", opts: { notify: [] } });
O("a notify list keeps its order and its repeats", { at: ws, prompt: "go", opts: { notify: ["b", "me", "b"] } });
O("a title", { at: ws, prompt: "go", opts: { title: "T" } });
O("an empty title is a title", { at: ws, prompt: "go", opts: { title: "" } });
O("replaces", { at: ws, prompt: "go", opts: { replaces: "thread-aaaa11112222" } });
O("the picks alone, in the picks' order", { at: ws, prompt: "go", opts: { fast: true, access: "plan", effort: "e", model: "m" } });
O("the picks written in the other order come out in the same one", { at: ws, prompt: "go", opts: { model: "m", effort: "e", access: "plan", fast: true } });
O("an access word of the harness's own is refused", { at: ws, prompt: "go", opts: { access: "bypassPermissions" } });
O("fast false is left out", { at: ws, prompt: "go", opts: { fast: false } });
O("a file travels as an attachment, after the title and replaces, before the picks", { at: ws, prompt: "go", opts: { files: ["<cwd>/a.txt"], title: "T", replaces: "r", model: "m" } }, { files: { "a.txt": "hello" } });
O("two files", { at: ws, prompt: "go", opts: { files: ["<cwd>/b.txt", "<cwd>/a.txt"] } }, { files: { "a.txt": "hello", "b.txt": "world" } });
O("an image and a file", { at: ws, prompt: "go", opts: { files: ["<cwd>/shot.png", "<cwd>/a.txt"] }, filesBase64: { "shot.png": PNG } }, { files: { "a.txt": "hello" } });
O("no files is no attachments", { at: ws, prompt: "go", opts: { files: [] } });
O("a file that is not there", { at: ws, prompt: "go", opts: { files: ["<cwd>/missing.txt"] } });
O("elsewhere with a file is refused", { at: ws, prompt: "go", opts: { files: ["<cwd>/a.txt"], elsewhere: true } }, { files: { "a.txt": "hello" } });
O("elsewhere with no file is not", { at: ws, prompt: "go", opts: { elsewhere: true } });
O("elsewhere with an empty file list is not", { at: ws, prompt: "go", opts: { files: [], elsewhere: true } });
O("elsewhere false reads the file", { at: ws, prompt: "go", opts: { files: ["<cwd>/a.txt"], elsewhere: false } }, { files: { "a.txt": "hello" } });
O("a bad folder beside a bad file: the folder is refused first", { at: ws, prompt: "go", opts: { cwd: "lab", files: ["<cwd>/missing.txt"] } });
O("a bad folder beside a bad access: the folder first", { at: ws, prompt: "go", opts: { cwd: "lab", access: "bogus" } });
O("a bad file beside a bad access: the file first", { at: ws, prompt: "go", opts: { files: ["<cwd>/missing.txt"], access: "bogus" } });
O("a resume", { at: ws, prompt: "go", opts: { resume: { id: "conv-1" } } });
O("a resume of a copy", { at: ws, prompt: "go", opts: { resume: { id: "conv-1", copy: true } } });
O("a resume leaves the folder off", { at: ws, prompt: "go", opts: { resume: { id: "conv-1" }, cwd: "/work/lab" } });
O("a resume with a relative folder is not refused: the folder is not read", { at: ws, prompt: "go", opts: { resume: { id: "conv-1" }, cwd: "lab" } });
O("a resume after the attachments, before the picks", { at: ws, prompt: "go", opts: { resume: { id: "conv-1", copy: true }, files: ["<cwd>/a.txt"], model: "m", title: "T" } }, { files: { "a.txt": "hello" } });
O("a resume and a turn token", { at: ws, prompt: "go", opts: { resume: { id: "c" }, harness: "claude" } }, { env: { WSP_TURN: "t" } });
O("here with nothing named", { at: here(), prompt: "go", opts: {} });
O("here with a project", { at: here({ project: { id: "p-1", name: "wsp", path: "/root/wsp" } }), prompt: "go", opts: {} });
O("here with a project and a branch", { at: here({ project: { id: "p-1", name: "wsp", path: "/root/wsp" }, branch: "feat/x" }), prompt: "go", opts: {} });
O("here with a branch and no project", { at: here({ branch: "feat/x" }), prompt: "go", opts: {} });
O("here with a folder: the folder is the target's", { at: here({ cwd: "/work/lab" }), prompt: "go", opts: {} });
O("here with a folder and the options' folder: the target's wins", { at: here({ cwd: "/work/lab" }), prompt: "go", opts: { cwd: "/work/other" } });
O("here with no folder never takes the options' folder", { at: here({ project: { id: "p-1", name: "wsp", path: "/root/wsp" } }), prompt: "go", opts: { cwd: "/work/other" } });
O("here with a relative folder is refused", { at: here({ cwd: "lab" }), prompt: "go", opts: {} });
O("here with a relative folder beside a resume is not read", { at: here({ cwd: "lab" }), prompt: "go", opts: { resume: { id: "c" } } });
O("here with project, branch, folder and every option", { at: here({ project: { id: "p-1", name: "wsp", path: "/root/wsp" }, branch: "feat/x", cwd: "/work/lab" }), prompt: "go", opts: ALL }, { env: { WSP_TURN: "t" }, files: { "a.txt": "hello" } });
O("here, the project's id alone is carried", { at: here({ project: { id: "p-1", name: "ignored", path: "/ignored" } }), prompt: "go", opts: { harness: "claude" } });
O("a fork carries no target and the options' folder", { at: { fork: { id: "p-2", name: "site", computer: "pl-attic" } }, prompt: "go", opts: { cwd: "/work/lab" } });
O("a fork with nothing else", { at: { fork: { id: "p-2", name: "site", computer: "pl-attic" } }, prompt: "go", opts: {} });
O("a fork reads its files before the machine is made", { at: { fork: { id: "p-2", name: "site", computer: "pl-attic" } }, prompt: "go", opts: { files: ["<cwd>/a.txt"] } }, { files: { "a.txt": "hello" } });
O("a fork with a file that is not there", { at: { fork: { id: "p-2", name: "site", computer: "pl-attic" } }, prompt: "go", opts: { files: ["<cwd>/missing.txt"] } });
O("a fork with every option", { at: { fork: { id: "p-2", name: "site", computer: "pl-attic" } }, prompt: "go", opts: ALL }, { env: { WSP_TURN: "t" }, files: { "a.txt": "hello" } });
O("a fork with a relative folder", { at: { fork: { id: "p-2", name: "site", computer: "pl-attic" } }, prompt: "go", opts: { cwd: "x" } });
O("a thread's fork", { at: { forks: { threadId: "thread-7f3a9c21e4b5", at: 2 } }, prompt: "go", opts: {} });
O("a thread's fork with no turn counted", { at: { forks: { threadId: "thread-7f3a9c21e4b5" } }, prompt: "go", opts: {} });
O("a thread's fork onto a branch", { at: { forks: { threadId: "thread-7f3a9c21e4b5", at: 1 }, branch: "feat/y" }, prompt: "go", opts: {} });
O("a thread's fork with no turn counted onto a branch", { at: { forks: { threadId: "t" }, branch: "b" }, prompt: "go", opts: {} });
O("a thread's fork with every option", { at: { forks: { threadId: "t", at: 3 }, branch: "b" }, prompt: "go", opts: ALL }, { env: { WSP_TURN: "t" }, files: { "a.txt": "hello" } });
O("a thread's fork takes the options' folder", { at: { forks: { threadId: "t" } }, prompt: "go", opts: { cwd: "/work/lab" } });
O("a thread's fork with a relative folder", { at: { forks: { threadId: "t" } }, prompt: "go", opts: { cwd: "lab" } });

// ---- threadFork
const fork = (case_: string, args: Obj, script: Obj = forkScript, extra: Partial<Case> = {}): void => add({ fn: "threadFork", case: case_, args: { task: "go", picks: {}, ...args }, script, ...extra });
fork("by id", { ref: "thread-7f3a9c21e4b5" });
fork("by a prefix", { ref: "thread-7f3a" });
fork("by id with a turn counted as a word", { ref: "thread-7f3a9c21e4b5", at: "2" });
fork("by id with a turn counted as a number", { ref: "thread-7f3a9c21e4b5", at: 2 });
fork("the first turn", { ref: "thread-7f3a9c21e4b5", at: 1 });
fork("a branch", { ref: "thread-7f3a9c21e4b5", branch: "feat/y" });
fork("a turn and a branch", { ref: "thread-7f3a9c21e4b5", at: "3", branch: "feat/y" });
fork("an empty branch is a branch", { ref: "thread-7f3a9c21e4b5", branch: "" });
fork("the thread's agent is the one asked for", { ref: "thread-7f3b00000000" });
fork("a named agent over the thread's", { ref: "thread-7f3b00000000", harness: "claude" });
fork("a named agent the host lacks", { ref: "thread-7f3a9c21e4b5", harness: "gemini" }, { replies: { ...forkScript.replies, "harnesses.list #1": BOTH, "harnesses.list #2": BOTH } });
fork("a row with no thread id is named by its own id", { ref: "s-5" });
fork("good picks", { ref: "thread-7f3a9c21e4b5", picks: { model: "claude-opus-5-5", effort: "max", access: "full" } });
fork("a model the thread's agent does not list", { ref: "thread-7f3a9c21e4b5", picks: { model: "gpt-5" } });
fork("a bad access word", { ref: "thread-7f3a9c21e4b5", picks: { access: "bogus" } });
fork("an access the agent maps to no mode", { ref: "thread-7f3a9c21e4b5", picks: { access: "plan" } });
fork("an empty task is refused after the thread is read", { ref: "thread-7f3a9c21e4b5", task: "" });
for (const [word, at] of [["zero", "0"], ["negative", "-1"], ["a word", "abc"], ["a fraction", "1.5"], ["an empty word", ""], ["a lone minus", "-"], ["infinity", "Infinity"], ["not a number", "NaN"], ["an underscore", "1_0"], ["a unit", "2nd"]] as const) {
  fork(`a turn count that is ${word} is refused before the host is asked`, { ref: "thread-7f3a9c21e4b5", at });
}
for (const [word, at] of [["spaces", " 3 "], ["a plus sign", "+3"], ["an exponent", "1e2"], ["hex", "0x10"], ["binary", "0b11"], ["octal", "0o7"], ["a trailing point", "3."], ["a zero fraction", "3.0"], ["leading zeros", "007"]] as const) {
  fork(`a turn count with ${word}`, { ref: "thread-7f3a9c21e4b5", at });
}
fork("a turn count as the number one", { ref: "thread-7f3a9c21e4b5", at: 1 });
fork("a turn count as the number zero is refused", { ref: "thread-7f3a9c21e4b5", at: 0 });
fork("a turn count as a negative number is refused", { ref: "thread-7f3a9c21e4b5", at: -3 });
fork("a turn count as a fraction is refused", { ref: "thread-7f3a9c21e4b5", at: 2.5 });
fork("a turn count at the largest safe integer", { ref: "thread-7f3a9c21e4b5", at: 9007199254740991 });
fork("a turn count one past the largest safe integer is refused", { ref: "thread-7f3a9c21e4b5", at: 9007199254740992 });
fork("a turn count as the word of the largest safe integer", { ref: "thread-7f3a9c21e4b5", at: "9007199254740991" });
fork("a turn count as the word one past it is refused", { ref: "thread-7f3a9c21e4b5", at: "9007199254740992" });
fork("a ref naming no thread", { ref: "zzz" });
fork("a ref naming two threads", { ref: "thread-7f3" });
fork("a ref naming the empty prefix names every thread", { ref: "" });
fork("a ref that is a project reads as no thread and says so", { ref: "wsp" }, { replies: { ...forkScript.replies, "projects.list": frame({ projects: [project("p-1", "wsp", "here", "/root/wsp")] }) } });
fork("a ref that is a project, with the cloud", { ref: "wsp" }, { replies: { ...forkScript.replies, "projects.list": frame({ projects: [project("p-1", "wsp", "here", "/root/wsp")] }) } }, { cloud: true });
fork("a ref that is a machine, with the cloud", { ref: "attic-work" }, { replies: { ...forkScript.replies, "projects.list": frame({ projects: [] }), "workspaces.list": frame({ workspaces: [workspace("ws-9", "attic-work", "cloud", project("p-1", "wsp", "here", "/root/wsp"))] }) } }, { cloud: true });
fork("a ref that is a machine, with no cloud, is only no thread", { ref: "attic-work" }, { replies: { ...forkScript.replies, "projects.list": frame({ projects: [] }), "workspaces.list": frame({ workspaces: [workspace("ws-9", "attic-work", "cloud", project("p-1", "wsp", "here", "/root/wsp"))] }) } });
fork("a workspace the host answers in another version", { ref: "thread-7f3a9c21e4b5" }, { replies: { ...forkScript.replies, "workspaces.get": frame({ workspace: { id: 1 } }) } });
fork("a workspace the host refuses", { ref: "thread-7f3a9c21e4b5" }, { replies: { ...forkScript.replies, "workspaces.get": refused("that workspace is not yours to read", "auth") } });
fork("the host refusing the lists", { ref: "thread-7f3a9c21e4b5" }, { replies: { ...forkScript.replies, "harnesses.list": refused("harnesses.list failed", "provider") } });

// ---- resumeAsked
const resume = (case_: string, args: Obj): void => add({ fn: "resumeAsked", case: case_, args });
resume("no conversation, no copy", { copy: false });
resume("no conversation, no copy, where named", { copy: false, where: { branch: "feat/x", cwd: "/w", beside: "t" } });
resume("a copy with no conversation", { copy: true });
resume("a copy with no conversation beside a branch", { copy: true, where: { branch: "feat/x" } });
resume("a conversation", { id: "7414323d-e71b-4957-8b56-eefdf6bfa350", copy: false });
resume("a conversation to copy", { id: "7414323d-e71b-4957-8b56-eefdf6bfa350", copy: true });
resume("an empty conversation id is a conversation", { id: "", copy: false });
resume("a conversation and an empty where", { id: "c", copy: false, where: {} });
resume("a conversation beside a branch", { id: "c", copy: false, where: { branch: "feat/x" } });
resume("a conversation beside a folder", { id: "c", copy: false, where: { cwd: "/work/acme/lab" } });
resume("a conversation beside a thread", { id: "c", copy: false, where: { beside: "thread-7f" } });
resume("a conversation beside all three", { id: "c", copy: true, where: { beside: "t", branch: "b", cwd: "/w" } });
resume("a copy of a conversation beside a branch", { id: "c", copy: true, where: { branch: "b" } });
resume("an empty branch counts as named", { id: "c", copy: false, where: { branch: "" } });
resume("an empty folder counts as named", { id: "c", copy: false, where: { cwd: "" } });
resume("an empty thread counts as named", { id: "c", copy: false, where: { beside: "" } });

// ---- replacedOf
const replaced = (case_: string, args: Obj, script: Obj = {}): void => add({ fn: "replacedOf", case: case_, args, script });
const replaceable = { replies: { "sessions.list": THREADS, "sessions.replaceable": frame({}) } };
replaced("none named asks nothing", {});
replaced("by id", { ref: "thread-7f3a9c21e4b5" }, replaceable);
replaced("by a prefix, answered by the full id", { ref: "thread-7f3a" }, replaceable);
replaced("a thread with no thread id is named by its row", { ref: "s-5" }, replaceable);
replaced("the empty word names every thread", { ref: "" }, replaceable);
replaced("a word naming no thread", { ref: "zzz" }, replaceable);
replaced("a word naming two", { ref: "thread-7f3" }, replaceable);
replaced("a thread the host says may not be replaced", { ref: "thread-7f3a9c21e4b5" }, { replies: { "sessions.list": THREADS, "sessions.replaceable": refused("that thread is still working; stop it first", "usage") } });
replaced("the host asked of nothing it knows", { ref: "thread-7f3a9c21e4b5" }, { replies: { "sessions.list": THREADS } });

// ---- notifyOf
const notify = (case_: string, args: Obj, script: Obj = { replies: { "sessions.list": THREADS } }): void => add({ fn: "notifyOf", case: case_, args, script });
notify("none named", { refs: [] }, {});
notify("me", { refs: ["me"] }, {});
notify("me twice", { refs: ["me", "me"] }, {});
notify("a thread by id", { refs: ["thread-7f3a9c21e4b5"] });
notify("a thread by a prefix is named by its full id", { refs: ["thread-7f3a"] });
notify("me and a thread, one listing read for the thread", { refs: ["me", "thread-7f3a"] });
notify("a thread and me, in the order named", { refs: ["thread-7f3a", "me"] });
notify("two threads read the listing twice", { refs: ["thread-7f3a", "thread-aaaa"] });
notify("the same thread twice", { refs: ["thread-7f3a", "thread-7f3a"] });
notify("me between two threads", { refs: ["thread-7f3a", "me", "thread-aaaa"] });
notify("a row with no thread id is named by its row", { refs: ["s-5"] });
notify("the word me is exact: capitals name no thread", { refs: ["ME"] });
notify("a word naming no thread", { refs: ["zzz"] });
notify("a word naming two", { refs: ["thread-7f3"] });
notify("the first good ref is read before the bad one is refused", { refs: ["thread-7f3a", "zzz"] });
notify("a bad ref stops the rest", { refs: ["zzz", "thread-7f3a"] });
notify("the host refusing the listing", { refs: ["thread-7f3a"] }, { replies: { "sessions.list": refused("sessions.list failed", "provider") } });

// ---- messageTo
const msg = (case_: string, args: Obj, extra: Partial<Case> = {}): void => add({ fn: "messageTo", case: case_, args, ...extra });
const row = thread("row_1", "ws_1", { title: "hello", sessionId: "row_1" });
msg("names the thread by its runtime id", { thread: { ...row, threadId: "thr_1", claudeSessionId: "sess_1" }, prompt: "again" });
msg("a row with a thread id and no session", { thread: { ...row, threadId: "thr_1" }, prompt: "again" });
msg("a row with no thread id is named by its own id", { thread: { ...row, claudeSessionId: "sess_1" }, prompt: "again" });
msg("the thread's own agent", { thread: { ...row, harness: "codex" }, prompt: "again" });
msg("an empty prompt is sent as it is", { thread: row, prompt: "" });
msg("the picks follow", { thread: row, prompt: "again", picks: { fast: true, access: "plan", effort: "e", model: "m" } });
msg("a pick access word of the harness's own is refused", { thread: row, prompt: "again", picks: { access: "acceptEdits" } });
msg("fast false is left out", { thread: row, prompt: "again", picks: { fast: false } });
msg("a file travels before the picks", { thread: row, prompt: "again", picks: { model: "m" }, files: ["<cwd>/a.txt"] }, { files: { "a.txt": "hello" } });
msg("two files", { thread: row, prompt: "again", files: ["<cwd>/a.txt", "<cwd>/b.txt"] }, { files: { "a.txt": "hello", "b.txt": "world" } });
msg("a file that is not there", { thread: row, prompt: "again", files: ["<cwd>/missing.txt"] });
msg("elsewhere with a file is refused", { thread: row, prompt: "again", files: ["<cwd>/a.txt"], elsewhere: true }, { files: { "a.txt": "hello" } });
msg("elsewhere with no file passes", { thread: row, prompt: "again", elsewhere: true });
msg("the files are read before the picks are checked", { thread: row, prompt: "again", picks: { access: "bogus" }, files: ["<cwd>/missing.txt"] });
msg("six files", { thread: row, prompt: "again", files: Array.from({ length: 6 }, (_, i) => `<cwd>/f${i}.txt`) }, { files: Object.fromEntries(Array.from({ length: 6 }, (_, i) => [`f${i}.txt`, `f${i}`])) });
msg("takes no folder, no target and no token", { thread: row, prompt: "again" }, { env: { WSP_TURN: "tok-1" } });

// ---- the runs
type Pre = (cwd: string, args: Obj) => void;
const prepare: Pre = (cwd, args) => {
  for (const [name, b64] of Object.entries((args["filesBase64"] as Obj | undefined) ?? {})) {
    mkdirSync(join(cwd, name, ".."), { recursive: true });
    writeFileSync(join(cwd, name), Buffer.from(b64 as string, "base64"));
  }
  for (const [name, size] of Object.entries((args["sizes"] as Obj | undefined) ?? {})) {
    const path = join(cwd, name);
    mkdirSync(join(path, ".."), { recursive: true });
    closeSync(openSync(path, "a"));
    truncateSync(path, size as number);
  }
};
const summary = (args: Obj, files: unknown): unknown => (args["summary"] === true ? (files as Obj[]).map(f => ({ mediaType: f["mediaType"], name: f["name"], length: (f["bytes"] as string).length })) : files);

const runs: Record<string, Run> = {
  picksOf: ({ mod, args }) => Promise.resolve((mod["turns"] as any).picksOf(args["picks"])),
  pickFlags: ({ mod, args }) => Promise.resolve((mod["turns"] as any).pickFlags(args["flags"])),
  filesFrom: async ({ mod, args, cwd }) => {
    prepare(cwd, args);
    return summary(args, (mod["turns"] as any).filesFrom(args["paths"], args["elsewhere"]));
  },
  checkedStart: async ({ mod, client, args }) => {
    await (mod["turns"] as any).checkedStart(client, args["task"], args["harness"], args["picks"] ?? {}, args["workspaceId"], args["forkOf"]);
    return null;
  },
  projectDefaultsOf: ({ mod, client }) => (mod["turns"] as any).projectDefaultsOf(client),
  preferencesOf: ({ mod, client }) => (mod["turns"] as any).preferencesOf(client),
  worktreeFor: ({ mod, client, args }) => (mod["turns"] as any).worktreeFor(client, args["project"], args["branch"]),
  openingOf: async ({ mod, c, args, cwd }) => {
    prepare(cwd, args);
    return (mod["turns"] as any).openingOf(c.env ?? {}, args["at"], args["prompt"], args["opts"]);
  },
  threadFork: ({ mod, client, args }) => (mod["turns"] as any).threadFork(client, args["ref"], args["at"], args["branch"], args["task"], args["harness"], args["picks"] ?? {}),
  resumeAsked: async ({ mod, args }) => (mod["turns"] as any).resumeAsked(args["id"], args["copy"], args["where"] ?? {}) ?? null,
  replacedOf: async ({ mod, client, args }) => (await (mod["turns"] as any).replacedOf(client, args["ref"])) ?? null,
  notifyOf: async ({ mod, client, args }) => (await (mod["turns"] as any).notifyOf(client, args["refs"])) ?? null,
  messageTo: async ({ mod, args, cwd }) => {
    prepare(cwd, args);
    return (mod["turns"] as any).messageTo(args["thread"], args["prompt"], args["picks"], args["files"], args["elsewhere"]);
  },
};

// What a refusal says of the case's own folder is written `<cwd>`, as the frames and lines are.
const scrubbed = (run: Run): Run => async ctx => {
  try {
    const value = await run(ctx);
    return value === undefined ? value : (JSON.parse(JSON.stringify(value).split(ctx.cwd).join("<cwd>")) as unknown);
  } catch (e) {
    if (e instanceof Error) {
      e.message = e.message.split(ctx.cwd).join("<cwd>");
      const fix = (e as unknown as Obj)["fix"];
      if (typeof fix === "string") (e as unknown as Obj)["fix"] = fix.split(ctx.cwd).join("<cwd>");
    }
    throw e;
  }
};

it("turns_picks", async () => {
  await record("turns_picks", cases, Object.fromEntries(Object.entries(runs).map(([k, v]) => [k, scrubbed(v)])));
});
