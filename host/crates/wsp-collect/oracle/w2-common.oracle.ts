// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/common.md: common.ts on the edge inputs of the wave-2 audit (group e).
// The TS fake host logs no stat, so a found/exists case's "calls" is what the fake logged (an empty list).
// row takes the TS RowSpec as JSON; entry takes the EntrySpec; item takes the spec without paths and bytes; present
// takes rows with null where the TS has undefined.
import { it } from "vitest";
import { entry, exists, firstLine, found, item, present, row } from "../../../../packages/collect/src/detect/common.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args)) });
const onHost = async (name: string, laptop: Laptop, f: (h: ReturnType<typeof host>) => unknown, args: unknown[]): Promise<Case> => {
  const h = host(laptop);
  const r = await settle(() => f(h));
  return { name, laptop, args, ...r, calls: h.calls };
};

const SMALL: Laptop = { files: { "~/.zshrc": 10 } };
const BIG: Laptop = { files: { "~/.zshrc": 10, "~/.config/x/": 300, "~/dir/a": 7, "~/dir/b": 8, "~/dir/": 100, "/etc/abs": 4, "~/empty": 0, "~/a b": 3, "~/.hidden/": 0 } };
const ROW = { rung: "shell", id: "shell/x", label: "x" };

it("common fixtures", async () => {
  const fixture: Record<string, Case[]> = {};

  fixture["found"] = [];
  const f = async (name: string, laptop: Laptop, paths: string[]): Promise<void> => void fixture["found"]!.push(await onHost(name, laptop, h => found(h, paths), [paths]));
  await f("a bare tilde is the home folder", SMALL, ["~"]);
  await f("tilde slash is not", SMALL, ["~/"]);
  await f("tilde user, no slash, relative, trailing slash", SMALL, ["~.zshrc", "~/.zshrc/", ".zshrc", "~user/x"]);
  await f("absolute home spelling", SMALL, ["/Users/dev/.zshrc"]);
  await f("a repeated candidate counts twice", BIG, ["~/.zshrc", "~/.zshrc", "~/.zshrc"]);
  await f("a file, a missing one and an absolute one", BIG, ["~/missing", "~/.zshrc", "/etc/abs", "~/a b"]);
  await f("empty file and empty dir marker", BIG, ["~/empty", "~/.hidden"]);
  await f("a dir sums its files", BIG, ["~/dir/a", "~/dir/b", "~/.config/x"]);
  await f("nothing to stat", SMALL, []);
  await f("dot dot is not resolved", SMALL, ["~/../dev/.zshrc"]);

  fixture["exists"] = [];
  for (const [n, l, p] of [["bare tilde", SMALL, "~"], ["tilde slash", SMALL, "~/"], ["tilde user", SMALL, "~user"], ["file", SMALL, "~/.zshrc"], ["absolute home", SMALL, "/Users/dev"], ["absolute home with slash", SMALL, "/Users/dev/"], ["relative", SMALL, ".zshrc"], ["absolute file", SMALL, "/Users/dev/.zshrc"], ["missing", SMALL, "~/.bashrc"], ["root", SMALL, "/"], ["empty file", BIG, "~/empty"], ["dir marker", BIG, "~/.hidden"]] as [string, Laptop, string][]) {
    fixture["exists"].push(await onHost(n, l, h => exists(h, p), [p]));
  }

  fixture["row"] = [];
  const r = async (name: string, laptop: Laptop, spec: Record<string, unknown>): Promise<void> => void fixture["row"]!.push(await onHost(name, laptop, h => row(h, spec as never), [spec]));
  await r("none", BIG, { ...ROW, paths: ["~/nope"] });
  await r("always, nothing found", BIG, { ...ROW, paths: ["~/nope"], always: true });
  await r("always with volatile", BIG, { ...ROW, paths: ["~/nope", "~/.zshrc"], always: true, volatile: ["~/.zshrc", "~/nope"] });
  await r("volatile keeps its own order and duplicates", BIG, { ...ROW, paths: ["~/nope", "~/.zshrc", "~/dir"], volatile: ["~/dir", "~/nope", "~/.zshrc", "~/dir"] });
  await r("duplicate volatile", BIG, { ...ROW, paths: ["~/.zshrc"], volatile: ["~/.zshrc", "~/.zshrc"] });
  await r("duplicate paths", BIG, { ...ROW, paths: ["~/.zshrc", "~/.zshrc"], volatile: ["~/.zshrc"] });
  await r("every optional field, empty strings kept", BIG, { ...ROW, paths: ["~/.zshrc"], group: "G", default: "skip", reason: "", required: false, linux: "no", version: "", arch: "", detail: "", volatile: [] });
  await r("every optional field set", BIG, { ...ROW, paths: ["~/.zshrc"], group: "G", default: "skip", reason: "r", required: true, linux: "yes", version: "1.2", arch: "arm64", detail: "d", volatile: ["~/.zshrc"] });
  await r("zero byte file", BIG, { ...ROW, paths: ["~/empty"] });
  await r("volatile by the absolute spelling is not on the row", BIG, { ...ROW, paths: ["~/.zshrc"], volatile: ["/Users/dev/.zshrc"] });
  await r("a dir and a file", BIG, { ...ROW, group: "Editors", paths: ["~/.nvimrc", "~/dir"] });

  const E = { rung: "tools", id: "tools/x", label: "x" };
  fixture["entry"] = [
    pure("volatile order and duplicates", entry, { ...E, paths: ["~/a", "~/b"], bytes: 1, volatile: ["~/b", "~/a", "~/b"] }),
    pure("volatile by absolute spelling", entry, { ...E, paths: ["~/a"], bytes: 1, volatile: ["/Users/dev/a"] }),
    pure("paths not deduped", entry, { ...E, paths: ["~/a", "~/a"], bytes: 1, volatile: ["~/a"] }),
    pure("every field", entry, { ...E, paths: ["~/a"], bytes: 1, detail: "d", arch: "a", version: "v", linux: "yes", required: true, reason: "r", default: "skip", group: "g", volatile: ["~/a"] }),
    pure("bare", entry, { ...E, paths: [], bytes: 0 }),
    pure("required false", entry, { ...E, paths: [], bytes: 0, required: false }),
    pure("empty strings", entry, { ...E, paths: [], bytes: 0, reason: "", version: "", arch: "", detail: "", group: "" }),
    pure("empty volatile", entry, { ...E, paths: ["~/a"], bytes: 0, volatile: [] }),
  ];
  fixture["item"] = [
    pure("default and version", item, { ...E, default: "skip", version: "1" }),
    pure("volatile is ignored", item, { ...E, volatile: ["~/a"] }),
    pure("group and linux", item, { ...E, group: "Homebrew", linux: "yes" }),
  ];
  const rowA = { rung: "tools", id: "tools/a", label: "a", paths: [], bytes: 0, default: "bring" };
  const rowB = { rung: "tools", id: "tools/b", label: "b", paths: [], bytes: 0, default: "skip" };
  fixture["present"] = [pure("drops the absent, keeps order", present, [undefined, rowB, undefined, rowA, undefined]), pure("all absent", present, [undefined, undefined]), pure("empty", present, [])];

  const fl = (rows: (string | undefined)[]): Case[] => rows.map(t => pure(JSON.stringify(t) ?? "undefined", firstLine, t));
  fixture["first_line"] = fl([undefined, "", " ", "\n", "\nv1", "v1", " v1 ", "v1\r\n", "v1\rv2\nv3", "a\rb\nc", "\r", "\r\nv1", " x \n", " ", " x ", "﻿x﻿", "﻿", "x᠎", "x​", "\t\u000bx\u000c\n", "   \nv1", "v1\n", "v1\n\nv2", "\u0085x", "  /opt/homebrew  \nsecond"]);

  write("common", fixture);
});
