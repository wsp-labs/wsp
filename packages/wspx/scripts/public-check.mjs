// SPDX-License-Identifier: AGPL-3.0-only
// node scripts/public-check.mjs <folder or .tgz>...: whether a public build
// carries Solari, the cloud it leaves out. Every script, page, stylesheet,
// picture, manifest and README in it is read whole, comments and the bundler's
// file markers included, and every daemon binary for its strings. Solari's
// module or its words is a line naming the file, and the exit is 1. The release
// runs it on the packed tarball and on the desktop app's folder before either
// leaves the runner.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { extname, join, relative } from "node:path";
import { pathToFileURL } from "node:url";

/** What Solari leaves in a build: its name, matched inside a longer word (SolariBackend, getsolari.com), and its key's
 * prefix. Boat ships in a public build, so its words pass. Solarized is a colour theme the diff viewer ships, and
 * Solaris an operating system a binary may name. */
export const PROVIDER_WORDS = [/solari(?![sz])/i, /slr_live_/];

const TEXT = new Set([".js", ".mjs", ".cjs", ".json", ".html", ".css", ".svg", ".md"]);
/** The first bytes of an ELF file and of a Mach-O, thin or universal, either byte order: a daemon binary. */
const BINARY_MAGIC = ["7f454c46", "cffaedfe", "feedfacf", "cafebabe", "bebafeca"];

/** A daemon binary's text, its strings among it, or nothing for any other file. */
function binaryText(path) {
  const bytes = readFileSync(path);
  return BINARY_MAGIC.includes(bytes.subarray(0, 4).toString("hex")) ? bytes.toString("latin1") : undefined;
}

/** The content hash the bundler puts in an emitted file's name, eight letters of base64url with a digit among them
 * before its extension: a name a script imports another chunk by can spell a provider's name by chance
 * (chunk-2Q5K7J3B-sOlarI6A.js), and a name with no digit (my-solari-pic.js) is a word, still read. */
const ASSET_HASH = /-(?=[\w-]{0,7}\d)[\w-]{8}(?=\.(?:m?js|css)\b)/g;

/** Every provider word in one file's text, each once, with the code around it. */
export function wordsIn(read) {
  const text = read.replace(ASSET_HASH, "");
  return PROVIDER_WORDS.flatMap(word => {
    const at = text.search(word);
    return at < 0 ? [] : [{ word: word.source, near: text.slice(Math.max(0, at - 40), at + 40).replace(/\s+/g, " ") }];
  });
}

const walk = dir => readdirSync(dir).flatMap(name => {
  const path = join(dir, name);
  return statSync(path).isDirectory() ? walk(path) : [path];
});

/** Every provider word under a folder, file by file.
 * @param {string} root
 * @returns {{ file: string, word: string, near: string }[]} */
export function providerWordsUnder(root) {
  return walk(root).flatMap(path => {
    const ext = extname(path);
    const text = TEXT.has(ext) ? readFileSync(path, "utf8") : binaryText(path);
    return text === undefined ? [] : wordsIn(text).map(hit => ({ file: relative(root, path), ...hit }));
  });
}

/** Every provider word in a folder or a tarball, which is unpacked into a folder of its own and taken away after.
 * @param {string} path
 * @returns {{ file: string, word: string, near: string }[]} */
export function providerWords(path) {
  if (!path.endsWith(".tgz")) return providerWordsUnder(path);
  const dir = mkdtempSync(join(tmpdir(), "wsp-public-check-"));
  try {
    execFileSync("tar", ["-xzf", path, "-C", dir]);
    return providerWordsUnder(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const paths = process.argv.slice(2);
  if (paths.length === 0) {
    console.error("usage: node public-check.mjs <folder or .tgz>...");
    process.exit(3);
  }
  const hits = paths.flatMap(path => providerWords(path).map(hit => ({ path, ...hit })));
  for (const hit of hits) console.error(`${hit.path}: ${hit.file}: ${hit.word} in "${hit.near}"`);
  if (hits.length > 0) {
    console.error(`a public build carries ${hits.length} Solari word${hits.length === 1 ? "" : "s"}: build it with PUBLIC_BUILD=1 and every package built again`);
    process.exit(1);
  }
  console.log(`no Solari word in ${paths.join(", ")}`);
}
