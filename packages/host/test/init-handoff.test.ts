// SPDX-License-Identifier: AGPL-3.0-only
// The sign-in hand-off over a scripted daemon link: the page the tool printed,
// the code beside it and the command that opens it are said once and printed as
// JSON, the tool's own status is asked on the machine until the person is
// through, and a deadline that passes ends the row instead of the run.
import { PassThrough } from "node:stream";
import { stripVTControlCharacters } from "node:util";
import type { ManifestEntry } from "@wsp/collect";
import { SIGN_IN_DEFERRED_WORD } from "@wsp/protocol";
import { describe, expect, it } from "vitest";
import { CALLBACK_DISPLAY, cadence, codeIn, signInEnv } from "../src/init-handoff.js";
import { SIGN_IN_CAP_MS, SignInCodes, type SignInFlow } from "../src/init-signin.js";
import { signInFor } from "../src/signin-table.js";
import { fakePtyLink, type FakePty, type FakePtyLink } from "./fake-pty-link.js";
import { ASKED, loginOf } from "./signin-questions.js";

const GH: ManifestEntry = { rung: "logins", id: "logins/gh", label: "GitHub CLI login", group: "CLI logins", paths: [], bytes: 0, default: "bring", choice: "machine" };
const CLAUDE: ManifestEntry = { rung: "logins", id: "logins/claude", label: "Claude Code login", group: "Agent logins", paths: [], bytes: 0, default: "bring", choice: "copy" };
const KUBE: ManifestEntry = { rung: "logins", id: "logins/kube", label: "kubeconfig", group: "CLI logins", paths: [], bytes: 0, default: "bring", choice: "machine" };
const GCLOUD: ManifestEntry = { rung: "logins", id: "logins/gcloud", label: "Google Cloud login", group: "CLI logins", paths: [], bytes: 0, default: "bring", choice: "machine" };
const DEVICE = "https://github.com/login/device";
const CODE = "8F4A-C21B";
/** gcloud's own two pages (client id cut, state and challenge placeholders): with a browser to reach it redirects to
 * a port on the machine, and with none it redirects to the page that shows a code to paste. */
const GCLOUD_PAGE =
  "https://accounts.google.com/o/oauth2/auth?response_type=code&client_id=32555940559&redirect_uri=http%3A%2F%2Flocalhost%3A8085%2F&scope=openid+email&state=S&code_challenge=C&code_challenge_method=S256";
const GCLOUD_PASTE =
  "https://accounts.google.com/o/oauth2/auth?response_type=code&client_id=32555940559&redirect_uri=https%3A%2F%2Fsdk.cloud.google.com%2FauthCode.html&scope=openid+email&state=S&code_challenge=C&code_challenge_method=S256";
/** aws sso's page: its redirect comes back to the machine without naming a port, which its listener gives later. */
const AWS_BARE = "https://d-1234.awsapps.com/start/authorize?response_type=code&redirect_uri=http%3A%2F%2F127.0.0.1%2Foauth%2Fcallback&state=S";
/** Shaped like a Google authorization code; nothing real. */
const PASTED = "4/0AfakeCodeFromThePage";
/** The page the machine asks the host to open, which returns through the forwarded port. */
const PAGE = "https://github.com/login/oauth/authorize?redirect_uri=http%3A%2F%2Flocalhost%3A42485%2Fcallback";
const BANNER = "https://cli.github.com/upgrade";
const BUILDER = { id: "m_builder", name: "default" };
const GH_LOGIN = loginOf("gh");
const AWS: ManifestEntry = { rung: "logins", id: "logins/aws", label: "AWS SSO login", group: "CLI logins", paths: [], bytes: 0, default: "bring", choice: "machine" };
const DOPPLER: ManifestEntry = { rung: "logins", id: "logins/doppler", label: "Doppler login", group: "CLI logins", paths: [], bytes: 0, default: "bring", choice: "machine" };
const DOPPLER_PAGE = "https://dashboard.doppler.com/workplace/auth/cli";
const GH_CODE = (() => {
  const s = signInFor("gh");
  return s.kind === "device" ? s.code : undefined;
})();

/** gh's device flow on a builder: the code and its page, the login command then blocking on the person. The
 * status command answers not-signed-in until `after` checks have been made. */
function ghLink(o: { after: number; hold?: boolean; exitCode?: number }): FakePtyLink {
  const link = fakePtyLink();
  let checks = 0;
  link.script = (pty: FakePty, line: string) => {
    if (line.includes("WSP_STATUS")) {
      checks += 1;
      const on = checks > o.after;
      link.data(pty, `${on ? "github.com\n  ✓ Logged in to github.com account Zingzy" : "You are not logged into any GitHub hosts"}\r\nWSP_STATUS ${on ? 0 : 1}\r\n`);
      link.exit(pty, 0);
      return;
    }
    link.data(pty, `! First copy your one-time code: ${CODE}\r\nOpen this page to continue: ${DEVICE}\r\nWaiting for you to sign in...\r\n`);
    if (o.hold !== true) link.exit(pty, o.exitCode ?? 0);
  };
  return link;
}

describe("the code beside a page", () => {
  it("reads a device code in the shape the row names, never out of the page's own query string, and nothing at all for a row with no shape", () => {
    expect(GH_CODE).toBeInstanceOf(RegExp);
    expect(codeIn(`! First copy your one-time code: ${CODE}\nOpen ${DEVICE}`, GH_CODE)).toBe(CODE);
    expect(codeIn(`visit: https://claude.com/oauth/authorize?state=ABCD-1234&code=true\nPaste code here > `, GH_CODE)).toBeUndefined();
    expect(codeIn(`Opening ${DEVICE} in your browser`, GH_CODE)).toBeUndefined();
    // claude's row shows no code, so a token of that shape in its output is never announced as one.
    expect(codeIn(`! First copy your one-time code: ${CODE}`, undefined)).toBeUndefined();
  });
});

describe("how often the status is asked", () => {
  it("every pollMs for the first minute, then every twenty seconds or the caller's slower cadence", () => {
    expect(cadence(0, 5_000)).toBe(5_000);
    expect(cadence(59_999, 5_000)).toBe(5_000);
    expect(cadence(60_000, 5_000)).toBe(20_000);
    expect(cadence(15 * 60_000, 5_000)).toBe(20_000);
    expect(cadence(60_000, 30_000)).toBe(30_000);
  });
});

