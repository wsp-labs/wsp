// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/brew_bottles.md: brew-bottles.ts on the edge inputs of the wave-2 audit (group a).
// render_snapshot: the TS renders a TypeScript module, the Rust crate renders its JSON file (spec rule 3), so the
// recorded `out` is JSON.stringify(snapshot, null, 2) + "\n", which is serde_json's pretty form; it is not a TS output.
import { it } from "vitest";
import { LINUX_BOTTLES } from "../../../../packages/collect/src/data/linux-bottles.js";
import { classifyFormulae, linuxSupport } from "../../../../packages/collect/src/brew-bottles.js";
import { write } from "./w2-host.js";

const b = (files: unknown) => ({ bottle: { stable: { files } } });
const all = { all: {} };

it("brew_bottles fixtures", () => {
  const classify: [string, unknown][] = [
    ["a tag counts when its key is present, whatever its value", [{ name: "a", ...b({ all: null }) }, { name: "b", ...b({ x86_64_linux: 0 }) }, { name: "c", ...b({ arm64_linux: "" }) }, { name: "d", ...b({ all: false }) }]],
    ["files as an array", [{ name: "a", ...b(["all"]) }]], ["files as null", [{ name: "a", ...b(null) }]], ["files as a string", [{ name: "a", ...b("all") }]],
    ["stable as an array", [{ name: "a", bottle: { stable: [] } }]], ["bottle as an array", [{ name: "a", bottle: [] }]], ["bottle as null", [{ name: "a", bottle: null }]],
    ["files directly under bottle", [{ name: "a", bottle: { files: all } }]],
    ["tags that are not the three", [{ name: "a", ...b({ arm64_linux_x: {}, linux: {}, ALL: {}, "x86_64_linux ": {} }) }]],
    ["another bottle key beside a valid stable", [{ name: "a", full_name: "b", bottle: { stable: { files: all }, other: { files: all } } }]],
    ["a numeric name", [{ name: 7, ...b(all) }]], ["a null name", [{ name: null }]], ["an empty name", [{ name: "", ...b(all) }]],
    ["an array item and a string item", [["a"], "jq"]],
    ["duplicates stay, in both lists", [{ name: "a", ...b(all) }, { name: "a" }, { name: "a", ...b(all) }]],
    ["names sort by UTF-16 units", ["b", "B", "a", "A", "é", "e", "Z", "z", "_", "0", "~", "😀", "Ａ", "", "a b", "a-b", "a@1", "a.1", "ab"].map(name => ({ name }))],
    ["data as an object", { name: "a" }], ["data as null", null], ["data as a string", "x"], ["data as an empty array", []],
  ];
  const snap = { fetched: "x", source: "y", linux: ["a", ""], noLinux: ["a", "b", "constructor"] };
  const support = (name: string) => ({ name: `${JSON.stringify(name)} against a snapshot holding a, "" (linux) and a, b, constructor (no linux)`, args: [name, snap], out: linuxSupport(name, snap) });
  const render = (name: string, s: unknown) => ({ name, args: [s], out: `${JSON.stringify(s, null, 2)}\n` });
  write("brew_bottles", {
    classify_formulae: classify.map(([name, data]) => ({ name, args: [data], out: classifyFormulae(data) })),
    linux_support: ["a", "b", "", "constructor", "__proto__", "toString", "A", " a", "a ", "hasOwnProperty", "c"].map(support),
    linux_support_embedded: ["gh", "jq", "mas", "ca-certificates", "python@3.12", "oven-sh/bun/bun", "homebrew/core/jq", "GH"].map(name => ({ name: `${name} against the embedded snapshot`, args: [name], out: linuxSupport(name) })),
    render_snapshot: [
      render("quotes, backslash, control, unicode, DEL, U+2028, tab, newline", { fetched: 'f"', source: "s\\", linux: ['a"b', "c\\d", "e\u0001", "é😀", "\u007f", " ", "tab\t", "n\n"], noLinux: [] }),
      render("empty lists and strings", { fetched: "", source: "", linux: [], noLinux: [] }),
    ],
    embedded_table: [{ name: "counts and ends of the shipped table", out: { linux: LINUX_BOTTLES.linux.length, noLinux: LINUX_BOTTLES.noLinux.length, fetched: LINUX_BOTTLES.fetched, firstLinux: LINUX_BOTTLES.linux[0], lastNoLinux: LINUX_BOTTLES.noLinux.at(-1) } }],
  });
});
