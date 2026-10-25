// SPDX-License-Identifier: AGPL-3.0-only
import { readFileSync, writeFileSync } from "node:fs";
import { it } from "vitest";
import { fileURLToPath } from "node:url";

const src = fileURLToPath(new URL("../../../../packages/collect/src/detect/shell-rc.ts", import.meta.url));
const shellTs = fileURLToPath(new URL("../../../../packages/collect/src/detect/shell.ts", import.meta.url));

async function load() {
  let t = readFileSync(src, "utf8").replace('"./shell.js"', JSON.stringify(shellTs));
  t += "\nexport { assignedIn, shellWords, aliasBodies, commandWords, commandSpans, scanLine, opensShell, skipKeywords, commentAt, assignedNames };\n";
  const p = "/tmp/e-oracle/shell-rc-x.ts";
  writeFileSync(p, t);
  return (await import(/* @vite-ignore */ p)) as any;
}
const J = (v: unknown): string => JSON.stringify(v, (_k, x) => (x === undefined ? "<undefined>" : x));
const show = (label: string, v: unknown): void => console.log(`${label} => ${J(v)}`);

it("secret names", async () => {
  const m = await load();
  for (const n of ["", "_", "__KEY__", "key", "Key", "Api_Key", "apikey", "MyToken", "MYTOKEN", "TOKEN1", "KEY_2", "password_store_dir", "PASSWORD_STORE_EXTRA", "PASSWORD_STORE_GPG_OPTS", "CLAUDE_CODE_SKIP__AUTH", "CLAUDE_CODE_SKIP_É_AUTH", "claude_code_skip_x_auth", "CLAUDE_CODE_SKIP_X_AUTHX", "API-KEY", "MY-TOKEN", "DB__PAT", "PAT_", "XPASS", "PREPASS", "OUTPASS", "TRESPASS", "NONTRESPASS", "Github_Token", "GithubToken", "ßtoken", "SSTOKEN", "PASS_", "FISH_KEY_BINDINGS_X", "fish_key_bindings", "TOKEN\n", "KEY ", "PASSWORD_STORE_DIR_X", "TIKTOKEN_CACHE_DIR", "tiktoken_cache_dir", "AUTH_X", "X_AUTH", "x_auth", "MYSQL_pwd"]) show(`isSecretName(${J(n)})`, m.isSecretName(n));
});

it("rc paths", async () => {
  const m = await load();
  for (const r of [".config/fish/conf.d/.fish", ".config/fish/conf.d/x.FISH", ".config/fish/conf.d/a.fish.bak", ".config/fish/conf.d/", ".config/fish/conf.d", "/.config/fish/conf.d/x.fish", ".zshrc/", ".config/fish/config.fish/", "x.fish", ".config/fish/conf.d//x.fish"]) show(`isRcPath(${J(r)})`, m.isRcPath(r));
  show("rcFiles([])", m.rcFiles([]).slice(10));
  show("rcFiles dup", m.rcFiles([".fish", "a.fish", "a.fish", "b.fish.bak", "sub/c.fish", "A.FISH"]).slice(10));
});

it("simpleCommands", async () => {
  const m = await load();
  const cases = ["a#b", "(#x", "x=(#c) y", "echo a #c", "echo a\t#c", "echo a;#c", "echo a&#c", "echo a|#c", "echo a(#c", "echo a{#c", "echo ${#v}", "a=(#c\n)", "echo $'a\\'b;c'", "echo a\\", "echo \"a;b", "echo 'a;b", "echo \"a\\\";b\"", "echo 'a\\';b", "echo `a;b`", "echo $(a;b)", "  a ;  b", "then x", "then", "time", "time ", "time -p x", "do if ! x", "done x", "\\time x", "\\then x", "- x", "! x", "exec -a n cmd", "exec -cl cmd", "exec -c -l cmd", "repeat 3 x", "repeat x", "command -p x", "command -v x", "a && b || c | d & e", "a &&", "{ a; }", "a}b{c", "f() { x; }", "elif x", "else x", "until x", "while x", "if x", "noglob x", "nocorrect x", "builtin x", "and x", "or x", "not x", "coproc x", "x\ty", "then\tx", "then\nx", "a\nb;c", "a\u2028#c", "a\u00a0#c", "a\ufeff#c", "echo \\#x", "echo a\\ #c", "if if x", "time  time x"];
  for (const c of cases) show(`simpleCommands(${J(c)})`, m.simpleCommands(c));
});

it("sourcedPath and sourceCommand", async () => {
  const m = await load();
  for (const [t, h] of [["'$HOME/x'", "/h"], ["\"~/x\"", "/h"], ["\"/a'b\"", "/h"], ["'/a\"b'", "/h"], ["\"", "/h"], ["''", "/h"], ["\"\"", "/h"], ["~/a", "/h/"], ["~/a", ""], ["~/a", "/"], ["~/a", "/h$1"], ["~/a", "/h*"], ["~/a", "/h$&x"], ["~/", "/h"], ["/", "/h"], ["/a/", "/h"], ["~user/x", "/h"], ["$HOMEX/x", "/h"], ["$HOME", "/h"], ["${HOME}", "/h"], ["$HOME/a/$HOME/b", "/h"], ["~/a~/b", "/h"], ["/a{b,c}/x", "/h"], ["/a b/c", "/h"], ["\"/a b/c\"", "/h"], ["/a#b", "/h"], ["/a!b", "/h"], ["/a;b", "/h"], ["/a\nb", "/h"], ["\"/a\nb\"", "/h"], ["/a//b", "/h"], ["/a/.hidden/b", "/h"], ["/a/.../b", "/h"], ["/a/..b", "/h"], ["~/.", "/h"], ["~/..", "/h"], ["~//a", "/h"], ["\"$HOME\"/x", "/h"], ["\\$HOME/x", "/h"], ["/a'", "/h"], ["'/a", "/h"], ["\"/a'", "/h"], ["/A/é", "/h"], ["~/a", "relative"], ["~/a", "/h/x"], ["${HOME}/a", "/h"], ["$HOME/a", "/h w"]]) show(`sourcedPath(${J(t)}, ${J(h)})`, m.sourcedPath(t, h));
  for (const c of ["source ~/a", "source\t~/a", "source  ~/a  x", "\tsource ~/a", " . ~/a", "\\. ~/a", "\\\\. ~/a", "source ~/a>b", "source ~/a>/dev/null", "source ~/a;b", "source ~/a\rfoo", "source ~/a\r", "source ~/a\u2028x", "source ~/a\u00a0x", "source\u00a0~/a", "source \"a\"b", "source \"a b\" c", "source 'a b'", "source ~/a\"b\"", "source ~/a'b'", "source ~/a\\ b", "source ~/a`x`", "source ~/a(x)", "source", "source ", "sourcex ~/a", ".x ~/a", "..  ~/a", "source ~/a\nsource ~/b", "source \"unterminated", "source ''", "source \"\"", "source <(x)", "source ~/a&", "source ~/a|x", "source ~/a<x", "SOURCE ~/a", "builtin source ~/a", "x=1 source ~/a"]) show(`sourceCommand(${J(c)})`, m.sourceCommand(c));
});

it("sourcedPaths", async () => {
  const m = await load();
  const h = "/h";
  for (const t of ["", "\n", "echo $(source ~/a.sh)", "{ source ~/a.sh; }", "time source ~/a.sh", "then source ~/a.sh", "if x; then . ~/a.sh; fi", "source ~/a; source ~/b; source ~/a", "source ${HOME}/a", "source \"${HOME}/a\"", "source ${HOME}", "source ${HOME}x/a", "source ${HOME}/a/${HOME}/b", "echo ${#v}; source ~/a", "# ${HOME}\nsource ~/a", "source ~/a # c", "source ~/a#c", "source ~/a \\\n  ~/b", "eval \"source ~/a\"", "echo 'x'; source ~/a; echo \"it's\"", "echo \"a; source ~/a\"", "echo 'a; source ~/a'", "source ~/a\r\nsource ~/b\r\n", "source ~/a\rsource ~/b", "source ~/a\u2028source ~/b", "a=$(source ~/a)", "(source ~/a)", "x && source ~/a || source ~/b", "x | source ~/a", "source ~/a &", "source '~/a'", "source $HOME/a", "source \\$HOME/a", "source ~/b/../a", "source ~/a/", "source ~/a/b/", "source /", "source ~", "! source ~/a", "exec source ~/a", "command source ~/a", "sudo source ~/a", "\\source ~/a", "source ~/a b c", "  source ~/a", "\tsource ~/a", "source ~/a\n\nsource ~/c"]) show(`sourcedPaths(${J(t)})`, m.sourcedPaths(t, h));
  show("sourcedPaths home with space", m.sourcedPaths("source ~/a", "/h w"));
  show("sourcedPaths home trailing slash", m.sourcedPaths("source ~/a", "/h/"));
  show("sourcedPaths home $", m.sourcedPaths("source ~/a", "/h$"));
  show("sourcedPaths home rel", m.sourcedPaths("source ~/a", "h"));
  show("sourcedPaths home empty", m.sourcedPaths("source ~/a", ""));
  show("sourcedPaths abs only home empty", m.sourcedPaths("source /x/a", ""));
});
