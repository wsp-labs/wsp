// SPDX-License-Identifier: AGPL-3.0-only
// The blog's posts: what a draft is, and what a post's Markdown becomes.
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { blogMarkdown } from "../scripts/blog-md.mjs";
import { shown, type Post } from "../src/blog";

const post = (slug: string, date: string, draft = false): Post => ({ slug, title: slug, date, summary: "", image: null, cover: null, coverAlt: "", updated: null, author: null, draft, html: "" });

function render(body: string, embeds: Record<string, unknown> = {}): string {
  const dir = mkdtempSync(join(tmpdir(), "blog-"));
  mkdirSync(join(dir, "embeds"));
  for (const [name, record] of Object.entries(embeds)) writeFileSync(join(dir, "embeds", `${name}.json`), JSON.stringify(record));
  const out = blogMarkdown().transform(`---\ntitle: T\ndate: 2026-10-24\nsummary: S\n---\n\n${body}\n`, join(dir, "p.md"));
  return (JSON.parse(out!.code.replace(/^export default /, "").replace(/;$/, "")) as Post).html;
}

describe("the blog's posts", () => {
  it("leaves drafts out unless asked, newest first", () => {
    const posts = [post("old", "2026-01-01"), post("draft", "2026-12-01", true), post("new", "2026-06-01")];
    expect(shown(posts, false).map(p => p.slug)).toEqual(["new", "old"]);
    expect(shown(posts, true).map(p => p.slug)).toEqual(["draft", "new", "old"]);
  });

  it("refuses a post with no title, a bad date or no front matter", () => {
    const plugin = blogMarkdown();
    expect(() => plugin.transform("no fences", "/x/a.md")).toThrow(/front matter/);
    expect(() => plugin.transform("---\ndate: 2026-10-24\nsummary: s\n---\n", "/x/a.md")).toThrow(/title/);
    expect(() => plugin.transform("---\ntitle: t\ndate: 24 Oct\nsummary: s\n---\n", "/x/a.md")).toThrow(/YYYY-MM-DD/);
  });

  it("makes a lone picture a figure with its caption, and a lone clip a muted looping video", () => {
    expect(render('![A diagram](/d.svg "The caption")')).toBe('<figure><img src="/d.svg" alt="A diagram" loading="lazy" /><figcaption>The caption</figcaption></figure>\n');
    expect(render("![A clip](/c.mp4)")).toContain('<video src="/c.mp4" autoplay muted loop playsinline aria-label="A clip"></video>');
    expect(render("Text with ![inline](/i.png) in it.")).toMatch(/^<p>/);
  });

  it("turns a bare tweet or YouTube link into its saved card, and refuses one nothing is saved for", () => {
    const tweet = { kind: "tweet", url: "https://x.com/a/status/1", name: "A", handle: "a", avatar: "/av.jpg", text: "hi <b>", date: "2026-10-24T00:00:00.000Z", media: [] };
    const html = render("https://x.com/a/status/1", { "tweet-1": tweet });
    expect(html).toContain('<figure class="tweet"><a href="https://x.com/a/status/1"');
    expect(html).toContain("hi &lt;b&gt;");
    expect(render("https://www.youtube.com/watch?v=dQw4w9WgXcQ", { "youtube-dQw4w9WgXcQ": { id: "dQw4w9WgXcQ", title: "V", thumb: "/t.jpg" } })).toContain('data-youtube="dQw4w9WgXcQ"');
    expect(() => render("https://x.com/a/status/2")).toThrow(/pnpm embed/);
  });
});
