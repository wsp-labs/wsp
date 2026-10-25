// SPDX-License-Identifier: AGPL-3.0-only
// A post under content/blog becomes a module of its front matter and its finished HTML when vite loads it, so no
// Markdown parser ships to the browser and the prerender and the hydrating page read the same string.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { Marked } from "marked";

/** The few front matter keys a post has, one `key: value` per line between `---` fences. */
function frontMatter(source, file) {
  const match = /^---\n([\s\S]*?)\n---\n/.exec(source);
  if (match === null) throw new Error(`${file} has no front matter between --- fences`);
  const meta = {};
  for (const line of match[1].split("\n")) {
    const at = line.indexOf(":");
    if (at === -1) continue;
    meta[line.slice(0, at).trim()] = line.slice(at + 1).trim().replace(/^"(.*)"$/, "$1");
  }
  for (const key of ["title", "date", "summary"]) if (!meta[key]) throw new Error(`${file} names no ${key}`);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(meta.date)) throw new Error(`${file}'s date is ${meta.date}, not YYYY-MM-DD`);
  return { meta, body: source.slice(match[0].length) };
}

const escape = text => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const TWEET = /^https:\/\/(?:x|twitter)\.com\/[^/]+\/status\/(\d+)/;
const YOUTUBE = /^https:\/\/(?:www\.)?(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]{11})/;
const X_MARK = '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M18.9 2H22l-6.8 7.8L23 22h-6.2l-4.8-6.3L6.4 22H3.3l7.3-8.3L1 2h6.3l4.4 5.8L18.9 2Zm-1.1 18h1.7L6.3 3.9H4.5L17.8 20Z"/></svg>';
const PLAY = '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M8 5.5v13a1 1 0 0 0 1.5.86l10.4-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5Z"/></svg>';

/** What `pnpm embed` saved for a tweet or a video; the post cannot build without it, so nothing is fetched at build. */
function saved(file, name, url) {
  const path = join(dirname(file), "embeds", `${name}.json`);
  if (!existsSync(path)) throw new Error(`${file} shows ${url}, but nothing is saved for it; run pnpm embed ${url}`);
  return JSON.parse(readFileSync(path, "utf8"));
}

const day = date => new Date(date).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });

/** A tweet as a card in the site's own type, the whole card one link to X; its text has no links inside the link. */
function tweetCard(t) {
  const media = t.media.map(m => `<img src="${escape(m.src)}" alt="" loading="lazy" />`).join("");
  return [
    `<figure class="tweet"><a href="${escape(t.url)}" rel="noopener">`,
    `<span class="tweet-head"><img src="${escape(t.avatar)}" alt="" loading="lazy" /><span class="tweet-who"><span class="tweet-name">${escape(t.name)}</span><span class="tweet-handle">@${escape(t.handle)}</span></span>${X_MARK}</span>`,
    `<span class="tweet-text">${escape(t.text).replace(/\n/g, "<br />")}</span>`,
    media === "" ? "" : `<span class="tweet-media tweet-media-${Math.min(t.media.length, 4)}">${media}</span>`,
    `<time datetime="${escape(t.date)}">${day(t.date)}</time>`,
    "</a></figure>\n",
  ].join("");
}

/** A video's thumbnail and a play button; the player, and YouTube with it, loads only when pressed (pages/blog.tsx). */
function youtubeCard(v) {
  return `<figure class="youtube"><button type="button" data-youtube="${escape(v.id)}" aria-label="Play ${escape(v.title)}"><img src="${escape(v.thumb)}" alt="" loading="lazy" /><span class="youtube-play">${PLAY}</span></button><figcaption>${escape(v.title)}</figcaption></figure>\n`;
}

/** An image alone in its paragraph is a figure, its title the caption, and an .mp4 or .webm one is a muted looping
 * video; one wrapped in a link (a screenshot of somewhere else) links out from inside the figure, so no figure ever
 * sits in a <p> or an <a>. */
function figure(token, file) {
  if (token.type === "link" && token.text === token.href) {
    const t = TWEET.exec(token.href);
    if (t !== null) return tweetCard(saved(file, `tweet-${t[1]}`, token.href));
    const v = YOUTUBE.exec(token.href);
    if (v !== null) return youtubeCard(saved(file, `youtube-${v[1]}`, token.href));
  }
  const link = token.type === "link" ? token : null;
  const image = link === null ? token : link.tokens.find(t => t.type === "image");
  if (image?.type !== "image") return null;
  const img = /\.(mp4|webm)$/i.test(image.href)
    ? `<video src="${escape(image.href)}" autoplay muted loop playsinline aria-label="${escape(image.text)}"></video>`
    : `<img src="${escape(image.href)}" alt="${escape(image.text)}" loading="lazy" />`;
  const caption = image.title ? `<figcaption>${escape(image.title)}</figcaption>` : "";
  return `<figure>${link === null ? img : `<a href="${escape(link.href)}">${img}</a>`}${caption}</figure>\n`;
}

const markedFor = file => new Marked({
  gfm: true,
  renderer: {
    paragraph(token) {
      const only = token.tokens.filter(t => !(t.type === "text" && t.raw.trim() === ""));
      return (only.length === 1 && figure(only[0], file)) || false;
    },
    link({ href, tokens }) {
      const outside = /^https?:\/\//.test(href) && !href.startsWith("https://usewsp.com");
      return `<a href="${escape(href)}"${outside ? ' rel="noopener"' : ""}>${this.parser.parseInline(tokens)}</a>`;
    },
  },
});

export function blogMarkdown() {
  return {
    name: "blog-markdown",
    transform(source, id) {
      if (!id.endsWith(".md")) return null;
      const slug = id.split("/").pop().replace(/\.md$/, "");
      const { meta, body } = frontMatter(source, id);
      const post = { slug, title: meta.title, date: meta.date, summary: meta.summary, image: meta.image ?? null, cover: meta.cover ?? null, coverAlt: meta.cover_alt ?? "", updated: meta.updated ?? null, author: meta.author ? { name: meta.author, url: meta.author_url ?? null, x: meta.author_x ?? null, avatar: meta.author_avatar ?? null } : null, draft: meta.draft === "true", html: markedFor(id).parse(body) };
      return { code: `export default ${JSON.stringify(post)};`, map: null };
    },
  };
}
