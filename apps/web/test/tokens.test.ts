// SPDX-License-Identifier: AGPL-3.0-only
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { appTerminalFontSize } from "../src/terminal/ghostty/surface.js";

const css = readFileSync(join(__dirname, "../src/index.css"), "utf8");

describe("index.css", () => {
  it("carries the upstream token set with no brand strings", () => {
    const body = css.split("\n").slice(1).join("\n");
    expect(css.split("\n")[0]).toBe(
      "/* Adapted from pingdotgg/t3code apps/web/src/index.css at 57a66608 (MIT). */",
    );
    expect(body).not.toMatch(/t3|T3/);
    expect(body).toContain('@import "tailwindcss";');
    expect(body).toContain("@theme inline {");
  });

  it("the mono size token is the 11px the sidebar and composer set their code and meta text in", () => {
    expect(css).toMatch(/^\s*--font-size-mono: 11px;$/m);
  });

  it("the terminal size token beside it is the app's text size, and the pane's default is the number that token names", () => {
    expect(css).toMatch(/^\s*--font-size-terminal: 14px;$/m);
    // A page with no stylesheet yet falls back to the number in the code; the two must be the same size.
    expect(appTerminalFontSize()).toBe(14);
  });

  it("registers each thread status ink as a colour utility off the theme's own token", () => {
    const inline = /@theme inline \{([\s\S]*?)\n\}/.exec(css)![1]!;
    for (const tone of ["input", "working", "failed", "done"]) expect(inline).toContain(`--color-status-${tone}: var(--status-${tone});`);
  });

  it("carries no second theme system: no contrast layer, no inherited theme ids, no workspace theme on the sidebar", () => {
    expect(css).not.toMatch(/--contrast-|--appearance-contrast|--app-theme-|data-theme-id|theme-inspector|data-space-|--space-grain|--toolbar-/);
    expect(css).not.toContain('[data-theme="dark"]');
  });

  it("the Mac's dark material takes its grounds from the theme, so a dark theme's window stands on its own ground", () => {
    const material = css.slice(css.indexOf(".desktop-mac.dark [data-slot=\"sidebar-inset\"]"), css.indexOf("/* The window paints the glass behind the page"));
    expect(material.length).toBeGreaterThan(0);
    expect(material).not.toMatch(/rgb\((6 6 6|24 24 26|31 31 33)/);
    expect(material).toContain("var(--material-ground)");
    expect(material).toContain("var(--material-raised)");
    expect(material).toContain("var(--material-edge)");
  });

  it("the sidebar's scope names no colour of its own: it maps the page's roles onto the theme's sidebar tokens, on both sides", () => {
    const scope = /\n\[data-app-sidebar\] \{([^}]*)\}/.exec(css)![1]!;
    expect(scope).not.toMatch(/#|rgb|oklch|--color-/);
    expect(scope).toContain("--background: var(--sidebar-canvas);");
    expect(scope).toContain("--muted-foreground: var(--sidebar-quiet);");
  });

  it("draws no blur of the page's own on the Mac, where the window's glass is macOS's: each pane's blur is written for the other platforms alone, the composer's excepted, and no rule turns one off", () => {
    // Each blur declaration with the blocks it sits in, outermost first. A block's head is the text before its brace.
    const blurs: string[][] = [];
    const heads: string[] = [];
    let head = "";
    for (const ch of css.replace(/\/\*[\s\S]*?\*\//g, "")) {
      if (ch === "{") {
        heads.push(head.trim());
        head = "";
      } else if (ch === "}") {
        heads.pop();
        head = "";
      } else if (ch === ";") {
        if (/^(-webkit-)?backdrop-filter:\s*blur\(/.test(head.trim())) blurs.push([...heads]);
        head = "";
      } else head += ch;
    }
    expect(blurs.length).toBeGreaterThan(0);
    // A dialog's scrim is the one blur the window's glass cannot stand in for: it blurs the page under the dialog. The
    // frost utility carries its blur bare, and every use of it names the condition.
    const onMac = blurs.filter(within => !within.includes("@variant off-mac") && !within.includes("@utility dialog-backdrop") && within.join() !== "@utility glass-backdrop");
    expect(onMac).toEqual([]);
    expect(css).not.toMatch(/backdrop-filter:\s*none/);
    const applied = [...css.matchAll(/@variant off-mac \{\s*@apply glass-backdrop;/g)].length;
    expect(applied).toBe([...css.matchAll(/@apply glass-backdrop;/g)].length);
    const uses = execFileSync("git", ["grep", "-hoE", "[^ \"'`]*glass-backdrop", "--", "src", ":!*.css", ":!*.test.*"], { cwd: join(__dirname, ".."), encoding: "utf8" }).trim().split("\n");
    expect(uses.length).toBeGreaterThan(0);
    // The composer and the rows attached to it frost on the Mac too, its tray among them: the owner wants the page
    // under them to show.
    expect(uses.filter(use => !use.startsWith("off-mac:")).sort()).toEqual(
      ["before:glass-backdrop", "before:glass-backdrop", "before:glass-backdrop", "glass-backdrop", "glass-backdrop", "group-data-banner-attached/composer-surface:glass-backdrop"].sort(),
    );
  });

  it("takes the glass inks only while the page draws its glass: with transparency off the sidebar keeps the theme's quiet inks", () => {
    // Each declaration that reads the glass ink, with the heads of the blocks it sits in, outermost first.
    const reads: string[][] = [];
    const heads: string[] = [];
    let head = "";
    for (const ch of css.replace(/\/\*[\s\S]*?\*\//g, "")) {
      if (ch === "{") {
        heads.push(head.trim());
        head = "";
      } else if (ch === "}") {
        heads.pop();
        head = "";
      } else if (ch === ";") {
        if (/^--[\w-]+:.*var\(--glass-ink\)/.test(head.trim())) reads.push([...heads]);
        head = "";
      } else head += ch;
    }
    expect(reads.length).toBeGreaterThan(6);
    expect(reads.filter(within => !within.some(h => h.includes(":not(.solid)")))).toEqual([]);
    const sidebar = /\n(\S[^{\n]*\[data-app-sidebar\]) \{\n {2}@variant dark \{\n {4}--muted-foreground: var\(--glass-ink\);/.exec(css)!;
    expect(sidebar[1]).toBe(".desktop-mac:not(.solid) [data-app-sidebar]");
  });
});
