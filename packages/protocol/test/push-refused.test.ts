// SPDX-License-Identifier: AGPL-3.0-only
// Why a sub-thread's push was refused, read off the push's own output, which a
// not pushed strip says in place of asking the agent to push again.
import { describe, expect, it } from "vitest";
import { isGitPush, pushRefusedOf } from "../src/index.js";

describe("pushRefusedOf", () => {
  it("names the remote's own words over git's fatal line, measured on git 2.43 against github.com and a hook", () => {
    expect(pushRefusedOf("remote: Permission to acme/web.git denied to wsp-bot.\nfatal: unable to access 'https://github.com/acme/web.git/': The requested URL returned error: 403")).toBe("Permission to acme/web.git denied to wsp-bot");
    expect(pushRefusedOf("remote: protected branch        \nTo ../bare\n ! [remote rejected] HEAD -> main (pre-receive hook declined)\nerror: failed to push some refs to '../bare'")).toBe("protected branch");
  });

  it("falls back to the rejected ref, then git's fatal or error line, skipping a remote's progress", () => {
    expect(pushRefusedOf("To github.com:acme/web.git\n ! [rejected]        HEAD -> main (fetch first)\nerror: failed to push some refs\nhint: Updates were rejected")).toBe("[rejected] HEAD -> main (fetch first)");
    expect(pushRefusedOf("remote: Enumerating objects: 5, done.\nremote: Total 3 (delta 0)\nfatal: Authentication failed for 'https://github.com/acme/web.git/'")).toBe("fatal: Authentication failed for 'https://github.com/acme/web.git/'");
    expect(pushRefusedOf("Everything up-to-date")).toBeUndefined();
  });

  it("knows a git push among the commands an agent runs", () => {
    expect(isGitPush('{"command":"git push -u origin wsp/fix"}')).toBe(true);
    expect(isGitPush("git -C /root/web push origin HEAD")).toBe(true);
    expect(isGitPush("git status && git log")).toBe(false);
    expect(isGitPush("echo push")).toBe(false);
    // Claude Code's input is JSON: its description is words about the call, not the call.
    expect(isGitPush(JSON.stringify({ command: "git status", description: "Check the tree before git push" }))).toBe(false);
    expect(isGitPush(JSON.stringify({ command: "git push -u origin wsp/fix", description: "Push the branch" }))).toBe(true);
  });
});
