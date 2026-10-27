// SPDX-License-Identifier: AGPL-3.0-only
// gh as it printed in a pty, played over a scripted link.
import type { FakePty, FakePtyLink } from "./fake-pty-link.js";

/** gh 2.103.2 in a pty on a fresh home (no git credential helper), the bytes it printed in the order it printed
 * them; its prompt sends a cursor position query and reads no key before the terminal answers. The code is dead.
 * The pinned 2.101.0 prints the same bytes without the "Did you know" line, which is what the docs link is. */
const GH_RECORDED = [
  "\x1b]11;?\x1b\\\x1b[6n\x1b[0;33m!\x1b[0m Did you know that expiring tokens can be requested with \x1b[0;1;39m--expiring\x1b[0m? Read more at https://gh.io/gh-expiring-tokens\r\n\x1b[0G\x1b[2K\x1b[0;1;92m? \x1b[0m\x1b[0;1;99mAuthenticate Git with your GitHub credentials? \x1b[0m\x1b[0;39m(Y/n) \x1b[0m\x1b[?25l\x1b7\x1b[999;999f\x1b[6n",
  "\x1b8\x1b[?25h\x1b[6n",
  "\r\n\x1b[1B\x1b[0G\x1b[1A\x1b[0G\x1b[0G\x1b[2K\x1b[0;1;92m? \x1b[0m\x1b[0;1;99mAuthenticate Git with your GitHub credentials? \x1b[0m\x1b[0;36mYes\x1b[0m\r\n\r\n\x1b[0;31m!\x1b[0m Failed to copy one-time code to clipboard\r\n  No clipboard utilities available. Please install xsel, xclip, wl-clipboard or Termux:API add-on for termux-clipboard-get/set.\r\n\x1b[0;33m!\x1b[0m First copy your one-time code: \x1b[0;1;39m1290-DB7E\x1b[0m\r\n\x1b[0;1;39mPress Enter\x1b[0m to open https://github.com/login/device in your browser... ",
];

/** The recording played as gh plays it: the question and its first query when started, a third query after two
 * answers, and the page and code only once all three are answered and a Return has been typed on that pty. Answers
 * the pty the caller hands to the returned function; the link's other ptys are left alone. */
export function ghOn(link: FakePtyLink): (pty: FakePty) => void {
  let flow: FakePty | undefined;
  let replies = 0;
  let returned = false;
  let shown = false;
  const wrote = link.op;
  link.op = async (op, extra) => {
    const out = await wrote(op, extra);
    if (op !== "pty.write" || flow === undefined || extra?.["ptyId"] !== flow.id) return out;
    const data = String(extra["data"]);
    const answered = (data.match(/\x1b\[\d+;\d+R/g) ?? []).length;
    replies += answered;
    if (answered === 0 && data.includes("\r")) returned = true;
    if (answered > 0 && replies === 2) link.data(flow, GH_RECORDED[1]!);
    if (!shown && replies >= 3 && returned) {
      shown = true;
      link.data(flow, GH_RECORDED[2]!);
    }
    return out;
  };
  return pty => {
    if (flow !== undefined || pty.ran === undefined) return;
    flow = pty;
    link.data(pty, GH_RECORDED[0]!);
  };
}
