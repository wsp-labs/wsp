// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/detect_agents.md on the edge inputs of the wave-2 audit (group c): agents.ts and sources.ts.
// hook_scripts is not exported by the TS; its answers show in detect_agents rows. A rung that threw is `error`.
// guard_sources cases: args [text, home, present_rels]; out {text, asked}. shell_sources cases: args [rows]; out is the rows afterwards.
// collect cases: out is the manifest, on_rung the (rung, count) pairs onRung saw before any throw.
import { it } from "vitest";
import { AGENTS, agentRowId, detectAgents } from "../../../../packages/collect/src/detect/agents.js";
import { bareSources, guardSources, shellSources } from "../../../../packages/collect/src/detect/sources.js";
import { DETECTORS, collect } from "../../../../packages/collect/src/collect.js";
import { host, plain, settle, write, type Laptop } from "./w2-host.js";

const HOME = "/Users/dev";
const hc = async (name: string, laptop: Laptop, f: (h: ReturnType<typeof host>) => unknown, args: unknown[] = []) => {
  const h = host(laptop);
  return { name, laptop, args, ...(await settle(() => f(h))), calls: h.calls };
};
const col = async (name: string, laptop: Laptop) => {
  const h = host(laptop);
  const on: [string, number][] = [];
  const r = await settle(() => collect(h, { onRung: (rung, n) => on.push([rung, n]) }));
  return { name, laptop, args: [], ...r, on_rung: on, calls: h.calls };
};
const cmd = (command: string) => ({ type: "command", command });
const hooks = (...cmds: string[]) => JSON.stringify({ hooks: { Stop: [{ hooks: cmds.map(cmd) }] } });
const SET = "~/.claude/settings.json";
const dup = (a: string, b: string) => `{"hooks":{"Stop":[{"hooks":[${JSON.stringify(cmd(a))}]}]},"hooks":{"Stop":[{"hooks":[${JSON.stringify(cmd(b))}]}]}}`;

it("detect_agents fixtures", async () => {
  const da = (h: ReturnType<typeof host>) => detectAgents(h);
  const claude = (files: Record<string, string | number>, which: string[] = ["claude"]): Laptop => ({ files, which });
  const HK = "~/.claude/hooks";
  const HOOKS_A = JSON.stringify(JSON.parse(hooks("~/.claude/hooks/a")).hooks);

  const detect = [
    await hc("input A: a repeated hooks key fails the rung", claude({ [SET]: dup("~/.claude/hooks/a", "~/.claude/hooks/b"), [`${HK}/a`]: 1, [`${HK}/b`]: 2 }), da),
    await hc("a repeated event key fails the rung", claude({ [SET]: `{"hooks":{"Stop":[{"hooks":[${JSON.stringify(cmd("~/.claude/hooks/a"))}]}],"Stop":[{"hooks":[${JSON.stringify(cmd("~/.claude/hooks/b"))}]}]}}`, [`${HK}/a`]: 1, [`${HK}/b`]: 2 }), da),
    await hc("a repeated command key fails the rung", claude({ [SET]: `{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"~/.claude/hooks/a","command":"~/.claude/hooks/b"}]}]}}`, [`${HK}/a`]: 1, [`${HK}/b`]: 2 }), da),
    await hc("first hooks block holds no path, the last does: fails", claude({ [SET]: dup("/bin/ls", "~/.claude/hooks/a"), [`${HK}/a`]: 1 }), da),
    await hc("counter-case: the last hooks block holds only /bin/ls, no throw", claude({ [SET]: dup("~/.claude/hooks/a", "/bin/ls"), [`${HK}/a`]: 1 }), da),
    await hc("counter-case: a repeated model key before the hooks block", claude({ [SET]: `{"model":"a","model":"b","hooks":${HOOKS_A}}`, [`${HK}/a`]: 1 }), da),
    await hc("counter-case: a repeated model key after the hooks block", claude({ [SET]: `{"hooks":${HOOKS_A},"model":"a","model":"b"}`, [`${HK}/a`]: 1 }), da),
    await hc("counter-case: not JSON", claude({ [SET]: "not json {", [`${HK}/a`]: 1 }), da),
    await hc("counter-case: hooks is an array", claude({ [SET]: "{\"hooks\":[1]}" }), da),
    await hc("counter-case: an array root", claude({ [SET]: "[{\"hooks\":{}}]" }), da),
    await hc("counter-case: a UTF-8 BOM before the object", claude({ [SET]: `﻿${hooks("~/.claude/hooks/a")}`, [`${HK}/a`]: 1 }), da),
    await hc("counter-case: settings.json is a folder", claude({ "~/.claude/settings.json/x": 5 }), da),
    await hc("counter-case: JSONC with comments and trailing commas", claude({ [SET]: "// c\n{\"hooks\":{\"Stop\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"~/.claude/hooks/a\",},],},],}, /* x */ }", [`${HK}/a`]: 1 }), da),
    await hc("counter-case: CRLF pretty settings", claude({ [SET]: `{\r\n  "hooks": ${JSON.stringify(JSON.parse(hooks("~/.claude/hooks/a")).hooks)}\r\n}\r\n`, [`${HK}/a`]: 1 }), da),
    await hc("a quote that is valid JSON: found", claude({ [SET]: hooks("~/.claude/hooks/a \"x"), [`${HK}/a`]: 1 }), da),
    await hc("a single quote after the path: found", claude({ [SET]: hooks("~/.claude/hooks/a 'x"), [`${HK}/a`]: 1 }), da),
    await hc("a quoted path then an argument: found", claude({ [SET]: hooks("\"~/.claude/hooks/a\" x"), [`${HK}/a`]: 1 }), da),
    await hc("a single-quoted path: found", claude({ [SET]: hooks("'~/.claude/hooks/a'"), [`${HK}/a`]: 1 }), da),
    await hc("bash with a quoted path: found", claude({ [SET]: hooks("bash \"~/.claude/hooks/a\""), [`${HK}/a`]: 1 }), da),
    await hc("a quote opened before the path: not found", claude({ [SET]: hooks("\"~/.claude/hooks/a"), [`${HK}/a`]: 1 }), da),
    await hc("a quote glued after the path: not found", claude({ [SET]: hooks("~/.claude/hooks/a\""), [`${HK}/a`]: 1 }), da),
    await hc("home word forms and interpreters", claude({ [SET]: hooks("$HOME/.claude/hooks/a && ${HOME}/.claude/hooks/b; bash ~/.claude/hooks/c.sh -x | node --check ~/.claude/hooks/d.js && FOO=1 ~/.claude/hooks/e"), [`${HK}/a`]: 1, [`${HK}/b`]: 2, [`${HK}/c.sh`]: 4, [`${HK}/d.js`]: 8, [`${HK}/e`]: 16 }), da),
    await hc("which words count as a script file", claude({ [SET]: hooks("~/.claude/hooks/dir", "~/.claude/hooks/missing", "~/.claude/hooks/ok.sh arg ~/.claude/hooks/arg2", "python3 -u ~/.claude/hooks/p.py", "env FOO=1 ~/.claude/hooks/e.sh", "bash", "", "   ", "~/", "~/.claude/hooks/q;~/.claude/hooks/r", "~/.claude/hooks/s ; ~/.claude/hooks/t"), [`${HK}/dir/x`]: 1, [`${HK}/ok.sh`]: 2, [`${HK}/arg2`]: 9, [`${HK}/p.py`]: 4, [`${HK}/e.sh`]: 5, [`${HK}/q`]: 6, [`${HK}/r`]: 7, [`${HK}/s`]: 8, [`${HK}/t`]: 10 }), da),
    await hc("one script named many ways is listed once", claude({ [SET]: JSON.stringify({ hooks: { Stop: [{ hooks: ["~/.claude/hooks/a", "$HOME/.claude/hooks/a", "bash ~/.claude/hooks/a", "/Users/dev/.claude/hooks/a"].map(cmd) }], PreToolUse: [{ hooks: [cmd("~/.claude/hooks/a")] }] } }), [`${HK}/a`]: 3 }), da),
    await hc("scripts under a config path, folders, outside home", claude({ [SET]: hooks("~/.claude/skills/x/run.sh", "~/.claude/hooks", "~/.claude/skillsX/run.sh", "/Users/dev/.claude/agents/a.sh", "/Users/devx/.claude/h", "/Users/dev/other/script.sh", "~/.claude.json"), "~/.claude/skills/x/run.sh": 5, "~/.claude/hooks/q": 6, "~/.claude/skillsX/run.sh": 7, "~/.claude/agents/a.sh": 8, "/Users/devx/.claude/h": 9, "~/other/script.sh": 10, "~/.claude.json": 11 }), da),
    await hc("hook shapes that carry nothing", claude({ [SET]: JSON.stringify({ hooks: { Stop: [{ hooks: [{ type: "prompt", prompt: "x" }, { command: 5 }, null, "~/.claude/hooks/a", cmd("~/.claude/hooks/ok")] }, { hooks: "x" }, "y"], Other: { hooks: [cmd("~/.claude/hooks/ok2")] } } }), [`${HK}/ok`]: 1, [`${HK}/ok2`]: 2, [`${HK}/a`]: 3 }), da),
    await hc("the hooks files are read on an empty laptop", {}, da),
    await hc("settings with empty hooks and no agent on PATH", claude({ [SET]: "{\"hooks\":{}}" }, []), da),
    await hc("a hook script and no PATH entry", claude({ [SET]: hooks("~/.claude/hooks/a"), [`${HK}/a`]: 3 }, []), da),
    await hc("settings exist, an unreferenced hook file", claude({ [`${HK}/x`]: 1, [SET]: "{}" }, []), da),
    await hc("Claude on PATH only: no volatile", claude({}), da),
    await hc("only ~/.claude.json", claude({ "~/.claude.json": 5 }, []), da),
    await hc("cursor on PATH is not the cursor bin", { which: ["cursor"] }, da),
    await hc("Codex notify names a script", { files: { "~/.codex/config.toml": "notify = [\"bash\", \"~/.codex/notify.sh\"]\n", "~/.codex/notify.sh": 33 }, which: ["codex"] }, da),
    await hc("Codex notify under a table", { files: { "~/.codex/config.toml": "[x]\nnotify = [\"bash\", \"~/.codex/notify.sh\"]\n", "~/.codex/notify.sh": 33 }, which: ["codex"] }, da),
    await hc("Codex notify over several lines", { files: { "~/.codex/config.toml": "notify = [\n \"node\", \"--x\",\n \"~/.codex/n.js\",\n]\n", "~/.codex/n.js": 33 }, which: ["codex"] }, da),
    await hc("Codex notify never closed", { files: { "~/.codex/config.toml": "notify = [\"bash\",\n", "~/.codex/notify.sh": 33 }, which: ["codex"] }, da),
    await hc("Claude Code brings settings, memory, skills, commands, MCP config", { files: { "~/.claude/settings.json": 400, "~/.claude/CLAUDE.md": 900, "~/.claude/skills/x/SKILL.md": 3000, "~/.claude/agents/a.md": 100, "~/.claude/commands/c.md": 50, "~/.claude/.credentials.json": 800, "~/.claude.json": 2000 }, which: ["claude"] }, da),
    await hc("Pi brings AGENTS.md and SYSTEM.md", { files: { "~/.pi/agent/AGENTS.md": 5, "~/.pi/agent/SYSTEM.md": 6, "~/.pi/agent/APPEND_SYSTEM.md": 7 } }, da),
    await hc("Aider config", { files: { "~/.aider.conf.yml": 40, "~/.aider.model.settings.yml": 2, "~/.aider.model.metadata.json": 3 } }, da),
  ];
  for (const a of AGENTS) detect.push(await hc(`${a.id} on PATH alone (bin ${a.bin})`, { which: [a.bin] }, da));

  const throwing = claude({ [SET]: dup("~/.claude/hooks/a", "~/.claude/hooks/b"), [`${HK}/a`]: 1, [`${HK}/b`]: 2, "~/.zshrc": 5 });
  const fine = claude({ [SET]: dup("~/.claude/hooks/a", "/bin/ls"), [`${HK}/a`]: 1, "~/.zshrc": 5 });

  const srcCases = [
    "source ~/.a   ", "source ~/.a\t", "source ~/.a\t# c", "source ~/.a # c  ", "source ~/.a #", "source ~/.a#x", "source ~/.a ;", "source  ~/.a", "\tsource ~/.a", ". ~/.a", "\\. ~/.a", "source \"~/.a\"", "source '${HOME}/.a'", "source \"$HOME/.a\"", "source '$HOME/.a'", "source ~/.a\rsource ~/.b", "source ~/.a x", "source ~/.a ", "source ~/.a # c d", "source ~/.a #c\r", "source /Users/dev/.a # c", "source \"/Users/dev/.a\"#c", "source /Users/dev", "source /Users/dev/", "source /Users/dev//a", "source \"/Users/dev/.a b\"", "source ~/.a b", "source ~/.a)", "source ~/.a\"", "source ~/.a ~/.b", "SOURCE ~/.a", "source\t~/.a", "sourcе ~/.a", "source ~/.é", "source ~/a\\ b", "source $HOME/.a", "source ${HOME}/.a", "source \"${HOME}/.a\"", "source ~/.a\n", "a\r\nsource ~/.x\nb\n", "a\nsource ~/.x\r\nb", "source ~/.a\nsource ~/.a\nsource /opt/x\nsource ~/.b\nsource /Users/dev/.c\nsource ~/.a", "", "alias g=git",
  ];
  const guard = (text: string, present: string[]) => {
    const asked: string[] = [];
    const out = guardSources(text, HOME, rel => (asked.push(rel), present.includes(rel)));
    return { text: out, asked };
  };
  const RC = ["export PATH=\"$HOME/bin:$PATH\"", ". \"$HOME/.cargo/env\"", "source ~/.deno/env  # deno", "  source \"${HOME}/.zsh/functions.zsh\"", ". /Users/dev/.local/share/x/env", "source '/Users/dev/.aliases'", "\\. \"$HOME/.bun/_bun\"", "[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh", "if [ -s \"$HOME/.sdkman/bin/sdkman-init.sh\" ]; then", "  source \"$HOME/.sdkman/bin/sdkman-init.sh\"", "fi", "source $ZSH/oh-my-zsh.sh", ". \"$NVM_DIR/nvm.sh\"", "source /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh", "source ~/.secrets 2>/dev/null", "source ~/.extra || true", "# source ~/.disabled", "source ~/.deno/env", "source ~/.zsh/*.zsh", "source", ".", "sourced=1", "source ~/.x; source ~/.y", ". \"/Users/dev/.zsh/functions.zsh\""].join("\n");
  const row = (id: string, paths: string[]) => ({ rung: "shell", id, label: id, paths, bytes: 0, default: "bring" });
  const rows = [row("shell/zshrc", []), row("shell/bashrc", ["~/.bashrc", "~/.other"]), row("shell/inputrc", ["~/.inputrc"]), row("shell/zinit", []), row("shell/fish", ["~/.config/fish"]), row("shell/zsh_aliases", ["~/.zsh_aliases"]), row("shell/profile", ["/abs/profile"]), row("shell/zshenv", ["~/.zshenv"])];
  const ssLaptop: Laptop = { files: { "~/.bashrc": "source ~/.b1\n", "~/.other": "source ~/.o\n", "~/.inputrc": "source ~/.i\n", "~/.zsh_aliases": ". ~/.aa\nsource ~/.aa\n", "/abs/profile": "source ~/.p\n", "~/.zshenv": "echo hi\n", "~/.config/fish": 4 } };
  const ssHost = host(ssLaptop);
  await shellSources(ssHost, rows as never);

  write("detect_agents", {
    agents: [{ name: "the agents in order, Aider last", args: [], out: AGENTS.map(a => ({ id: a.id, label: a.label, bin: a.bin, config: [...a.config], ...(a.volatile !== undefined ? { volatile: [...a.volatile] } : {}), hooks_file: a.hooks?.file ?? null })) }],
    agent_row_id: ["claude", "aider", "hermes", "x y"].map(i => ({ name: i, args: [i], out: agentRowId(i) })),
    detect_agents: detect,
    run_detector: [
      await hc("agents rung with a repeated hooks key", throwing, h => DETECTORS.agents(h, []), ["agents", []]),
      await hc("agents rung, the last hooks block holds only /bin/ls", fine, h => DETECTORS.agents(h, []), ["agents", []]),
    ],
    collect: [await col("a repeated hooks key stops collect at the agents rung", throwing), await col("counter-case: the last hooks block holds only /bin/ls", fine)],
    bare_sources: [...srcCases, RC].map(t => ({ name: JSON.stringify(t).slice(0, 60), args: [t, HOME], out: bareSources(t, HOME) })),
    guard_sources: [
      ...srcCases.map(t => ({ name: `${JSON.stringify(t).slice(0, 60)} nothing present`, args: [t, HOME, []], out: guard(t, []) })),
      { name: "present only .b", args: ["source ~/.a\nsource ~/.a\nsource /opt/x\nsource ~/.b\nsource /Users/dev/.c\nsource ~/.d", HOME, [".b"]], out: guard("source ~/.a\nsource ~/.a\nsource /opt/x\nsource ~/.b\nsource /Users/dev/.c\nsource ~/.d", [".b"]) },
      { name: "everything present, mixed line endings", args: ["a\r\nsource ~/.x\nsource /opt/y\nb\n", HOME, [".x"]], out: guard("a\r\nsource ~/.x\nsource /opt/y\nb\n", [".x"]) },
      { name: "the rc file of the TS test, two files present", args: [RC, HOME, [".zsh/functions.zsh", ".aliases"]], out: guard(RC, [".zsh/functions.zsh", ".aliases"]) },
      { name: "the literal-home lines, everything present", args: [[". \"/Users/dev/.zsh/functions.zsh\"", "source /Users/dev/.zsh/functions.zsh", "source '/Users/dev/.zsh/functions.zsh'  # fns", "source /Users/developer/.x", ""].join("\n"), HOME, [".zsh/functions.zsh"]], out: guard([". \"/Users/dev/.zsh/functions.zsh\"", "source /Users/dev/.zsh/functions.zsh", "source '/Users/dev/.zsh/functions.zsh'  # fns", "source /Users/developer/.x", ""].join("\n"), [".zsh/functions.zsh"]) },
      { name: "CRLF text, everything present", args: ["# x\r\n. \"$HOME/.cargo/env\"\r\nalias g=git\r\n", HOME, [".cargo/env"]], out: guard("# x\r\n. \"$HOME/.cargo/env\"\r\nalias g=git\r\n", [".cargo/env"]) },
    ],
    shell_sources: [{ name: "rows in order, first path only, rc ids only", laptop: ssLaptop, args: [rows.map(r => ({ ...r, sources: undefined }))], out: plain(rows), calls: ssHost.calls }],
  });
});
