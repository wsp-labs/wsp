// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/commands.md: commands.ts on the edge inputs of the wave-2 audit (group e).
// A drop argument is data: the list of names for which drop answers true. without_array_literals and call_of are
// private in the TS, so they are reached through called_commands.
import { it } from "vitest";
import { calledCommands, dropPlugins, guardCommands } from "../../../../packages/collect/src/detect/commands.js";
import { plain, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const called = (rows: string[]): Case[] => rows.map(t => ({ name: JSON.stringify(t), args: [t, "sh"], out: calledCommands(t) }));
const DROP = ["eza", "gcloud", "kubectl"];
const drop = (rows: string[], names = DROP): Case[] => rows.map(t => ({ name: JSON.stringify(t), args: [t, names], out: plain(dropPlugins(t, n => names.includes(n))) }));
const B = "# >>> wsp guard: commands this machine does not have >>>";
const E = "# <<< wsp guard <<<";
const guard = (name: string, text: string, missing: string[], dropped?: string[]): Case => ({ name, args: dropped === undefined ? [text, missing, []] : [text, missing, dropped], out: dropped === undefined ? guardCommands(text, missing) : guardCommands(text, missing, dropped) });

it("commands fixtures", () => {
  const fixture: Record<string, Case[]> = {};

  fixture["called_commands"] = [
    ...called([
      "git diff src/git\n", "ls ~/bin/ls\n", "tool a/tool\n", "tool a/b/tool2\n", "FOO=bar/git git\n", "tool x=a/tool\n", "git status a/b/git\n", "ls -la /usr/bin/ls\n", 'tool "a/tool"\n', "cat a/cat\n", "A=x/y tool\n", "./run\n", "/usr/bin/git status\n", "bin/tool x\n", "git status\n",
      "alias x='A=1 B=2 cmd'\n", "alias x='FOO=1 cmd'\n", "alias x='A=1 cmd' y='B=2 cmd2'\n", "alias a=b b=c\n", "alias a='b c' d='a e'\n", "alias x='git a/git'\n", "alias -g L='| less'\n", "alias x='eza -l' y\n", "alias ll='-x'\n", "alias x='cd /tmp'\n", "alias x='command -v git'\n", "alias x='git --version'\n", "alias ll\n", "alias\n", "alias x=\n", "alias -- ll=eza\n",
      "  alias ll=eza\n", "\talias ll=eza\n", "alias ll=eza # c\n", "alias ll='eza' && alias lg='lazygit'\n", "alias gs='git status'; gs\n", "alias ll='eza -l'\nll\n", "alias ll='sudo eza'\n",
      "autoload foo bar\nfoo\nbar\nbaz\n", "autoload -U\nU\n", "autoload +X foo\n", "autoload -Uz\n", "autoload -Uz compinit\ncompinit\n",
      "foo ()\n{\n  :\n}\nfoo\n", "function foo-bar { :; }\nfoo-bar\n", "function foo- { :; }\nfoo\n", "# foo() { :; }\nfoo\n", "cat <<EOF\nfoo() { :; }\nEOF\nfoo\n", "x; foo() { :; }\nfoo\n", "if x; then foo() { :; }; fi\nfoo\n", "  foo() { :; }\nfoo\n", "foo( )\nfoo\n", "function  foo\nfoo\n", "foo.bar() { :; }\nfoo.bar\n", "foo+bar() { :; }\nfoo+bar\n", "1f() { :; }\n1f\n", "function() { :; }\nfunction\n", "tool(){ :; }\ntool\n", "mytool\nmytool() { :; }\n",
      "arr+=(a b)\nafter\n", "arr[0]=(a)\nafter\n", "echo a=(b)\nafter\n", "--opt=(x) tool\n", "x==(a)\nb\n", "=(a)\nb\n", "x=( a ) y\n", "x=(a)y\n", "x=(a\\)b)\nafter\n", "x=('a)')\nafter\n", "x=(a 'b\\')c')\nafter\n", "x=(open\nnever\n", "x=$(a)\n", "x=((1+2))\n", "x=(a (b) c) tool\n", "x=(a\nb) tool\n", "# c =(x)\nafter\n", "x=(a)#c\nafter\n", "x=(a) #c\nafter\n", "x=(a # b )\nc)\nafter\n", 'x=("a)"\nb)\nafter\n', "a=(b (#c)\n)z\n", "arr=(\n  one # (c)\n  (two three)\n)\nafter\n", "x=(\n<<EOF\n)\nafter\n",
      "tool-x\n", "tool.x\n", "tool+x\n", "tool_x\n", "Tool\n", "1tool\n", "tool@x\n", "tool=1\n", "tool\nTOOL\ntool\n", "other\ntool\nother\n", "tool\r\nother\r\n", "tool a\n\n\ntool b\n", "command -v tool\n", "tool --version\n", "tool -v\n", "tool --help\n", "which tool\n", "command tool\n", 'FOO="a b" tool\n', 'echo "a $(tool x) b"\n', "sh -c 'tool x'\n", "cat <<EOF\ntool\nEOF\nafter\n", "cat <<-EOF\n\ttool\n\tEOF\nafter\n", "tool # c\n", "# tool\n", "tool \\\n  arg\nother\n", "echo $(tool x)\n", "echo `tool x`\n", 'echo "$(tool x)"\n', "if tool; then other; fi\n", "case x in a) tool;; esac\n", "[ -f x ] && tool\n", "export FOO=1 && tool\n", "zshz\n", "bat\n", "fd\n", "sched2\n", "compdescribe2\n", "",
    ]),
    { name: "fish yields nothing", args: ["tool\n", "fish"], out: calledCommands("tool\n", "fish") },
    { name: "fish config", args: ["set -gx EDITOR nvim\nstarship init fish | source\n", "fish"], out: [] },
  ];
  // Every member of RC_BUILTIN as a one-line file gives no call (the set is private, so the names are listed here).
  const RC = "autoload,bg,bindkey,bye,chdir,compadd,compdef,compdescribe,compinit,complete,compgen,compopt,coproc,dirs,disable,disown,echotc,echoti,emulate,enable,fc,fg,float,functions,getln,getopts,hash,history,in,integer,jobs,let,limit,logout,mapfile,nocorrect,noglob,popd,print,private,pushd,pushln,r,readarray,rehash,repeat,sched,select,setopt,shopt,suspend,times,ttyctl,umask,unalias,unfunction,unhash,unlimit,unsetopt,vared,whence,where,zcompile,zformat,zle,zmodload,zparseopts,zprof,zstyle,add-zsh-hook,add-zle-hook-widget,bashcompinit,colors,compaudit,is-at-least,promptinit,prompt,run-help,select-word-style,vcs_info,zargs,zcalc,zed,zmv,zrecompile,bracketed-paste-magic,url-quote-magic,edit-command-line,up-line-or-beginning-search,down-line-or-beginning-search,history-search-end".split(",");
  fixture["called_commands_rc_builtin"] = called(RC.map(n => `${n}\n`));

  fixture["drop_plugins"] = [
    ...drop([
      "plugins=(git) eza\n", "plugins=(git eza)\n", "plugins=(z) eza # eza\n", "plugins=(z) ; eza\n", "plugins=(a\n b) c eza\n", "plugins=(eza)(eza)\n", "plugins=(a (eza) b)\n", "plugins=((eza))\n", "plugins +=(eza z)\n", "plugins = (eza z)\n", "export plugins=(eza z)\n", "plugins=eza\n", "\tplugins+=(eza z)\n", "  plugins=(eza z)\n", "plugins=(eza z) plugins=(eza z)\n", "plugins=(z\n plugins=(eza z)\n)\n",
      'plugins=("eza" git)\n', "plugins=('eza' z)\n", "plugins=(eza,z)\n", "plugins=(eza-x z)\n", "plugins=(a $eza b)\n", "plugins=(z#c eza)\n", "plugins=(\n  eza#t\n)\n", "plugins=(a ${eza} eza)\n",
      "plugins=( eza )\n", "plugins=(git  eza  z)\n", "plugins=(git\teza\tz)\n", "plugins=(z eza eza z)\n", "plugins=(eza # c )\n  eza\n  z\n)\nafter eza\n", "plugins=(z # c )\n)\n", "plugins=(z #c)\n)\n", "plugins=(z eza)# trailing\n", "plugins=(z eza)  # trailing\n", "plugins=(\n  eza # a\n  z\n)\n", "plugins=(\n eza)\n", "plugins=(\n z eza)\n", "plugins=(z\neza)\n", "plugins=(a \\\n eza)\n", "plugins=(\n\n  eza\n\n)\n", "plugins=(eza", "plugins=(\n  eza", "plugins=(", "plugins=(\n", "plugins=( )\n", "plugins=()\n", "plugins=(z) # eza\n", "plugins=(z\n)\neza\n", "plugins=(eza # c\n z\n)\n", "plugins=(\n  # eza\n  z\n)\n",
      "plugins=(git eza)\nfoo\r\nbar\n", "plugins=(eza z)\r", "plugins=(git eza)\r\nfoo\r\n", "plugins=(eza)\r\n\r\n", "", "\n", "plugins=(eza)", "plugins=(eza)\n\n\n", "plugins=(git eza)\nplugins=(git eza)\n", "plugins=(z)\nplugins+=(eza)\nplugins+=(\n eza\n)\n",
    ]),
    ...drop(["plugins=(x git)\n", "plugins=(plugins x)\n"], ["x", "git", "plugins"]),
  ];
  fixture["drop_plugins_calls"] = (() => {
    const t = "plugins=(git eza z eza)\nplugins+=(x)\n";
    const calls: string[] = [];
    dropPlugins(t, n => (calls.push(n), DROP.includes(n)));
    return [{ name: "drop is offered every word part, repeats included", args: [t, DROP], out: calls }];
  })();

  fixture["guard_commands"] = [
    guard("a file, one name", "a\nb\n", ["x"]), guard("empty text", "", ["x"]), guard("plugins only, empty text", "", [], ["p"]), guard("nothing to say, empty text", "", []), guard("no trailing newline", "a", ["x"]), guard("nothing to say, no newline", "a", []),
    guard("duplicate names", "a\n", ["x", "x"]), guard("a name with a space", "a\n", ["x y"]), guard("names are pasted raw", "a\n", ["a.b", "c-d", "e+f", "$HOME", 'x"y']), guard("duplicate plugin notes", "a\n", ["x"], ["p", "p"]), guard("plugin notes alone, duplicated", "a\n", [], ["p", "p"]), guard("one plugin, one name", "a\n", ["x"], ["p"]),
    guard("empty block, nothing to say", `${B}\n${E}\n`, []), guard("empty block, one name", `${B}\n${E}\n`, ["x"]),
    guard("block in the middle, one blank after, nothing to say", `before\n${B}\nold\n${E}\n\nafter\n`, []), guard("block in the middle, one name", `before\n${B}\nold\n${E}\n\nafter\n`, ["x"]),
    guard("no blank after the block", `before\n${B}\nold\n${E}\nafter\n`, []), guard("two blanks after the block", `before\n${B}\nold\n${E}\n\n\nafter\n`, []),
    guard("two blocks, nothing to say", `${B}\nold\n${E}\n${B}\nold2\n${E}\n\nafter\n`, []), guard("two blocks, one name", `${B}\nold\n${E}\n${B}\nold2\n${E}\n\nafter\n`, ["x"]),
    guard("end before begin, nothing to say", `${E}\nx\n${B}\ny\n`, []), guard("end before begin, one name", `${E}\nx\n${B}\ny\n`, ["z"]),
    guard("no end line, nothing to say", `${B}\nold\nno end\n`, []), guard("no end line, one name", `${B}\nold\nno end\n`, ["x"]),
    guard("end line is the last, no newline, nothing to say", `${B}\nold\n${E}`, []), guard("end line is the last, no newline, one name", `${B}\nold\n${E}`, ["x"]),
    guard("end line with a trailing space", `${B}\nold\n${E} \n\nafter`, []), guard("begin line indented", ` ${B}\nold\n${E}\nafter`, []),
    guard("CRLF block, nothing to say", `${B}\r\nold\r\n${E}\r\n\r\nafter\r\n`, []), guard("CRLF block, one name", `${B}\r\nold\r\n${E}\r\n\r\nafter\r\n`, ["x"]),
    guard("mixed endings, LF first", "a\nb\r\nc\n", ["x"]), guard("mixed endings, CRLF first", "a\r\nb\nc\n", ["x"]),
  ];

  write("commands", fixture);
});
