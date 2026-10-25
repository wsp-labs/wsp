// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/terminal.md: terminal.ts on the edge inputs of the wave-2 audit (group f).
import { it } from "vitest";
import { alacrittyFont, detectTerminalFont, ghosttyFont, itermFont, kittyFont, weztermFont } from "../../../../packages/collect/src/detect/terminal.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args)) });
const table = (f: (...a: never[]) => unknown, rows: [string, string][]): Case[] => rows.map(([n, t]) => pure(n, f, t));
const onHost = async (name: string, laptop: Laptop): Promise<Case> => {
  const h = host(laptop);
  const r = await settle(() => detectTerminalFont(h));
  return { name, laptop, args: [], ...r, calls: h.calls };
};

const plist = (guid: string | undefined, books: [string, string | undefined][]): string =>
  `<plist><dict>${guid === undefined ? "" : `\n\t<key>Default Bookmark Guid</key>\n\t<string>${guid}</string>`}\n\t<key>New Bookmarks</key>\n\t<array>${books.map(([g, f]) => `\n\t\t<dict>\n\t\t\t<key>Guid</key>\n\t\t\t<string>${g}</string>${f === undefined ? "" : `\n\t\t\t<key>Normal Font</key>\n\t\t\t<string>${f}</string>`}\n\t\t</dict>`).join("")}\n\t</array>\n</dict></plist>`;
const HACK_TOML = '[font.normal]\nfamily = "Hack"\n';
const ALA = "~/.config/alacritty/alacritty.toml";
const KITTY = "~/.config/kitty/kitty.conf";
const PLIST = "~/Library/Preferences/com.googlecode.iterm2.plist";

it("terminal fixtures", async () => {
  const fixture: Record<string, Case[]> = {};

  fixture["kitty_font"] = table(kittyFont, [
    ["bare font_family takes the next line", "font_family\nfont_size 12\n"], ["bare, blank line, then the value", "font_family\n\n  Hack\n"], ["bare at end of file", "font_family\n"], ["bare with spaces", "font_family   \n"],
    ["comment tail kept", "font_family Fira Code # nice\n"], ["first match only: generic then real", "font_family monospace\nfont_family Hack\n"], ["first match only: real then generic", "font_family Hack\nfont_family monospace\n"],
    ["indented", "   font_family  Hack  \n"], ["tab", "font_family\tHack\n"], ["CRLF", "font_family Hack\r\nfont_size 12\r\n"], ["family empty quotes", 'font_family family="" style="x"\n'],
    ["family unquoted with style", "font_family family=Hack style=Bold\n"], ["family with spaces around =", 'font_family family = "Iosevka Term"\n'], ["bold_font_family", "bold_font_family X\n"],
    ["font_family_extra", "font_family_extra X\n"], ["capitals", "Font_Family X\n"], ["no word boundary before family=", "font_family monofamily=Hack\n"], ["family= later in the value", "font_family Fira Code family=Hack\n"],
    ["family in single quotes is cut at the space", "font_family family='Hack Nerd' style=Bold\n"], ["leading blank lines", "\n\n\nfont_family Hack\n"], ["quoted value", 'font_family "Fira Code"\n'],
    ["a lone quote", 'font_family "\n'], ["empty text", ""], ["existing: mismatched quotes", "font_family \"Fira Code'\n"], ["existing: MONO", "font_family 'MONO'\n"],
  ]);

  fixture["wezterm_font"] = table(weztermFont, [
    ["weight before family", 'config.font = wezterm.font({ weight = "Bold", family = "Fira Code" })\n'], ["apostrophe inside double quotes", 'config.font = wezterm.font("Hack\'s")\n'],
    ["generic first, the later real one is never tried", 'config.font = wezterm.font("monospace")\nconfig.font = wezterm.font("Fira Code")\n'], ["real first", 'config.font = wezterm.font("Fira Code")\nconfig.font = wezterm.font("Hack")\n'],
    ["long bracket comment is not stripped", '--[=[\nconfig.font = wezterm.font("Old")\n]=]\nconfig.font = wezterm.font("New")\n'], ["dashes inside a name", 'config.font = wezterm.font("Fira--Code")\n'],
    ["block comment never closed", '--[[\nconfig.font = wezterm.font("Old")\n'], ["block comments closed on one line", '--[[ a ]] config.font = wezterm.font("X") --[[ b ]]\n'],
    ["mixed quotes", "config.font = wezterm.font('Fira Code\")\n"], ["font_size string", 'config.font_size = "14"\n'], ["bare font string", 'config.font = "Fira"\n'], ["fontconfig call", 'fontconfig("X")\n'],
    ["font = { family }", 'font = { family = "Hack" }\n'], ["font_with_fallback nested family", 'config.font = wezterm.font_with_fallback({ { family = "A B" } })\n'],
    ["font_with_fallback family not the first key", 'config.font = wezterm.font_with_fallback({ { weight = "Bold", family = "A" } })\n'], ["CRLF and a line comment", 'config.font = wezterm.font("Fira Code")\r\n-- c\r\n'],
    ["empty text", ""], ["name padded with spaces", 'config.font = wezterm.font(" Fira ")\n'], ["name of spaces only", 'config.font = wezterm.font("  ")\n'], ["Monospace", 'config.font = wezterm.font("Monospace")\n'],
    ["escaped quotes in the Lua", 'config.font = wezterm.font("Fira \\"x\\" Code")\n'], ["myfont", 'myfont("Hack")\n'], ["subfont", 'config.subfont("Hack")\n'], ["call split over lines", 'config.font = wezterm.font(\n  "Hack"\n)\n'],
  ]);

  fixture["alacritty_font"] = table(alacrittyFont, [
    ["family under the next table", '[font.normal]\nstyle = "Regular"\n[font.bold]\nfamily = "Bold"\n'], ["array before the family", '[font.normal]\nstyle = ["a"]\nfamily = "Hack"\n'],
    ["single quotes", "[font.normal]\nfamily = 'Fira Code'\n"], ["inline normal with no header", 'normal = { family = "Hack" }\n'], ["inline normal under another table", '[colors]\nnormal = { family = "X" }\n'],
    ["generic family in toml hides a real yaml one", '[font.normal]\nfamily = "mono"\nfont:\n  normal:\n    family: Real\n'], ["generic family in toml", '[font.normal]\nfamily = "mono"\n'],
    ["yaml style before family", "font:\n  normal:\n    style: Regular\n    family: Hack\n"], ["yaml: the bold family leaks", "font:\n  normal:\n    style: R\n  bold:\n    family: Bold\n"],
    ["yaml: normal family first", "font:\n  normal:\n    family: N\n  bold:\n    family: B\n"], ["yaml comment tail", "font:\n  normal:\n    family: Hack # c\n"], ["yaml single quotes", "font:\n  normal:\n    family: 'Hack Nerd'\n"],
    ["yaml no final newline", "font:\n  normal:\n    family: Hack"], ["yaml normal with trailing spaces", "font:\n  normal:   \n    family: Hack\n"], ["yaml empty family takes the next line", "font:\n  normal:\n    family:\n    style: x\n"],
    ["first toml family wins", '[font.normal]\nfamily = "A"\n[font.bold]\nfamily = "B"\n'], ["toml spaces", '[font.normal]\n  family   =   "A B"  \n'], ["toml comment line", '[font.normal]\n# family = "Old"\nfamily = "New"\n'],
    ["commented header still matches", '# [font.normal]\nfamily = "X"\n'], ["CRLF toml", '[font.normal]\r\nfamily = "Hack"\r\n'], ["CRLF yaml", "font:\r\n  normal:\r\n    family: Hack\r\n"],
    ["empty text", ""], ["normal.family spaced", '[font]\nnormal.family   =   "Hack"\n'], ["normal.family under another table", '[other]\nnormal.family = "X"\n'], ["header with spaces", '[ font.normal ]\nfamily = "X"\n'],
    ["existing: [font] size only", "[font]\nsize = 13\n"],
  ]);

  fixture["ghostty_font"] = table(ghosttyFont, [
    ["a generic last family resets the list", "font-family = Hack\nfont-family = monospace\n"], ["an empty quoted family resets the list", 'font-family = Hack\nfont-family = ""\nfont-family = Fira\n'],
    ["quoted", 'font-family = "Fira Code"\n'], ["single quoted", "font-family = 'Fira'\n"], ["empty value then a family", "font-family =\nfont-family = X\n"], ["no value", "font-family\n"],
    ["Mono then a real family", "font-family = Mono\nfont-family = Real\n"], ["a real family then Mono", "font-family = Hack\nfont-family = Mono\n"], ["indented", "  font-family = Spaced\n"], ["no spaces", "font-family=Tight\n"],
  ]);

  fixture["iterm_font"] = table(itermFont, [
    ["default bookmark has no font, an earlier bookmark has one", plist("B", [["A", "Monaco 12"], ["B", undefined]])], ["default bookmark has no font, a later bookmark has one", plist("A", [["A", undefined], ["B", "Menlo 11"]])],
    ["default last", plist("B", [["A", "Monaco 12"], ["B", "Menlo 11"]])], ["a space after the size keeps the size", plist("A", [["A", "Monaco 12 "]])], ["decimal size", plist("A", [["A", "Fira Code 13.5"]])],
    ["no size", plist("A", [["A", "Menlo-Regular"]])], ["digits only", plist("A", [["A", "12"]])], ["12pt is not a size", plist("A", [["A", "Hack 12pt"]])], ["no entity decoding", plist("A", [["A", "Fira &amp; Co 12"]])],
    ["guid with regex characters is literal", plist("B(1).*", [["A", "Monaco 12"], ["B(1).*", "Menlo 11"]])], ["a dot in the guid is not a wildcard", plist("B.", [["A", "Monaco 12"], ["BX", "Menlo 11"], ["B.", "Hack 10"]])],
    ["generic font", plist("A", [["A", "monospace 12"]])], ["no default, first Normal Font is generic: no fallback", plist(undefined, [["A", "monospace 12"], ["B", "Menlo 11"]])], ["empty font string", plist("A", [["A", ""]])],
    ["two spaces before the size", plist("A", [["A", "Fira  Code   13"]])], ["a tab before the size", plist("A", [["A", "Hack\t12"]])], ["a name with leading digits", plist("A", [["A", "3270 Nerd Font 14"]])],
    ["only the last number goes", plist("A", [["A", "Source Code Pro 2 12"]])], ["non-ASCII name", plist("A", [["A", "Café Mono 12"]])], ["no default key: the whole text is searched", plist(undefined, [["A", undefined], ["B", "Menlo 11"]])],
    ["empty default guid", "<key>Default Bookmark Guid</key><string></string><key>Guid</key><string></string><key>Normal Font</key><string>X 1</string>"],
    ["no whitespace between tags", "<key>Default Bookmark Guid</key><string>A</string><key>Guid</key><string>A</string><key>Normal Font</key><string>Hack 9</string>"],
    ["the first bookmark of a repeated guid is used", "<key>Default Bookmark Guid</key><string>A</string><key>Guid</key><string>A</string><key>Normal Font</key><string>One 1</string><key>Guid</key><string>B</string><key>Guid</key><string>A</string><key>Normal Font</key><string>Two 2</string>"],
    ["Normal Font before the Guid key", "<key>Default Bookmark Guid</key><string>A</string><key>Normal Font</key><string>Before 1</string><key>Guid</key><string>A</string><key>Name</key><string>n</string>"],
    ["no keys at all", "<plist><dict></dict></plist>"],
  ]);

  const WEZ = 'config.font = wezterm.font("Fira Code")\n';
  fixture["detect_terminal_font"] = [
    await onHost("own kitty with no config: alacritty's config answers after every other read", { terminal: "kitty", files: { [ALA]: HACK_TOML } }),
    await onHost("own kitty with no config: an installed Ghostty answers", { terminal: "kitty", files: { "/Applications/Ghostty.app/": 5 } }),
    await onHost("own kitty with no config: WezTerm.app is no installed check", { terminal: "kitty", files: { "/Applications/WezTerm.app/": 5 } }),
    await onHost("no own terminal: WezTerm.app alone gives nothing", { files: { "/Applications/WezTerm.app/": 5 } }),
    await onHost("own alacritty with nothing at all", { terminal: "alacritty" }),
    await onHost("an empty ~/.wezterm.lua is readable and wins over the second path", { terminal: "WezTerm", files: { "~/.wezterm.lua": "", "~/.config/wezterm/wezterm.lua": WEZ } }),
    await onHost("an empty kitty.conf is a config with no font", { files: { [KITTY]: "" } }),
    await onHost("an empty kitty.conf then alacritty", { files: { [KITTY]: "", [ALA]: HACK_TOML } }),
    await onHost("ghostty config whose list resets to nothing is the built-in", { files: { "~/.config/ghostty/config": "font-family = Hack\nfont-family = monospace\n" } }),
    await onHost("own ghostty with a list that resets", { terminal: "ghostty", files: { "~/.config/ghostty/config": "font-family = Hack\nfont-family = monospace\n" } }),
    await onHost("a terminal name that matches two rows: the first in table order, the other's config is never read", { terminal: "ghostty-kitty", files: { [KITTY]: "font_family Fira\n" } }),
    await onHost("an unknown terminal is no terminal", { terminal: "Apple_Terminal", files: { [KITTY]: "font_family Fira\n" } }),
    await onHost("an empty terminal is not own", { terminal: "", files: { [KITTY]: "font_family Fira\n" } }),
    await onHost("own iTerm2 on linux falls to kitty and runs no defaults", { terminal: "iTerm.app", platform: "linux", files: { [KITTY]: "font_family Fira\n" } }),
    await onHost("iTerm2 plist present, defaults answers empty", { terminal: "iTerm.app", files: { [PLIST]: 5 }, exec: { "defaults export com.googlecode.iterm2 -": "" } }),
    await onHost("iTerm2 plist present, defaults fails", { terminal: "iTerm.app", files: { [PLIST]: 5 } }),
    await onHost("iTerm2 with no plist", { terminal: "iTerm.app" }),
    await onHost("linux ghostty with an empty config file", { platform: "linux", terminal: "ghostty", files: { "~/.config/ghostty/config": "" } }),
    await onHost("a Mac ignores PATH for Ghostty", { which: ["ghostty"] }),
    await onHost("linux ignores the Mac's Ghostty.app folder", { platform: "linux", files: { "/Applications/Ghostty.app/": 5 } }),
    await onHost("own WezTerm with no config on linux: Ghostty on PATH is not asked", { terminal: "WezTerm", platform: "linux", which: ["ghostty"] }),
    await onHost("own wezterm whose config says monospace is the built-in", { terminal: "wezterm", files: { "~/.config/wezterm/wezterm.lua": 'config.font = wezterm.font("monospace")\n' } }),
    await onHost("own kitty with a generic font falls to Ghostty's config", { terminal: "kitty", files: { [KITTY]: "font_family monospace\n", "~/.config/ghostty/config": "font-family = Hack\n" } }),
    await onHost("an uppercase terminal name matches", { terminal: "WEZTERM" }),
    await onHost("alacritty path order: the first readable file of the four", { files: { "~/.alacritty.yml": "font:\n  normal:\n    family: Y\n", "~/.alacritty.toml": '[font.normal]\nfamily = "T"\n', "~/.config/alacritty/alacritty.yml": "font:\n  normal:\n    family: CY\n" } }),
    await onHost("alacritty: the first readable file names no font and later files are not tried", { files: { "~/.alacritty.toml": "[font]\nsize = 1\n", "~/.config/alacritty/alacritty.yml": "font:\n  normal:\n    family: CY\n" } }),
    await onHost("a WezTerm config with no font beats kitty's config", { files: { "~/.wezterm.lua": "return {}\n", [KITTY]: "font_family Fira\n" } }),
    await onHost("a Mac with a kitty config and an iTerm2 plist: iTerm2 comes before kitty in table order", { files: { [PLIST]: 4000, [KITTY]: "font_family Fira\n" }, exec: { "defaults export com.googlecode.iterm2 -": plist("A", [["A", "Menlo 11"]]) } }),
  ];

  write("terminal", fixture);
});
