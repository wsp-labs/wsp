// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/shell_rc.md: shell-rc.ts's public functions on the edge inputs of the wave-2 audit (group e).
// The private helpers (shell_words, alias_bodies, command_words, assigned_in) are reached through stripExports.
import { it } from "vitest";
import { isRcPath, isSecretName, rcFiles, simpleCommands, sourceCommand, sourcedPath, sourcedPaths, stripExports } from "../../../../packages/collect/src/detect/shell-rc.js";
import { plain, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args)) });
const table = (f: (...a: never[]) => unknown, rows: string[]): Case[] => rows.map(t => pure(JSON.stringify(t), f, t));
const strip = (text: string): Case => {
  const r = stripExports(text, undefined, [], false);
  return { name: JSON.stringify(text), args: [text, null, [], false], out: { names: r.names, carried: r.carried, held: [] } };
};

it("shell_rc fixtures", () => {
  const fixture: Record<string, Case[]> = {};

  fixture["is_rc_path"] = table(isRcPath, [
    ".config/fish/conf.d/.fish", ".config/fish/conf.d/x.FISH", ".config/fish/conf.d/a.fish.bak", ".config/fish/conf.d/", ".config/fish/conf.d//x.fish", "/.config/fish/conf.d/x.fish",
    ".zshrc/", ".config/fish/config.fish/", "x.fish", ".config/fish/conf.d", ".zshrc", ".config/fish/config.fish", ".config/fish/conf.d/work.fish", "x/.zshrc", ".config/fish/conf.d/sub/deep.fish",
  ]);
  fixture["rc_files"] = [
    pure("empty listing is RC_PATHS", rcFiles, []),
    pure("hidden, duplicate, sub folder, other extensions", rcFiles, [".fish", "a.fish", "a.fish", "b.fish.bak", "sub/c.fish", "A.FISH"]),
  ];
  fixture["is_secret_name"] = table(isSecretName, [
    "CLAUDE_CODE_SKIP_É_AUTH", "CLAUDE_CODE_SKIP__AUTH", "claude_code_skip_x_auth", "CLAUDE_CODE_SKIP_X_AUTHX", "password_store_dir", "PASSWORD_STORE_EXTRA", "PASSWORD_STORE_DIR_X", "FISH_KEY_BINDINGS_X", "tiktoken_cache_dir",
    "", "_", "__KEY__", "KEY_2", "TOKEN1", "MyToken", "Github_Token", "apikey", "API-KEY", "MY-TOKEN", "DB__PAT", "PAT_", "XPASS", "PREPASS", "OUTPASS", "TRESPASS", "NONTRESPASS", "PASS_", "SSTOKEN", "ßtoken", "ß_token", "TOKEN\n", "KEY ", "MYSQL_pwd", "X_AUTH", "AUTH_X", "x_auth",
    "PWD", "_PWD", "MYSQL_PWD", "password_store_token", "PASSWORD_STORE_GPG_OPTS", "fish_key_bindings", "PASSWORD_STORE_X_SELECTION",
  ]);
  fixture["simple_commands"] = table(simpleCommands, [
    "a#b", "(#x", "echo a(#c", "echo a{#c", "echo ${#v}", "echo a;#c", "echo a&#c", "echo a|#c", "echo \\#x", "echo a\\ #c", "echo a\t#c", "echo a\u2028#c", "echo a\ufeff#c", "a\u0085#c", "echo a\u00a0#c", "x=(#c) y", "a=(#c\n)",
    "echo $'a\\'b;c'", "echo `a;b`", "echo $(a;b)", "echo 'a\\';b", 'echo "a\\";b"', 'echo "a;b', "echo 'a;b", "echo a\\",
    "then", "time", "time ", "command -v x", "- x", "done x", "\\time x", "\\then x", "repeat x", "repeat 3 x", "exec -cl cmd", "exec -c -l cmd", "exec -a n cmd", "do if ! x", "if if x", "time  time x", "  a ;  b",
    "then\u00a0x", "then\u0085x", "\u00a0then x", "then\tx", "then\nx", "a && b || c | d & e", "", "a;;b", "if true; then export A=1; fi", "echo 'a;b' \"c|d\" e\;f # g; h", "f() { x; }", "a}b{c",
  ]);
  fixture["source_command"] = table(sourceCommand, [
    "source ~/a", "source\t~/a", "\tsource ~/a", " . ~/a", "\\. ~/a", "source  ~/a  x", "source ~/a>/dev/null", "source ~/a;b", "source ~/a(x)", "source ~/a\u00a0x", 'source "a b" c', "source ''", 'source ""',
    "source ~/a\rfoo", "source ~/a\r", "source ~/a\u2028x", "source\u00a0~/a", 'source "a"b', "source ~/a'b'", "source ~/a\\ b", "source ~/a`x`", "source <(x)", "source", "source ", ".x ~/a", "..  ~/a", "\\\\. ~/a", "SOURCE ~/a",
    "builtin source ~/a", "x=1 source ~/a", 'source "unterminated', "source ~/a\nsource ~/b", "sourcex ~/a", "source ~/a|x", "source ~/a&", "source ~/a<x",
  ]);
  const sp = (t: string, h: string): Case => pure(`${JSON.stringify(t)} home ${JSON.stringify(h)}`, sourcedPath, t, h);
  fixture["sourced_path"] = [
    ...["'$HOME/x'", '"~/x"', '"/a\'b"', '\'/a"b\'', '"', "''", '""', "/a\nb", '"/a\nb"', "/a{b,c}/x", "/a b/c", '"/a b/c"', "/a#b", "/a!b", "/a;b", "/a/.hidden/b", "/a/.../b", "/a/..b", "~/a~/b", "~/.", "~/..", "~//a", "~/", "/", "/a/", "~", "$HOME", "${HOME}",
      "~user/x", "$HOMEX/x", "$HOME/a/$HOME/b", '"$HOME"/x', "\\$HOME/x", "'/a", '"/a\'', "~/a/b", '"$HOME/x"', "${HOME}/y", "'/abs/z'", "~/a//b", "/a/./b", "~/a*", "rel", "/A/é"].map(t => sp(t, "/h")),
    ...["/h/", "", "/", "/h$1", "/h*", "/h$&x", "relative", "/h/x", "/h w"].map(h => sp("~/a", h)),
    sp("/x/a", ""),
  ];
  const sps = (t: string, h = "/h"): Case => pure(`${JSON.stringify(t)} home ${JSON.stringify(h)}`, sourcedPaths, t, h);
  fixture["sourced_paths"] = [
    ...["", "\n", "echo $(source ~/a.sh)", "{ source ~/a.sh; }", "time source ~/a.sh", "then source ~/a.sh", "! source ~/a.sh", "exec source ~/a.sh", "command source ~/a.sh", "a=$(source ~/a.sh)", "(source ~/a.sh)", "x | source ~/a.sh", "x && source ~/a || source ~/b",
      "sudo source ~/a", "\\source ~/a", 'eval "source ~/a"', 'echo "a; source ~/a"', "echo 'a; source ~/a'", "source ~/a#c", "source ~/a # c", "source '~/a'", "source ${HOME}/a/${HOME}/b", "source ${HOME}x/a", "source ${HOME}", "source ${HOME}/a", 'source "${HOME}/a"',
      "echo ${#v}; source ~/a", "# ${HOME}\nsource ~/a", "source ~/a \\\n  ~/b", "source ~/a\nsource /h/a\n", "source ~/a\nsource $HOME/a\nsource ${HOME}/a\nsource \"~/a\"\n", "source /h/b\nsource ~/a\nsource /h/b\n",
      "source ~/a\r\nsource ~/b\r\n", "source ~/a\rsource ~/b", "source ~/a\u2028source ~/b", "source ~/a; source ~/b; source ~/a", "source ~/b/../a", "source ~/a/", "source /", "source ~", "source ~/a b c", "source ~/a\n\nsource ~/c", "source $HOME/a", "source \\$HOME/a"].map(t => sps(t)),
    sps("source ~/a", "/h w"), sps("source ~/a", "/h/"), sps("source ~/a", "/h$"), sps("source ~/a", "h"), sps("source ~/a", ""), sps("source /x/a", ""),
  ];
  // assigned_in, shell_words, command_words and alias_bodies: one command per line, the names a cut records.
  fixture["strip_exports_assigned"] = [
    "X=1 export Y_TOKEN=2", "X_TOKEN=1 export Y_TOKEN=2", "A=1 set -gx A_TOKEN x", "X=1 eval Y_TOKEN=2",
    "export X Y_TOKEN=2", "export A=1 foo B_TOKEN=2", 'export A="x B_TOKEN=y"', "export A='x B_TOKEN=y'", "sudo cmd arg A_TOKEN=1", "env echo A_TOKEN=1", "export A_TOKEN =1", "export A_TOKEN= 1", "ENV A_TOKEN=1", "export -x", "export", "export -p A_TOKEN=1", "export -- A_TOKEN=1", "export\u00a0A_TOKEN=1", "export  -x  A_TOKEN=1", "\\export A_TOKEN=1", "'export' A_TOKEN=1", "export A_TOKEN", "readonly A_TOKEN",
    'export A_TOKEN="x\u2028y"', 'export A_TOKEN="x\ry"', "export A_TOKEN=x\u2028y", "export A=1\u0085B_TOKEN=2", "export A=1\ufeffB_TOKEN=2", "export A=1\u00a0B_TOKEN=2", "export A_TOKEN=1\\\nB_TOKEN=2", 'export A="x\ny B_TOKEN=2"', '"then x" A_TOKEN=1', "'' A_TOKEN=1",
    "set -l A_TOKEN x B_TOKEN y", "set -x A_TOKEN -e", "set -g A_TOKEN -q", "set -U -e A_TOKEN", "set -ex A_TOKEN", "set -- A_TOKEN x", "set -x -- A_TOKEN x", "set -gx A_TOKEN[1] x", "set -gx A_TOKEN=x", "set A_TOKEN=x", "set -gx 1X x", "set -Ux A_TOKEN x", "set A_TOKEN", "set -S", "set", "set -gx", "set -n", "set --names A_TOKEN", "set --show A_TOKEN", "set --path A_TOKEN x", "not and set A_TOKEN x", "set -gx 'A_TOKEN' x",
    "eval -- A_TOKEN=1", "eval eval A_TOKEN=1", 'eval "a=1\nB_TOKEN=2"', 'eval "A_TOKEN=1" B_TOKEN=2', "eval", "eval set -gx A_TOKEN x", "\\eval A_TOKEN=1", "eval 'A_TOKEN=1; B_TOKEN=2'",
    "alias - A_TOKEN=1", "alias + A_TOKEN=1", "alias A_TOKEN=1", "alias x A_TOKEN=1", "alias x A_TOKEN=1 y", "alias x=1 y=A_TOKEN=2", "alias a=b=c_TOKEN=1", "alias -x=A_TOKEN=1", "alias x=", "alias -p", "alias -- -x=A_TOKEN=1",
    "abbr --add x A_TOKEN=1", "abbr -a -f fn x A_TOKEN=1", "abbr -f fn x A_TOKEN=1", "abbr -ap x A_TOKEN=1", "abbr -pa x A_TOKEN=1", "abbr -cf x A_TOKEN=1", "abbr -r x A_TOKEN=1", "abbr -a -- -x A_TOKEN=1", "abbr --regex", "abbr -f", "abbr -s x A_TOKEN=1", "abbr --set-cursor x A_TOKEN=1", "abbr x=A_TOKEN=1",
    "X_TOKEN=1 Y_TOKEN=2", "A_TOKEN=1 B_TOKEN=2 A_TOKEN=3", "X='a b' A_TOKEN=1", "X=a\\ b A_TOKEN=1", "'A_TOKEN'=1 cmd", "A_TOKEN+=x", "A_TOKEN[0]=1", "1A_TOKEN=2", "=1",
    "$'a\\nb' A_TOKEN=1", "\"a\\\\\" A_TOKEN=1", "a\\ A_TOKEN=1",
  ].map(c => strip(`${c}\nnext\n`));

  write("shell_rc", fixture);
});
