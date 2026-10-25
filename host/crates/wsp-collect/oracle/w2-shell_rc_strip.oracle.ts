// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/shell_rc_strip.md: stripExports on the edge inputs of the wave-2 audit (group e).
// A secret_in argument is data: {"contains": T, "name": N} names N for any line holding T; N "$line" names the line
// itself; null is no function. `held` is the vec passed in; the case's out carries it as it is after the call.
// scanLine is not exported by the TS module, so it is reached only through stripExports here.
import { it } from "vitest";
import { stripExports } from "../../../../packages/collect/src/detect/shell-rc.js";
import { write } from "./w2-host.js";

type Case = Record<string, unknown>;
type Sec = { contains: string; name: string } | null;
const fn = (s: Sec, calls?: string[]) =>
  s === null ? undefined : (l: string): string | undefined => {
    calls?.push(l);
    return l.includes(s.contains) ? (s.name === "$line" ? l : s.name) : undefined;
  };
const SK: Sec = { contains: "sk-", name: "OPENAI" };
const LOGIN: Sec = { contains: "plain0", name: "a login" };
const run = (text: string, sec: Sec, script: boolean, held: string[] = []): Case => {
  const h = [...held];
  const r = stripExports(text, fn(sec), h, script);
  return { name: `${script ? "script " : ""}${JSON.stringify(text)}`, args: [text, sec, held, script], out: { names: r.names, carried: r.carried, held: h } };
};
const calls = (text: string, sec: Sec, script: boolean): Case => {
  const c: string[] = [];
  stripExports(text, fn(sec, c), [], script);
  return { name: `calls of ${script ? "script " : ""}${JSON.stringify(text)}`, args: [text, sec, [], script], out: c };
};
const plainRows = (rows: string[], script = false): Case[] => rows.map(t => run(t, null, script));
const login = (rows: string[]): Case[] => rows.map(t => run(t, LOGIN, true));

it("shell_rc_strip fixtures", () => {
  const fixture: Record<string, Case[]> = {};

  fixture["strip_exports"] = [
    ...plainRows(["a\nb\r\nc", "a\r\nb\nexport A_TOKEN=1\nc\n", "a\rb\nc", "a", "\n", "\r\n", "\r\n\r\n", "export A_TOKEN=1\r\nb\nexport B_TOKEN=1\n", "a\r\nexport A_TOKEN=1", "export A_TOKEN=1", "export A_TOKEN=1\n", "export A_TOKEN=1\r\n", "", "export A_TOKEN=1\n\n\nb\n", "\n\nexport A_TOKEN=1\n\n", "export A_TOKEN=1\n# c\n"]),
    ...plainRows(["export A_TOKEN=1", "a\r\nexport A_TOKEN=1", "", "export A_TOKEN=1\n", "export A_TOKEN=1\r\n", "export A_TOKEN=1\n\n"], true),
    // G2: an open cut at the end of the file
    ...plainRows(['x\nexport A_TOKEN="a\n', 'a\nexport A_TOKEN="a\n', 'export A_TOKEN="a\nb\n', 'export A_TOKEN="a', "export A_TOKEN=$(", 'x\nexport A_TOKEN="a\nb"']),
    ...plainRows(['x\nexport A_TOKEN="a\n', 'export A_TOKEN="a\nb\n', 'export A_TOKEN="a', "export A_TOKEN=$(", 'x\nexport A_TOKEN="a\nb"'], true),
    ...login(['y; echo "a\nplain0"; z\n', 'echo "a\nplain0"\n']),
    // G3, G4: secret_in
    run("export FOO=sk-1 BAR=2\nnext\n", SK, false), run("FOO=sk-1 cmd; BAR=2 cmd2\nnext\n", SK, false), run("export GH_TOKEN=x\nexport A=sk-1\n", SK, false), run("echo sk-1\nnext\n", SK, false),
    run("cat <<EOF\nx=sk-1\nEOF\n", SK, false), run("cat <<EOF\nGH_TOKEN=sk-1\nEOF\n", SK, false), run("echo ok; echo sk-1\n", SK, false),
    run("echo sk-1\n", { contains: "sk-", name: "" }, false), run("echo sk-1\necho sk-22\necho sk-1\n", { contains: "sk-", name: "$line" }, false), run("echo sk-1\necho sk-1\n", SK, false, ["KEEP", "OPENAI"]),
    run("echo sk-1\necho sk-2\nexport PLAIN=1\necho sk-3\n", SK, true, ["KEEP"]),
    run("export A=1; export B=sk-2\n", SK, true), run("export A=1; echo sk-2; export C=3\n", SK, true), run("# sk-1\nnext\n", SK, true), run("# sk-1\nnext\n", SK, false), run("echo a # sk-1\nnext\n", SK, true), run("echo sk-1 # c\nnext\n", SK, true),
    run("if x; then\n  echo sk-1\nfi\n", SK, true), run('echo sk-"a\nb"\nnext\n', SK, true), run('echo sk-"a\nb"\nnext\n', SK, false), run("x=1\ntrue sk-1\nx=2\n", SK, true), run("echo sk-1 && echo ok\n", SK, true), run("echo ok && echo sk-1\n", SK, true), run("echo ok; echo sk-1; echo ok2\n", SK, true), run("a=1\r\necho sk-1\r\nb=2\r\n", SK, true),
    // G10: heredoc bodies with secret_in
    run('cat <<EOF\necho sk-"abc\nrest"\nnext\nEOF\n', SK, false), run('cat <<EOF\nGH_TOKEN="sk-abc\nrest"\nnext\nEOF\n', SK, false), run("cat <<EOF\nsk-1\nEOF\n", SK, false),
    // G5, G6: quotes and continuations on kept lines
    ...plainRows(['echo "a\nexport A_TOKEN=1\nb"\n', "echo a \\\nexport A_TOKEN=1\nnext\n", "echo 'a\nexport A_TOKEN=1\nb'\n"]), run("echo 'a\nexport A_TOKEN=1\nb'\n", null, true),
    run("if x \\\n  && plain0; then y; fi\n", LOGIN, true), run("echo a\\\n  plain0\nnext\n", LOGIN, true),
    // G7: joins
    ...plainRows(["eval 'a\nb\nc'\nexport A_TOKEN=1\n", "eval 'a\nexport A_TOKEN=1\n", "x=1 eval 'a\nexport B_TOKEN=1'\nnext\n", "echo hi; eval 'a\nexport B_TOKEN=1'\nnext\n", "eval 'a\nb'; echo hi\nexport B_TOKEN=1\n", "eval 'a' ; eval 'b\nexport A_TOKEN=1'\nn\n", "eval 'a\n# note\nexport A_TOKEN=1'\nn\n", "echo 'a\neval 'b\nexport A_TOKEN=1'\nn\n", 'eval "a \\\nb"\nexport A_TOKEN=1\n', "eval a \\\nexport A_TOKEN=1\nn\n", 'eval "$(\nx\n)"\nnext\n']),
    run("eval 'a\nexport A_TOKEN=1\n", null, true),
    // G8, G9, G10: heredocs
    ...plainRows(["cat <<EOF\nEOF \nexport A_TOKEN=1\n", "cat <<EOF\nEOF # c\nexport A_TOKEN=1\nEOF\nB\n", "cat <<EOF\n\u00a0EOF\u00a0\nexport A_TOKEN=1\n", 'cat <<EOF\nA_TOKEN="a\nEOF\nb"\nEOF\nnext\n', "cat <<EOF\nA_TOKEN=1\nB=2\n", "cat <<EOF\nEOF\u0085\nexport A_TOKEN=1\nEOF\n",
      "cat <<END-OF\nA_TOKEN=1\nEND\nB=2\nEND-OF\nC=3\n", "cat <<A <<B\nx\nA\nB_TOKEN=1\nB\nnext\n", "cat <<a.b\nA_TOKEN=1\na\nB=2\n", "cat <<aé\nA_TOKEN=1\na\nB=2\n", "cat <<é\nA_TOKEN=1\nB=2\n", "cat <<1\nA_TOKEN=1\nB=2\n", "cat <<_\nA_TOKEN=1\n_\nB=2\n", "cat <<'E OF'\nA_TOKEN=1\nB=2\n", "cat <<$X\nA_TOKEN=1\nB=2\n", "cat <<\u00a0EOF\nA_TOKEN=1\nEOF\nB=2\n", "cat <<EOF'\nA_TOKEN=1\nEOF\nB=2\n", "cat <<<EOF\nA_TOKEN=1\n", "cat <<<<EOF\nA_TOKEN=1\n", "cat < <EOF\nA_TOKEN=1\n", "cat <<(x)\nA_TOKEN=1\n", 'echo "<<EOF"\nA_TOKEN=1\n', "echo '<<EOF'\nA_TOKEN=1\n", "echo \\<<EOF\nA_TOKEN=1\n", "((1<<EOF\nA_TOKEN=1\n))\nB=2\n", "(( 1 )) <<EOF\nA_TOKEN=1\nEOF\nB=2\n", "x=$(cat <<EOF)\nA_TOKEN=1\nEOF\n)\nB=2\n", "echo ${x:-<<EOF\nA_TOKEN=1\nEOF\n}\nB=2\n",
      "cat <<EOF\nfoo \\\nexport A_TOKEN=1\nexport B_TOKEN=2\nEOF\n", "cat <<EOF\nfoo \\\\\nexport A_TOKEN=1\nEOF\n", 'cat <<EOF\nA_TOKEN="a\nb"\nC=1\nEOF\nnext\n', "cat <<EOF\nA_TOKEN=1\nEOF\ncat <<EOF\nB_TOKEN=2\nEOF\n", "cat <<-EOF\n\tA_TOKEN=1\n\tEOF\nB_TOKEN=2\n"]),
    run('cat <<EOF\nA_TOKEN="a\nb"\nC=1\nEOF\nnext\n', null, true), run("cat <<EOF\nA_TOKEN=1\nB=2\n", null, true),
    // G12: comments next to a cut command
    ...["export A_TOKEN=x # keep me\n", "export A_TOKEN=x # c1\nexport B_TOKEN=y # c2\n", "export A_TOKEN=x   \nnext\n", "export A_TOKEN=1;\n", "export A_TOKEN=1;;\n", "export A_TOKEN=1 # c\n  # d\nnext\n", "export A_TOKEN=1 # c; cat <<EOF\nnext\n", "export A_TOKEN=x # it's\nnext\n", 'export A_TOKEN="a\nb" # c\nnext\n'].map(t => run(t, null, true)),
    ...login(["x; # plain0\n", "echo a \t # plain0\nnext\n", "echo a   # plain0\nnext\n", "echo a;# plain0\nnext\n", "a;b # plain0\n", "export A_TOKEN=1 # plain0\n", "  # plain0\n  x\n", "# plain0\n", "x # plain0 \"\ny\n", 'echo "a # plain0"\n', "echo a#plain0\n", "x && y # plain0\n", "plain0 # c\n"]),
    // G13: where the cut command sits
    ...["f() export A_TOKEN=1\n", "{export A_TOKEN=1;}\n", "{ export A_TOKEN=1 }\n", "(export A_TOKEN=1)\n", "(export A_TOKEN=1; echo hi)\n", "f() { export A_TOKEN=1; }\n", "{ export A_TOKEN=1; }; x\n", "[[ -n $x ]] && export A_TOKEN=1\n", 'echo ok | export A_TOKEN="a\nb" | cat\n', "a | export A_TOKEN=1\n", "export A_TOKEN=1 | cat\n", "cmd 2>&1 && export A_TOKEN=1\n", "cmd &>f && export A_TOKEN=1 || y\n", "x >| f; export A_TOKEN=1\n", "cmd <&3; export A_TOKEN=1\n", "cmd >&2; export A_TOKEN=1\n", "else export A_TOKEN=1\n", "time export A_TOKEN=1\n", "! export A_TOKEN=1\n", "export A_TOKEN=1 & export B_TOKEN=2\n", "begin; set -gx A_TOKEN 1; end\n", "function f; set -gx A_TOKEN 1; end\n", "export A_TOKEN=1 &\n", "export A_TOKEN=1; export B_TOKEN=2\n", "export A_TOKEN=1 && export B_TOKEN=2\n", "echo $(export A_TOKEN=1)\n", "x=1 && A_TOKEN=2 || true\n", "  \texport A_TOKEN=1\n", "if x; then :; else export A_TOKEN=1; fi\n", "export B=1; export A_TOKEN=\"a\nb\"; export C=1\n", "export A_TOKEN=${x:-\n  y}; z\n", "export A_TOKEN=`a\nb`; z\n", "export A_TOKEN=(a\nb); z\n", 'export A_TOKEN="a\nb" && echo ok\n', 'echo ok && export A_TOKEN="a\nb"\n', "(( x = 1 +\nexport A_TOKEN=2 ))\nnext\n", "export A_TOKEN=1; (( y = 1 +\nz ))\nnext\n", "case x in\n a|b) export A_TOKEN=1 ;;\nesac\n", "case x in\n  a) export A_TOKEN=\"a\nb\" ;;\nesac\n", "x) export A_TOKEN=1 ;& \n"].map(t => run(t, null, true)),
    ...login(["if (plain0); then y; fi\n", "if { plain0; }; then y; fi\n", "if $(plain0); then y; fi\n", "if plain0 \n{\ny\n}\n", "if [[ plain0 ]] \n{\ny\n}\n", "[[ $x = plain0 ]] && y\n", "[[ $x = plain0 ]]\n", "x && [[ $x = plain0 ]] && y\n", "x || plain0\n", "x ; plain0 ; y\n", "plain0 & y\n", "{ plain0; } && y\n", "echo $(\n plain0\n)\n", "echo `\n plain0\n`\n", "echo ${x:-\nplain0}\n", 'echo "a\nplain0"\n', "plain0\n", "  plain0\n", "x\n  plain0 y\nz\n", "plain0 && y\n", "plain0; y\n", "x; plain0\n", "plain0 &\n", 'echo "plain0"; y\n', "echo 'plain0 \n x'; y\n"]),
    // G14: guards
    ...['if A_TOKEN="a\nb"; then y; fi\n', "if A_TOKEN=$(\n cmd\n); then y; fi\n", "while A_TOKEN=$(\n cmd\n); do y; done\n", "until A_TOKEN=$(\n cmd\n); do y; done\n"].map(t => run(t, null, true)),
    ...login(["if x && plain0; then y; fi\n", "if x | plain0; then y; fi\n", "if plain0 && x; then y; fi\n", "if plain0 | x; then y; fi\n", "if ! x; then plain0; fi\n", "if x; then if plain0; then y; fi; fi\n", "if time plain0; then y; fi\n", "echo a; elif plain0 x; then y; fi\n", "echo a; if plain0 x; then y; fi\n", "if x; plain0; then y; fi\n", "if\nplain0; then y; fi\n", "if plain0 {\ny\n}\n", "while plain0 {\ny\n}\n", "if ! plain0; then y; fi\n", "if plain0 \\\n  x; then y; fi\n", "echo if plain0\n"]),
    // G15: for, select, case heads
    ...login(["x; for plain0 in a; do y; done\n", "for\tplain0 in a; do y; done\n", "select plain0 in a; do y; done\n", "case plain0 in\n a) y;;\nesac\n", "forx plain0\n", "for f in a; do plain0; done\n", "for plain0; do y; done\n", "for plain0 in a; do y; done\n"]),
    // G16: indent, CRLF
    ...["if x; then\r\n  export A_TOKEN=\"a\r\nb\"\r\nfi\r\n", '  export A_TOKEN="a\n\n\nb"\nnext\n', "  export A_TOKEN=1 \\\n  --flag\nnext\n", "\texport A_TOKEN=<<EOF\nbody\nEOF\nnext\n", "if x; then export A_TOKEN=1 \\\n  --flag; fi\nnext\n"].map(t => run(t, null, true)),
    run('export A_TOKEN="a\r\nb"\r\nnext\n', null, false),
    // G17: names
    ...plainRows(["export É_TOKEN=1\nnext\n", "export xÉ_TOKEN=1\nnext\n", "export ÀPI_KEY=1\nnext\n", "export Token=1\nnext\n", "export mytoken=1\nnext\n", "export MyToken=1\nnext\n", "export my_Token=1\nnext\n", "export api_key=1\nnext\n", "export APIKEY=1\nnext\n", "set -gx token 1\nnext\n", "export HISTKEY=1\nnext\n", "export PASSWORD_STORE_DIR=~/.pass PASSWORD=1\nnext\n", "export PASSWORD_STORE_DIR=~/.pass\nnext\n", "A_TOKEN=1 A_TOKEN=2 B_TOKEN=3 A_TOKEN=4\n", "export A_TOKEN=é\nnext\n"]),
  ];

  fixture["strip_exports_calls"] = [
    calls("echo sk-1\nnext\n", SK, false), calls("export FOO=sk-1 BAR=2\nnext\n", SK, false), calls("export A=1; export B=sk-2\n", SK, true), calls("echo ok; echo sk-1\n", SK, false),
  ];

  write("shell_rc_strip", fixture);
});
