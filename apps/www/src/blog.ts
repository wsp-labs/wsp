// SPDX-License-Identifier: AGPL-3.0-only
// The posts under content/blog, newest first. scripts/blog-md.mjs turns each into its front matter and finished HTML.

export type Post = {
  slug: string;
  title: string;
  /** YYYY-MM-DD, the day it went out. */
  date: string;
  summary: string;
  /** A link-preview image under public/ named in its front matter; without one the build writes /og-blog-<slug>.png. */
  image: string | null;
  /** A picture across the top of the post and its alt text, front matter `cover` and `cover_alt`. */
  cover: string | null;
  coverAlt: string;
  /** YYYY-MM-DD of the last real change, when there was one. */
  updated: string | null;
  /** Who wrote it, front matter `author`, `author_url` and `author_x` (an X handle with its @); wsp labs without one. */
  author: { name: string; url: string | null; x: string | null } | null;
  draft: boolean;
  html: string;
};

/** The posts a build shows: every published one, and the drafts too when it is asked for them. */
export function shown(posts: readonly Post[], drafts: boolean): Post[] {
  return posts.filter(p => drafts || !p.draft).sort((a, b) => b.date.localeCompare(a.date) || a.slug.localeCompare(b.slug));
}

const files = import.meta.glob<Post>("../content/blog/*.md", { eager: true, import: "default" });

export const POSTS = shown(Object.values(files), __BLOG_DRAFTS__);

/** "14 October 2026", the way the legal pages write a day. */
export function day(date: string): string {
  return new Date(`${date}T00:00:00Z`).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });
}
