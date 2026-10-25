import { it } from "vitest";
import { readGhosttyConfig } from "../../../../packages/collect/src/ghostty-config.js";
import { CLAUDE_HOOKS, hookFiles } from "../../../../packages/catalog/src/index.js";
import { fakeHost } from "../../../../packages/collect/test/fake-host.js";

it("ghostty order", async () => {
  const cases: Record<string, Record<string, string>> = {
    blink: { "~/.config/ghostty/config": "cursor-style-blink = true\nfont-size = 12\ncursor-style-blink =\ncursor-style-blink = false\n" },
    themefirst: { "~/.config/ghostty/config": "font-size = 13\ntheme = dark:Mine,light:Other\nbackground = #000000\n", "~/.config/ghostty/themes/Mine": "foreground = #ffffff\nbackground = #111111\npalette = 2=#222222\n" },
    emptyfont: { "~/.config/ghostty/config": "window-padding-y = 2\nfont-family =\nbackground-opacity = 0.5\n" },
  };
  for (const [name, files] of Object.entries(cases)) console.log(name, JSON.stringify(await readGhosttyConfig(fakeHost({ files }), "dark")));
});
it("hooks throw", () => {
  const texts = ["{bad", '{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"~/bin/x.sh"}]}]}}', '{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"$HOME/bin/x.sh && \\"/Users/dev/a b.sh\\""}]}]}}', '{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"\\"unterminated"}]}]}}'];
  for (const t of texts) { try { console.log(JSON.stringify(hookFiles(CLAUDE_HOOKS, t, "/Users/dev"))); } catch (e) { console.log("THROW", String(e)); } }
});
import { aptPackages } from "../../../../packages/collect/src/detect/apt.js";
import { detectTools } from "../../../../packages/collect/src/detect/tools.js";
import { collect } from "../../../../packages/collect/src/collect.js";
it("apt throw", async () => {
  const mk = (first: string) => {
    const h = fakeHost({ platform: "linux", which: ["apt-mark"], exec: { "apt-mark showmanual": "tool\nother\n", "dpkg-query -Wf ${Package} ${Priority}\n": "tool optional\nother optional\n" }, files: { "/var/lib/dpkg/info/tool.list": "x", "/var/lib/dpkg/info/other.list": "/usr/bin/other\n" } });
    const orig = h.fs.lines.bind(h.fs);
    h.fs.lines = async function* (path: string, holding?: string) { if (path.endsWith("tool.list")) { yield first; throw new Error("EIO: i/o error, read"); } yield* orig(path, holding); };
    return h;
  };
  for (const first of ["/usr/share/doc/tool", "/usr/bin/tool"]) {
    try { console.log("APT", first, JSON.stringify(await aptPackages(mk(first)))); } catch (e) { console.log("APT", first, "THROW", String(e)); }
    try { console.log("TOOLS", first, JSON.stringify((await detectTools(mk(first))).map(r => r.id))); } catch (e) { console.log("TOOLS", first, "THROW", String(e)); }
    const h = mk(first); const seen: string[] = [];
    try { await collect(h, { onRung: (r, n) => seen.push(`${r}:${n}`) }); console.log("COLLECT ok", seen.join(",")); } catch (e) { console.log("COLLECT", first, "THROW", String(e), "rungs before:", seen.join(",")); }
  }
});
import { detectAgents } from "../../../../packages/collect/src/detect/agents.js";
it("agents bad hooks", async () => {
  console.log("AGENTS", JSON.stringify((await detectAgents(fakeHost({ files: { "~/.claude/settings.json": "{bad", "~/.claude/hooks/x.sh": 10 }, which: ["claude"] })))[0]));
});
