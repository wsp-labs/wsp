---
title: A sample post
date: 2026-10-14
summary: Every piece a post can hold, kept as a draft so the blog has something to show on this computer.
draft: true
author: Aditya Singhi
author_url: https://aditya.singhi.me
cover: /blog/sample/cover.webp
cover_alt: The wsp app with a lead thread and its sub-threads
---

This post is a draft. A production build leaves it out; `BLOG_DRAFTS=1 pnpm build` puts it in, for a look before a real post goes out.

## A heading

A paragraph with a [link to the docs](https://usewsp.com/docs), some `inline code`, and **a strong phrase**.

- A list item
- Another one, a little longer so it wraps on a phone and shows the line height

1. A numbered step
2. The next step

### A smaller heading

```sh
curl -fsSL https://usewsp.com/install | sh
```

> A quote from somebody, set apart from the paragraphs around it.

![The wsp link preview card](/og.png "A picture with its caption under it.")

[![A post on X](/og-blog.png "A screenshot of a post on X, which opens the post when clicked.")](https://x.com/wsplabs)

## A video

A `.mp4` or `.webm` file with the image syntax plays muted, on a loop, with its caption.

![Adding a computer in wsp](/blog/sample/setup.mp4 "A clip with its caption, playing on its own.")

## A diagram

Written in Mermaid in `content/blog/sample-flow.mmd`, drawn to an SVG with `pnpm diagram`, shown like any picture.

![A lead thread handing work to three computers](/blog/sample/flow.svg "A diagram drawn in the site's colours.")

## A tweet

`pnpm embed` saves the tweet once; the bare link on its own line becomes the card.

https://x.com/jack/status/20

## A YouTube video

The same command saves its thumbnail; YouTube loads only when the reader presses play.

https://www.youtube.com/watch?v=dQw4w9WgXcQ

## Any other embed

A plain iframe gets the same frame as the rest.

<iframe src="https://www.openstreetmap.org/export/embed.html?bbox=77.55%2C12.93%2C77.65%2C13.0&layer=mapnik" title="A map"></iframe>

---

The end of the sample.
