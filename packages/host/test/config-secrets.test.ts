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
import { stripExports, type ManifestEntry } from "@wsp/collect";
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

  for (const name of ["bash", "zsh"]) {
    const shell = (process.env["PATH"] ?? "").split(delimiter).filter(d => d !== "").map(d => join(d, name)).find(p => existsSync(p));
    it.skipIf(shell === undefined)(`a cut line that was the only line of an if, a function or a loop stands as a no-op, so ${name} reads the landed file and runs past it, on a box and on an image`, async () => {
      const home = laptop();
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
      writeFileSync(join(home, ".zshrc"), blocks);
      writeFileSync(join(home, ".bashrc"), blocks);
      const landed: [string, string][] = [["a box's ~/.zshrc", configTexts("shell", home)[0]!.text]];
      for (const cloud of [false, true]) {
        const root = extract((await importFor(ROWS.slice(0, 2), { home, secrets: new Map(), platform: "darwin", cloud }).files!.pack()).tar);
        for (const rc of [".zshrc", ".bashrc"]) landed.push([`${cloud ? "a provider's" : "an owned"} image's ~/${rc}`, readFileSync(join(root, rc), "utf8")]);
      }
      // An empty home and no stdin: bash on a socket for stdin reads ~/.bashrc first, as if sshd had started it.
      const bare = mkdtempSync(join(tmpdir(), "wsp-config-secrets-sh-"));
      dirs.push(bare);
      const read = landed.map(([where, text]) => {
        const file = join(bare, "landed.sh");
        writeFileSync(file, text);
        const opts: SpawnSyncOptionsWithStringEncoding = { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: { PATH: process.env["PATH"], HOME: bare } };
        const parse = spawnSync(shell!, ["-n", file], opts);
        const run = spawnSync(shell!, ["-c", '. "$1"; printf %s "$AFTER"', name, file], opts);
        return [where, text.includes("plain0"), parse.status, run.stdout];
      });
      expect(read).toEqual(landed.map(([where]) => [where, false, 0, "1"]));
    });
  }

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
