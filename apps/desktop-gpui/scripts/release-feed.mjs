// SPDX-License-Identifier: AGPL-3.0-only
// node release-feed.mjs <dir> <port>: GitHub's release API for one release, vV from <dir>/VERSION, every other file an asset.
import { createHash } from "node:crypto";
import { createReadStream, readdirSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { join, resolve } from "node:path";

const dir = resolve(process.argv[2]);
const port = Number(process.argv[3]);
const REPO = "/repos/wsp-labs/wsp";
const DOWNLOAD = "/wsp-labs/wsp/releases/download";
const sums = new Map();
function sha256(path, st) {
  const key = `${st.size}:${st.mtimeMs}`;
  const kept = sums.get(path);
  if (kept?.key === key) return kept.sum;
  const sum = createHash("sha256").update(readFileSync(path)).digest("hex");
  sums.set(path, { key, sum });
  return sum;
}
function release() {
  const versionFile = join(dir, "VERSION");
  const tag = `v${readFileSync(versionFile, "utf8").trim()}`;
  const assets = readdirSync(dir)
    .filter(name => name !== "VERSION" && !name.startsWith(".") && !name.endsWith(".part"))
    .map(name => ({ name, path: join(dir, name), st: statSync(join(dir, name)) }))
    .filter(a => a.st.isFile())
    .map(a => ({ name: a.name, size: a.st.size, digest: `sha256:${sha256(a.path, a.st)}` }));
  return { tag_name: tag, name: tag, html_url: `https://github.com/wsp-labs/wsp/releases/tag/${tag}`, published_at: statSync(versionFile).mtime.toISOString(), draft: false, prerelease: false, assets };
}
function json(res, status, body) {
  const text = `${JSON.stringify(body)}\n`;
  res.writeHead(status, { "content-type": "application/json; charset=utf-8", "content-length": Buffer.byteLength(text) });
  res.end(text);
}
const server = createServer((req, res) => {
  console.log(`${new Date().toISOString()} ${req.method} ${req.url}`);
  const path = new URL(req.url ?? "/", "http://feed").pathname;
  if (req.method !== "GET" && req.method !== "HEAD") return json(res, 405, { message: "Method Not Allowed" });
  let rel;
  try {
    rel = release();
  } catch (e) {
    return json(res, 500, { message: String(e) });
  }
  if (path === `${REPO}/releases/latest` || path === `${REPO}/releases/tags/${rel.tag_name}`) return json(res, 200, rel);
  const prefix = `${DOWNLOAD}/${rel.tag_name}/`;
  if (path.startsWith(prefix)) {
    const name = decodeURIComponent(path.slice(prefix.length));
    const asset = rel.assets.find(a => a.name === name);
    if (asset === undefined) return json(res, 404, { message: "Not Found" });
    res.writeHead(200, { "content-type": "application/octet-stream", "content-length": asset.size });
    if (req.method === "HEAD") return res.end();
    const stream = createReadStream(join(dir, name));
    stream.on("error", () => res.destroy());
    return void stream.pipe(res);
  }
  return json(res, 404, { message: "Not Found" });
});
const stop = () => {
  server.close(() => process.exit(0));
  server.closeAllConnections();
};
for (const sig of ["SIGHUP", "SIGINT", "SIGTERM"]) process.on(sig, stop);
server.listen(port, "127.0.0.1", () => console.log(`feed: ${dir} on http://127.0.0.1:${port}`));
