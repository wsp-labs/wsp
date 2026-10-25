// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/small.md: host.ts, ssh-config.ts and cache-names.ts on the edge inputs of the wave-2 audit (group a).
import { it } from "vitest";
import { CACHE_DIRS, FINDER_METADATA } from "../../../../packages/collect/src/cache-names.js";
import { expand, tilde } from "../../../../packages/collect/src/host.js";
import { SSH_CONFIG_PATH, SSH_IGNORE_UNKNOWN_DETAIL, withIgnoreUnknown } from "../../../../packages/collect/src/ssh-config.js";
import { write } from "./w2-host.js";

const H = "/Users/dev";
const PRE = "# wsp: ssh on the machine does not know every option yours does, and one unknown option would stop it reading the file.\nIgnoreUnknown *\n";

it("small fixtures", () => {
  const e = (home: string, path: string) => ({ name: `home ${JSON.stringify(home)}, path ${JSON.stringify(path)}`, args: [home, path], out: expand({ home }, path) });
  const t = (home: string, path: string) => ({ name: `home ${JSON.stringify(home)}, path ${JSON.stringify(path)}`, args: [home, path], out: tilde(home, path) });
  const w = (name: string, text: string) => ({ name, args: [text], out: withIgnoreUnknown(text) });
  write("small", {
    expand: [...["~", "~/", "~/x", "~/~/x", "~x", "~/../x", "x/~", "", "/", " ~", "~\\", "~//x", "~/ "].map(p => e(H, p)), e("/Users/dev/", "~/x"), e("", "~/x"), e("", "~"), e("/", "~/x")],
    tilde: [...["/Users/dev", "/Users/dev/", "/Users/dev//x", "/Users/dev/x/", "/Users/DEV/x", "/a/Users/dev/x", "Users/dev/x", "", "~", "/Users/dev/~/x", "/Users/dev/.", "/Users/dev/.."].map(p => t(H, p)),
      t("", ""), t("", "/x"), t("/", "/x"), t("/", "/"), t("/Users/dev/", "/Users/dev/x"), t("/Users/dev/", "/Users/dev/"), t("/Users/dev/", "/Users/dev")],
    with_ignore_unknown: [
      w("the prelude alone", PRE), w("the prelude then text", `${PRE}Host x\n`), w("the prelude twice", PRE + PRE),
      w("the prelude without its final newline", PRE.slice(0, -1)), w("a blank line before the prelude", `\n${PRE}`), w("a BOM before the prelude", `﻿${PRE}`),
      w("the prelude with CRLF line ends", PRE.replace(/\n/g, "\r\n")), w("a BOM before text", "﻿Host x\n"), w("CRLF text", "Host x\r\n  User me\r\n"),
      w("text with no final newline", "Host x"), w("unicode text", "Host é😀\n"), w("the prelude's comment line alone", PRE.split("\n")[0] + "\n"), w("empty text", ""),
    ],
    constants: [
      { name: "SSH_CONFIG_PATH", out: SSH_CONFIG_PATH }, { name: "SSH_IGNORE_UNKNOWN_DETAIL", out: SSH_IGNORE_UNKNOWN_DETAIL }, { name: "PRELUDE", out: PRE },
      { name: "FINDER_METADATA", out: FINDER_METADATA }, { name: "CACHE_DIRS in order", out: [...CACHE_DIRS] },
    ],
  });
});
