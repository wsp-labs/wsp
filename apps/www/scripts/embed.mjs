// SPDX-License-Identifier: AGPL-3.0-only
// Saves what a post shows of a tweet or a YouTube video into the repo, once, so the page needs no third-party script
// and never breaks when the post or the platform changes: `pnpm embed <url>`, then put the bare URL on its own line in
// the post. Tweets come from X's public syndication endpoint, the one its own embed reads.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const url = process.argv[2];
if (url === undefined) throw new Error("usage: pnpm embed <tweet or YouTube url>");

async function save(file, from) {
  const res = await fetch(from);
  if (!res.ok) throw new Error(`${from} answered ${res.status}`);
  mkdirSync(join(root, "public", file, ".."), { recursive: true });
  writeFileSync(join(root, "public", file), Buffer.from(await res.arrayBuffer()));
  return `/${file}`;
}

function write(name, record) {
  mkdirSync(join(root, "content/blog/embeds"), { recursive: true });
  writeFileSync(join(root, "content/blog/embeds", `${name}.json`), `${JSON.stringify(record, null, 2)}\n`);
  console.log(`wrote content/blog/embeds/${name}.json`);
}

const tweet = /^https:\/\/(?:x|twitter)\.com\/[^/]+\/status\/(\d+)/.exec(url);
const youtube = /^https:\/\/(?:www\.)?(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]{11})/.exec(url);

if (tweet !== null) {
  const id = tweet[1];
  const token = ((Number(id) / 1e15) * Math.PI).toString(36).replace(/(0+|\.)/g, "");
  const res = await fetch(`https://cdn.syndication.twimg.com/tweet-result?id=${id}&token=${token}&lang=en`);
  if (!res.ok) throw new Error(`X answered ${res.status} for tweet ${id}; is it public?`);
  const t = await res.json();
  let text = t.text;
  for (const u of t.entities?.urls ?? []) text = text.replace(u.url, u.display_url);
  for (const m of t.entities?.media ?? []) text = text.replace(m.url, "");
  const dir = `blog/embeds/tweet-${id}`;
  const media = [];
  for (const [i, m] of (t.mediaDetails ?? []).entries()) {
    media.push({ kind: m.type === "photo" ? "photo" : "video", src: await save(`${dir}/media-${i}.jpg`, m.media_url_https) });
  }
  write(`tweet-${id}`, {
    kind: "tweet",
    url: `https://x.com/${t.user.screen_name}/status/${id}`,
    name: t.user.name,
    handle: t.user.screen_name,
    avatar: await save(`${dir}/avatar.jpg`, t.user.profile_image_url_https.replace("_normal", "_bigger")),
    text: text.trim(),
    date: t.created_at,
    media,
  });
} else if (youtube !== null) {
  const id = youtube[1];
  const page = `https://www.youtube.com/watch?v=${id}`;
  const meta = await (await fetch(`https://www.youtube.com/oembed?url=${encodeURIComponent(page)}&format=json`)).json();
  let thumb;
  try {
    thumb = await save(`blog/embeds/youtube-${id}.jpg`, `https://i.ytimg.com/vi/${id}/maxresdefault.jpg`);
  } catch {
    thumb = await save(`blog/embeds/youtube-${id}.jpg`, `https://i.ytimg.com/vi/${id}/hqdefault.jpg`);
  }
  write(`youtube-${id}`, { kind: "youtube", id, url: page, title: meta.title, thumb });
} else {
  throw new Error(`${url} is neither a tweet nor a YouTube video`);
}
