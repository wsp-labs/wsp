// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/detect_small.md on the edge inputs of the wave-2 audit (group c): presence, toolchains, shell and identity.
// presence_of cases: args [config_paths, bin].
import { it } from "vitest";
import { detectIdentity } from "../../../../packages/collect/src/detect/identity.js";
import { presenceOf } from "../../../../packages/collect/src/detect/presence.js";
import { detectShell, loginShell } from "../../../../packages/collect/src/detect/shell.js";
import { detectToolchains } from "../../../../packages/collect/src/detect/toolchains.js";
import { host, settle, write, type Laptop } from "./w2-host.js";

const hc = async (name: string, laptop: Laptop, f: (h: ReturnType<typeof host>) => unknown, args: unknown[] = []) => {
  const h = host(laptop);
  return { name, laptop, args, ...(await settle(() => f(h))), calls: h.calls };
};
const G = (k: string): string => `git config --global --get ${k}`;

it("detect_small fixtures", async () => {
  const tc = (h: ReturnType<typeof host>) => detectToolchains(h);
  const sh = (h: ReturnType<typeof host>) => detectShell(h);
  const id = (h: ReturnType<typeof host>) => detectIdentity(h);

  const allShell: Record<string, number> = {};
  for (const n of ["zshrc", "zshenv", "zprofile", "zlogin", "bashrc", "bash_profile", "profile", "inputrc", "aliases", "zsh_aliases"]) allShell[`~/.${n}`] = 10;
  Object.assign(allShell, { "~/.config/fish/config.fish": 1, "~/.config/sheldon/p.toml": 2, "~/.config/starship.toml": 3, "~/.p10k.zsh": 4, "~/.config/oh-my-posh/t.json": 5, "~/.tmux.conf": 6, "~/.config/tmux/t.conf": 7, "~/.config/zellij/c.kdl": 8, "~/.config/direnv/d": 9, "~/.direnvrc": 11, "~/.oh-my-zsh/oh-my-zsh.sh": 100, "~/.oh-my-zsh/custom/a.zsh": 12, "~/.local/share/zinit/z": 1, "~/.zinit/z": 1, "~/.zplug/z": 1, "~/.antidote/a": 1, "~/.zsh_plugins.txt": 13, "~/.config/fish/functions/fisher.fish": 5, "~/.config/fish/fish_plugins": 14, "~/.zsh_history": 15, "~/.bash_history": 16, "~/.local/share/fish/fish_history": 17, "~/.local/share/atuin/h.db": 18 });

  const goV = (line: string): Laptop => ({ which: ["go"], exec: { "go version": line } });
  const sshFiles = { "~/.ssh/.DS_Store": 6148, "~/.ssh/authorized_keys2": 5, "~/.ssh/authorized_keys": 5, "~/.ssh/config.bak": 7, "~/.ssh/known_hosts.old": 3, "~/.ssh/agent.sock.1": 0, "~/.ssh/agent.sock": 0, "~/.ssh/controlmasters/x": 1, "~/.ssh/id.pub.bak": 4, "~/.ssh/rc": 1, "~/.ssh/environment": 1, "~/.ssh/allowed_signers": 1, "~/.ssh/known_hosts.pub": 2, "~/.ssh/my_known_hosts_backup": 9, "~/.ssh/id_ed25519": 11, "~/.ssh/id_ed25519.pub": 12, "~/.ssh/Config": 13, "~/.ssh/config": 14 };
  const gitOn = (exec: Record<string, string>, files: Record<string, number> = {}): Laptop => ({ which: ["git"], exec, files });

  write("detect_small", {
    presence_of: [
      await hc("duplicates counted twice, input spelling kept", { files: { "~/.b": 5, "~/.c/x": 7 }, which: ["t"] }, h => presenceOf(h, { configPaths: ["~/.a", "~/.b", "~/.c", "~/.b"], bin: "t" }), [["~/.a", "~/.b", "~/.c", "~/.b"], "t"]),
      await hc("nothing", {}, h => presenceOf(h, { configPaths: [], bin: "t" }), [[], "t"]),
      await hc("bin only", { which: ["t"] }, h => presenceOf(h, { configPaths: [], bin: "t" }), [[], "t"]),
      await hc("absolute path and an empty bin", { files: { "/abs/p": 3 } }, h => presenceOf(h, { configPaths: ["/abs/p", "~"], bin: "" }), [["/abs/p", "~"], ""]),
      await hc("paths without the bin", { files: { "~/.b": 5, "~/.c/x": 7 } }, h => presenceOf(h, { configPaths: ["~/.a", "~/.b", "~/.c"], bin: "none" }), [["~/.a", "~/.b", "~/.c"], "none"]),
      await hc("no path, no bin", { files: { "~/.b": 5 } }, h => presenceOf(h, { configPaths: ["~/.a"], bin: "none" }), [["~/.a"], "none"]),
    ],
    detect_toolchains: [
      await hc("everything present: order, shared pins, sums, label asks", { files: { "~/.config/mise/config.toml": 300, "~/.tool-versions": 40, "~/.asdf/x": 1, "~/.asdfrc": 2, "~/.node-version": 8, "~/.nvmrc": 3, "~/.nvm/alias/default": "lts/iron\n", "~/.pyenv/version": 6, "~/.config/uv/uv.toml": 20, "~/.python-version": 5, "~/.rustup/settings.toml": 50, "~/.cargo/config.toml": 30, "~/.config/go/env": 12 }, which: ["go", "mise", "fnm", "uv", "pyenv", "asdf"], exec: { "fnm current": "v22.12.0\n", "go version": "go version go1.23.1 darwin/arm64\n" } }, tc),
      await hc("pins alone make no manager except through a marker dir", { files: { "~/.node-version": 8, "~/.python-version": 5, "~/.cargo/config.toml": 3, "~/.config/go/env": 1, "~/.pyenv/version": 1, "~/.asdfrc": 1, "~/.tool-versions": 1, "~/.nvmrc": 1 } }, tc),
      await hc("binaries alone make rows with no paths", { which: ["mise", "fnm", "go", "uv", "asdf", "pyenv"] }, tc),
      await hc("fnm says none, go gives no version", { which: ["fnm", "go"], exec: { "fnm current": "none\n", "go version": "command not found\n" } }, tc),
      await hc("blank first lines give the bare labels", { which: ["fnm", "go"], exec: { "fnm current": "\n", "go version": "\ngo version go1.2\n" }, files: { "~/.nvm/alias/default": "\n20\n" } }, tc),
      await hc("CRLF and padding in answers", { which: ["fnm", "go"], exec: { "fnm current": "  v20.1.0  \r\nv18\n", "go version": "go version go1.24.0 linux/amd64\r\n" }, files: { "~/.nvm/alias/default": "v20\r\n" } }, tc),
      await hc("go devel", goV("go version devel go1.24-abc123 Tue Jan 1 00:00:00 2025 +0000 linux/amd64\n"), tc),
      await hc("go rc", goV("go version go1.22rc1 linux/amd64\n"), tc),
      await hc("go major only", goV("go version go1 linux/amd64\n"), tc),
      await hc("go with no digits", goV("go version go linux/amd64\n"), tc),
      await hc("go with a dangling dot", goV("go version go1.23. x\n"), tc),
      await hc("go gives no answer", { which: ["go"] }, tc),
      await hc("nvm alias is an empty file", { files: { "~/.nvm/alias/default": "", "~/.nvm/x": 1 } }, tc),
      await hc("nvm home is a plain file", { files: { "~/.nvm": 5 } }, tc),
      await hc("rustup dir without settings.toml", { files: { "~/.rustup/toolchains/x": 5 } }, tc),
      await hc("a directory pin sums its tree", { files: { "~/.tool-versions/a": 7, "~/.tool-versions/b": 8 }, which: ["mise"] }, tc),
      await hc("mise brings its config and tool-versions", { files: { "~/.config/mise/config.toml": 300, "~/.tool-versions": 40 }, which: ["mise"] }, tc),
      await hc("nothing installed", {}, tc),
    ],
    detect_shell: [
      await hc("the whole ladder: order, labels, sums, login", { files: { ...allShell, "/etc/shells": "/bin/zsh\n" }, shell: "/bin/zsh" }, sh),
      await hc("bare managers and oh-my-zsh without custom", { files: { "~/.oh-my-zsh/oh-my-zsh.sh": 100, "~/.antidote/a": 1, "~/.config/fish/functions/fisher.fish": 5, "~/.zinit/z": 1 }, shell: "/bin/zsh" }, sh),
      await hc("SHELL empty, /etc/shells listing /bin/zsh", { files: { "~/.zshrc": 1, "/etc/shells": "/bin/zsh\n" }, shell: "" }, sh),
      await hc("SHELL empty, no /etc/shells", { files: { "~/.zshrc": 1 }, shell: "" }, sh),
      await hc("etc/shells with a comment, padding and CRLF", { files: { "~/.zshrc": 1, "/etc/shells": "# /bin/fish\n  /bin/zsh  \r\n/bin/bash\r\n" }, shell: "/bin/zsh" }, sh),
      await hc("SHELL without a slash", { files: { "~/.zshrc": 1, "/etc/shells": "zsh\n" }, shell: "zsh" }, sh),
      await hc("SHELL under /usr/bin, list says /bin", { files: { "~/.zshrc": 1, "/etc/shells": "/bin/zsh\n" }, shell: "/usr/bin/zsh" }, sh),
      await hc("SHELL with a trailing slash", { files: { "~/.zshrc": 1, "/etc/shells": "/bin/zsh/\n" }, shell: "/bin/zsh/" }, sh),
      await hc("an item row gets the login too", { files: { "~/.zplug/z": 1, "/etc/shells": "/bin/zsh\n" }, shell: "/bin/zsh" }, sh),
      await hc("history alone: bash", { files: { "~/.bash_history": 5 } }, sh),
      await hc("history alone: fish", { files: { "~/.local/share/fish/fish_history": 5 } }, sh),
      await hc("directories and files are one path each", { files: { "~/.config/fish/a/b": 5, "~/.config/tmux": 3 } }, sh),
      await hc("rc files by name", { files: { "~/.zprofile": 10, "~/.zlogin": 11, "~/.zsh_aliases": 12 } }, sh),
      await hc("empty laptop", {}, sh),
    ],
    login_shell: [
      await hc("listed", { shell: "/bin/zsh", files: { "/etc/shells": "/bin/zsh" } }, h => loginShell(h)),
      await hc("listed after a BOM", { shell: "/bin/zsh", files: { "/etc/shells": "﻿/bin/zsh\n" } }, h => loginShell(h)),
      await hc("listed with a trailing space", { shell: "/bin/zsh", files: { "/etc/shells": "/bin/zsh \n" } }, h => loginShell(h)),
      await hc("case differs", { shell: "/bin/zsh", files: { "/etc/shells": "/bin/Zsh\n" } }, h => loginShell(h)),
      await hc("a trailing comment on the line", { shell: "/bin/zsh", files: { "/etc/shells": "/bin/zsh # c\n" } }, h => loginShell(h)),
      await hc("only a commented line", { shell: "/bin/zsh", files: { "/etc/shells": "# /bin/zsh\n" } }, h => loginShell(h)),
      await hc("fish by full path", { shell: "/opt/homebrew/bin/fish", files: { "/etc/shells": "/bin/zsh\n/opt/homebrew/bin/fish\n" } }, h => loginShell(h)),
      await hc("no SHELL", { files: { "/etc/shells": "/bin/zsh\n" } }, h => loginShell(h)),
      await hc("empty SHELL, no /etc/shells", { shell: "" }, h => loginShell(h)),
      await hc("empty SHELL, trailing newline in the list", { shell: "", files: { "/etc/shells": "/bin/zsh\n" } }, h => loginShell(h)),
      await hc("SHELL by bare name", { shell: "zsh", files: { "/etc/shells": "zsh\n" } }, h => loginShell(h)),
    ],
    detect_identity: [
      await hc("git on PATH, nothing answered, a gitconfig", gitOn({}, { "~/.gitconfig": 5 }), id),
      await hc("no git on PATH: no asks at all", { files: { "~/.gitconfig": 5 }, exec: { [G("user.name")]: "A\n" } }, id),
      await hc("a name and no config file", gitOn({ [G("user.name")]: "A\n" }), id),
      await hc("blank answers are no answers", gitOn({ [G("user.name")]: "\n", [G("user.email")]: "  \n" }, { "~/.gitconfig": 5 }), id),
      await hc("both config files in constant order", gitOn({}, { "~/.config/git/config": 7, "~/.gitconfig": 5 }), id),
      await hc("commit.gpgsign false still counts", gitOn({ [G("commit.gpgsign")]: "false\n" }), id),
      await hc("commit.gpgsign answered skips user.signingkey", gitOn({ [G("commit.gpgsign")]: "true\n", [G("user.signingkey")]: "ABC\n" }), id),
      await hc("user.signingkey alone", gitOn({ [G("user.signingkey")]: "ABC\n" }), id),
      await hc("ssh format, signers file absent", gitOn({ [G("gpg.format")]: "ssh\n", [G("gpg.ssh.allowedSignersFile")]: "~/.ssh/allowed_signers\n" }), id),
      await hc("ssh format, no signers answer", gitOn({ [G("gpg.format")]: "ssh\n" }), id),
      await hc("ssh format, absolute signers file", gitOn({ [G("gpg.format")]: "ssh\n", [G("gpg.ssh.allowedSignersFile")]: "/etc/signers\n" }, { "/etc/signers": 9 }), id),
      await hc("ssh format, tilde signers file present", gitOn({ ...{ [G("user.name")]: "Dev\n", [G("user.email")]: "d@x\n" }, [G("gpg.format")]: "ssh\n", [G("gpg.ssh.allowedSignersFile")]: "~/.ssh/allowed_signers\n" }, { "~/.gitconfig": 512, "~/.ssh/allowed_signers": 90 }), id),
      await hc("x509 format: signers not asked", gitOn({ [G("gpg.format")]: "x509\n", [G("gpg.ssh.allowedSignersFile")]: "~/.ssh/allowed_signers\n" }, { "~/.ssh/allowed_signers": 9 }), id),
      await hc("openpgp format", gitOn({ [G("gpg.format")]: "openpgp\n" }), id),
      await hc("SSH in upper case is not ssh", gitOn({ [G("gpg.format")]: "SSH\n", [G("gpg.ssh.allowedSignersFile")]: "~/.ssh/allowed_signers\n" }, { "~/.ssh/allowed_signers": 9 }), id),
      await hc("odd names under .ssh", { files: sshFiles }, id),
      await hc("only ssh config", { files: { "~/.ssh/config": 14 } }, id),
      await hc("gnupg as a file", { files: { "~/.gnupg": 5 } }, id),
      await hc("a zero-byte private key", { files: { "~/.ssh/id_rsa": 0 } }, id),
      await hc("public keys are one required row", { files: { "~/.ssh/id_ed25519.pub": 100, "~/.ssh/work.pub": 120, "~/.ssh/authorized_keys": 300, "~/.ssh/agent.sock": 0 } }, id),
      await hc("empty laptop", {}, id),
    ],
  });
});
