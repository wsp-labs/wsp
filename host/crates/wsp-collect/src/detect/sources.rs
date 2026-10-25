// SPDX-License-Identifier: AGPL-3.0-only
//! The rc lines that read another file by a literal path with nothing around them, as rustup writes
//! `. "$HOME/.cargo/env"`. Port of packages/collect/src/detect/sources.ts at a3b870a6e; the spec is
//! spec/detect_agents.md.
use crate::host::Host;
use crate::manifest::ManifestEntry;

/// The files an rc file reads on a bare line, `~`-relative under home and absolute outside it, once each, in the order
/// they first appear.
// detect/sources.ts:16
// interface Bare {
//   line: number;
//   /** `~`-relative under home, else absolute. */
//   path: string;
//   indent: string;
//   word: string;
//   /** The path as written, quotes included, a literal home prefix rewritten as `"$HOME"`. */
//   token: string;
//   /** Whether the token was rewritten, so the line is never shipped as written. */
//   rewritten: boolean;
//   comment: string;
// }
//
// /** One source command, an optional comment, nothing else. */
// const TAIL = /^[ \t]*(#.*)?$/;
//
// /** The token with a literal home prefix written as `"$HOME"`; the token's own quoting stays around the rest. */
// function portable(token: string, home: string): string {
//   const quote = /^["']/.test(token) ? token[0]! : "";
//   const raw = quote === "" ? token : token.slice(1, -1);
//   if (!raw.startsWith(`${home}/`)) return token;
//   const rest = raw.slice(home.length);
//   return quote === '"' ? `"$HOME${rest}"` : `"$HOME"${quote}${rest}${quote}`;
// }
//
// function scan(text: string, home: string): Bare[] {
//   const out: Bare[] = [];
//   text.split(/\r?\n/).forEach((line, i) => {
//     const cmd = sourceCommand(line);
//     const tail = cmd === undefined ? null : TAIL.exec(cmd.rest);
//     if (cmd === undefined || tail === null) return;
//     const abs = sourcedPath(cmd.token, home);
//     if (abs === undefined) return;
//     const token = portable(cmd.token, home);
//     out.push({ line: i, path: tilde(home, abs), indent: cmd.indent, word: cmd.word, token, rewritten: token !== cmd.token, comment: tail[1] ?? "" });
//   });
//   return out;
// }
//
// /** The files an rc file reads on a bare line, `~`-relative under home and absolute outside it, once each, in the order they first appear. */
// export function bareSources(text: string, home: string): string[] {
//   return [...new Set(scan(text, home).map(b => b.path))];
// }
pub fn bare_sources(text: &str, home: &str) -> Vec<String> {
    todo!()
}

/// The text with each bare source line whose file is not on the machine wrapped as `[ -r path ] && . path`; every
/// other byte as it was.
// detect/sources.ts:60
// /** A source line that reads its file only when the file is there. */
// function guardLine(token: string, word: string): string {
//   return `[ -r ${token} ] && ${word} ${token}`;
// }
//
// /** The text with each bare source line whose file is not on the machine wrapped as `[ -r path ] && . path`, keeping
//  * its indent, word, token and comment; every other byte as it was. `present` is asked for each path under home,
//  * minus its `~/`; a path outside home is this computer's and is always wrapped, as is a line naming this computer's
//  * home literally, with the home written as `"$HOME"`. */
// export function guardSources(text: string, home: string, present: (rel: string) => boolean): string {
//   const bare = scan(text, home).filter(b => b.rewritten || !b.path.startsWith("~/") || !present(b.path.slice(2)));
//   if (bare.length === 0) return text;
//   const eol = text.includes("\r\n") ? "\r\n" : "\n";
//   const lines = text.split(/\r?\n/);
//   for (const b of bare) lines[b.line] = `${b.indent}${guardLine(b.token, b.word)}${b.comment === "" ? "" : ` ${b.comment}`}`;
//   return lines.join(eol);
// }
pub fn guard_sources(text: &str, home: &str, present: &dyn Fn(&str) -> bool) -> String {
    todo!()
}

/// Puts on each zsh or bash rc row (shell/<RC_FILES but inputrc>) the files it reads on a bare line.
// detect/sources.ts:78
// /** The rc files zsh and bash read, and the alias files those source; inputrc is readline's and fish has its own syntax. */
// const SH_RC = new Set(RC_FILES.filter(n => n !== "inputrc").map(n => `shell/${n}`));
//
// /** Puts on each zsh or bash rc row the files it reads on a bare line. */
// export async function shellSources(host: Host, rows: readonly ManifestEntry[]): Promise<void> {
//   for (const row of rows) {
//     const path = row.paths[0];
//     if (!SH_RC.has(row.id) || path === undefined) continue;
//     const text = await host.fs.readText(expand(host, path));
//     if (text === undefined) continue;
//     const sources = bareSources(text, host.home);
//     if (sources.length > 0) row.sources = sources;
//   }
// }
pub async fn shell_sources(host: &Host, rows: &mut [ManifestEntry]) {
    todo!()
}
