// A slate frame's page read for the window: an .html file of text from any path, each thing that is not one refused
// with its code and one sentence, a page the window holds answered unchanged, and the same rules through another
// computer's daemon's fs.page, an older daemon's unknown op said as the update line.
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { unknownOpLine, type FsImageReply, type SlatesPageAnswer } from "@wsp/protocol";
import { slatePage, type PageOn } from "../src/slate-pages.js";

const PAGE = "<!doctype html><title>Dial</title><p>The needle reads each block.</p>";
const CAP = 1024 * 1024;

const said = (a: SlatesPageAnswer): string => ("problem" in a ? `${a.problem.code} ${a.problem.message}` : "unchanged" in a ? "unchanged" : `shown ${a.html.length}`);
const here = (src: string, folder?: string, have?: string) => slatePage(src, { thread: "t-here", have, folder, on: undefined });

describe("a frame's page on this computer", () => {
  let dir = "";
  beforeAll(() => {
    dir = mkdtempSync("/tmp/wsp-slate-pages-");
    mkdirSync(join(dir, "figures"));
    writeFileSync(join(dir, "figures", "dial.html"), PAGE);
    writeFileSync(join(dir, "bytes.html"), Buffer.from([0xff, 0xfe, 0x00, 0x3c]));
    writeFileSync(join(dir, "huge.html"), "x".repeat(CAP + 1));
    writeFileSync(join(dir, "edge.html"), "x".repeat(CAP));
    mkdirSync(join(dir, "folder.html"));
    execFileSync("mkfifo", [join(dir, "pipe.html")]);
  });
  afterAll(() => rmSync(dir, { recursive: true, force: true }));

  it("shows a page by its whole path and under the thread's folder by a relative one, with its version", async () => {
    const shown = await here(join(dir, "figures", "dial.html"));
    expect(shown).toMatchObject({ html: PAGE, version: expect.any(String) });
    expect(said(await here("figures/dial.html", dir))).toBe(`shown ${PAGE.length}`);
    expect(said(await here(join(dir, "edge.html")))).toBe(`shown ${CAP}`);
    expect(await here(join(dir, "figures", "dial.html"), undefined, (shown as { version: string }).version)).toEqual({ unchanged: true, version: (shown as { version: string }).version });
  });

  it("refuses what is not a page of text under the cap, and never an address", async () => {
    expect(said(await here(join(dir, "bytes.html")))).toBe(`R918 ${join(dir, "bytes.html")} is not an .html file; a frame shows an HTML page`);
    expect(said(await here(join(dir, "huge.html")))).toBe(`R919 ${join(dir, "huge.html")} is 1.1 MB, over the 1 MB a frame's page may weigh`);
    expect(said(await here(join(dir, "gone.html")))).toMatch(/^R900 /);
    expect(said(await here(join(dir, "folder.html")))).toMatch(/^R914 /);
    // A read that waited on the pipe would never answer.
    expect(said(await here(join(dir, "pipe.html")))).toMatch(/^R914 /);
    expect(said(await here("https://acme.test/page.html"))).toMatch(/^R917 /);
    expect(said(await here("/root/.ssh/id_ed25519"))).toMatch(/^R918 /);
    expect(said(await here("figures/dial.html"))).toMatch(/^R903 figures\/dial.html is not a whole path/);
  });
});

describe("a frame's page on another computer", () => {
  const reply = (content: string | undefined, size: number, mediaType?: string): FsImageReply => ({ size, modified: 1_700_000_000_000, inode: 7, changed: 9, ...(mediaType !== undefined ? { mediaType } : {}), ...(content !== undefined ? { content: Buffer.from(content).toString("base64") } : {}) });
  const on = (read: PageOn["read"]): PageOn => ({ name: "lab", read });
  const there = (src: string, road: PageOn, have?: string) => slatePage(src, { thread: "t-box", have, folder: "/root/acme", on: road });

  it("reads a page there by its whole path, under the thread's folder there for a relative one", async () => {
    const asked: string[] = [];
    const road = on(async path => (asked.push(path), reply(PAGE, PAGE.length, "text/html")));
    const shown = await there("figures/dial.html", road);
    expect(shown).toMatchObject({ html: PAGE });
    expect(asked).toEqual(["/root/acme/figures/dial.html"]);
    expect(await there("/tmp/wsp-slates/t/dial.html", road, (shown as { version: string }).version)).toMatchObject({ unchanged: true });
  });

  it("says what the daemon refused in the slate's words, and an older daemon's unknown op as the update line", async () => {
    const coded = (code: string) => on(async () => Promise.reject(Object.assign(new Error("no"), { code })));
    expect(said(await there("/tmp/a.html", coded("not-found")))).toMatch(/^R900 /);
    expect(said(await there("/tmp/a.html", coded("not-a-file")))).toMatch(/^R914 /);
    expect(said(await there("/tmp/a.html", on(async () => reply(undefined, 3))))).toMatch(/^R918 /);
    expect(said(await there("/tmp/a.html", on(async () => reply(undefined, CAP + 5))))).toMatch(/^R919 /);
    expect(said(await there("/tmp/a.html", on(async () => Promise.reject(new Error(unknownOpLine("fs.page"))))))).toMatch(/^R903 lab runs a daemon too old to read pages; .* updates it$/);
  });
});
