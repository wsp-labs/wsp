// SPDX-License-Identifier: AGPL-3.0-only
// Every page the site serves, and what search engines, crawlers and link previews read about it. The build renders each
// one to its own HTML file and lists the indexed ones in the sitemap and llms.txt.
import type { ComponentType } from "react";
import { DOWNLOADS } from "./downloads";
import { EMAIL, ORG, SITE, X } from "./links";
import { TOOLS } from "./compare";
import { POSTS } from "./blog";
import { BlogIndex, BlogPost } from "./pages/blog";
import { Compare, ToolPage } from "./pages/compare";
import { MacDownload } from "./pages/download-mac";
import { Home } from "./pages/home";
import { Privacy, Security, Terms } from "./pages/legal";
import { NotFound } from "./pages/not-found";

export type Route = {
  path: string;
  title: string;
  description: string;
  /** The link-preview image under public/, and the line `pnpm images` writes on it. */
  image: { path: string; line: string; alt: string };
  /** False keeps the page out of the sitemap and out of search. */
  index: boolean;
  /** An article's day, YYYY-MM-DD: its head says article, not website. */
  published?: string;
  author?: { name: string; url: string | null; x: string | null } | null;
  data?: Record<string, unknown>[];
  Page: ComponentType;
};

const PITCH = "Run Claude Code, Codex and more on your Mac, an old laptop or a server you rent, from one window. Your agents can start each other on any of them.";
const MAIN_IMAGE = { path: "/og.png", line: "Coding agents on every computer you own.", alt: "wsp: coding agents on every computer you own." };

const ORGANIZATION = {
  "@type": "Organization",
  "@id": `${SITE}/#org`,
  name: "wsp labs",
  url: `${SITE}/`,
  logo: `${SITE}/logo.png`,
  email: EMAIL,
  sameAs: [ORG, X],
};

export const ROUTES: readonly Route[] = [
  {
    path: "/",
    title: "wsp: coding agents on every computer you own",
    description: PITCH,
    image: MAIN_IMAGE,
    index: true,
    data: [
      ORGANIZATION,
      { "@type": "WebSite", "@id": `${SITE}/#site`, name: "wsp", url: `${SITE}/`, publisher: { "@id": ORGANIZATION["@id"] } },
      {
        "@type": "SoftwareApplication",
        name: "wsp",
        url: `${SITE}/`,
        description: PITCH,
        applicationCategory: "DeveloperApplication",
        operatingSystem: "macOS, Linux",
        license: "https://www.gnu.org/licenses/agpl-3.0.html",
        downloadUrl: [DOWNLOADS.mac.href, DOWNLOADS.linux.href],
        offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
        publisher: { "@id": ORGANIZATION["@id"] },
      },
    ],
    Page: Home,
  },
  { path: "/privacy", title: "Privacy: wsp", description: "What wsp collects and sends, where it goes, and how to turn each part off.", image: MAIN_IMAGE, index: true, Page: Privacy },
  { path: "/terms", title: "Terms: wsp", description: "The terms for usewsp.com, the docs, the relay and the waitlist.", image: MAIN_IMAGE, index: true, Page: Terms },
  { path: "/security", title: "Security: wsp", description: "How the parts of wsp trust each other, what it does with your keys, and how to report a problem.", image: MAIN_IMAGE, index: true, Page: Security },
  {
    path: "/compare",
    title: "Compare: wsp",
    description: "wsp beside Conductor, herdr, Orca, Claude Code on the web and six more: where each runs agents, and what a second computer has on it.",
    image: { path: "/og-compare.png", line: "How wsp compares.", alt: "wsp: how wsp compares." },
    index: true,
    Page: Compare,
  },
  ...TOOLS.map(
    (tool): Route => ({
      path: `/compare/${tool.slug}`,
      title: `wsp vs ${tool.name}`,
      description: tool.differs,
      image: { path: `/og-compare-${tool.slug}.png`, line: `wsp vs ${tool.name}.`, alt: `wsp vs ${tool.name}.` },
      index: true,
      Page: () => <ToolPage tool={tool} />,
    }),
  ),
  {
    path: "/blog",
    title: "Blog: wsp",
    description: "What we build, what broke, and what we learned shipping wsp.",
    image: { path: "/og-blog.png", line: "The wsp blog.", alt: "The wsp blog." },
    index: POSTS.some(p => !p.draft),
    data: [
      ORGANIZATION,
      {
        "@type": "Blog",
        name: "The wsp blog",
        url: `${SITE}/blog`,
        publisher: { "@id": ORGANIZATION["@id"] },
        blogPost: POSTS.filter(p => !p.draft).map(p => ({ "@type": "BlogPosting", headline: p.title, url: `${SITE}/blog/${p.slug}`, datePublished: p.date })),
      },
    ],
    Page: BlogIndex,
  },
  ...POSTS.map(
    (post): Route => ({
      path: `/blog/${post.slug}`,
      title: `${post.title}: wsp`,
      description: post.summary,
      image: { path: post.image ?? `/og-blog-${post.slug}.png`, line: post.title, alt: post.title },
      author: post.author,
      index: !post.draft,
      published: post.date,
      data: [
        ORGANIZATION,
        {
          "@type": "BlogPosting",
          headline: post.title,
          description: post.summary,
          datePublished: post.date,
          dateModified: post.updated ?? post.date,
          mainEntityOfPage: `${SITE}/blog/${post.slug}`,
          url: `${SITE}/blog/${post.slug}`,
          image: `${SITE}${post.image ?? `/og-blog-${post.slug}.png`}`,
          author: post.author === null ? { "@id": ORGANIZATION["@id"] } : { "@type": "Person", name: post.author.name, ...(post.author.url === null ? {} : { url: post.author.url }) },
          publisher: { "@id": ORGANIZATION["@id"] },
        },
      ],
      Page: () => <BlogPost post={post} />,
    }),
  ),
  {
    path: "/download/mac",
    title: "Download wsp for Mac",
    description: "Your wsp download has started. How to open it the first time, while the app is not signed by Apple yet.",
    image: MAIN_IMAGE,
    index: false,
    Page: MacDownload,
  },
  {
    path: "/404",
    title: "Not found: wsp",
    description: "There is no page at this address on usewsp.com.",
    image: MAIN_IMAGE,
    index: false,
    Page: NotFound,
  },
];

const NOT_FOUND = ROUTES.find(r => r.path === "/404")!;

/** The page a path shows; any path not in the table shows the 404, as the host serves 404.html for it. */
export function routeAt(pathname: string): Route {
  const path = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
  return ROUTES.find(r => r.path === path) ?? NOT_FOUND;
}

/** Where a route's HTML lands in the build: `/` at index.html, the 404 at 404.html, the rest at <path>/index.html. */
export function fileOf(path: string): string {
  if (path === "/") return "index.html";
  if (path === "/404") return "404.html";
  return `${path.slice(1)}/index.html`;
}
