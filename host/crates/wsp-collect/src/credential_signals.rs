// SPDX-License-Identifier: AGPL-3.0-only
//! What makes a file look like a credential: its name, its mode, the key names in a JSON, YAML, TOML or KEY=value
//! file, a PEM header, a password in a URL. A value that is a template or environment reference, a placeholder or
//! nothing points at a secret held elsewhere and raises nothing. No value read here leaves these functions. Port of
//! packages/collect/src/credential-signals.ts at a3b870a6e; the spec is spec/credential_signals.md.
use std::future::Future;

pub use wsp_wire::views::project_bundle::CredentialSignal as Signal;

/// The largest file mode and content are believed for, in bytes (credential-signals.ts:13).
pub const MAX_BYTES: u64 = 100 * 1024;
/// How deep key names are read into a JSON object (credential-signals.ts:15).
pub const KEY_DEPTH: usize = 3;

/// A file's size and permission bits, as the caller stat'ed it (TS `{ bytes, mode }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileShape {
    pub bytes: u64,
    pub mode: u32,
}

/// The lower-cased text after the last dot, or empty when the dot is the first character or there is none.
// credential-signals.ts:45
// function extension(name: string): string {
//   const dot = name.lastIndexOf(".");
//   return dot <= 0 ? "" : name.slice(dot + 1).toLowerCase();
// }
pub fn extension(name: &str) -> String {
    todo!()
}

// credential-signals.ts:17
// const NAME_PATTERNS = [
//   /^\.?credentials/i,
//   /^auth\.json$/,
//   /^hosts\.yml$/,
//   /access[-_]token/i,
//   /\.pem$/,
//   /\.key$/,
//   /^key$/,
//   /^id_[a-z0-9]+$/,
//   /^\.?netrc$/,
//   /^\.env(\..+)?$/,
//   /^\.npmrc$/,
//   /^\.pypirc$/,
//   /^\.git-credentials$/,
//   /(^|[._-])tokens?([._-]|$)/i,
//   /(^|[._-])secrets?([._-]|$)/i,
// ];
// const SKIP_SUFFIXES = [".example", ".sample", ".template", ".dist", ".pub", ".lock"];
// /** Owner-only by habit, never a secret. */
// const SKIP_NAMES = new Set([".viminfo", ".CFUserTextEncoding", ".z", "known_hosts", ".envrc"]);
// const SOURCE_EXTENSIONS = new Set(["go", "ts", "tsx", "js", "mjs", "cjs", "py", "rs", "c", "h", "cc", "cpp", "java", "rb", "md", "html", "css", "svg", "png", "jpg", "gif", "ico", "woff", "woff2", "ttf", "zip", "gz", "tar", "wasm", "node", "so", "dylib"]);
// credential-signals.ts:50
// function skippedName(name: string): boolean {
//   return SKIP_NAMES.has(name) || /_history$/.test(name) || SKIP_SUFFIXES.some(s => name.endsWith(s)) || SOURCE_EXTENSIONS.has(extension(name));
// }
//
// export function nameSignal(name: string): boolean {
//   if (skippedName(name)) return false;
//   return NAME_PATTERNS.some(p => p.test(name));
// }
pub fn name_signal(name: &str) -> bool {
    todo!()
}

/// Owner-only (no group or other bit), not empty, and at most MAX_BYTES.
// credential-signals.ts:59
// export function modeSignal(e: { mode: number; bytes: number }): boolean {
//   return (e.mode & 0o077) === 0 && e.bytes > 0 && e.bytes <= MAX_BYTES;
// }
pub fn mode_signal(e: FileShape) -> bool {
    todo!()
}

/// The key names the pairs carry, in the order they were read: every key of a JSON object to KEY_DEPTH, else the
/// `key:` / `key =` heads of each line.
// credential-signals.ts:63
// /** A key with what it holds; `value` is undefined when the format gave a structure rather than text. */
// interface KeyValue {
//   key: string;
//   value: string | undefined;
// }
//
// /** A value that names where the secret lives instead of holding one: a workflow's `${{ secrets.NAME }}`, a compose
//  * or toml `${NAME}` or `$NAME`, an angle-bracket placeholder, or nothing at all. */
// const REFERENCE_VALUE = /^(?:\$\{\{[^}]*\}\}|\$\{[^}]*\}|\$[A-Za-z_]\w*|<[^<>]*>)?$/;
//
// const isReference = (value: string | undefined): boolean => value !== undefined && REFERENCE_VALUE.test(value);
//
// function jsonKeys(data: unknown, depth: number, out: KeyValue[]): void {
//   if (typeof data !== "object" || data === null || Array.isArray(data) || depth > KEY_DEPTH) return;
//   for (const [key, v] of Object.entries(data)) {
//     out.push({ key, value: typeof v === "string" ? v : undefined });
//     jsonKeys(v, depth + 1, out);
//   }
// }
//
// /** What a line holds after its key: the comment a config file trails the value with dropped, since it sits outside the
//  * quotes and only whitespace-then-`#` starts one, so a literal `abc#def` keeps its hash; then the comma a JSONC line
//  * ends with, then one layer of quotes. */
// function lineValue(raw: string): string {
//   const t = raw.replace(/\s+#[^\n]*$/, "").trim().replace(/,$/, "").trim();
//   const quoted = /^"([^"]*)"$/.exec(t) ?? /^'([^']*)'$/.exec(t);
//   return (quoted?.[1] ?? t).trim();
// }
//
// /** Key names with their values, at any depth of a JSON object or from the `key:` / `key =` heads of a YAML, TOML, JSONC or KEY=value file. */
// function keyValues(text: string): KeyValue[] {
//   try {
//     const out: KeyValue[] = [];
//     jsonKeys(JSON.parse(text), 1, out);
//     return out;
//   } catch {
//     const out: KeyValue[] = [];
//     for (const line of text.split("\n")) {
//       // The value is captured as [^\n]* and not .*, which does not match the carriage return a CRLF line ends with.
//       const m = /^\s*(?:\/\/[^\s:]+\/:)?"?([A-Za-z_][\w.-]*)"?\s*[:=]([^\n]*)$/.exec(line);
//       if (m?.[1] !== undefined) out.push({ key: m[1], value: lineValue(m[2] ?? "") });
//     }
//     return out;
//   }
// }
//
// /** The key names those pairs carry, in the order they were read. */
// export const topLevelKeys = (text: string): string[] => keyValues(text).map(e => e.key);
pub fn top_level_keys(text: &str) -> Vec<String> {
    todo!()
}

// credential-signals.ts:38
// /** Shapes worth opening for key names: the structured formats, plus files without an extension (config, token, .boto). */
// const READABLE_EXTENSIONS = new Set(["json", "yaml", "yml", "toml", "pem", "key", "env", ""]);
// /** Mode on its own is only believed for a structured file. */
// const STRUCTURED_EXTENSIONS = new Set(["json", "yaml", "yml", "toml"]);
// /** Key names that hold a secret, after `accessToken` and `client-secret` are read as snake case. Bare `key` is left out (a keybinding or a public key far more often) and so is a `_password` suffix (`generate_password` is a command). */
// const KEY_NAMES = new Set(["token", "password", "passwd", "secret", "bearer", "auths", "credentials", "_authtoken", "apikey", "api_key", "private_key", "secret_key", "access_key", "secret_access_key"]);
// const KEY_SUFFIXES = ["_token", "_secret", "_api_key", "_apikey", "_private_key", "_secret_access_key"];
// credential-signals.ts:112
// /** `accessToken` reads as `access_token`. */
// function normalizeKey(k: string): string {
//   return k.replace(/([a-z0-9])([A-Z])/g, "$1_$2").replace(/-/g, "_").toLowerCase();
// }
//
// export function keysSignal(text: string): boolean {
//   return keyValues(text).some(e => {
//     const n = normalizeKey(e.key);
//     return (KEY_NAMES.has(n) || KEY_SUFFIXES.some(sfx => n.endsWith(sfx))) && !isReference(e.value);
//   });
// }
pub fn keys_signal(text: &str) -> bool {
    todo!()
}

// credential-signals.ts:124
// export function pemSignal(text: string): boolean {
//   return /^-----BEGIN[ A-Z0-9_-]*PRIVATE KEY/.test(text.trimStart());
// }
pub fn pem_signal(text: &str) -> bool {
    todo!()
}

// credential-signals.ts:128
// /** A URL whose userinfo carries a password, `scheme://user:secret@host` or `scheme://:secret@host`; a username alone
//  * is a name, not a secret, and a password that is a reference names one rather than holding it. */
// const URL_CREDENTIAL = /\b([a-z][a-z0-9+.-]*:\/\/)[^\s/@:]*:([^\s/@]+)@([^\s"']+)/gi;
//
// export function urlSignal(text: string): boolean {
//   return bareUrls(text).urls.length > 0;
// }
pub fn url_signal(text: &str) -> bool {
    todo!()
}

/// The text with every credential URL's userinfo removed, and those URLs as they now read (TS `{ text, urls }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareUrls {
    pub text: String,
    pub urls: Vec<String>,
}

// credential-signals.ts:136
// /** The text with every such URL's userinfo removed, and those URLs as they now read. */
// export function bareUrls(text: string): { text: string; urls: string[] } {
//   const urls: string[] = [];
//   const bare = text.replace(URL_CREDENTIAL, (whole: string, scheme: string, password: string, rest: string) => {
//     if (isReference(password)) return whole;
//     urls.push(`${scheme}${rest}`);
//     return `${scheme}${rest}`;
//   });
//   return { text: bare, urls };
// }
pub fn bare_urls(text: &str) -> BareUrls {
    todo!()
}

/// The signals one file carries, in the order name, mode, keys, pem, url, or `None` when it is not secret-shaped.
/// `read` is awaited at most once, and only for a non-empty file of at most MAX_BYTES with a readable extension.
// credential-signals.ts:147
// // Mode alone is believed only for a structured file right inside its app directory whose content did
// // not parse as plain config: deeper down, or once the keys read as settings, it marks tracker files,
// // lock files and themes far more often than secrets.
// /** The signals one file carries, or nothing when it is not secret-shaped. `read` is called only for a small file
//  * of a shape worth opening; `shallow` says the file sits right inside its app directory. */
// export async function fileSignals(name: string, e: { bytes: number; mode: number }, read: () => Promise<string | undefined>, shallow: boolean): Promise<Signal[] | undefined> {
//   const ext = extension(name);
//   if (e.bytes === 0 || skippedName(name)) return undefined;
//   const signals: Signal[] = [];
//   if (nameSignal(name)) signals.push("name");
//   if (modeSignal(e)) signals.push("mode");
//   let parsed = false;
//   if (e.bytes > 0 && e.bytes <= MAX_BYTES && READABLE_EXTENSIONS.has(ext)) {
//     const text = await read();
//     if (text !== undefined && /^\s*(\{\s*\}|\[\s*\])?\s*$/.test(text)) return undefined;
//     if (text !== undefined) {
//       parsed = topLevelKeys(text).length > 0;
//       if (keysSignal(text)) signals.push("keys");
//       if (pemSignal(text)) signals.push("pem");
//       if (urlSignal(text)) signals.push("url");
//     }
//   }
//   if (signals.length === 0) return undefined;
//   if (signals.length === 1 && signals[0] === "mode" && !(shallow && STRUCTURED_EXTENSIONS.has(ext) && !parsed)) return undefined;
//   return signals;
// }
pub async fn file_signals<F, Fut>(name: &str, e: FileShape, read: F, shallow: bool) -> Option<Vec<Signal>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<String>>,
{
    todo!()
}
