// SPDX-License-Identifier: AGPL-3.0-only
// The skills the GPUI port keeps in .claude/skills: the gpui-kit pair vendored at a recorded upstream commit,
// wsp-gpui, wsp-gpui-handin and gpui-agent's stub. Each SKILL.md's links into the repo must name files that exist.
// URLs are checked by hand, never in the suite.
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { skillMdFrontmatter } from "@wsp/collect";

const skills = new URL("../../../.claude/skills/", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, skills), "utf8");

describe("the GPUI port's skills", () => {
  it.each(["gpui-kit", "gpui-kit-design-guides", "wsp-gpui", "wsp-gpui-handin", "gpui-agent"])("%s has a SKILL.md whose frontmatter names it and describes it", name => {
    const front = skillMdFrontmatter(read(`${name}/SKILL.md`));
    expect(front.name).toBe(name);
    expect(front.description).toMatch(/\S/);
  });

  it.each(["gpui-kit", "gpui-kit-design-guides"])("%s records the upstream repo, path and commit it was vendored from", name => {
    expect(read(`${name}/UPSTREAM`)).toMatch(new RegExp(`^repo=https://github\\.com/\\S+\\npath=skills/${name}\\ncommit=[0-9a-f]{40}\\n$`));
  });

  it("keeps wsp-gpui under 400 lines", () => {
    expect(read("wsp-gpui/SKILL.md").trimEnd().split("\n").length).toBeLessThan(400);
  });

  it("keeps wsp-gpui-handin under 200 lines", () => {
    expect(read("wsp-gpui-handin/SKILL.md").trimEnd().split("\n").length).toBeLessThan(200);
  });

  it.each(["gpui-kit", "gpui-kit-design-guides", "wsp-gpui", "wsp-gpui-handin"])("every link in %s's SKILL.md that is not an https URL names a repo file that exists", name => {
    const text = read(`${name}/SKILL.md`);
    const local = [...text.matchAll(/\]\(([^)\s]+)\)/g)].map(m => m[1]!).filter(link => !link.startsWith("https://"));
    expect(local.length).toBeGreaterThan(0);
    const missing = local.filter(link => !existsSync(fileURLToPath(new URL(link.split("#")[0]!, new URL(`${name}/`, skills)))));
    expect(missing).toEqual([]);
  });
});
