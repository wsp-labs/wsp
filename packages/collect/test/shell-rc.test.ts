// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { FISH_CONF_D, RC_NAMES, RC_PATHS, bareSources, isRcPath, isSecretName, rcFiles, sourcedPaths, stripExports } from "../src/index.js";

describe("shell rc exports", () => {
  it("names the rc files the shell rung carries, plus fish's config and its conf.d", () => {
    expect(RC_PATHS).toEqual([".zshrc", ".zshenv", ".zprofile", ".zlogin", ".bashrc", ".bash_profile", ".profile", ".inputrc", ".aliases", ".zsh_aliases", ".config/fish/config.fish"]);
    expect(FISH_CONF_D).toBe(".config/fish/conf.d");
    expect([...RC_NAMES]).toEqual([".zshrc", ".zshenv", ".zprofile", ".zlogin", ".bashrc", ".bash_profile", ".profile", ".inputrc", ".aliases", ".zsh_aliases", "config.fish"]);
    expect(rcFiles(["work.fish", "notes.txt", "env.fish"])).toEqual([...RC_PATHS, ".config/fish/conf.d/work.fish", ".config/fish/conf.d/env.fish"]);
    for (const rel of [".zshrc", ".config/fish/config.fish", ".config/fish/conf.d/work.fish"]) expect(isRcPath(rel), rel).toBe(true);
    for (const rel of ["x/.zshrc", ".config/fish/conf.d/sub/deep.fish", ".config/fish/conf.d/notes.txt", ".config/fish/functions/x.fish", "config.fish"]) expect(isRcPath(rel), rel).toBe(false);
  });

  it("the paths an rc file sources by a literal path: ~, $HOME and absolute forms, quoted or not, behind a test or a keyword; a variable, a glob, a substitution, a comment or an alias body is not followed", () => {
    const text = [
      "source ~/.zsh/secrets.zsh",
      '. "$HOME/.zsh/work.zsh"',
      "[ -f ~/.zsh/local.zsh ] && source ~/.zsh/local.zsh",
      "if [ -f ${HOME}/.zsh/if.zsh ]; then . ${HOME}/.zsh/if.zsh; fi",
      "source /Users/dev/.zsh/abs.zsh 2>/dev/null",
      "source '/Users/dev/.zsh/quoted.zsh'",
      "source $ZDOTDIR/.zshrc",
      "source ~/.zsh/*.zsh",
      "for f in ~/.zsh/*.zsh; do source $f; done",
      'source "$(brew --prefix)/etc/x"',
      "# source ~/.zsh/commented.zsh",
      "alias s='source ~/.zsh/aliased.zsh'",
      "source relative/path.zsh",
      "source ~/.zsh/secrets.zsh",
      '\\. "$HOME/.nvm/nvm.sh"',
    ].join("\n");
    expect(sourcedPaths(text, "/Users/dev")).toEqual(["/Users/dev/.zsh/secrets.zsh", "/Users/dev/.zsh/work.zsh", "/Users/dev/.zsh/local.zsh", "/Users/dev/.zsh/if.zsh", "/Users/dev/.zsh/abs.zsh", "/Users/dev/.zsh/quoted.zsh", "/Users/dev/.nvm/nvm.sh"]);
  });

  it("the alias fallback and the source guard read the same files: nvm's escaped dot counts, a dot segment, an empty segment or a token cut by a backslash or a quote does not", () => {
    const nvm = '\\. "$HOME/.nvm/nvm.sh"\n';
    expect(sourcedPaths(nvm, "/Users/dev")).toEqual(["/Users/dev/.nvm/nvm.sh"]);
    expect(bareSources(nvm, "/Users/dev")).toEqual(["~/.nvm/nvm.sh"]);
    for (const line of ["source ~/../etc/profile\n", "source /Users/dev/../etc/profile\n", "source ~//x\n", ". ~/.zsh//x\n", "source ~/.zsh/my\\ file.zsh\n", 'source ~/.x"y"\n']) {
      expect(sourcedPaths(line, "/Users/dev"), line).toEqual([]);
      expect(bareSources(line, "/Users/dev"), line).toEqual([]);
    }
  });

  it("names that mean a secret, as whole words between underscores", () => {
    expect(["ANTHROPIC_API_KEY", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "DB_PASSWORD", "KEY", "npm_token", "OPENAI_APIKEY"].map(isSecretName)).toEqual(Array<boolean>(7).fill(true));
    expect(["KEYTIMEOUT", "PATH", "EDITOR", "SECRETS_DIR", "TOKENIZERS_PARALLELISM"].map(isSecretName)).toEqual(Array<boolean>(5).fill(false));
  });

  it("lists the names and cuts the lines, exported or plain, single or multiple per line", () => {
    const text = "export PATH=$HOME/bin:$PATH\nexport ANTHROPIC_API_KEY=sk-redacted\nexport KEYTIMEOUT=1\nGITHUB_TOKEN=ghp_redacted\n  export A=1 DB_PASSWORD=x\nalias ll='ls -l'\n";
    expect(stripExports(text)).toEqual({
      names: ["ANTHROPIC_API_KEY", "GITHUB_TOKEN", "DB_PASSWORD"],
      carried: "export PATH=$HOME/bin:$PATH\nexport KEYTIMEOUT=1\nalias ll='ls -l'\n",
    });
  });

  it("in a script a cut command stands as true at its own indent, so a block it was the only line of is never empty; a cut comment or heredoc line leaves nothing, and outside a script nothing stands", () => {
    const login = (l: string): string | undefined => (l.includes("plain0") ? "a login" : undefined);
    const text = 'if [ -f ~/.work ]; then\n  export GH_TOKEN=x\nfi\ndb() {\n\texport API_KEY="a\nb"\n}\n';
    expect(stripExports(text, login, [], true)).toEqual({ names: ["GH_TOKEN", "API_KEY"], carried: "if [ -f ~/.work ]; then\n  true\nfi\ndb() {\n\ttrue\n}\n" });
    expect(stripExports(text, login).carried).toBe("if [ -f ~/.work ]; then\nfi\ndb() {\n}\n");
    expect(stripExports("cat <<EOF\nplain0\nEOF\n", login, [], true).carried).toBe("cat <<EOF\nEOF\n");
    expect(stripExports("# psql plain0\nalias g=git\n", login, [], true).carried).toBe("alias g=git\n");
    expect(stripExports("case $OSTYPE in\n  darwin*) export GH_TOKEN=x ;;\n  *) export A=1 ;;\nesac\n", login, [], true).carried).toBe("case $OSTYPE in\n  darwin*) true ;;\n  *) export A=1 ;;\nesac\n");
  });

  it("in a script a cut command that shares its line stands as true and the rest of the line stays, the block's keyword or terminator with it", () => {
    const login = (l: string): string | undefined => (l.includes("plain0") ? "a login" : undefined);
    const cut = (text: string): string => stripExports(text, login, [], true).carried;
    expect(cut("if x; then\n  export GH_TOKEN=x; fi\n")).toBe("if x; then\n  true; fi\n");
    expect(cut("for f in a; do\n  export LAB_TOKEN=x; done\n")).toBe("for f in a; do\n  true; done\n");
    expect(cut("if x\nthen export GH_TOKEN=x\n  y\nfi\n")).toBe("if x\nthen true\n  y\nfi\n");
    expect(cut("{ export API_KEY=x; export IN=1; }\ndb() { export API_KEY=x; }\n")).toBe("{ true; export IN=1; }\ndb() { true; }\n");
    expect(cut("case $x in\n  a)\n    export GH_TOKEN=x ;;\n  b) export B_TOKEN=y ;&\n  c) export C_TOKEN=z ;|\nesac\n")).toBe("case $x in\n  a)\n    true ;;\n  b) true ;&\n  c) true ;|\nesac\n");
    expect(cut('if x; then\n  export API_KEY="a\nb"; export IN=1; fi\n')).toBe("if x; then\n  true; export IN=1; fi\n");
    expect(cut("[ -n x ] && export GH_TOKEN=x || y\n")).toBe("[ -n x ] && true || y\n");
    expect(stripExports("if x; then export A=1 GH_TOKEN=x; elif y; then eval 'export B_KEY=1; z'; fi\n", login, [], true)).toEqual({ names: ["GH_TOKEN", "B_KEY"], carried: "if x; then true; elif y; then true; fi\n" });
  });

  it("a cut command holds together through its substitutions, quotes, arrays and redirects, and a cut that cannot split stands as one true", () => {
    const login = (l: string): string | undefined => (l.includes("plain0") ? "a login" : undefined);
    const cut = (text: string): string => stripExports(text, login, [], true).carried;
    expect(cut('  export GH_TOKEN="$(pass show "gh; x" | head -1)" 2>&1 && y; fi\n')).toBe("  true && y; fi\n");
    expect(cut("export A_TOKEN=$((1 + 2)) `echo ;` $'a\\';' ${X:-;}; fi\n")).toBe("true; fi\n");
    expect(cut("typeset -a API_KEY=(a b); fi\nif test -n x; set -gx GH_TOKEN (pass show gh); end\n")).toBe("true; fi\nif test -n x; true; end\n");
    expect(cut("export GH_TOKEN=x; cat <<EOF; fi\nbody\nEOF\nnext\n")).toBe("true\nnext\n");
    expect(cut("  y; fi # plain0\n")).toBe("  y; fi\n");
    expect(cut("for t in plain0; do y; done\ncase $x in (a) y plain0;; (b) z;; esac\n")).toBe("true\n");
  });

  it("in a script what a cut command decided never runs: a condition holding one stands as false, or true after until, and a list it leads into stands as one true; a cut assignment answers 0, so it stays true", () => {
    const login = (l: string): string | undefined => (l.includes("plain0") ? "a login" : undefined);
    const cut = (text: string): string => stripExports(text, login, [], true).carried;
    expect(cut('if [ "$T" = plain0 ]; then\n  y\nfi\n')).toBe("if false; then\n  y\nfi\n");
    expect(cut("if ! [[ -z x || $T = plain0 ]]; then y; elif x && test $T = plain0; then y; fi\n")).toBe("if false; then y; elif false; then y; fi\n");
    expect(cut('while [ "$T" = plain0 ]; do y; done\nuntil ! [[ -z x && $T = plain0 ]]; do y; done\nwhile x; do y plain0; done\n')).toBe("while false; do y; done\nuntil true; do y; done\nwhile x; do true; done\n");
    expect(cut("if [[ $T = plain0 ]] { y } elif [[ $T = plain0 ]] { y }\nuntil [[ $T = plain0 ]] { y }\n")).toBe("if [[ -n '' ]] { y } elif [[ -n '' ]] { y }\nuntil [[ -z '' ]] { y }\n");
    expect(cut('[ "$T" != plain0 ] && return\ncurl -fs https://plain0@h || y\necho plain0 | gh auth login --with-token\n  x && grep -q plain0 f && y; z\n')).toBe("true\ntrue\ntrue\n  true; z\n");
    expect(cut("if export GH_TOKEN=x; then y; fi\nexport GH_TOKEN=x && y\nGH_TOKEN=$(pass show gh) || y\nGH_TOKEN=x gh auth status && y\n")).toBe("if true; then y; fi\ntrue && y\ntrue\ntrue\n");
  });

  it("an assignment behind a precommand word, a declaration builtin or fish's and, or and not is cut, the word staying, and a command behind one is kept; fish's long flags assign too, and a set that queries, erases or shows assigns nothing", () => {
    const text = [
      "time -p export A_TOKEN=x",
      "noglob builtin typeset -x B_TOKEN=x",
      "nocorrect export C_TOKEN=x",
      "- export D_TOKEN=x",
      "command -p export E_TOKEN=x",
      "exec -a lab env F_TOKEN=x make",
      "float -x G_KEY=x",
      "integer H_KEY=1",
      "private I_TOKEN=x",
      "and set -gx J_TOKEN x",
      "or set -x K_TOKEN x",
      "not set -gx L_TOKEN x",
      "set --global --export M_TOKEN x",
      "and set -gx --path N_TOKEN x",
      "if not set -q GH_TOKEN",
      "set -qg GH_TOKEN; or echo none",
      "set --erase GH_TOKEN; set -S GH_TOKEN",
      "time make && noglob git status",
      "and git status; or command -v git",
      "",
    ].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["A_TOKEN", "B_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "F_TOKEN", "G_KEY", "H_KEY", "I_TOKEN", "J_TOKEN", "K_TOKEN", "L_TOKEN", "M_TOKEN", "N_TOKEN"],
      carried: "time -p true\nnoglob builtin true\nnocorrect true\n- true\ncommand -p true\nexec -a lab true\ntrue\ntrue\ntrue\nand true\nor true\nnot true\ntrue\nand true\nif not set -q GH_TOKEN\nset -qg GH_TOKEN; or echo none\nset --erase GH_TOKEN; set -S GH_TOKEN\ntime make && noglob git status\nand git status; or command -v git\n",
    });
  });

  it("zsh's repeat and coproc, a backslash before the word and exec's bundled flags hide no assignment, and a condition behind a precommand word stands as false", () => {
    const text = [
      "repeat 1 export A_TOKEN=x",
      "coproc export B_TOKEN=x",
      "\\export C_TOKEN=x",
      "\\builtin export D_TOKEN=x",
      "exec -cla lab env E_TOKEN=x zsh",
      "while noglob F_TOKEN=x check; do sleep 1; done",
      "until time G_TOKEN=x check; do sleep 1; done",
      "repeat 3 echo hi",
      "",
    ].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["A_TOKEN", "B_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "F_TOKEN", "G_TOKEN"],
      carried: "repeat 1 true\ncoproc true\ntrue\n\\builtin true\nexec -cla lab true\nwhile false; do sleep 1; done\nuntil true; do sleep 1; done\nrepeat 3 echo hi\n",
    });
  });

  it("an assignment sudo, doas, nohup or nice passes on is cut, and a line naming the variable without a value stays", () => {
    const text = ["sudo -u root GH_TOKEN=x gh auth status", "doas GH_TOKEN=x cmd", "nohup env GH_TOKEN=x cmd &", "nice -n 5 env GH_TOKEN=x cmd", "sudo --preserve-env=GH_TOKEN gh auth status", "sudo printenv GH_TOKEN", ""].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["GH_TOKEN"],
      carried: "true\ntrue\ntrue &\ntrue\nsudo --preserve-env=GH_TOKEN gh auth status\nsudo printenv GH_TOKEN\n",
    });
  });

  it("a script with nothing to cut comes back byte for byte", () => {
    const text = "if [ -f ~/.work ]; then\n  export EDITOR=vim\nfi\n\nalias g=git\n";
    expect(stripExports(text, () => undefined, [], true)).toEqual({ names: [], carried: text });
  });

  it("a quoted value spanning lines and a backslash continuation are cut whole", () => {
    expect(stripExports('export A=1\nexport API_KEY="first\nsecond"\nexport B=2\n')).toEqual({ names: ["API_KEY"], carried: "export A=1\nexport B=2\n" });
    expect(stripExports("export DB_PASSWORD='p\nq\nr'\nalias x=y\n").carried).toBe("alias x=y\n");
    expect(stripExports("export A_TOKEN=abc\\\n  def\\\n  ghi\nexport C=3\n")).toEqual({ names: ["A_TOKEN"], carried: "export C=3\n" });
    expect(stripExports('export QUOTED="a\\"b"\nexport A_KEY="x\\"y\nz"\nlast\n').carried).toBe('export QUOTED="a\\"b"\nlast\n');
  });

  it("an array spanning lines under a secret name is cut whole, quotes, comments and nested parentheses in it held, while one under a plain name stays", () => {
    const text = [
      "API_KEY=(",
      "  hunter2-prod",
      "  hunter2-stage",
      ")",
      "declare -a GH_TOKEN=(",
      '  "a ) b"',
      "  'c)'",
      "  $(pass show gh)",
      ")",
      "typeset -a LAB_SECRET=(",
      "  x # note (",
      "  *(.)",
      ")",
      "LAB_TOKEN+=(",
      "  more",
      ")",
      "if x; then",
      "  DB_PASSWORD=(",
      "    p # (",
      "  ); export IN=1; fi",
      "PLAIN=(",
      "  one",
      "  two",
      ")",
      "",
    ].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["API_KEY", "GH_TOKEN", "LAB_SECRET", "LAB_TOKEN", "DB_PASSWORD"],
      carried: "true\ntrue\ntrue\ntrue\nif x; then\n  true; export IN=1; fi\nPLAIN=(\n  one\n  two\n)\n",
    });
    expect(stripExports("PLAIN=(\n  one\n  two\n)\nLIST=( a\n  b )\n", () => undefined, [], true).carried).toBe("PLAIN=(\n  one\n  two\n)\nLIST=( a\n  b )\n");
  });

  it("an array under a secret name is cut whole past a comment right after its =(, a ) inside ${...} and a ) quoted inside $(...)", () => {
    const text = [
      "API_KEY=(#prod)",
      "  hunter2-prod",
      ")",
      "DB_PASSWORD=(#it's here",
      "  hunter2-db",
      ")",
      "GH_TOKEN=(",
      "  ${x%)}",
      "  hunter2-gh",
      ")",
      "LAB_SECRET=(",
      '  $(op read "op://Private/Lab (work)/key")',
      "  hunter2-lab",
      ")",
      "export AFTER=1",
      "",
    ].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["API_KEY", "DB_PASSWORD", "GH_TOKEN", "LAB_SECRET"],
      carried: "true\ntrue\ntrue\ntrue\nexport AFTER=1\n",
    });
  });

  it("CRLF files are stripped too and keep their line endings", () => {
    expect(stripExports("export A_TOKEN=x\r\nexport B=1\r\n")).toEqual({ names: ["A_TOKEN"], carried: "export B=1\r\n" });
  });

  it("typeset, declare and fish set are assignments as well", () => {
    const text = "typeset -gx ANTHROPIC_API_KEY=sk-x\ndeclare -x DB_PASSWORD=pw\ndeclare -rx EDITOR=vim\nset -gx OPENAI_API_KEY sk-y\nset -g fish_greeting ''\nset -Ux HOMEBREW_GITHUB_API_TOKEN t\n";
    expect(stripExports(text)).toEqual({
      names: ["ANTHROPIC_API_KEY", "DB_PASSWORD", "OPENAI_API_KEY", "HOMEBREW_GITHUB_API_TOKEN"],
      carried: "declare -rx EDITOR=vim\nset -g fish_greeting ''\n",
    });
  });

  it("heredocs, command substitutions and backticks spanning lines are cut whole", () => {
    expect(stripExports("export API_KEY=$(cat <<EOF\nsecret-value\nEOF\n)\nexport B=2\n")).toEqual({ names: ["API_KEY"], carried: "export B=2\n" });
    expect(stripExports("export A_TOKEN=$(\n  op read x\n)\nnext\n").carried).toBe("next\n");
    expect(stripExports("export DB_PASSWORD=`cat \n  file`\nnext\n").carried).toBe("next\n");
    expect(stripExports('export X_SECRET="$(cat <<-EOT\n\tv\n\tEOT\n)"\nnext\n').carried).toBe("next\n");
    expect(stripExports("cat <<EOF\nexport NOT_A_TOKEN=inside-heredoc\nPLAIN=1\nEOF\n")).toEqual({ names: ["NOT_A_TOKEN"], carried: "cat <<EOF\nPLAIN=1\nEOF\n" });
  });

  it("readonly, local, a quoted assignment and a flagless fish set are assignments too", () => {
    expect(stripExports('readonly MY_TOKEN=x\nlocal DB_PASSWORD=y\nexport "GH_TOKEN=z"\nexport \'API_KEY=w\'\nset MY_KEY val\nset plain val\nreadonly PLAIN=1\n')).toEqual({
      names: ["MY_TOKEN", "DB_PASSWORD", "GH_TOKEN", "API_KEY", "MY_KEY"],
      carried: "set plain val\nreadonly PLAIN=1\n",
    });
  });

  it("a comment's stray quote, an arithmetic shift or a stray backtick on a kept line never hides a later export", () => {
    const template = "# Set list of themes to pick from when loading at random\n# Setting this variable when ZSH_THEME=random will cause zsh to load\n# a theme from this variable instead of looking in $ZSH/themes/\n# If set to an empty array, this variable will have no effect.\n# ZSH_THEME_RANDOM_CANDIDATES=( \"robbyrussell\" \"agnoster\" )\n# Uncomment the following line to use case-sensitive completion.\n# Caution: this setting can cause issues with multiline prompts in zsh < 5.7.1 (see #5765)\n# Uncomment one of the following lines to change the auto-update behavior\n# zstyle ':omz:update' mode disabled  # disable automatic updates\n# Uncomment if you don't want to see the prompt.\nplugins=(git)\nexport ANTHROPIC_API_KEY=sk-ant-fake\n";
    expect(stripExports(template)).toEqual({ names: ["ANTHROPIC_API_KEY"], carried: template.replace("export ANTHROPIC_API_KEY=sk-ant-fake\n", "") });
    expect(stripExports("(( y = x << 2 ))\nexport GH_TOKEN=fake\nnext\n")).toEqual({ names: ["GH_TOKEN"], carried: "(( y = x << 2 ))\nnext\n" });
    expect(stripExports("# see `man zsh\nexport DB_PASSWORD=fake\nnext\n")).toEqual({ names: ["DB_PASSWORD"], carried: "# see `man zsh\nnext\n" });
    expect(stripExports("echo 'a' # it's fine\nexport A_TOKEN=fake\n").names).toEqual(["A_TOKEN"]);
  });

  it("a here-string or a shift inside arithmetic on a kept line opens nothing, so the export after it is still cut", () => {
    expect(stripExports("read x <<< hello\nexport A_TOKEN=fake\nnext\n")).toEqual({ names: ["A_TOKEN"], carried: "read x <<< hello\nnext\n" });
    expect(stripExports('cat <<< "hello"\nexport B_TOKEN=fake\n').names).toEqual(["B_TOKEN"]);
    expect(stripExports("(( f = 1 << SHIFT ))\nexport C_TOKEN=fake\nnext\n")).toEqual({ names: ["C_TOKEN"], carried: "(( f = 1 << SHIFT ))\nnext\n" });
    expect(stripExports("(( f = 1 << SHIFT ))\ncat <<EOF\nexport NOT_TOKEN=inside\nEOF\nexport D_TOKEN=fake\n")).toEqual({ names: ["NOT_TOKEN", "D_TOKEN"], carried: "(( f = 1 << SHIFT ))\ncat <<EOF\nEOF\n" });
    expect(stripExports("read x <<< $y\nexport E_TOKEN=fake\n").names).toEqual(["E_TOKEN"]);
  });

  it("an arithmetic block spanning kept lines opens no heredoc on its later line", () => {
    expect(stripExports("(( f = 1 +\n  (1 << SHIFT) ))\nexport A_TOKEN=fake\nnext\n")).toEqual({ names: ["A_TOKEN"], carried: "(( f = 1 +\n  (1 << SHIFT) ))\nnext\n" });
  });

  it("a zsh glob flag or a brace expansion holding # is not a comment, so an export after it on the same line is still cut", () => {
    expect(stripExports("[[ $x == (#i)yes ]] && export A_TOKEN=fake\nnext\n")).toEqual({ names: ["A_TOKEN"], carried: "next\n" });
    expect(stripExports("echo {#a,b}; export B_TOKEN=fake\nnext\n")).toEqual({ names: ["B_TOKEN"], carried: "next\n" });
    expect(stripExports("echo {#a,b} # it's\nexport C_TOKEN=fake\n")).toEqual({ names: ["C_TOKEN"], carried: "echo {#a,b} # it's\n" });
  });

  it("(( inside double quotes counts for nothing; an unbalanced (( suppresses heredoc recognition until a )), which over-cuts and never leaks", () => {
    const text = 'echo "(("\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport A_TOKEN=fake\nnext\n';
    expect(stripExports(text)).toEqual({ names: ["NOT_TOKEN", "A_TOKEN"], carried: 'echo "(("\ncat <<EOF\nEOF\nnext\n' });
    expect(stripExports("(( x = 1\nl1\nl2\nl3\nl4\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport B_TOKEN=fake\n")).toEqual({ names: ["NOT_TOKEN", "B_TOKEN"], carried: "(( x = 1\nl1\nl2\nl3\nl4\ncat <<EOF\nEOF\n" });
    expect(stripExports('echo "$((1+2))"\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport C_TOKEN=fake\n')).toEqual({ names: ["NOT_TOKEN", "C_TOKEN"], carried: 'echo "$((1+2))"\ncat <<EOF\nEOF\n' });
  });

  it("an arithmetic block spanning kept lines keeps << a shift on every one of them, however long", () => {
    expect(stripExports("((\na = 1 +\nb << SHIFT\n))\nexport A_TOKEN=fake\nnext\n")).toEqual({ names: ["A_TOKEN"], carried: "((\na = 1 +\nb << SHIFT\n))\nnext\n" });
    expect(stripExports("((\na = 1 +\nb = 2 +\nc = 3 +\nd << SHIFT\n))\nexport B_TOKEN=fake\nexport C_KEY=fake\nalias x=y\n")).toEqual({ names: ["B_TOKEN", "C_KEY"], carried: "((\na = 1 +\nb = 2 +\nc = 3 +\nd << SHIFT\n))\nalias x=y\n" });
  });

  it("the arithmetic depth is one stream across cut and kept lines", () => {
    expect(stripExports("export A_TOKEN=fake; (( y = 1 +\nexport B_KEY=fake\n")).toEqual({ names: ["A_TOKEN", "B_KEY"], carried: "" });
    expect(stripExports("export A_TOKEN=fake; (( y = 1 +\nz << SHIFT ))\nexport B_KEY=fake\nnext\n")).toEqual({ names: ["A_TOKEN", "B_KEY"], carried: "z << SHIFT ))\nnext\n" });
    expect(stripExports("(( x = 1 +\nexport A_TOKEN=fake )); y=2\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport B_KEY=fake\n")).toEqual({ names: ["A_TOKEN", "NOT_TOKEN", "B_KEY"], carried: "(( x = 1 +\ncat <<EOF\nEOF\n" });
    expect(stripExports('(( x = 1 +\nexport A_TOKEN="a\nb" )); y=2\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport B_KEY=fake\n')).toEqual({ names: ["A_TOKEN", "NOT_TOKEN", "B_KEY"], carried: "(( x = 1 +\ncat <<EOF\nEOF\n" });
    expect(stripExports("(( x = 1 +\nexport A_TOKEN=fake +\n2 << SHIFT ))\nexport B_TOKEN=fake\nnext\n")).toEqual({ names: ["A_TOKEN", "B_TOKEN"], carried: "(( x = 1 +\n2 << SHIFT ))\nnext\n" });
  });

  it("a parameter expansion spanning lines and a backslash-quoted heredoc word are cut whole", () => {
    expect(stripExports("export A_TOKEN=${SECRET:-\nfake}\nnext\n")).toEqual({ names: ["A_TOKEN"], carried: "next\n" });
    expect(stripExports("export API_KEY=$(cat <<\\EOF\nline)\nfake\nEOF\n)\nnext\n")).toEqual({ names: ["API_KEY"], carried: "next\n" });
    expect(stripExports("cat <<\\EOF\nexport NOT_TOKEN=inside\nEOF\nexport F_TOKEN=fake\n")).toEqual({ names: ["NOT_TOKEN", "F_TOKEN"], carried: "cat <<\\EOF\nEOF\n" });
  });

  it("a comment right after ; & or ( on a cut line ends the scan there, and ${#var} is a length, not a comment", () => {
    expect(stripExports("export A_TOKEN=x;# don't\nalias a=b\nexport B_KEY=y &# it's\nalias c=d\n")).toEqual({ names: ["A_TOKEN", "B_KEY"], carried: "alias a=b\nalias c=d\n" });
    expect(stripExports("export A_TOKEN=${#x}rest\nnext\n")).toEqual({ names: ["A_TOKEN"], carried: "next\n" });
    expect(stripExports("echo ${#arr}\nexport B_KEY=v\nnext\n")).toEqual({ names: ["B_KEY"], carried: "echo ${#arr}\nnext\n" });
  });

  it("a cut line's trailing comment does not swallow the lines after it", () => {
    expect(stripExports("export A_TOKEN=fake # don't share\nalias b=c\nexport X_SECRET=fake # `note\nalias d=e\n")).toEqual({ names: ["A_TOKEN", "X_SECRET"], carried: "alias b=c\nalias d=e\n" });
  });

  it("an export that is not the first word of the line is found and the whole line cut", () => {
    const shapes = ['[ -z "$X" ] && export FOO_TOKEN=lit', "if true; then export BAR_KEY=lit; fi", "cd /tmp && export BAZ_SECRET=lit", "env QUX_TOKEN=lit somecommand", "{ export A_KEY=lit; }", "alias x='y'; export E_TOKEN=lit", "LAB=1 H_TOKEN=lit cmd"];
    const names = ["FOO_TOKEN", "BAR_KEY", "BAZ_SECRET", "QUX_TOKEN", "A_KEY", "E_TOKEN", "H_TOKEN"];
    for (const [i, line] of shapes.entries()) {
      expect(stripExports(`${line}\nnext\n`), line).toEqual({ names: [names[i]], carried: "next\n" });
    }
    expect(stripExports("exec env J_TOKEN=lit cmd\nnext\n")).toEqual({ names: ["J_TOKEN"], carried: "next\n" });
    expect(stripExports("eval 'export I_TOKEN=lit'\nnext\n")).toEqual({ names: ["I_TOKEN"], carried: "next\n" });
    expect(stripExports('eval "cd /tmp; export K_SECRET=lit"\ncommand export L_KEY=lit\nbuiltin export M_TOKEN=lit\nnext\n')).toEqual({ names: ["K_SECRET", "L_KEY", "M_TOKEN"], carried: "next\n" });
    expect(stripExports("echo \"a && export NOT_TOKEN=1\"\n").names).toEqual([]);
  });

  it("an alias or abbr body is shell again, so a secret it assigns is cut like a bare one, and a body that only names one stays", () => {
    const text = [
      "alias gh='GH_TOKEN=lit gh'",
      'alias gl="GITLAB_TOKEN=lit glab"',
      "alias x=';export FAKE_TOKEN=1'",
      "alias -g DBP='DB_PASSWORD=lit' a1='A1_TOKEN=lit'",
      "\\alias bs='env BS_TOKEN=lit gh'",
      "alias -- dd='DD_TOKEN=lit gh'",
      "alias fa 'FA_TOKEN=lit gh'",
      "alias fs 'set -lx FS_TOKEN lit; gh'",
      "abbr -a ab 'AB_TOKEN=lit gh'",
      "abbr --add aw AW_TOKEN=lit gh",
      "abbr ap --position anywhere -- 'AP_TOKEN=lit'",
      "abbr -a -r '^g.+' ar 'AR_TOKEN=lit'",
      "abbr -ac git ac 'AC_TOKEN=lit'",
      "alias lp='LAB=1 LP_TOKEN=lit gh'",
      "if x; then alias in='IN_TOKEN=lit gh'; fi",
      "alias tok='printenv GH_TOKEN'",
      "alias ft 'printenv GH_TOKEN'",
      "abbr -a fe 'echo $GH_TOKEN'",
      "alias g=git",
      "",
    ].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["GH_TOKEN", "GITLAB_TOKEN", "FAKE_TOKEN", "DB_PASSWORD", "A1_TOKEN", "BS_TOKEN", "DD_TOKEN", "FA_TOKEN", "FS_TOKEN", "AB_TOKEN", "AW_TOKEN", "AP_TOKEN", "AR_TOKEN", "AC_TOKEN", "LP_TOKEN", "IN_TOKEN"],
      carried: [...Array<string>(14).fill("true"), "if x; then true; fi", "alias tok='printenv GH_TOKEN'", "alias ft 'printenv GH_TOKEN'", "abbr -a fe 'echo $GH_TOKEN'", "alias g=git", ""].join("\n"),
    });
  });

  it("a quoted builtin, set or variable name is read as the bare one, a case pattern's too", () => {
    const text = ['"export" A_TOKEN=lit', "ex'port' B_TOKEN=lit", 'export "C_TOKEN=lit"', 'export "D_TOKEN"=lit', "'typeset' -x E_TOKEN=lit", 'set -gx "F_TOKEN" lit', "\"set\" -gx G_TOKEN lit", "set -gx 'PLAIN' 1", 'export "PATH=$HOME/bin:$PATH"', "case $x in", '  "I_KEY="*) echo has ;;', "esac", ""].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["A_TOKEN", "B_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "F_TOKEN", "G_TOKEN", "I_KEY"],
      carried: ["true", "true", "true", "true", "true", "true", "true", "set -gx 'PLAIN' 1", 'export "PATH=$HOME/bin:$PATH"', "case $x in", "  true) echo has ;;", "esac", ""].join("\n"),
    });
  });

  it("a heredoc body line, or an eval or alias body over lines, that assigns a secret is cut like a bare assignment, and one that assigns none stays", () => {
    const text = [
      "cat > ~/.lab.env <<EOF",
      "API_KEY=lit",
      "PLAIN=1",
      "EOF",
      "cat >> ~/.lab.env <<'EOF'",
      "  export GH_TOKEN=lit",
      'DB_PASSWORD="lit',
      'lit"',
      "LAB=2",
      "EOF",
      "eval 'cd /tmp",
      "export LAB_TOKEN=lit'",
      'eval "',
      "  LAB_SECRET=lit",
      '"',
      "alias gh='cd /tmp",
      "AL_TOKEN=lit gh'",
      "eval 'echo it'\\''s",
      "export QT_TOKEN=lit'",
      "x=\\'; eval $'echo it\\'s",
      "export AC_TOKEN=lit'",
      "if x; then eval '",
      "  export IF_KEY=lit",
      "'; y; fi",
      "eval 'echo kept",
      "export EVAL_PLAIN=1'",
      "cat <<EOF",
      "KEYTIMEOUT=1",
      "it's",
      "EOF",
      "export AFTER=1",
      "",
    ].join("\n");
    expect(stripExports(text, undefined, [], true)).toEqual({
      names: ["API_KEY", "GH_TOKEN", "DB_PASSWORD", "LAB_TOKEN", "LAB_SECRET", "AL_TOKEN", "QT_TOKEN", "AC_TOKEN", "IF_KEY"],
      carried: ["cat > ~/.lab.env <<EOF", "PLAIN=1", "EOF", "cat >> ~/.lab.env <<'EOF'", "LAB=2", "EOF", "true", "true", "true", "true", "x=\\'; true", "if x; then true; y; fi", ...text.split("\n").slice(24)].join("\n"),
    });
    expect(stripExports("eval 'cd /tmp\nexport LAB_TOKEN=lit'\nnext\n")).toEqual({ names: ["LAB_TOKEN"], carried: "next\n" });
    for (const word of ["\\eval", '"eval"', "'eval'", "e\\val", "LANG=C eval"]) {
      expect(stripExports(`${word} 'export LAB_TOKEN=lit'\n${word} 'cd /tmp\nexport LAB_TOKEN=lit'\nnext\n`, undefined, [], true)).toEqual({ names: ["LAB_TOKEN"], carried: "true\ntrue\nnext\n" });
    }
    expect(stripExports("cat <<EOF\nexport \\\nAPI_KEY=lit\nEOF\nnext\n")).toEqual({ names: ["API_KEY"], carried: "cat <<EOF\nexport \\\n\nEOF\nnext\n" });
    const held: string[] = [];
    expect(stripExports("eval 'cd /tmp\necho tok-lit'\nnext\n", l => (l.includes("tok-") ? "a token" : undefined), held, true)).toEqual({ names: [], carried: "true\nnext\n" });
    expect(held).toEqual(["a token"]);
    expect(stripExports('X="$(cat <<EOF\nAPI_KEY=lit\nPLAIN=1\nEOF\n)"\nnext\n', undefined, [], true)).toEqual({ names: ["API_KEY"], carried: 'X="$(cat <<EOF\nPLAIN=1\nEOF\n)"\nnext\n' });
    const fish = "abbr -a gw 'git commit -m \\'wip\\''\nset -gx API_KEY lit\nabbr -a gs 'git status'\n";
    expect(stripExports(fish, undefined, [], true)).toEqual({ names: ["API_KEY"], carried: fish.replace("set -gx API_KEY lit", "true") });
  });

  it("ANSI-C quoting on a cut line closes at its own quote", () => {
    expect(stripExports("export D_PASSWORD=$'it\\'s'\nline1\nline2\nline3\n")).toEqual({ names: ["D_PASSWORD"], carried: "line1\nline2\nline3\n" });
  });
});
