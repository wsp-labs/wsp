// SPDX-License-Identifier: AGPL-3.0-only
// The other configs a recipe carries to a computer of the person's, read off
// this one with the cuts already made, so the digest a recipe resolves to and
// the bytes that land there are the same bytes. Nothing a login shell under
// root reads is here: bash's own files never travel, since root's home on a
// box is the one every workspace there writes.
import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { RC_NAMES, isSecretName, stripExports } from "@wsp/collect";
import { folderFiles } from "./folder-files.js";
import { secretIn } from "./token-shapes.js";

/** The configs a recipe can tick that land as files. The GitHub row is a vault token and lands none. */
export type ConfigFiles = "git" | "shell";

/** Home-relative files and folders each row reads. git's own credentials file is not among them, and no bash file
 * is: a login bash under /root reads .bashrc, .bash_profile and .profile, and history never travels. */
export const CONFIG_PATHS: Record<ConfigFiles, readonly string[]> = {
  git: [".gitconfig", ".config/git/config", ".config/git/ignore", ".config/git/attributes"],
  shell: [
    ".zshrc",
    ".zshenv",
    ".zprofile",
    ".zlogin",
    ".zsh_aliases",
    ".zsh_plugins.txt",
    ".p10k.zsh",
    ".tmux.conf",
    ".config/starship.toml",
    ".config/fish",
    ".config/tmux",
    ".config/zellij",
    ".config/direnv",
    ".config/oh-my-posh",
    ".oh-my-zsh/custom",
  ],
};

/** Names inside a shell folder that are state rather than config: fish keeps its universal variables, secrets
 * among them, in fish_variables, and a plugin checked out under oh-my-zsh carries its own history. */
export const FISH_VARIABLES = "fish_variables";
const NEVER_NAMES = new Set([FISH_VARIABLES, ".git"]);

/** A file past this is no config a shell reads and stays here. */
const CONFIG_MAX_BYTES = 1024 * 1024;

/** Git sections that never travel: a credential helper is this computer's, a url rewrite points at keys that stay
 * here, an include names a path on this computer, signing needs the key that stays here, and a filter, a pager
 * table or a diff or merge tool is a command root's own git on the box would run. */
const GIT_DROPPED_SECTIONS = /^(credential|url|gpg|include|includeif|filter|pager|difftool|mergetool)$/i;
/** Keys cut out of sections that otherwise travel, by `section.key`: signing, every key whose value git runs, and
 * rerere, which replays a resolution recorded on one copy into a later merge with nobody asked. */
const GIT_DROPPED_KEYS = new Set([
  "core.sshcommand",
  "core.fsmonitor",
  "core.hookspath",
  "core.pager",
  "core.editor",
  "core.askpass",
  "core.gitproxy",
  "sequence.editor",
  "diff.external",
  "interactive.difffilter",
  "user.signingkey",
  "commit.gpgsign",
  "tag.gpgsign",
  "rerere.enabled",
  "rerere.autoupdate",
]);
/** Keys of a named diff or merge driver that git runs, cut whatever the driver's name. */
const GIT_DRIVER_KEYS = new Set(["textconv", "command", "driver"]);
/** git's own password keys whose name glues the word on, from `git help -c`: send-email's and cvsserver's; and http's
 * extra header, whose `Authorization: basic` login is base64 that no token shape matches. */
const GIT_PASSWORD_KEYS = new Set(["sendemail.smtppass", "gitcvs.dbpass", "http.extraheader"]);
/** What a landed .gitconfig asks git for credentials with, so a box with GH_TOKEN in a run's environment clones a
 * private repo and nothing writes the file there afterwards. */
export const GH_CREDENTIAL_HELPER = "!gh auth git-credential";

const GIT_HEADER = /^\s*\[\s*([^\]\s"]+)(\s+"[^"]*")?\s*\]/;

/** What a git config loses on its way: the sections that go whole, whether the keys a box's root git would run go
 * too, and whether the gh helper is added always or only in place of a credential section that went. */
export interface GitCuts {
  sections: RegExp;
  runs: boolean;
  helper: "always" | "replacing";
}

const BOX_GIT_CUTS: GitCuts = { sections: GIT_DROPPED_SECTIONS, runs: true, helper: "always" };
/** A provider's machine keeps the person's git as it is, less what holds or points at a credential: a credential
 * helper, and a url rewrite, whose section name can carry a token. A config that loses nothing lands as it is. */
export const CLOUD_GIT_CUTS: GitCuts = { sections: /^(credential|url)$/i, runs: false, helper: "replacing" };

/** Whether a file is one a shell reads as commands; readline's inputrc and tmux, starship and zellij files are not.
 * `rc` is for a caller that already knows the file is an rc file, which a linked copy's name need not say. */
export const shellScript = (path: string, rc = false): boolean => {
  const name = path.slice(path.lastIndexOf("/") + 1);
  if (/^\.?inputrc$/.test(name)) return false;
  return rc || RC_NAMES.has(name) || /^direnvrc$|\.(?:sh|bash|zsh|zsh-theme|fish)$/.test(name);
};

/** A shell file with every line that holds a secret cut: the variables that held one, and what a cut line that
 * assigns none held. In a `script`, each cut command leaves a `true`. */
export const shellCut = (text: string, script: boolean): { names: string[]; held: string[]; carried: string } => {
  const held: string[] = [];
  return { ...stripExports(text, secretIn, held, script), held };
};

/** A git config with what never travels cut, and on the main file the gh credential helper added; `names` gets each
 * key cut as a secret, by the name git gives it. */
export function gitCut(text: string, main: boolean, cuts: GitCuts = BOX_GIT_CUTS, names: string[] = []): string {
  const out: string[] = [];
  let dropped = false;
  let credential = false;
  let section = "";
  let named = false;
  let sub = "";
  let dropping = false;
  // git reads a key written after its section's header on the same line as that section's key, so it is cut as one.
  const lines = text.split(/\r?\n/).flatMap(line => {
    const header = GIT_HEADER.exec(line)?.[0];
    return header === undefined || line.slice(header.length).trim() === "" ? [line] : [header, line.slice(header.length)];
  });
  for (const line of lines) {
    const header = GIT_HEADER.exec(line);
    if (header !== null) {
      section = header[1]!.toLowerCase();
      named = header[2] !== undefined;
      sub = named ? `.${header[2]!.trim().slice(1, -1)}` : "";
      // A subsection named by an address, as `[http "https://user:pass@host/"]`, can hold the login itself.
      const held = named ? secretIn(header[2]!) : undefined;
      if (held !== undefined) names.push(held);
      dropping = cuts.sections.test(section) || held !== undefined;
      if (dropping && section === "credential") credential = true;
      if (dropping) dropped = true;
      else out.push(line);
      continue;
    }
    if (dropping) continue;
    const pair = /^\s*([A-Za-z][A-Za-z0-9-]*)\s*(?:=\s*(.*))?$/.exec(line);
    const key = pair?.[1]?.toLowerCase();
    const cut = key === undefined ? undefined : gitKeyCut(section, named, key, pair?.[2] ?? "", cuts.runs);
    if (cut !== undefined) {
      if (cut === "secret") names.push(`${section}${sub}.${key}`);
      dropped = true;
      continue;
    }
    out.push(line);
  }
  if (cuts.helper === "replacing" && !dropped) return text;
  while (out.length > 0 && out[out.length - 1]!.trim() === "") out.pop();
  if (main && (cuts.helper === "always" || credential)) out.push("[credential]", `\thelper = ${GH_CREDENTIAL_HELPER}`);
  return `${out.join("\n")}\n`;
}

/** Why one key of a section that travels is cut: a secret always, a key git runs only where `runs` says. */
function gitKeyCut(section: string, named: boolean, key: string, value: string, runs: boolean): "secret" | "runs" | undefined {
  // A secret never lands: a key named like one, or a value holding one.
  if (isSecretName(key) || GIT_PASSWORD_KEYS.has(`${section}.${key}`) || secretIn(value) !== undefined) return "secret";
  if (!runs) return undefined;
  if (!named && GIT_DROPPED_KEYS.has(`${section}.${key}`)) return "runs";
  if (named && (section === "diff" || section === "merge") && GIT_DRIVER_KEYS.has(key)) return "runs";
  // An alias that starts with ! is a shell command.
  return section === "alias" && value.trim().startsWith("!") ? "runs" : undefined;
}

/** One file a config row lands: home-relative, with the bytes that land and the secrets cut from them by name. */
export interface ConfigText {
  rel: string;
  text: string;
  cut: string[];
}

/** Every file under one path, home-relative, in order: the path itself read where it points, what is under it by the
 * rule everything a recipe ships is read by, a folder named above left out. */
function filesUnder(home: string, rel: string): string[] {
  const st = statSync(join(home, rel), { throwIfNoEntry: false });
  if (st?.isFile() === true) return st.size <= CONFIG_MAX_BYTES ? [rel] : [];
  if (st?.isDirectory() !== true) return [];
  return folderFiles(join(home, rel), name => NEVER_NAMES.has(name))
    .files.filter(f => statSync(f.path).size <= CONFIG_MAX_BYTES)
    .map(f => `${rel}/${f.rel}`);
}

/** What one config row lands, read off this computer with its cuts made: the git files with what never travels
 * taken out, the shell files with every exported secret cut. Empty where this computer has none of them. */
export function configTexts(id: ConfigFiles, home: string): ConfigText[] {
  return CONFIG_PATHS[id].flatMap(path =>
    filesUnder(home, path).map(rel => {
      const raw = readFileSync(join(home, rel), "utf8");
      if (id === "shell") {
        const { names, held, carried } = shellCut(raw, shellScript(rel));
        return { rel, text: carried, cut: [...names, ...held] };
      }
      const cut: string[] = [];
      return { rel, text: gitCut(raw, rel === ".gitconfig", BOX_GIT_CUTS, cut), cut };
    }),
  );
}

/** The digest a config row resolves to: its files' paths and landed bytes, in order. */
export function configDigest(texts: readonly ConfigText[]): string {
  const hash = createHash("sha256");
  for (const t of texts) hash.update(`${t.rel}\0${t.text}\0`);
  return texts.length === 0 ? "" : hash.digest("hex");
}
