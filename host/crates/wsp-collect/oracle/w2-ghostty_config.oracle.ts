// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/ghostty_config.md on the edge inputs of the wave-2 audit (group c). hex_color is not exported by the TS.
// read_ghostty_config cases: args [scheme], out_text is JSON.stringify of the answer (key order kept), calls the fake host's log.
import { it } from "vitest";
import { ghosttyConfigPaths, parseGhosttyDirectives, readGhosttyConfig, themeFor } from "../../../../packages/collect/src/ghostty-config.js";
import { host, plain, settle, write, type Laptop } from "./w2-host.js";

const C = "~/.config/ghostty/config";
const BUNDLED = "/Applications/Ghostty.app/Contents/Resources/ghostty/themes";

const read = async (name: string, laptop: Laptop, scheme: "light" | "dark" = "dark") => {
  const h = host(laptop);
  const r = await settle(() => readGhosttyConfig(h, scheme));
  const answer = "out" in r ? { out_text: JSON.stringify("out" in r ? await readOnce(laptop, scheme) : null) } : { error: r.error };
  return { name, laptop, args: [scheme], ...answer, calls: h.calls };
};
const readOnce = async (laptop: Laptop, scheme: "light" | "dark") => readGhosttyConfig(host(laptop), scheme);
const line = (name: string, text: string) => read(name, { files: { [C]: `${text}\n` } });

it("ghostty fixtures", async () => {
  const texts = ["font-size = 12 # big", "font-family = \"a\" \"b\"", "font-family = \"", "font-family = \"\"", "font-family = \" \"", "font-family = \"  Menlo  \"", "font-family = \"A B\"", "font-family = \"A\rB\"", "Font-Size = 12", "a\n﻿b = c\n﻿", "k = v\rk2 = v2", "= =", "k==v", "k = \"a\" ", "k = \"\"x\"\"", "  =\n=x", "﻿font-size = 12", "a=b=c", " = x", "key =", "k = \"q\"", "k = \"a\"b\"", "k = 'a'", "#c = d", "  # c", "k=v\r", "k\t=\tv", ""];
  const themes = ["Mocha", " Mocha ", "light:A,dark:B", "dark:B", "light:A", "light:A,Mocha", ":x", "a:b:c", "light: A , dark: B ", "light:,dark:B", "", "Foo: Bar", "light:A,light:B,dark:C", "a,b", "light:A;dark:B"];

  const reads = [
    await read("no file", {}),
    await read("CRLF, BOM, key with spaces", { files: { [C]: "﻿font-size = 14\r\nfont-family = \"Fira Code\"\r\n  background   =   #102030  \r\n" } }),
    await read("xdg trailing slash is normalised when loading", { files: { "/Users/dev/cfg/ghostty/config": "font-size = 9" }, xdgConfigHome: "/Users/dev/cfg/" }),
    await read("empty XDG_CONFIG_HOME is used as is", { files: { "/ghostty/config": "font-size = 9", [C]: "font-size = 8" }, xdgConfigHome: "" }),
    await read("xdg dir changes the pair on linux", { platform: "linux", files: { "/cfg/ghostty/config.ghostty": "font-size = 20\n", "/cfg/ghostty/config": "font-size = 10\n", [C]: "font-size = 1\n" }, xdgConfigHome: "/cfg" }),
    await read("an include missing twice is read twice", { files: { [C]: "config-file = ?gone\nconfig-file = gone\nfont-size = 5\n" } }),
    await read("include double unquote", { files: { [C]: "config-file = \"\"b\"\"\nconfig-file = ?\"\"c\"\"\nconfig-file = \"?\"d\"\"\n", "~/.config/ghostty/b": "font-size = 2", "~/.config/ghostty/c": "font-size = 3", "~/.config/ghostty/d": "font-size = 4" } }),
    await read("include odd values", { files: { [C]: "config-file = ?\nconfig-file = \"\"\nconfig-file = ?\"\"\nconfig-file = ./a/../b\nconfig-file = ~\nconfig-file = ~/h\nconfig-file = ~other/x\nconfig-file = a, b\n", "~/.config/ghostty/b": "font-size = 2", "~/h": "font-size = 3", "~/.config/ghostty/~other/x": "font-size = 4", "~/.config/ghostty/a, b": "font-size = 5" } }),
    await read("include order across the default files", { files: { "~/.config/ghostty/config.ghostty": "font-size = 1\nconfig-file = ~/Library/Application Support/com.mitchellh.ghostty/config\n", "~/.config/ghostty/config": "font-size = 2\n", "~/Library/Application Support/com.mitchellh.ghostty/config.ghostty": "font-size = 3\n", "~/Library/Application Support/com.mitchellh.ghostty/config": "font-size = 4\nconfig-file = ../../../.config/ghostty/config\n" } }),
    await read("includes normalised past the root", { files: { [C]: "config-file = /../../etc/x\nconfig-file = /a/b/../../../c\n", "/etc/x": "font-size = 7", "/c": "font-size = 8" } }),
    await read("an empty last theme line cancels an earlier one", { files: { [C]: "theme = A\ntheme =\n", "~/.config/ghostty/themes/A": "background = #000001\n" } }),
    await read("the theme line of an include wins, light side", { files: { [C]: "theme = A\nconfig-file = m\n", "~/.config/ghostty/m": "theme = light:L,dark:D", "~/.config/ghostty/themes/D": "background = #000009", "~/.config/ghostty/themes/L": "background = #00000a" } }, "light"),
    await read("a theme name with .. is joined as written", { files: { [C]: "theme = ../x\n", "~/.config/ghostty/themes/../x": "background = #010203\n", "~/.config/ghostty/x": "background = #040506\n" } }),
    await read("the first theme file that reads wins even when empty", { files: { [C]: "theme = T\n", "~/.config/ghostty/themes/T": "", [`${BUNDLED}/T`]: "background = #000001\n" } }),
    await read("the theme is the config file itself", { files: { [C]: "theme = /Users/dev/.config/ghostty/config\nfont-size = 3\ncursor-style-blink = true\n" } }),
    await read("linux theme order", { platform: "linux", files: { [C]: "theme = T\n", "/usr/share/ghostty/themes/T": "background = #000003\n", [`${BUNDLED}/T`]: "background = #000005\n", "~/.config/ghostty/themes/T/x": "a" } }),
    await read("linux theme in the local share first", { platform: "linux", files: { [C]: "theme = T\n", "/usr/share/ghostty/themes/T": "background = #000003\n", "/usr/local/share/ghostty/themes/T": "background = #000004\n" } }),
    await read("theme name with spaces is trimmed", { files: { [C]: "theme =   My Theme  \n", "~/.config/ghostty/themes/My Theme": "foreground = #000003\n" } }),
    await read("a theme nobody has keeps its name", { files: { [C]: "theme = T\n" } }),
    await read("theme and config fonts merge, theme first", { files: { [C]: "font-family = Mine\ntheme = T\n", [`${BUNDLED}/T`]: "font-family = FromTheme\nfont-family = Second\nfont-size = 9\ncursor-style-blink = true\n" } }),
    await read("config clears the theme's fonts and blink; fontSize keeps the theme's place", { files: { [C]: "font-family =\ntheme = T\ncursor-style-blink =\nfont-size = 4\n", [`${BUNDLED}/T`]: "font-family = FromTheme\nfont-size = 9\ncursor-style-blink = true\nbackground = #000000\n" } }),
    await read("a re-set key keeps its place", { files: { [C]: "cursor-style-blink = true\nbackground = #000000\ncursor-style-blink = false\n" } }),
    await read("fonts: clear, then add, then re-size", { files: { [C]: "font-family = A\nfont-family =\nfont-family = B\nfont-size = 1\nfont-size = 2\nbackground = #000000\nfont-size = 3\n" } }),
    await read("fonts keep what the value says", { files: { [C]: "font-family = Menlo\nfont-family = \" \"\nfont-family = \"  Padded  \"\nfont-family = Menlo\n" } }),
    await read("an empty quoted family clears", { files: { [C]: "font-family = A\nfont-family = \"\"\nfont-family = B\n" } }),
    await read("a family with =, a lone quote, a quoted pair, a U+2028", { files: { [C]: "font-family = Fira=Code\nfont-family = \"\nfont-family = \"a\" \"b\"\nfont-family = \"A B\"\n" } }),
    await read("two blur keys feed one field", { files: { [C]: "background-blur = 5\nbackground-blur-radius = false\n" } }),
    await read("a bad blur leaves the earlier one", { files: { [C]: "background-blur-radius = 5\nbackground-blur = bad\n" } }),
  ];
  const table = async (key: string, values: string[]) => { for (const v of values) reads.push(await line(`${key} = ${JSON.stringify(v)}`, `${key} = ${v}`)); };
  await table("font-size", ["12", "+12", "-12", ".5", "5.", "1e1", "1E1", "1e400", "0x10", "0X1F", "-0x10", "+0x10", "0b11", "0o7", "0b102", "1_000", " 12 ", "12px", "0", "-0", "0.0000001", "1e21", "123456789012345680000", "1e-7", "0.000001", "1.5e300", "0x7fffffffffffffff", "Infinity", "-Infinity", "NaN", "inf", "infinity", "0x", "1e", "012", "00012", "٣", "1 2", "1,5", " 12", "9007199254740993", "100000000000000000000", "1e20"]);
  await table("window-padding-x", ["-0", "-1", "1,-1", "-1,1", "0x10", "1e1,2", " , 3", "", "1,2", " 1 , 2 ", "Infinity", "1,Infinity", "1;2", "1,,2", "+1", "1e-7", "1,0x2", "1,2,3", "3,", "0", "2 , 4"]);
  await table("window-padding-y", ["2", "3,", "0"]);
  await table("background-opacity", ["-0", "0.5", "2", "-3", "1e-400", "nan", "", "0x1", "50%", ".25", "1e0", "0.1234567890123456789", "1.5", "lots"]);
  await table("background-blur", ["false", "FALSE", "True", "true", "macos-glass-regular", "MACOS-GLASS-CLEAR", "macos-glass-", "macos-glass", "0x10", "1e1", "20.0", "1.5", "-1", "-0", "+5", "", "1e21", "1e30", "99999999999999999999", "9007199254740993", "yes", "0", " 7 "]);
  await table("background-blur-radius", ["3", "false", "1.5"]);
  await table("palette", ["=#112233", " =#112233", "1.0=#111111", "1e1=#111111", " 7 =#111111", "+3=#111111", "0x0F=#111111", "0b1111=#111111", "0o7=#040506", "-0=#111111", "15=#111111", "16=#111111", "0x10=#111111", "08=#111111", "1_0=#111111", "1,5=#111111", "Infinity=#111111", "2 = #aabbcc ", "2=#abc", "2=aabbcc", "2=#AABBCC", "2=##aabbcc", "2", "2=", "0=#111111=#222222", "3=#ff0000 # c", "-1=#111111", "1.5=#111111", " 5=#111111", "x=#0a0b0c", "3=#zzzzzz"]);
  await table("cursor-style", ["Block", "block_hollow", "bar ", "\"bar\"", "block", "underline", "wobble"]);
  await table("cursor-style-blink", ["True", "yes", "", "\"true\"", "true", "false"]);
  for (const key of ["background", "foreground", "selection-background", "cursor-color"]) await table(key, ["#FFF", "#11223344", "112233", "##112233", "\"#112233\"", "#11223g", "#ABCDEF", "abcdef", "cell-foreground", "#abcdef  "]);

  write("ghostty_config", {
    parse_ghostty_directives: texts.map(t => ({ name: JSON.stringify(t), args: [t], out: plain(parseGhosttyDirectives(t)) })),
    theme_for: themes.flatMap(v => (["light", "dark"] as const).map(s => ({ name: `${JSON.stringify(v)} for ${s}`, args: [v, s], out: themeFor(v, s) }))),
    ghostty_config_paths: [
      { laptop: {}, name: "darwin default" }, { laptop: { platform: "linux" }, name: "linux default" }, { laptop: { xdgConfigHome: "" }, name: "empty xdg" }, { laptop: { platform: "linux", xdgConfigHome: "/cfg/" }, name: "xdg with a trailing slash" },
      { laptop: { xdgConfigHome: "/Users/dev/cfg" }, name: "xdg set, darwin" },
    ].map(({ laptop, name }) => ({ name, laptop, args: [], out: ghosttyConfigPaths(host(laptop as Laptop)) })),
    read_ghostty_config: reads,
  });
});
