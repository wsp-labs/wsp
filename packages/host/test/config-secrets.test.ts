// SPDX-License-Identifier: AGPL-3.0-only
// A copied shell file or git config lands no value that holds a secret: a login
// in an address, a signed address's signature or a token's shape, on a box and on
// an image alike, while a plain remote and a long repository name stay.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { type SpawnSyncOptionsWithStringEncoding, execFileSync, spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { gunzipSync } from "node:zlib";
import { afterEach, describe, expect, it } from "vitest";
import { GUARD_BEGIN, GUARD_END, stripExports, type ManifestEntry } from "@wsp/collect";
import { importFor } from "../src/init-import.js";
import { CLOUD_GIT_CUTS, configDigest, configTexts, gitCut, shellScript } from "../src/recipe-configs.js";
import { placeProvisioner } from "../src/place-provision.js";
import { FIXTURE } from "./init-fixture.js";

const dirs: string[] = [];
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

const PLAIN_ZSH = [
  "export PATH=$HOME/bin:$PATH",
  "export REPO=https://github.com/acme/terraform-provider-aws-v5-modules",
  "export ORIGIN=ssh://git@github.com/acme/lab.git",
  "export DB_REF=postgres://lab:${DB_PASS}@db.acme.dev/app",
  "export SAS_REF='https://lab.blob.core.windows.net/c/m.json?sv=2024-11-04&sig=${SAS_SIG}'",
  "alias g=git",
];
const ZSHRC = [
  PLAIN_ZSH[0],
  "export DATABASE_URL=postgres://lab:plain0login@db.acme.dev/app",
  PLAIN_ZSH[1],
  'export MARKET_URL="https://lab.blob.core.windows.net/c/m.json?sv=2024-11-04&sp=r&sig=plain0azure"',
  "export BUCKET_URL='https://lab.s3.us-east-1.amazonaws.com/m.json?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires=3600&X-Amz-Signature=plain0amz'",
  PLAIN_ZSH[2],
  `alias feed="curl -s 'https://storage.googleapis.com/lab/m.json?X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Signature=plain0goog'"`,
  "export LAB_VALUE=ghp_plain0tokenAbCdEfGh1234567890",
  PLAIN_ZSH[3],
  "# psql postgres://lab:plain0comment@db.acme.dev/app",
  PLAIN_ZSH[4],
  PLAIN_ZSH[5],
  "",
].join("\n");
/** ZSHRC as it lands: each command that held a value stands as `true`, the comment that held one is gone. */
const LANDED_ZSH = [PLAIN_ZSH[0], "true", PLAIN_ZSH[1], "true", "true", PLAIN_ZSH[2], "true", "true", PLAIN_ZSH[3], PLAIN_ZSH[4], PLAIN_ZSH[5], ""].join("\n");
const ZSH_VARIABLES = ["DATABASE_URL", "MARKET_URL", "BUCKET_URL", "LAB_VALUE"];
const ZSH_NAMES = [...ZSH_VARIABLES, "X-Goog-Signature", "a login in an address"];

const PLAIN_BASH = ["export REPO_SSH=git@github.com:acme/terraform-provider-aws-v5-modules.git", "alias l='ls -l'"];
const BASHRC = [
  "export DATABASE_URL=mysql://lab:plain0bash@db.acme.dev/app",
  PLAIN_BASH[0],
  "export CDN='https://cdn.acme.dev/m.json?Expires=1767290400&Signature=plain0cdn'",
  "export SLACK_HOOK=xoxb-plain0-1234567890",
  PLAIN_BASH[1],
  "",
].join("\n");

const LANDED_BASH = ["true", PLAIN_BASH[0], "true", "true", PLAIN_BASH[1], ""].join("\n");

const GITCONFIG = [
  "[user]",
  "\tname = Dev",
  '[remote "origin"]',
  "\turl = ssh://git@github.com/acme/terraform-provider-aws-v5-modules.git",
  "\tfetch = +refs/heads/*:refs/remotes/origin/*",
  '[remote "mirror"]',
  "\turl = https://dev@github.com/acme/lab.git",
  "[http]",
  "\tproxy = http://lab:plain0proxy@proxy.acme.dev:3128",
  "[lfs]",
  "\turl = https://lab.s3.us-east-1.amazonaws.com/lfs?X-Amz-Expires=3600&X-Amz-Signature=plain0amz",
  "[alias]",
  "\tfeed = !curl -s 'https://storage.googleapis.com/lab/m.json?X-Goog-Signature=plain0goog'",
  "\tst = status",
  "[lab]",
  "\tvalue = glpat-plain0token1234567890",
  '[remote "azure"]',
  "\turl = https://lab.blob.core.windows.net/c/repo.git?sv=2024-11-04&sig=plain0azure",
  "",
].join("\n");
/** What a plain git config keeps on any road: the user, a remote over ssh with a long repository name, a login with no password. */
const PLAIN_GIT = [
  "[user]",
  "\tname = Dev",
  '[remote "origin"]',
  "\turl = ssh://git@github.com/acme/terraform-provider-aws-v5-modules.git",
  "\tfetch = +refs/heads/*:refs/remotes/origin/*",
  '[remote "mirror"]',
  "\turl = https://dev@github.com/acme/lab.git",
];

function laptop(): string {
  const home = realpathSync(mkdtempSync(join(tmpdir(), "wsp-config-secrets-")));
  dirs.push(home);
  for (const [name, text] of [
    [".zshrc", ZSHRC],
    [".bashrc", BASHRC],
    [".gitconfig", GITCONFIG],
  ] as const) {
    writeFileSync(join(home, name), text);
    chmodSync(join(home, name), 0o644);
  }
  return home;
}

const row = (over: Partial<ManifestEntry> & Pick<ManifestEntry, "rung" | "id">): ManifestEntry => ({ label: over.id, paths: [], bytes: 0, default: "bring", bring: true, ...over });
const ROWS = [
  row({ rung: "shell", id: "shell/zshrc", paths: ["~/.zshrc"] }),
  row({ rung: "shell", id: "shell/bashrc", paths: ["~/.bashrc"] }),
  row({ rung: "identity", id: "identity/git-user", paths: ["~/.gitconfig"] }),
];

function extract(tar: Buffer): string {
  const dir = mkdtempSync(join(tmpdir(), "wsp-config-secrets-x-"));
  dirs.push(dir);
  writeFileSync(join(dir, "a.tgz"), tar);
  execFileSync("tar", ["-xzf", join(dir, "a.tgz"), "-C", dir]);
  return dir;
}

describe("a copied shell file or git config", () => {
  it("on a provider's image lands no login, signature or token, keeps every plain line, and the notes name what stayed", async () => {
    const home = laptop();
    const imp = importFor(ROWS, { home, secrets: new Map(), platform: "darwin", cloud: true });
    const packed = await imp.files!.pack();
    expect(gunzipSync(packed.tar).includes("plain0")).toBe(false);
    const root = extract(packed.tar);
    expect(readFileSync(join(root, ".zshrc"), "utf8")).toBe(LANDED_ZSH);
    expect(readFileSync(join(root, ".bashrc"), "utf8")).toBe(LANDED_BASH);
    expect(readFileSync(join(root, ".gitconfig"), "utf8")).toBe([...PLAIN_GIT, "[http]", "[lfs]", "[alias]", "\tst = status", "[lab]", '[remote "azure"]', ""].join("\n"));
    expect(packed.cut).toEqual([
      { path: "~/.bashrc", names: ["DATABASE_URL", "CDN", "SLACK_HOOK"] },
      { path: "~/.zshrc", names: ZSH_VARIABLES },
    ]);
    expect(packed.blanked).toEqual([
      { id: "shell/bashrc", path: "~/.bashrc", note: "DATABASE_URL, CDN, SLACK_HOOK blanked or left out; the values stay on this computer" },
      { id: "shell/zshrc", path: "~/.zshrc", note: `${ZSH_NAMES.join(", ")} blanked or left out; the values stay on this computer` },
      { id: "identity/git-user", path: "~/.gitconfig", note: "http.proxy, lfs.url, alias.feed, lab.value, remote.azure.url blanked or left out; the values stay on this computer" },
    ]);
  });

  it("on a provider's image cuts a shell file that is no rc file the same way: a fish function, an oh-my-zsh custom file", async () => {
    const home = laptop();
    mkdirSync(join(home, ".config", "fish", "functions"), { recursive: true });
    writeFileSync(join(home, ".config", "fish", "functions", "db.fish"), "function db\n  psql postgres://lab:plain0fish@db.acme.dev/app\nend\n");
    mkdirSync(join(home, ".oh-my-zsh", "custom"), { recursive: true });
    writeFileSync(join(home, ".oh-my-zsh", "custom", "env.zsh"), "export GH_TOKEN=plain0named\nexport MIRROR=https://github.com/acme/lab\n");
    const rows = [row({ rung: "shell", id: "shell/fish", paths: ["~/.config/fish"] }), row({ rung: "shell", id: "shell/oh-my-zsh", paths: ["~/.oh-my-zsh/custom"] })];
    const packed = await importFor(rows, { home, secrets: new Map(), platform: "darwin", cloud: true }).files!.pack();
    expect(gunzipSync(packed.tar).includes("plain0")).toBe(false);
    const root = extract(packed.tar);
    expect(readFileSync(join(root, ".config", "fish", "functions", "db.fish"), "utf8")).toBe("function db\n  true\nend\n");
    expect(readFileSync(join(root, ".oh-my-zsh", "custom", "env.zsh"), "utf8")).toBe("true\nexport MIRROR=https://github.com/acme/lab\n");
    expect(packed.blanked?.map(b => [b.path, b.note])).toEqual([
      ["~/.config/fish/functions/db.fish", "a login in an address blanked or left out; the values stay on this computer"],
      ["~/.oh-my-zsh/custom/env.zsh", "GH_TOKEN blanked or left out; the values stay on this computer"],
    ]);
  });

  it("on an image of a computer the person owns lands no such shell line either, and init asks only for a variable a cut line set", async () => {
    const home = laptop();
    const packed = await importFor(ROWS, { home, secrets: new Map(), platform: "darwin" }).files!.pack();
    const root = extract(packed.tar);
    expect(readFileSync(join(root, ".zshrc"), "utf8")).toBe(LANDED_ZSH);
    expect(readFileSync(join(root, ".bashrc"), "utf8")).toBe(LANDED_BASH);
    expect(packed.cut).toEqual([
      { path: "~/.bashrc", names: ["DATABASE_URL", "CDN", "SLACK_HOOK"] },
      { path: "~/.zshrc", names: ZSH_VARIABLES },
    ]);
    writeFileSync(join(home, ".zshrc"), `export PATH=$HOME/bin:$PATH; export DB=postgres://lab:plain0multi@db/app\nalias blob="curl 'https://lab.blob.core.windows.net/c?sig=plain0azure'"\n`);
    const multi = await importFor(ROWS.slice(0, 1), { home, secrets: new Map(), platform: "darwin" }).files!.pack();
    expect(multi.cut).toEqual([{ path: "~/.zshrc", names: ["DB"] }]);
  });

  it("on a box lands no login, signature or token, keeps every plain line, and each file left lighter is noted", async () => {
    const home = laptop();
    const texts = configTexts("shell", home).concat(configTexts("git", home));
    expect(texts.map(t => t.rel)).toEqual([".zshrc", ".gitconfig"]);
    expect(texts[0]!.text).toBe(LANDED_ZSH);
    expect(texts[1]!.text).toBe([...PLAIN_GIT, "[http]", "[lfs]", "[alias]", "\tst = status", "[lab]", '[remote "azure"]', "[credential]", "\thelper = !gh auth git-credential", ""].join("\n"));
    const provision = placeProvisioner({ statePath: join(home, "state.json"), home, platform: "darwin", collect: async () => FIXTURE, brew: async () => new Map() });
    const picks = { name: "laptop", agents: {}, mcp: {}, clis: {}, skills: {}, plugins: {}, folders: {}, configs: { git: {}, shell: {} } };
    const plan = await provision.setup(picks, { home: "/root" });
    const packed = await plan.configs!.pack();
    expect(gunzipSync(packed.tar).includes("plain0")).toBe(false);
    expect(packed.skipped).toEqual([
      { id: "configs/git", path: "~/.gitconfig", note: "http.proxy, lfs.url, alias.feed, lab.value, remote.azure.url blanked or left out; the values stay on this computer" },
      { id: "configs/shell", path: "~/.zshrc", note: `${ZSH_NAMES.join(", ")} blanked or left out; the values stay on this computer` },
    ]);
  });

  it("leaves out a section whose address holds a login, on a box and on a provider's image", () => {
    const git = ['[http "https://lab:plain0header@proxy.acme.dev/"]', "\tsslVerify = false", "\tproxy = http://lab:plain0proxy@proxy.acme.dev:3128", ...PLAIN_GIT, ""].join("\n");
    for (const cuts of [undefined, CLOUD_GIT_CUTS]) {
      const names: string[] = [];
      expect(gitCut(git, false, cuts, names)).toBe(`${PLAIN_GIT.join("\n")}\n`);
      expect(names).toEqual(["a login in an address"]);
    }
  });

  it("keeps a plain remote and a long repository name on a box and on a provider's image", () => {
    const git = [...PLAIN_GIT, '[remote "lab"]', "\turl = https://github.com/acme/terraform-provider-aws-v5-modules.git", ""].join("\n");
    expect(gitCut(git, false)).toBe(git);
    expect(gitCut(git, false, CLOUD_GIT_CUTS)).toBe(git);
  });

  it("a heredoc's line that holds a login is left out, and the heredoc still closes", () => {
    const home = laptop();
    writeFileSync(join(home, ".zshrc"), "cat > ~/.pgpass.example <<EOF\npostgres://lab:plain0heredoc@db/app\nhost=db\nEOF\nalias g=git\n");
    expect(configTexts("shell", home)[0]!.text).toBe("cat > ~/.pgpass.example <<EOF\nhost=db\nEOF\nalias g=git\n");
  });

  it("a secret behind a precommand word, a declaration builtin or fish's and, or and not is cut like a bare export, and a command behind one stays, on a box and on an image", async () => {
    const home = laptop();
    const posix = [
      "time export A_TOKEN=plain0time",
      "time -p export B_TOKEN=plain0timep",
      "noglob export C_TOKEN=plain0noglob",
      "nocorrect export D_TOKEN=plain0nocorrect",
      "- export E_TOKEN=plain0dash",
      "builtin export F_TOKEN=plain0builtin",
      "command export G_TOKEN=plain0command",
      "command -p export H_TOKEN=plain0commandp",
      "noglob builtin typeset -x I_TOKEN=plain0both",
      "typeset -x J_TOKEN=plain0typeset",
      "declare -x K_TOKEN=plain0declare",
      "readonly L_TOKEN=plain0readonly",
      "db() { local -x M_TOKEN=plain0local; }",
      "integer -x N_KEY=plain0integer",
      "float -x O_KEY=plain0float",
      "env P_TOKEN=plain0env make",
      "time Q_TOKEN=plain0prefix make",
      "noglob export LAB_VALUE=ghp_plain0tokenAbCdEfGh1234567890",
      "time make",
      "noglob git status",
      "nocorrect git log",
      "command -v git",
      "export AFTER=1",
      "",
    ].join("\n");
    const fish = [
      "and set -gx GH_TOKEN plain0and",
      "or set -x LAB_TOKEN plain0or",
      "not set -gx API_KEY plain0not",
      "set --global --export LAB_KEY plain0long",
      "if not set -q GH_TOKEN; echo none; end",
      "and set -gx LAB_VALUE ghp_plain0tokenAbCdEfGh1234567890",
      "and git status",
      "time make",
      "set -gx AFTER 1",
      "",
    ].join("\n");
    writeFileSync(join(home, ".zshrc"), posix);
    writeFileSync(join(home, ".bashrc"), posix);
    mkdirSync(join(home, ".config", "fish"), { recursive: true });
    writeFileSync(join(home, ".config", "fish", "config.fish"), fish);
    const box = Object.fromEntries(configTexts("shell", home).map(t => [t.rel, t.text]));
    const landed: [string, string | undefined, string][] = [
      ["a box's ~/.zshrc", box[".zshrc"], "posix"],
      ["a box's config.fish", box[".config/fish/config.fish"], "fish"],
    ];
    const rows = [...ROWS.slice(0, 2), row({ rung: "shell", id: "shell/fish", paths: ["~/.config/fish"] })];
    for (const cloud of [false, true]) {
      const packed = await importFor(rows, { home, secrets: new Map(), platform: "darwin", cloud }).files!.pack();
      expect(packed.cut?.find(c => c.path === "~/.zshrc")?.names).toEqual([..."ABCDEFGHIJKLM".split("").map(l => `${l}_TOKEN`), "N_KEY", "O_KEY", "P_TOKEN", "Q_TOKEN", "LAB_VALUE"]);
      const root = extract(packed.tar);
      const where = cloud ? "a provider's" : "an owned";
      // An owned image's rc opens with a guard block for the commands the image lacks; only the cut is checked here.
      const cut = (rc: string): string => readFileSync(join(root, rc), "utf8").replace(new RegExp(`^${GUARD_BEGIN}\\n[^]*?${GUARD_END}\\n\\n`), "");
      for (const rc of [".zshrc", ".bashrc"]) landed.push([`${where} image's ~/${rc}`, cut(rc), "posix"]);
      landed.push([`${where} image's config.fish`, readFileSync(join(root, ".config", "fish", "config.fish"), "utf8"), "fish"]);
    }
    const want = {
      posix: [
        "time true",
        "time -p true",
        "noglob true",
        "nocorrect true",
        "- true",
        "builtin true",
        "command true",
        "command -p true",
        "noglob builtin true",
        "true",
        "true",
        "true",
        "db() { true; }",
        "true",
        "true",
        "true",
        "time true",
        "noglob true",
        "time make",
        "noglob git status",
        "nocorrect git log",
        "command -v git",
        "export AFTER=1",
        "",
      ].join("\n"),
      fish: ["and true", "or true", "not true", "true", "if not set -q GH_TOKEN; echo none; end", "and true", "and git status", "time make", "set -gx AFTER 1", ""].join("\n"),
    };
    expect(landed).toEqual(landed.map(([where, , kind]) => [where, want[kind as keyof typeof want], kind]));
  });

  /** `blocks` as each road lands it, on a box and on an owned and a provider's image; with `fish`, config.fish's too. */
  const landedTexts = async (blocks: string, fish?: string): Promise<[string, string][]> => {
    const home = laptop();
    writeFileSync(join(home, ".zshrc"), blocks);
    writeFileSync(join(home, ".bashrc"), blocks);
    const rcs = [".zshrc", ".bashrc"];
    if (fish !== undefined) {
      mkdirSync(join(home, ".config", "fish"), { recursive: true });
      writeFileSync(join(home, ".config", "fish", "config.fish"), fish);
      rcs.push(".config/fish/config.fish");
    }
    const box = Object.fromEntries(configTexts("shell", home).map(t => [t.rel, t.text]));
    const landed: [string, string][] = rcs.filter(rc => rc !== ".bashrc").map(rc => [`a box's ~/${rc}`, box[rc]!]);
    const rows = fish === undefined ? ROWS.slice(0, 2) : [...ROWS.slice(0, 2), row({ rung: "shell", id: "shell/fish", paths: ["~/.config/fish"] })];
    for (const cloud of [false, true]) {
      const root = extract((await importFor(rows, { home, secrets: new Map(), platform: "darwin", cloud }).files!.pack()).tar);
      for (const rc of rcs) landed.push([`${cloud ? "a provider's" : "an owned"} image's ~/${rc}`, readFileSync(join(root, rc), "utf8")]);
    }
    return landed;
  };

  /** Each file `landedTexts` gives as `name` reads it: whether a value is left, what `-n` exits and what `out` prints
   * after sourcing it. */
  const readLanded = async (shell: string, name: string, blocks: string, out: string): Promise<[string, boolean, number | null, string][]> => {
    const landed = await landedTexts(blocks);
    // An empty home and no stdin: bash on a socket for stdin reads ~/.bashrc first, as if sshd had started it.
    const bare = mkdtempSync(join(tmpdir(), "wsp-config-secrets-sh-"));
    dirs.push(bare);
    return landed.map(([where, text]) => {
      const file = join(bare, "landed.sh");
      writeFileSync(file, text);
      const opts: SpawnSyncOptionsWithStringEncoding = { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: { PATH: process.env["PATH"], HOME: bare } };
      const parse = spawnSync(shell, ["-n", file], opts);
      const run = spawnSync(shell, ["-c", `. "$1"; ${out}`, name, file], opts);
      return [where, text.includes("plain0"), parse.status, run.stdout];
    });
  };

  for (const name of ["bash", "zsh"]) {
    const shell = (process.env["PATH"] ?? "").split(delimiter).filter(d => d !== "").map(d => join(d, name)).find(p => existsSync(p));
    it.skipIf(shell === undefined)(`a cut line that was the only line of an if, a function or a loop stands as a no-op, so ${name} reads the landed file and runs past it, on a box and on an image`, async () => {
      const blocks = [
        "if [ -f ~/.work ]; then",
        "  export DATABASE_URL=postgres://lab:plain0if@db.acme.dev/app",
        "fi",
        "db() {",
        "  export GH_TOKEN=plain0fn",
        "}",
        "for f in a b; do",
        "  export LAB_VALUE=ghp_plain0tokenAbCdEfGh1234567890",
        "done",
        'case "$OSTYPE" in',
        "  darwin*) export HOMEBREW_GITHUB_API_TOKEN=plain0case ;;",
        "  *) export ON_ANY=1 ;;",
        "esac",
        "db",
        "export AFTER=1",
        "",
      ].join("\n");
      const read = await readLanded(shell!, name, blocks, 'printf %s "$AFTER"');
      expect(read).toEqual(read.map(([where]) => [where, false, 0, "1"]));
    });

    it.skipIf(shell === undefined)(`a cut command that shares its line keeps the rest of the line, so the block it closes or opens still parses in ${name} and the commands beside it run, on a box and on an image`, async () => {
      const blocks = [
        'if [ -n "$HOME" ]; then',
        "  export GH_TOKEN=plain0fi; export IN_FI=1; fi",
        "for f in a; do",
        "  export LAB_TOKEN=plain0done; done",
        'if [ -n "$HOME" ]',
        "then export DATABASE_URL=postgres://lab:plain0then@db.acme.dev/app",
        "  export IN_THEN=1",
        "fi",
        "{ export API_KEY=plain0brace; export IN_BRACE=1; }",
        "case lab in",
        "  lab)",
        "    export HOMEBREW_GITHUB_API_TOKEN=plain0case ;;",
        "  *) ;;",
        "esac",
        'if [ -n "$HOME" ]; then',
        '  export MULTI_KEY="plain0a',
        'plain0b"; export IN_MULTI=1; fi',
        'if [[ -z "$HOME" || "$U" = https://lab:plain0cond@db.acme.dev/app ]]; then :; fi; export IN_COND=1',
        "for u in https://lab:plain0for@db.acme.dev/app; do :; done",
        'if [ -n "$HOME" ]; then export IN_NOTE=1; fi # https://lab:plain0note@db.acme.dev/app',
        "export AFTER=1",
        "",
      ].join("\n");
      const left = ["GH_TOKEN", "LAB_TOKEN", "DATABASE_URL", "API_KEY", "HOMEBREW_GITHUB_API_TOKEN", "MULTI_KEY"].map(v => `\${${v}:+${v}}`).join("");
      const read = await readLanded(shell!, name, blocks, `printf "%s " "$IN_FI" "$IN_THEN" "$IN_BRACE" "$IN_MULTI" "$IN_COND" "$IN_NOTE" "$AFTER" "${left}"`);
      expect(read).toEqual(read.map(([where]) => [where, false, 0, "1 1 1 1 1 1 1  "]));
    });

    it.skipIf(shell === undefined)(`what a cut test decided never runs in ${name}, its loop never spins and it never ends the file early, on a box and on an image`, async () => {
      const blocks = [
        'if [ "$U" = https://lab:plain0if@db.acme.dev/app ]; then',
        "  export IN_IF=1",
        "else",
        "  export IN_ELSE=1",
        "fi",
        'while [ "$U" = https://lab:plain0while@db.acme.dev/app ]; do',
        "  export IN_LOOP=1",
        "done",
        '[ "$U" = https://lab:plain0and@db.acme.dev/app ] && export IN_AND=1',
        '[ "$U" != https://lab:plain0ret@db.acme.dev/app ] && return',
        "export AFTER=1",
        "",
      ].join("\n");
      const read = await readLanded(shell!, name, blocks, 'printf "%s " "$IN_IF" "$IN_ELSE" "$IN_LOOP" "$IN_AND" "$AFTER"');
      expect(read).toEqual(read.map(([where]) => [where, false, 0, " 1   1 "]));
    });

    it.skipIf(shell === undefined)(`a cut behind a precommand word, a declaration builtin, a backslash or nice leaves a file ${name} reads to its end, a condition behind one never runs, and a command behind one still runs, on a box and on an image`, async () => {
      const blocks = [
        "time export A_TOKEN=plain0time",
        "noglob export C_TOKEN=plain0noglob",
        "nocorrect export D_TOKEN=plain0nocorrect",
        "- export E_TOKEN=plain0dash",
        "command -p export H_TOKEN=plain0commandp",
        "integer -x N_KEY=plain0integer",
        "time Q_TOKEN=plain0prefix true",
        "repeat 1 export R_TOKEN=plain0repeat",
        "\\export S_TOKEN=plain0backslash",
        "\\builtin export T_TOKEN=plain0bsbuiltin",
        "if time U_TOKEN=plain0if true; then export IN_IF=1; fi",
        "while noglob V_TOKEN=plain0while true; do export IN_LOOP=1; break; done",
        "nice -n 5 env W_TOKEN=plain0nice true",
        "time export IN_TIME=1",
        "builtin export IN_BUILTIN=1",
        "export AFTER=1",
        "",
      ].join("\n");
      const left = ["A_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "H_TOKEN", "N_KEY", "Q_TOKEN", "R_TOKEN", "S_TOKEN", "T_TOKEN", "U_TOKEN", "V_TOKEN", "W_TOKEN"].map(v => `\${${v}:+${v}}`).join("");
      const read = await readLanded(shell!, name, blocks, `printf "%s " "$IN_TIME" "$IN_BUILTIN" "$IN_IF" "$IN_LOOP" "$AFTER" "${left}"`);
      expect(read).toEqual(read.map(([where]) => [where, false, 0, "1 1   1  "]));
    });
  }

  const ARRAYS = [
    "API_KEY=(",
    "  plain0arr",
    "  plain0arr2",
    ")",
    "declare -a GH_TOKEN=(",
    '  "plain0quoted ) a"',
    "  'plain0paren)'",
    ")",
    "typeset -a LAB_SECRET=(",
    "  plain0typeset # note (",
    '  "plain0multi',
    'plain0line"',
    ")",
    "LAB_TOKEN+=(",
    "  plain0append",
    ")",
    'if [ -n "$HOME" ]; then',
    "  DB_PASSWORD=(",
    "    plain0block # (",
    "  ); export IN_BLOCK=1; fi",
    "PLAIN=(",
    "  one",
    "  two",
    ")",
    "export AFTER=1",
    "",
  ].join("\n");

  it("an array spanning lines under a secret name lands as one true, its values gone, and one under a plain name lands as it was, on a box and on an image", async () => {
    const want = ["true", "true", "true", "true", 'if [ -n "$HOME" ]; then', "  true; export IN_BLOCK=1; fi", "PLAIN=(", "  one", "  two", ")", "export AFTER=1", ""].join("\n");
    // An owned image's rc opens with a guard block for the commands the image lacks; only the cut is checked here.
    const landed = (await landedTexts(ARRAYS)).map(([where, text]): [string, string] => [where, text.replace(new RegExp(`^${GUARD_BEGIN}\\n[^]*?${GUARD_END}\\n\\n`), "")]);
    expect(landed).toEqual(landed.map(([where]) => [where, want]));
  });

  for (const name of ["bash", "zsh"]) {
    const shell = (process.env["PATH"] ?? "").split(delimiter).filter(d => d !== "").map(d => join(d, name)).find(p => existsSync(p));
    it.skipIf(shell === undefined)(`an array spanning lines under a secret name is cut whole, so ${name} reads the landed file to its end with no value set, and a plain array keeps its values, on a box and on an image`, async () => {
      const left = ["API_KEY", "GH_TOKEN", "LAB_SECRET", "LAB_TOKEN", "DB_PASSWORD"].map(v => `\${${v}:+${v}}`).join("");
      const read = await readLanded(shell!, name, ARRAYS, `printf "%s " "$IN_BLOCK" "\${PLAIN[*]}" "$AFTER" "${left}"`);
      expect(read).toEqual(read.map(([where]) => [where, false, 0, "1 one two 1  "]));
    });
  }

  const ALIASES = [
    "alias gh='GH_TOKEN=plain0alias gh'",
    'alias gl="GITLAB_TOKEN=plain0double glab"',
    "alias ex='export API_KEY=plain0export'",
    "alias en='env LAB_TOKEN=plain0env make'",
    "alias a1='A1_TOKEN=plain0first' a2='A2_TOKEN=plain0second'",
    "alias -g DBP='DB_PASSWORD=plain0global'",
    "\\alias bs='BS_TOKEN=plain0backslash gh'",
    "alias lp='LAB=1 LP_TOKEN=plain0lead gh'",
    "alias gv='GH_VALUE=ghp_plain0tokenAbCdEfGh1234567890 gh'",
    'alias db="psql postgres://lab:plain0alias@db.acme.dev/app"',
    '"export" QB_TOKEN=plain0quotedbuiltin',
    "ex'port' QP_TOKEN=plain0partquoted",
    'export "QN_TOKEN=plain0quotedname"',
    'export "QE_TOKEN"=plain0quotedeq',
    "'typeset' -x QT_TOKEN=plain0typeset",
    "alias tok='printenv GH_TOKEN'",
    "export AFTER=1",
    "",
  ].join("\n");
  const FISH_ALIASES = [
    "alias gh 'GH_TOKEN=plain0alias gh'",
    'alias gl="GITLAB_TOKEN=plain0eq glab"',
    "alias ex 'set -gx API_KEY plain0set'",
    "abbr -a ab 'AB_TOKEN=plain0abbr gh'",
    "abbr --add aw AW_TOKEN=plain0words gh",
    "abbr ap --position anywhere -- 'AP_TOKEN=plain0position'",
    "abbr -a -r '^g.+' ar 'AR_TOKEN=plain0regex'",
    "abbr -ac git ac 'AC_TOKEN=plain0cluster'",
    "alias gv 'echo ghp_plain0tokenAbCdEfGh1234567890'",
    "abbr -a gw 'echo ghp_plain0tokenAbCdEfGh1234567890'",
    'set -gx "QN_TOKEN" plain0quotedname',
    '"set" -gx QB_TOKEN plain0quotedbuiltin',
    "alias tok 'printenv GH_TOKEN'",
    "abbr -a gs 'git status'",
    "set -gx AFTER 1",
    "",
  ].join("\n");
  const POSIX_LEFT = ["GH_TOKEN", "GITLAB_TOKEN", "API_KEY", "LAB_TOKEN", "A1_TOKEN", "A2_TOKEN", "DB_PASSWORD", "BS_TOKEN", "LP_TOKEN", "GH_VALUE", "QB_TOKEN", "QP_TOKEN", "QN_TOKEN", "QE_TOKEN", "QT_TOKEN"];
  const FISH_LEFT = ["GH_TOKEN", "GITLAB_TOKEN", "API_KEY", "AB_TOKEN", "AW_TOKEN", "AP_TOKEN", "AR_TOKEN", "QN_TOKEN", "QB_TOKEN"];

  it("an alias or abbr whose body assigns a secret or holds one, a quoted builtin and a quoted name land as true, and an alias that only names a variable lands as it was, on a box and on an image", async () => {
    const want = {
      posix: [...Array<string>(15).fill("true"), "alias tok='printenv GH_TOKEN'", "export AFTER=1", ""].join("\n"),
      fish: [...Array<string>(12).fill("true"), "alias tok 'printenv GH_TOKEN'", "abbr -a gs 'git status'", "set -gx AFTER 1", ""].join("\n"),
    };
    const landed = (await landedTexts(ALIASES, FISH_ALIASES)).map(([where, text]): [string, string] => [where, text.replace(new RegExp(`^${GUARD_BEGIN}\\n[^]*?${GUARD_END}\\n\\n`), "")]);
    expect(landed.map(([where]) => where)).toEqual(["a box's ~/.zshrc", "a box's ~/.config/fish/config.fish", ...["an owned", "a provider's"].flatMap(w => [".zshrc", ".bashrc", ".config/fish/config.fish"].map(rc => `${w} image's ~/${rc}`))]);
    expect(landed).toEqual(landed.map(([where]) => [where, where.endsWith(".fish") ? want.fish : want.posix]));
  });

  for (const name of ["bash", "zsh", "fish"]) {
    const shell = (process.env["PATH"] ?? "").split(delimiter).filter(d => d !== "").map(d => join(d, name)).find(p => existsSync(p));
    it.skipIf(shell === undefined)(`a cut alias leaves a file ${name} reads to its end with no value set, and the alias that only names a variable still runs, on a box and on an image`, async () => {
      const fish = name === "fish";
      const bare = mkdtempSync(join(tmpdir(), "wsp-config-secrets-sh-"));
      dirs.push(bare);
      const opts: SpawnSyncOptionsWithStringEncoding = { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: { PATH: process.env["PATH"], HOME: bare } };
      const left = fish ? `(for v in ${FISH_LEFT.join(" ")}; set -q $v; and echo $v; end)` : `"${POSIX_LEFT.map(v => `\${${v}:+${v}}`).join("")}"`;
      // bash expands an alias only with expand_aliases and only in text it reads after the alias is defined, so the call goes through eval.
      const out = fish ? `source $argv[1]; GH_TOKEN=kept tok; abbr --show; printf "%s " $AFTER ${left}` : `[ -n "$BASH_VERSION" ] && shopt -s expand_aliases; . "$1"; eval 'GH_TOKEN=kept tok'; printf "%s " "$AFTER" ${left}`;
      const read = (await landedTexts(ALIASES, FISH_ALIASES)).filter(([where]) => where.endsWith(".fish") === fish).map(([where, text]) => {
        const file = join(bare, fish ? "landed.fish" : "landed.sh");
        writeFileSync(file, text);
        const run = spawnSync(shell!, fish ? ["--no-config", "-c", out, file] : ["-c", out, name, file], opts);
        return [where, text.includes("plain0"), spawnSync(shell!, ["-n", file], opts).status, run.stdout];
      });
      expect(read).toEqual(read.map(([where]) => [where, false, 0, fish ? "kept\nabbr -a -- gs 'git status'\n1 " : "kept\n1  "]));
    });
  }

  const HEREDOCS = [
    "cat > ~/.lab.env <<EOF",
    "API_KEY=plain0heredoc",
    "PLAIN=1",
    "EOF",
    "cat >> ~/.lab.env <<'EOF'",
    "  export GH_TOKEN=plain0export",
    'DB_PASSWORD="plain0multi',
    'plain0line"',
    "LAB=2",
    "EOF",
    "cat >> ~/.lab.env <<-EOF",
    "\tLAB_SECRET=plain0tab",
    "\tEOF",
    "cat >> ~/.lab.env <<EOF",
    "export \\",
    "CONT_KEY=plain0cont",
    "EOF",
    "eval 'cd /tmp",
    "export LAB_TOKEN=plain0eval'",
    'eval "',
    "  EVAL_SECRET=plain0double",
    '"',
    "eval 'ARR_KEY=(",
    "  plain0array",
    ")'",
    "eval 'cd /tmp",
    "LAB_VALUE=ghp_plain0tokenAbCdEfGh1234567890'",
    "eval 'echo it'\\''s",
    "export QT_TOKEN=plain0idiom'",
    "x=\\'; eval $'echo it\\'s",
    "export AC_TOKEN=plain0ansi'",
    "\\eval 'cd /tmp",
    "export QE_TOKEN=plain0escaped'",
    "LANG=C eval 'cd /tmp",
    "export LC_TOKEN=plain0lead'",
    'if [ -n "$HOME" ]; then eval \'',
    "  export IF_TOKEN=plain0if",
    "'; export IN_BLOCK=1; fi",
    "export AFTER=1",
    "",
  ].join("\n");
  /** Heredocs and evals over lines that hold no secret, a name check's near misses among them: each lands as it was. */
  const PLAIN_HEREDOCS = [
    "cat > ~/.lab.env <<EOF",
    "PLAIN=1",
    "KEYTIMEOUT=1",
    "export LAB_PATH=\\$HOME/bin",
    "EOF",
    "eval 'cd /tmp",
    "export EVAL_PLAIN=1'",
    'eval "',
    '  export QUOTE=\\"it\'s\\"',
    '"',
    "alias up='cd ..",
    "ls'",
    "cat >> ~/.lab.env <<'EOF'",
    "it's a body with a stray quote $KEY",
    "EOF",
    "export AFTER=1",
    "",
  ].join("\n");
  const HEREDOC_LEFT = ["API_KEY", "GH_TOKEN", "DB_PASSWORD", "LAB_SECRET", "LAB_TOKEN", "EVAL_SECRET", "ARR_KEY", "LAB_VALUE", "QT_TOKEN", "AC_TOKEN", "IF_TOKEN", "CONT_KEY", "QE_TOKEN", "LC_TOKEN"];
  const unguarded = (landed: [string, string][]): [string, string][] => landed.map(([where, text]) => [where, text.replace(new RegExp(`^${GUARD_BEGIN}\\n[^]*?${GUARD_END}\\n\\n`), "")]);

  it("a heredoc body line or an eval body over lines that assigns a secret lands cut like a bare assignment, and one that holds none lands byte for byte, on a box and on an image", async () => {
    const want = ["cat > ~/.lab.env <<EOF", "PLAIN=1", "EOF", "cat >> ~/.lab.env <<'EOF'", "LAB=2", "EOF", "cat >> ~/.lab.env <<-EOF", "\tEOF", "cat >> ~/.lab.env <<EOF", "export \\", "", "EOF", "true", "true", "true", "true", "true", "x=\\'; true", "true", "true", 'if [ -n "$HOME" ]; then true; export IN_BLOCK=1; fi', "export AFTER=1", ""].join("\n");
    const landed = unguarded(await landedTexts(HEREDOCS));
    expect(landed).toEqual(landed.map(([where]) => [where, want]));
    const plain = unguarded(await landedTexts(PLAIN_HEREDOCS));
    expect(plain).toEqual(plain.map(([where]) => [where, PLAIN_HEREDOCS]));
  });

  for (const name of ["bash", "zsh"]) {
    const shell = (process.env["PATH"] ?? "").split(delimiter).filter(d => d !== "").map(d => join(d, name)).find(p => existsSync(p));
    it.skipIf(shell === undefined)(`a cut heredoc line or eval leaves a file ${name} reads to its end with no value set or written, and a plain one still runs, on a box and on an image`, async () => {
      const left = HEREDOC_LEFT.map(v => `\${${v}:+${v}}`).join("");
      const read = await readLanded(shell!, name, HEREDOCS, `printf "%s " "$IN_BLOCK" "$AFTER" "${left}"; cat ~/.lab.env`);
      expect(read).toEqual(read.map(([where]) => [where, false, 0, "1 1  PLAIN=1\nLAB=2\nexport \n"]));
      const plain = await readLanded(shell!, name, PLAIN_HEREDOCS, 'printf "%s " "$EVAL_PLAIN" "$QUOTE" "$AFTER"; cat ~/.lab.env');
      expect(plain).toEqual(plain.map(([where]) => [where, false, 0, "1 it's 1 PLAIN=1\nKEYTIMEOUT=1\nexport LAB_PATH=$HOME/bin\nit's a body with a stray quote $KEY\n"]));
    });
  }

  const GLUED = [
    "export PGPASSWORD=plain0pg",
    "MYSQL_PWD=plain0mysql",
    "export GHTOKEN=plain0token LAB_CLIENTSECRET=plain0secret",
    "alias pg='PGPASSWORD=plain0alias psql'",
    "",
  ].join("\n");
  const FISH_GLUED = ["set -gx PGPASSWORD plain0pg", "set -gx MYSQL_PWD plain0mysql", "set -gx GHTOKEN plain0token", "alias pg 'PGPASSWORD=plain0alias psql'", ""].join("\n");
  const SETTINGS = ["export KEYTIMEOUT=1", "HISTKEY=1", "DISABLE_FZF_KEY_BINDINGS=true", "export PASSWORD_STORE_DIR=~/.pass", "cd ~ && echo $OLDPWD", "export TIKTOKEN_CACHE_DIR=~/.cache/tiktoken", `alias yt='curl -s "https://www.googleapis.com/youtube/v3/search?part=snippet&pageToken=CAUQAA"'`, "export AFTER=1", ""].join("\n");
  const FISH_SETTINGS = ["set -g fish_key_bindings fish_vi_key_bindings", "set -U fish_sequence_key_delay_ms 200", "set -g fish_prompt_pwd_dir_length 3", "set -gx AFTER 1", ""].join("\n");

  it("a secret word glued to another part of the name is cut and a shell's own setting holding one lands as it was, on a box and on an image, and init asks only for the cut ones", async () => {
    const landed = unguarded(await landedTexts(GLUED + SETTINGS, FISH_GLUED + FISH_SETTINGS));
    expect(landed).toEqual(landed.map(([where]) => [where, where.endsWith(".fish") ? `true\ntrue\ntrue\ntrue\n${FISH_SETTINGS}` : `true\ntrue\ntrue\ntrue\n${SETTINGS}`]));
    const home = laptop();
    mkdirSync(join(home, ".config", "fish"), { recursive: true });
    const rows = [...ROWS.slice(0, 2), row({ rung: "shell", id: "shell/fish", paths: ["~/.config/fish"] })];
    const cut = async (posix: string, fish: string): Promise<{ path: string; names: string[] }[] | undefined> => {
      for (const rc of [".zshrc", ".bashrc"]) writeFileSync(join(home, rc), posix);
      writeFileSync(join(home, ".config", "fish", "config.fish"), fish);
      return (await importFor(rows, { home, secrets: new Map(), platform: "darwin" }).files!.pack()).cut;
    };
    const glued = ["PGPASSWORD", "MYSQL_PWD", "GHTOKEN", "LAB_CLIENTSECRET"];
    expect(await cut(GLUED + SETTINGS, FISH_GLUED + FISH_SETTINGS)).toEqual([
      { path: "~/.bashrc", names: glued },
      { path: "~/.config/fish/config.fish", names: glued.filter(n => n !== "LAB_CLIENTSECRET") },
      { path: "~/.zshrc", names: glued },
    ]);
    expect(await cut(SETTINGS, FISH_SETTINGS)).toEqual([]);
  });

  it("a shell row's file that is no script loses a cut line with nothing in its place, on a box and on a provider's image", async () => {
    const home = laptop();
    mkdirSync(join(home, ".config"));
    writeFileSync(join(home, ".config", "starship.toml"), '[custom.db]\ncommand = "psql postgres://lab:plain0toml@db.acme.dev/app"\nwhen = true\n');
    writeFileSync(join(home, ".tmux.conf"), 'set -g mouse on\nset -g @db "postgres://lab:plain0tmux@db.acme.dev/app"\n');
    const rows = [row({ rung: "shell", id: "shell/starship", paths: ["~/.config/starship.toml"] }), row({ rung: "shell", id: "shell/tmux", paths: ["~/.tmux.conf"] })];
    const root = extract((await importFor(rows, { home, secrets: new Map(), platform: "darwin", cloud: true }).files!.pack()).tar);
    const image = Object.fromEntries([".config/starship.toml", ".tmux.conf"].map(rel => [rel, readFileSync(join(root, rel), "utf8")]));
    for (const landed of [Object.fromEntries(configTexts("shell", home).map(t => [t.rel, t.text])), image]) {
      expect(landed[".config/starship.toml"]).toBe("[custom.db]\nwhen = true\n");
      expect(landed[".tmux.conf"]).toBe("set -g mouse on\n");
    }
    expect([".inputrc", "dotfiles/inputrc"].map(p => shellScript(p, true))).toEqual([false, false]);
    expect(["dotfiles/zshenv", ".zprofile", ".oh-my-zsh/custom/themes/lab.zsh-theme", ".config/direnv/direnvrc", ".config/direnv/direnv.toml", ".zsh_plugins.txt"].map(p => shellScript(p))).toEqual([false, true, true, true, false, false]);
    expect(shellScript("dotfiles/zshenv", true)).toBe(true);
  });
});

describe("an image or a box that already carries such a value", () => {
  it("an inputrc row digests the cut the pack lands, which leaves no true", () => {
    const home = laptop();
    writeFileSync(join(home, ".inputrc"), '"\\C-xg": "git clone https://lab:plain0rl@github.com/acme/x"\nset editing-mode vi\n');
    chmodSync(join(home, ".inputrc"), 0o644);
    const files = importFor([row({ rung: "shell", id: "shell/inputrc", paths: ["~/.inputrc"] })], { home, secrets: new Map(), platform: "darwin" }).recipe!.files;
    // Main's digest for this file: the pack lands the same bytes on main and here.
    expect(files[0]!.digest).toBe("bcba343886145fbdadbd4891afd1b91157b6d5cc6779c69864e6e6fbf3ae4179");
  });

  it("reads changed on a box: the config's digest moves off the one the names-only cut gave", () => {
    const home = laptop();
    const before = configDigest([{ rel: ".zshrc", text: stripExports(ZSHRC).carried, cut: [] }]);
    expect(configDigest(configTexts("shell", home))).not.toBe(before);
  });

  it("reads changed on an image, owned or a provider's, while a file with nothing to cut digests as it did", () => {
    const home = laptop();
    writeFileSync(join(home, ".zprofile"), "export EDITOR=vim\nexport GH_TOKEN=plain0named\n");
    chmodSync(join(home, ".zprofile"), 0o644);
    writeFileSync(join(home, ".zlogin"), "if [ -f ~/.work ]; then\n  export EDITOR=vim\nfi\n");
    chmodSync(join(home, ".zlogin"), 0o644);
    mkdirSync(join(home, ".config", "fish", "functions"), { recursive: true });
    writeFileSync(join(home, ".config", "fish", "functions", "db.fish"), "function db\n  psql postgres://lab:plain0fish@db.acme.dev/app\nend\n");
    chmodSync(join(home, ".config"), 0o755);
    chmodSync(join(home, ".config", "fish"), 0o755);
    chmodSync(join(home, ".config", "fish", "functions"), 0o755);
    chmodSync(join(home, ".config", "fish", "functions", "db.fish"), 0o644);
    // A dotfiles folder whose zshenv ~/.zshenv links to: the pack strips it as the rc file it is.
    mkdirSync(join(home, "dotfiles"));
    chmodSync(join(home, "dotfiles"), 0o755);
    writeFileSync(join(home, "dotfiles", "zshenv"), "export DATABASE_URL=postgres://lab:plain0twin@db.acme.dev/app\n");
    chmodSync(join(home, "dotfiles", "zshenv"), 0o644);
    symlinkSync(join(home, "dotfiles", "zshenv"), join(home, ".zshenv"));
    const rows = [
      ...ROWS,
      row({ rung: "shell", id: "shell/zprofile", paths: ["~/.zprofile"] }),
      row({ rung: "shell", id: "shell/zlogin", paths: ["~/.zlogin"] }),
      row({ rung: "shell", id: "shell/fish", paths: ["~/.config/fish"] }),
      row({ rung: "shell", id: "shell/dotfiles", paths: ["~/dotfiles"] }),
    ];
    const digests = (cloud: boolean): Record<string, string | undefined> => {
      const files = importFor(rows, { home, secrets: new Map(), platform: "darwin", cloud }).recipe!.files;
      return Object.fromEntries([".zshrc", ".zprofile", ".zlogin", ".config/fish", "dotfiles"].map(dest => [dest, files.find(f => f.dest === dest)?.digest]));
    };
    // Each digest as main's cut read it: a value cut since moves it, a file whose cut is unchanged keeps it.
    const main = {
      owned: { ".zshrc": "2cf364a38f65d3bd550a6ecd3eb20a28de7fe071ff8130ba39b56c910e867ca1", ".zprofile": "1790b8d4b3f92655798cfc1bb8c449bf0375c8551c344890da3312578e9367db", ".zlogin": "d6cb968c060d366b13aba61d7b684638cc44ff5e6ee02be22785cf9ed4dedf8e", ".config/fish": "9f377f40cb6cdc97d5c1f206cd7f78a848214e5ec3e27d90a9d8dc921ebf740f", dotfiles: "b535bbf9d58fb2666aa618ec9edc109d9690a98fb7943297409153e996e68af8" },
      cloud: { ".zshrc": "83be9c0cd7ac363af2118d8f2684ec139348334ddec7926dfc1b48a65da7d9e7", ".zprofile": "1790b8d4b3f92655798cfc1bb8c449bf0375c8551c344890da3312578e9367db", ".zlogin": "d6cb968c060d366b13aba61d7b684638cc44ff5e6ee02be22785cf9ed4dedf8e", ".config/fish": "9f377f40cb6cdc97d5c1f206cd7f78a848214e5ec3e27d90a9d8dc921ebf740f", dotfiles: "b535bbf9d58fb2666aa618ec9edc109d9690a98fb7943297409153e996e68af8" },
    };
    const now = { owned: digests(false), cloud: digests(true) };
    expect(now.owned[".zshrc"]).not.toBe(main.owned[".zshrc"]);
    expect(now.cloud[".zshrc"]).not.toBe(main.cloud[".zshrc"]);
    expect(now.cloud[".config/fish"]).not.toBe(main.cloud[".config/fish"]);
    expect(now.owned.dotfiles).not.toBe(main.owned.dotfiles);
    expect(now.cloud.dotfiles).not.toBe(main.cloud.dotfiles);
    // A cut export now lands as `true`, so an image whose cut left a block empty reads changed.
    expect(now.owned[".zprofile"]).not.toBe(main.owned[".zprofile"]);
    expect(now.cloud[".zprofile"]).not.toBe(main.cloud[".zprofile"]);
    expect(now.owned[".zlogin"]).toBe(main.owned[".zlogin"]);
    expect(now.cloud[".zlogin"]).toBe(main.cloud[".zlogin"]);
    expect(configTexts("shell", home).find(t => t.rel === ".zlogin")?.text).toBe(readFileSync(join(home, ".zlogin"), "utf8"));
    // A computer the person owns keeps a fish function's values, as it did.
    expect(now.owned[".config/fish"]).toBe(main.owned[".config/fish"]);
  });
});
