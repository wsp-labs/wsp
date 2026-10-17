// SPDX-License-Identifier: AGPL-3.0-only
// Called by fixture.sh as `node fixture.mjs <wsp root> <out.json>` once the bench's host serves, with HOME the
// fixture's folder: adds what the Settings pages draw beyond the bench's one project, through wsp lines run in this
// process and ops on the host's socket, and writes the ids the shots name into out.json beside the bench's.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { RECIPE, hostOf, ops, pair } from "./host.mjs";

const [root, out] = process.argv.slice(2);
const host = hostOf(out);
const dir = dirname(dirname(host.state));
if (process.env.HOME !== dir) {
  console.error("run fixture.sh <out.json>, which clears the environment first");
  process.exit(2);
}
const { cli } = await import(pathToFileURL(join(root, "packages/host/dist/index.js")).href);

async function wsp(...args) {
  const said = [];
  const asked = question => Promise.reject(new Error(`wsp ${args[0]} asked: ${question}`));
  const io = { log: line => void said.push(line), error: line => void said.push(line), ask: asked, askSecret: asked };
  const code = await cli([...args, "--state", host.state], io, undefined, process.env, false, { cwd: dir });
  if (code !== 0) throw new Error(`wsp ${args.join(" ")} failed:\n${said.join("\n")}`);
  return said.join("\n");
}
const op = async frame => (await ops(host.url, host.token, [frame]))[0];

// A second project with no threads, so its page offers Remove, and an origin, so its page shows the remote row.
const notes = join(dir, "notes");
mkdirSync(notes);
const git = (...args) => spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: notes, stdio: "ignore" });
git("init", "-q", "-b", "main");
git("commit", "-q", "--allow-empty", "-m", "notes");
git("remote", "add", "origin", "https://github.com/wsp-labs/notes.git");
await wsp("add", notes, "--name", "notes");

// A saved recipe for the Recipes list and its page.
await op({ op: "recipes.save", name: RECIPE.name, file: RECIPE });

// A tool server on claude, for the Servers tab and its page; wsp's own server and skill, for Skills.
await wsp("servers", "add", "docs", "--agent", "claude", "--command", "npx -y @example/docs-mcp");
// Run as wsp's own command, since the server it writes names the command line that ran it.
const installed = spawnSync(process.execPath, [join(root, "packages/host/dist/bin.js"), "mcp", "install", "--agent", "claude", "--state", host.state], { cwd: dir, encoding: "utf8" });
if (installed.status !== 0) throw new Error(`wsp mcp install failed:\n${installed.stdout}${installed.stderr}`);

// One paired device for the Devices list and its revoke dialog.
await pair(host.url, host.token, "Phone");

const projects = Object.fromEntries((await op({ op: "projects.list" })).projects.map(p => [p.name, p.id]));
const fixture = JSON.parse(readFileSync(out, "utf8"));
writeFileSync(out, `${JSON.stringify({ ...fixture, url: host.url, tokenFile: host.tokenFile, projects }, null, 2)}\n`);
console.log(`fixture: settings additions on ${host.url}: projects ${Object.keys(projects).join(", ")}, recipe Web app, server docs, device Phone`);
