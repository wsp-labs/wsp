// SPDX-License-Identifier: AGPL-3.0-only
//! The person's Ghostty config as the terminal pane takes it, read the way Ghostty's config reference says: config.ghostty
//! then config under XDG, then the same pair in the Mac's Application Support, a later file winning, each followed by
//! the files its config-file lines include; the last theme line resolved against the config's themes folder and the
//! bundled ones, its colors under the config's own. Port of packages/collect/src/ghostty-config.ts at a3b870a6e; the
//! spec is spec/ghostty_config.md.
use wsp_wire::views::project_bundle::{TerminalRgb, TerminalScheme};

use crate::host::Host;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhosttyDirective {
    pub key: String,
    pub value: String,
}

/// Ghostty ships JetBrains Mono and draws with it until a config names a font.
pub const GHOSTTY_BUILT_IN_FONT: &str = "JetBrains Mono";
pub const MAC_APP_SUPPORT: &str = "~/Library/Application Support/com.mitchellh.ghostty";
pub const MAC_BUNDLED_THEMES: &str = "/Applications/Ghostty.app/Contents/Resources/ghostty/themes";
pub const LINUX_SHARED_THEMES: [&str; 2] = ["/usr/local/share/ghostty/themes", "/usr/share/ghostty/themes"];
/// Ghostty's blur intensity for `background-blur = true`.
pub const DEFAULT_BLUR: i64 = 20;

/// The key = value lines of one file, in order and with repeats; comments, blanks and lines with no = are not
/// directives.
// ghostty-config.ts:27
// const unquote = (value: string): string => value.replace(/^"(.*)"$/, "$1");
//
// /** The key = value lines of one Ghostty file, in order and with repeats; comments, blanks and lines with no = are not directives. */
// export function parseGhosttyDirectives(text: string): GhosttyDirective[] {
//   const out: GhosttyDirective[] = [];
//   for (const raw of text.split("\n")) {
//     const line = raw.replace(/^\uFEFF/, "").trim();
//     if (line === "" || line.startsWith("#")) continue;
//     const at = line.indexOf("=");
//     if (at < 0) continue;
//     const key = line.slice(0, at).trim();
//     if (key === "") continue;
//     out.push({ key, value: unquote(line.slice(at + 1).trim()) });
//   }
//   return out;
// }
pub fn parse_ghostty_directives(text: &str) -> Vec<GhosttyDirective> {
    todo!()
}

/// Where Ghostty looks for its config on this computer, in load order.
// ghostty-config.ts:44
// function xdgConfigDir(host: Host): string {
//   return host.xdgConfigHome ?? `${host.home}/.config`;
// }
//
// /** Where Ghostty looks for its config on this computer, in the order it loads them: config.ghostty before the config
//  * it was named before 1.2.3, and conflicting values in a later file win, as the config reference has it. */
// export function ghosttyConfigPaths(host: Host): string[] {
//   const xdg = `${xdgConfigDir(host)}/ghostty`;
//   const paths = [`${xdg}/config.ghostty`, `${xdg}/config`];
//   if (host.platform === "darwin") paths.push(expand(host, `${MAC_APP_SUPPORT}/config.ghostty`), expand(host, `${MAC_APP_SUPPORT}/config`));
//   return paths;
// }
pub fn ghostty_config_paths(host: &Host) -> Vec<String> {
    todo!()
}

/// `#RRGGBB` or `RRGGBB`, surrounding whitespace allowed, any case.
// ghostty-config.ts:110
// function hexColor(value: string): TerminalRgb | undefined {
//   const m = /^#?([0-9a-fA-F]{6})$/.exec(value.trim());
//   if (m === null) return undefined;
//   const hex = m[1]!;
//   return { r: parseInt(hex.slice(0, 2), 16), g: parseInt(hex.slice(2, 4), 16), b: parseInt(hex.slice(4, 6), 16) };
// }
pub fn hex_color(value: &str) -> Option<TerminalRgb> {
    todo!()
}

/// The theme a `theme` value names for the scheme: the one name, or the light:/dark: pair's side.
// ghostty-config.ts:213
// /** The theme a `theme` value names for the scheme: the one name, or the light:/dark: pair's side. */
// export function themeFor(value: string, scheme: TerminalScheme): string {
//   const sides = new Map<string, string>();
//   for (const part of value.split(",")) {
//     const at = part.indexOf(":");
//     if (at > 0) sides.set(part.slice(0, at).trim(), part.slice(at + 1).trim());
//   }
//   return sides.get(scheme) ?? (sides.size === 0 ? value.trim() : "");
// }
pub fn theme_for(value: &str, scheme: TerminalScheme) -> String {
    todo!()
}

/// The person's Ghostty config as the pane applies it; with no file, files and fontFamily empty, sixteen `None`
/// palette slots and every other key absent.
// ghostty-config.ts:57
// /** Where a theme name is looked for, in order: the config's own themes folder, then the bundled ones. */
// function themePaths(host: Host, name: string): string[] {
//   if (name.startsWith("/")) return [name];
//   const dirs = [`${xdgConfigDir(host)}/ghostty/themes`, ...(host.platform === "darwin" ? [MAC_BUNDLED_THEMES] : LINUX_SHARED_THEMES)];
//   return dirs.map(dir => `${dir}/${name}`);
// }
//
// function dirOf(path: string): string {
//   const at = path.lastIndexOf("/");
//   return at <= 0 ? "/" : path.slice(0, at);
// }
//
// function resolveInclude(value: string, from: string, home: string): string | undefined {
//   const optional = value.startsWith("?");
//   const path = unquote(optional ? value.slice(1) : value);
//   if (path === "") return undefined;
//   const expanded = expand({ home }, path);
//   return expanded.startsWith("/") ? expanded : `${dirOf(from)}/${expanded}`;
// }
//
// /** Ghostty normalises a path lexically before comparing it, so an include naming its own file through .. is a cycle. */
// function normalize(path: string): string {
//   const out: string[] = [];
//   for (const part of path.split("/")) {
//     if (part === "" || part === ".") continue;
//     if (part === "..") out.pop();
//     else out.push(part);
//   }
//   return `/${out.join("/")}`;
// }
//
// /** One config file and, after it, the files it includes, each once; the directives in load order and the files read. */
// async function loadWithIncludes(host: Host, path: string, seen: Set<string>, files: string[], into: GhosttyDirective[]): Promise<void> {
//   const own = normalize(path);
//   if (seen.has(own)) return;
//   const text = await host.fs.readText(own);
//   if (text === undefined) return;
//   seen.add(own);
//   files.push(own);
//   const directives = parseGhosttyDirectives(text);
//   into.push(...directives);
//   const includes: string[] = [];
//   for (const { key, value } of directives) {
//     if (key !== "config-file") continue;
//     if (value === "") includes.length = 0;
//     else {
//       const resolved = resolveInclude(value, own, host.home);
//       if (resolved !== undefined) includes.push(resolved);
//     }
//   }
//   for (const include of includes) await loadWithIncludes(host, include, seen, files, into);
// }
// ghostty-config.ts:117
// function finite(value: string): number | undefined {
//   const n = Number(value.trim());
//   return value.trim() !== "" && Number.isFinite(n) ? n : undefined;
// }
//
// /** `N=COLOR` with N in decimal, 0x, 0o or 0b; only the first sixteen slots are the pane's. */
// function paletteEntry(value: string): { index: number; color: TerminalRgb } | undefined {
//   const at = value.indexOf("=");
//   if (at < 0) return undefined;
//   const index = Number(value.slice(0, at).trim());
//   const color = hexColor(value.slice(at + 1));
//   if (!Number.isInteger(index) || index < 0 || index > 15 || color === undefined) return undefined;
//   return { index, color };
// }
//
// /** One or two non-negative numbers, comma separated; one value sets both sides. */
// function padding(value: string): [number, number] | undefined {
//   const parts = value.split(",").map(finite);
//   const [a, b] = parts;
//   if (parts.length > 2 || a === undefined || a < 0 || (parts.length === 2 && (b === undefined || b < 0))) return undefined;
//   return [a, b ?? a];
// }
//
// function blur(value: string): number | undefined {
//   const word = value.trim().toLowerCase();
//   if (word === "false") return 0;
//   if (word === "true" || word.startsWith("macos-glass-")) return DEFAULT_BLUR;
//   const n = finite(word);
//   return n !== undefined && Number.isInteger(n) && n >= 0 ? n : undefined;
// }
//
// type Draft = Omit<TerminalConfig, "files" | "theme" | "palette"> & { palette: (TerminalRgb | null)[] };
//
// /** The keys the pane honours, applied in order over a draft; a value that does not parse leaves its key as it was. */
// function apply(draft: Draft, { key, value }: GhosttyDirective): void {
//   switch (key) {
//     case "font-family":
//       if (value === "") draft.fontFamily = [];
//       else draft.fontFamily.push(value);
//       return;
//     case "font-size": {
//       const n = finite(value);
//       if (n !== undefined && n > 0) draft.fontSize = n;
//       return;
//     }
//     case "background":
//     case "foreground":
//     case "selection-background":
//     case "cursor-color": {
//       const color = hexColor(value);
//       if (color === undefined) return;
//       if (key === "background") draft.background = color;
//       else if (key === "foreground") draft.foreground = color;
//       else if (key === "selection-background") draft.selectionBackground = color;
//       else draft.cursorColor = color;
//       return;
//     }
//     case "palette": {
//       const entry = paletteEntry(value);
//       if (entry !== undefined) draft.palette[entry.index] = entry.color;
//       return;
//     }
//     case "cursor-style":
//       if (CURSOR_STYLES.has(value)) draft.cursorStyle = value as TerminalCursorStyle;
//       return;
//     case "cursor-style-blink":
//       if (value === "true") draft.cursorStyleBlink = true;
//       else if (value === "false") draft.cursorStyleBlink = false;
//       else if (value === "") delete draft.cursorStyleBlink;
//       return;
//     case "window-padding-x": {
//       const p = padding(value);
//       if (p !== undefined) draft.windowPaddingX = { left: p[0], right: p[1] };
//       return;
//     }
//     case "window-padding-y": {
//       const p = padding(value);
//       if (p !== undefined) draft.windowPaddingY = { top: p[0], bottom: p[1] };
//       return;
//     }
//     case "background-opacity": {
//       const n = finite(value);
//       if (n !== undefined) draft.backgroundOpacity = Math.min(1, Math.max(0, n));
//       return;
//     }
//     case "background-blur":
//     case "background-blur-radius": {
//       const n = blur(value);
//       if (n !== undefined) draft.backgroundBlur = n;
//       return;
//     }
//     default:
//       return;
//   }
// }
// ghostty-config.ts:223
// /** The person's Ghostty config as the pane applies it, or the empty object when no file exists. */
// export async function readGhosttyConfig(host: Host, scheme: TerminalScheme = "dark"): Promise<TerminalConfig> {
//   const files: string[] = [];
//   const own: GhosttyDirective[] = [];
//   const seen = new Set<string>();
//   for (const path of ghosttyConfigPaths(host)) await loadWithIncludes(host, path, seen, files, own);
//   const themeLine = own.filter(d => d.key === "theme").at(-1);
//   const theme = themeLine === undefined ? undefined : themeFor(themeLine.value, scheme);
//   const draft: Draft = { fontFamily: [], palette: Array<TerminalRgb | null>(16).fill(null) };
//   if (theme !== undefined && theme !== "") {
//     for (const path of themePaths(host, theme)) {
//       const text = await host.fs.readText(path);
//       if (text === undefined) continue;
//       files.push(path);
//       for (const directive of parseGhosttyDirectives(text)) if (directive.key !== "theme" && directive.key !== "config-file") apply(draft, directive);
//       break;
//     }
//   }
//   for (const directive of own) apply(draft, directive);
//   return { files, ...draft, ...(theme !== undefined && theme !== "" ? { theme } : {}) };
// }
/// The answer is an ordered JSON map in the TS object's key order (coordinator's ruling on #2446): `files`,
/// `fontFamily`, `palette`, then each applied key in the order it was first set (theme file first, then the config),
/// a key that `cursor-style-blink =` deleted going to the end when set again, and `theme` last. Its keys and values
/// are wsp-wire's `TerminalConfig` fields in camelCase, so `serde_json::from_value` reads it into that struct.
pub async fn read_ghostty_config(host: &Host, scheme: TerminalScheme) -> serde_json::Map<String, serde_json::Value> {
    todo!()
}
