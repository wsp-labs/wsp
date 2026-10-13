// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { claudeMemoryDir, claudeProjectKey, defaultSeedChoice, exitClassOf, neverGitLine, notInTheMenuLine, projectSeedLines, projectSeedOf, type SeedFile, type SeedPlan, seedChoiceFrom, seedPicks, seedCommitsLandedLine, seedingLine, seedMenuRows, seedRowWords, seedSummaryLines, staysHereLine } from "../src/index.js";

const file = (path: string, kind: SeedFile["kind"], bytes: number, row?: { id: string; name: string }): SeedFile => ({
  path,
  dir: false,
  bytes,
  kind,
  ...(row === undefined ? {} : { row }),
  ticked: kind === "config",
});

const PLAN: SeedPlan = {
  source: "/Users/dev/spoo-landing",
  remote: "https://github.com/spoo-me/frontend.git",
  branch: "refactor/dashboard-polish",
  defaultBranch: "main",
  unpushed: { commits: 2, base: "9f1c2e4aa11b0c3d4e5f60718293a4b5c6d7e8f9" },
  uncommitted: 4,
  memory: { key: claudeProjectKey("/Users/dev/spoo-landing"), files: 5, bytes: 28_000 },
  files: [
    file(".env.local", "config", 4096, { id: "next", name: "Next" }),
    file(".claude/settings.local.json", "config", 4096, { id: "agents", name: "agents" }),
    file("node_modules", "rebuilt", 2_600_000_000, { id: "node", name: "Node" }),
    file("dev.db", "data", 900_000, { id: "databases", name: "databases" }),
    file(".git-credentials", "never", 300, { id: "logins", name: "logins" }),
    file(".next-mock", "unknown", 3_200_000_000),
  ],
  remembered: false,
};

describe("the key a project's memory sits under", () => {
  it("is the path with every other character dashed, so two paths one dash apart collide", () => {
    expect(claudeProjectKey("/Users/a/b-c")).toBe("-Users-a-b-c");
    expect(claudeProjectKey("/Users/a/b/c")).toBe(claudeProjectKey("/Users/a/b-c"));
  });

  it("names the memory folder under a state home once, for the collector, the runtime and the engine alike", () => {
    expect(claudeMemoryDir("/Users/dev/.claude", "-Users-dev-wsp")).toBe("/Users/dev/.claude/projects/-Users-dev-wsp/memory");
    expect(claudeMemoryDir("/root/.claude/", "-root-wsp")).toBe("/root/.claude/projects/-root-wsp/memory");
  });
});

describe("what a seed ticks before anybody touches the menu", () => {
  it("ticks the configuration, the memory and the commits, and nothing of any other kind", () => {
    expect(defaultSeedChoice(PLAN)).toEqual({ files: [".env.local", ".claude/settings.local.json"], memory: true, commits: true });
  });

  it("takes the ticks the plan carries, so an untick remembered for that folder holds", () => {
    // The menu a remembered choice was applied to carries their ticks, not the catalogue's: a config row they
    // unticked last time stays unticked, and a row they kept that no row names travels.
    const remembered: SeedPlan = {
      ...PLAN,
      remembered: true,
      files: PLAN.files.map(f => ({ ...f, ticked: f.path === ".next-mock" })),
    };
    expect(defaultSeedChoice(remembered)).toEqual({ files: [".next-mock"], memory: true, commits: true });
  });

  it("never ticks a login, whatever a remembered choice says about it", () => {
    const remembered: SeedPlan = { ...PLAN, remembered: true, files: PLAN.files.map(f => ({ ...f, ticked: true })) };
    expect(defaultSeedChoice(remembered).files).not.toContain(".git-credentials");
  });

  it("ticks no memory and no commits where the folder has neither", () => {
    expect(defaultSeedChoice({ ...PLAN, memory: null, unpushed: null })).toEqual({ files: [".env.local", ".claude/settings.local.json"], memory: false, commits: false });
  });
});

describe("what a person's own words make of the menu", () => {
  it("unticks a path they cut and ticks one the catalog does not carry when they keep it", () => {
    expect(seedChoiceFrom(PLAN, [], [".env.local"]).files).toEqual([".claude/settings.local.json"]);
    expect(seedChoiceFrom(PLAN, [".next-mock"], []).files).toEqual([".env.local", ".claude/settings.local.json", ".next-mock"]);
  });

  it("drops the memory and the patch on the words that say so, and remembers only when asked", () => {
    expect(seedChoiceFrom(PLAN, [], [], { memory: false })).toMatchObject({ memory: false, commits: true });
    expect(seedChoiceFrom(PLAN, [], [], { commits: false })).toMatchObject({ memory: true, commits: false });
    expect(seedChoiceFrom(PLAN, [], [], { remember: true }).remember).toBe(true);
    expect("remember" in seedChoiceFrom(PLAN, [], [])).toBe(false);
  });

  it("refuses to carry a login by name, even when it is the path they kept", () => {
    expect(() => seedChoiceFrom(PLAN, [".git-credentials"], [])).toThrow(/never travels in a seed/);
  });

  it("refuses a path the menu never showed, and says what it holds", () => {
    expect(() => seedChoiceFrom(PLAN, ["app/page.tsx"], [])).toThrow(/is not on the seed menu/);
  });

  it("takes a folder named with the slash its row prints", () => {
    expect(seedChoiceFrom(PLAN, ["node_modules/"], []).files).toContain("node_modules");
  });

  it("refuses a cut that names no row, so a typo never leaves the path it meant ticked, and the repository's own folder, as usage", () => {
    const shown = PLAN.files.map(f => f.path);
    for (const [keep, cut, line] of [
      [[], [".env.locl"], notInTheMenuLine(".env.locl", shown)],
      [[".git"], [], neverGitLine(".git")],
      [[], [".git/config"], neverGitLine(".git/config")],
    ] as const) {
      const refused = (() => {
        try {
          return seedChoiceFrom(PLAN, keep, cut);
        } catch (e) {
          return e;
        }
      })();
      expect(refused).toMatchObject({ message: line });
      expect(exitClassOf(refused)).toBe("usage");
    }
  });
});

describe("a project's seed picks", () => {
  it("start where they are told, and keep a start path the menu no longer shows as missing, until it is cut", () => {
    expect(seedPicks(PLAN, [".env.local", "old.env"], ["dev.db"], [])).toEqual({ files: [".env.local", "dev.db"], missing: ["old.env"] });
    expect(seedPicks(PLAN, [".env.local", "old.env"], [], ["old.env"])).toEqual({ files: [".env.local"], missing: [] });
    expect(() => seedPicks(PLAN, [], ["old.env"], [])).toThrow(/is not on the seed menu/);
    expect(() => seedPicks(PLAN, ["old.env"], ["old.env"], ["old.env"])).toThrow(/is not on the seed menu/);
  });

  it("draw the tick column one wide whether anything is picked or not, and name the missing ones under the table", () => {
    const none = projectSeedLines(projectSeedOf("lab", PLAN, { files: [], missing: [] }, false));
    expect(none.rows.slice(1).map(r => r[0])).toEqual(PLAN.files.map(() => " "));
    expect(none.foot).toEqual(["0 picked. Nothing was saved; add --yes to save these picks."]);
    const some = projectSeedLines(projectSeedOf("lab", PLAN, { files: [".env.local"], missing: ["old.env"] }, true));
    expect(some.rows[1]![0]).toBe("x");
    expect(some.foot).toEqual(["Picked but not in the folder now, so a sub-thread starts without: old.env.", "1 picked, 4 KB. Saved: every later sub-thread of lab gets these."]);
  });
});

describe("a file of values or a key", () => {
  const values = (provider: boolean): SeedPlan => ({
    ...PLAN,
    files: [{ ...file(".env", "secret", 120, { id: "node", name: "Node" }), ticked: true }, file(".claude/settings.local.json", "config", 4096, { id: "agents", name: "agents" })],
    ...(provider ? { provider: true } : {}),
  });

  it("is ticked for a box as configuration is, and named by its row", () => {
    const box = values(false);
    expect(defaultSeedChoice(box).files).toEqual([".env", ".claude/settings.local.json"]);
    expect(seedChoiceFrom(box, [], []).files).toContain(".env");
    expect(seedRowWords(box.files[0]!, box)).toBe("Node");
  });

  it("never goes to a provider place: unticked whatever was remembered, shown as staying here, and refused by name", () => {
    const cloud = values(true);
    expect(defaultSeedChoice(cloud).files).toEqual([".claude/settings.local.json"]);
    expect(seedMenuRows(cloud, defaultSeedChoice(cloud))).toContainEqual(["[ ]", ".env", "120 B", "stays on this computer"]);
    expect(() => seedChoiceFrom(cloud, [".env"], [])).toThrow(staysHereLine(".env"));
    expect(exitClassOf((() => { try { return seedChoiceFrom(cloud, [".env"], []); } catch (e) { return e; } })())).toBe("usage");
  });

  it("drops out of saved picks for a provider place and stays in them for a box", () => {
    expect(seedPicks(values(true), [".env", ".claude/settings.local.json"], [], [])).toEqual({ files: [".claude/settings.local.json"], missing: [] });
    expect(seedPicks(values(false), [".env", ".claude/settings.local.json"], [], [])).toEqual({ files: [".env", ".claude/settings.local.json"], missing: [] });
  });
});

describe("the words a row ends in", () => {
  it("say why each kind is ticked or not", () => {
    expect(seedRowWords(PLAN.files[0]!)).toBe("Next");
    expect(seedRowWords(PLAN.files[2]!)).toBe("rebuilt on the box");
    expect(seedRowWords(PLAN.files[3]!)).toBe("database, stop what uses it first");
    expect(seedRowWords(PLAN.files[4]!)).toBe("never travels");
    expect(seedRowWords(PLAN.files[5]!)).toBe("not in the catalogue");
  });
});

describe("what the person is told would travel", () => {
  it("is the ticked files, the memory, the patch and what stays here", () => {
    expect(seedSummaryLines(PLAN, defaultSeedChoice(PLAN))).toEqual([
      "2 files, 8 KB: .env.local, .claude/settings.local.json",
      "Claude Code memory, 5 files, 27 KB",
      "2 commits the remote does not have, as a patch from 9f1c2e4",
      "4 uncommitted changes stay on this computer",
    ]);
  });

  it("says no files travel where nothing is ticked, and nothing about a folder with no edits", () => {
    expect(seedSummaryLines({ ...PLAN, uncommitted: 0, memory: null, unpushed: null }, { files: [], memory: false, commits: false })).toEqual(["no files travel"]);
  });
});

describe("what the seeding stage says as the files go and once they have landed", () => {
  it("names the files, the memory and the commits in the menu's own counts, and where they came from", () => {
    expect(seedingLine(PLAN, defaultSeedChoice(PLAN))).toBe(
      "Seeding 2 files (8 KB), Claude Code memory (5 files, 27 KB), 2 commits the remote does not have, from /Users/dev/spoo-landing.",
    );
    // A memory folder is no file of the seed's: the menu counts it on a row of its own and so does this line,
    // so a seed of nothing but the memory reads as nought files here and as one file there.
    expect(seedingLine({ ...PLAN, unpushed: null, memory: { key: "-Users-dev-spoo-landing", files: 1, bytes: 69 } }, { files: [], memory: true, commits: false })).toBe(
      "Seeding 0 files, Claude Code memory (1 file, 69 B), from /Users/dev/spoo-landing.",
    );
    // Nothing ticked at all still says where it came from.
    expect(seedingLine({ ...PLAN, memory: null, unpushed: null }, { files: [], memory: false, commits: false })).toBe("Seeding 0 files, from /Users/dev/spoo-landing.");
  });

  it("says what the commits did, off the count the computer's own git read back", () => {
    expect(seedCommitsLandedLine(1)).toBe("1 commit landed");
    expect(seedCommitsLandedLine(2)).toBe("2 commits landed");
    expect(seedCommitsLandedLine(0)).toBe("no commits to land");
  });
});
