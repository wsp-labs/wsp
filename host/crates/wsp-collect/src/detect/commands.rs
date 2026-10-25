// SPDX-License-Identifier: AGPL-3.0-only
//! The commands an rc file calls, and the guard the pack prepends for the ones the machine lacks: each such name a
//! function that runs the real command once there is one and returns 127 until then. Port of
//! packages/collect/src/detect/commands.ts at a3b870a6e; the spec is spec/commands.md.

// commands.ts:11
// export const GUARD_BEGIN = "# >>> wsp guard: commands this machine does not have >>>";
pub const GUARD_BEGIN: &str = "# >>> wsp guard: commands this machine does not have >>>";
// commands.ts:12
// export const GUARD_END = "# <<< wsp guard <<<";
pub const GUARD_END: &str = "# <<< wsp guard <<<";

/// A name a function definition or a call may carry in zsh and bash, as a JavaScript regex source (commands.ts:15).
pub const WORD: &str = r"^[A-Za-z_][\w.+-]*$";

/// Builtins, reserved words and zsh's own functions: never a call to guard (commands.ts:20).
pub const RC_BUILTIN: [&str; 91] = [
    "autoload", "bg", "bindkey", "bye", "chdir", "compadd", "compdef", "compdescribe", "compinit", "complete", "compgen", "compopt",
    "coproc", "dirs", "disable", "disown", "echotc", "echoti", "emulate", "enable", "fc", "fg", "float", "functions", "getln", "getopts",
    "hash", "history", "in", "integer", "jobs", "let", "limit", "logout", "mapfile", "nocorrect", "noglob", "popd", "print", "private",
    "pushd", "pushln", "r", "readarray", "rehash", "repeat", "sched", "select", "setopt", "shopt", "suspend", "times", "ttyctl", "umask",
    "unalias", "unfunction", "unhash", "unlimit", "unsetopt", "vared", "whence", "where", "zcompile", "zformat", "zle", "zmodload",
    "zparseopts", "zprof", "zstyle", "add-zsh-hook", "add-zle-hook-widget", "bashcompinit", "colors", "compaudit", "is-at-least",
    "promptinit", "prompt", "run-help", "select-word-style", "vcs_info", "zargs", "zcalc", "zed", "zmv", "zrecompile",
    "bracketed-paste-magic", "url-quote-magic", "edit-command-line", "up-line-or-beginning-search", "down-line-or-beginning-search",
    "history-search-end",
];

/// `name() {` or `function name` at a line's start, as a JavaScript regex source with flags `gm` (commands.ts:29).
pub const FUNCTION_HEAD: &str = r"^\s*(?:function\s+)?([A-Za-z_][\w.+-]*)\s*\(\s*\)|^\s*function\s+([A-Za-z_][\w.+-]*)\b";

/// The shell a file is written for; fish is not shell and yields no calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Syntax {
    #[default]
    Sh,
    Fish,
}

/// What `drop_plugins` hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub text: String,
    pub dropped: Vec<String>,
}

// Private helpers the port keeps (names in snake case):
// commands.ts:30
// const ALIAS_TOKEN = /^([\w.+-]+)=(.*)$/;
// commands.ts:33
// /** The text with every `name=( ... )` array literal emptied, across lines, quotes and comments respected: its words are values, not calls. */
// function withoutArrayLiterals(text: string): string {
//   let out = "";
//   let quote: "" | "'" | '"' = "";
//   let depth = 0;
//   const commentAt = (i: number): boolean => text[i] === "#" && (i === 0 || /[\s;&|(]/.test(text[i - 1] ?? ""));
//   for (let i = 0; i < text.length; i += 1) {
//     const c = text[i]!;
//     if (depth > 0) {
//       if (c === "\\" && quote !== "'") i += 1;
//       else if (quote !== "") quote = c === quote ? "" : quote;
//       else if (c === "'" || c === '"') quote = c;
//       else if (commentAt(i)) i = text.indexOf("\n", i) === -1 ? text.length : text.indexOf("\n", i) - 1;
//       else if (c === "(") depth += 1;
//       else if (c === ")" && (depth -= 1) === 0) out += ")";
//       continue;
//     }
//     if (c === "\\" && quote !== "'") {
//       out += c + (text[i + 1] ?? "");
//       i += 1;
//       continue;
//     }
//     if (quote !== "") {
//       if (c === quote) quote = "";
//     } else if (c === "'" || c === '"') {
//       quote = c;
//     } else if (commentAt(i)) {
//       const end = text.indexOf("\n", i);
//       out += end === -1 ? text.slice(i) : text.slice(i, end);
//       i = end === -1 ? text.length : end - 1;
//       continue;
//     } else if (c === "(" && text[i - 1] === "=" && /[\w\]+]/.test(text[i - 2] ?? "")) {
//       depth = 1;
//       out += c;
//       continue;
//     }
//     out += c;
//   }
//   return out;
// }
// commands.ts:74
// /** The command a run of words calls, as the history parser reads it, when it is a plain name and no builtin. */
// function callOf(command: string): string | undefined {
//   const words = commandWords(command);
//   const head = words[0];
//   if (head === undefined || isVersionCheck(words) || !WORD.test(head) || RC_BUILTIN.has(head)) return undefined;
//   // The parser gives a path's basename; a call by path is not one a function by that name would answer.
//   return command.split(/\s+/).some(w => w.endsWith(`/${head}`)) ? undefined : head;
// }
// commands.ts:117
// const PLUGINS_OPEN = /^\s*plugins\+?=\(/;
// commands.ts:119
// /** A `#` at the start of a word begins the line's comment. */
// const COMMENT_AT = /(^|\s)#/;
// commands.ts:159
// /** The guard's notes: one line per plugin left out, then one naming what is guarded, so the file itself says why. */
// function notes(missing: readonly string[], dropped: readonly string[]): string[] {
//   return [
//     ...dropped.map(name => `# plugin ${name} left out: ${name} is not on the image`),
//     ...(missing.length > 0 ? [`# Guarded above: a call to one of these that is not on this machine is silent instead of an error: ${missing.join(", ")}. Tick them in wsp init to install them.`] : []),
//   ];
// }
// commands.ts:171
// /** A command that is on the machine after all (installed by hand, by an agent, unknown to the planner, or on a directory the rc
//  * itself adds to PATH below the block) always wins: the guard looks the name up at every call and hands over to the command
//  * once there is one. `command -v` would find the guard itself, so the lookup runs in a subshell with the guard unset; a
//  * function that unsets and redefines itself while it runs crashes zsh 5.9 (exit 139, measured), so the guard is only ever
//  * unset on the way to the real command. */
// const guardLine = (name: string): string => `${name}() { if (unset -f ${name}; command -v ${name}) >/dev/null 2>&1; then unset -f ${name}; ${name} "$@"; else return 127; fi; }`;

// commands.ts:86
// /** The commands an rc file calls, once each in order of first call, read with the history parser: the first word of
//  * every command, substitutions and pipelines included, past its environment assignments and wrappers; on top of
//  * that, the first word of every alias body. Names the file itself defines as functions or aliases or marks for
//  * autoload are its own and never listed; array literals are values; a fish file is not shell and yields nothing. */
// export function calledCommands(text: string, syntax: "sh" | "fish" = "sh"): string[] {
//   if (syntax === "fish") return [];
//   const called: string[] = [];
//   const own = new Set<string>();
//   const add = (name: string | undefined): void => {
//     if (name !== undefined && !called.includes(name)) called.push(name);
//   };
//   for (const m of text.matchAll(FUNCTION_HEAD)) own.add(m[1] ?? m[2]!);
//   for (const cmd of splitCommands(withoutHeredocs(withoutArrayLiterals(text)))) {
//     const words = cmd.split(/\s+/);
//     if (words[0] === "alias") {
//       // The parser has taken the quotes off, so a body runs to the next `name=` token.
//       words.slice(1).forEach((w, i) => {
//         const m = ALIAS_TOKEN.exec(w);
//         if (m === null) return;
//         own.add(m[1]!);
//         const rest = words.slice(i + 2);
//         const next = rest.findIndex(r => ALIAS_TOKEN.test(r));
//         add(callOf([m[2]!, ...rest.slice(0, next === -1 ? rest.length : next)].join(" ")));
//       });
//       continue;
//     }
//     if (words[0] === "autoload") {
//       for (const w of words.slice(1)) if (!w.startsWith("-")) own.add(w);
//       continue;
//     }
//     add(callOf(cmd));
//   }
//   return called.filter(n => !own.has(n));
// }
pub fn called_commands(text: &str, syntax: Syntax) -> Vec<String> {
    todo!()
}

// commands.ts:125
// /** The oh-my-zsh plugin list is a zsh array. A plugin whose tool is not on the image prints its own line at
//  * every start and a no-op function does not satisfy its `$+commands` check, so the name leaves the list instead:
//  * every `plugins=( )` and `plugins+=( )` region, one line or many, loses the names `drop` says, a line left with
//  * nothing but its indent goes with them and a trailing comment stays. Every other byte is as it was. */
// export function dropPlugins(text: string, drop: (name: string) => boolean): { text: string; dropped: string[] } {
//   const dropped: string[] = [];
//   const eol = text.includes("\r\n") ? "\r\n" : "\n";
//   const out: string[] = [];
//   let inList = false;
//   for (const line of text.split(/\r?\n/)) {
//     const opens = !inList && PLUGINS_OPEN.test(line);
//     if (!opens && !inList) {
//       out.push(line);
//       continue;
//     }
//     const at = COMMENT_AT.exec(line);
//     const code = at === null ? line : line.slice(0, at.index + at[1]!.length);
//     const comment = at === null ? "" : line.slice(at.index + at[1]!.length);
//     inList = !code.includes(")");
//     const parts = code.split(/(\s+|[()])/).filter(p => p !== "");
//     for (let i = 0; i < parts.length; i += 1) {
//       const word = parts[i]!;
//       if (!WORD.test(word) || !drop(word)) continue;
//       if (!dropped.includes(word)) dropped.push(word);
//       parts.splice(i, 1);
//       if (/^\s+$/.test(parts[i] ?? "")) parts.splice(i, 1);
//       else if (/^\s+$/.test(parts[i - 1] ?? "")) parts.splice(i - 1, 1);
//       i -= 1;
//     }
//     const kept = parts.join("");
//     const indent = /^\s*/.exec(line)![0];
//     if (kept.trim() === "" && comment === "" && code.trim() !== "") continue;
//     out.push(kept.trim() === "" && comment !== "" ? `${indent}${comment}` : `${kept}${comment}`);
//   }
//   return { text: out.join(eol), dropped };
// }
pub fn drop_plugins(text: &str, drop: &dyn Fn(&str) -> bool) -> Dropped {
    todo!()
}

// commands.ts:176
// /** The text with the guard block prepended: a conditional silent no-op for each `missing` command and the notes for
//  * those and for the plugins `dropped`. Any guard block an earlier pack wrote is taken out first, so a re-run
//  * replaces it; with nothing to say the text carries no block. Line endings follow the file. */
// export function guardCommands(text: string, missing: readonly string[], dropped: readonly string[] = []): string {
//   const eol = text.includes("\r\n") ? "\r\n" : "\n";
//   const lines = text.split(/\r?\n/);
//   const begin = lines.indexOf(GUARD_BEGIN);
//   const end = lines.indexOf(GUARD_END, begin);
//   const body = begin === -1 || end === -1 ? lines : [...lines.slice(0, begin), ...lines.slice(lines[end + 1] === "" ? end + 2 : end + 1)];
//   if (missing.length + dropped.length === 0) return body.join(eol);
//   const block = [GUARD_BEGIN, ...missing.map(guardLine), ...notes(missing, dropped), GUARD_END, ""];
//   return [...block, ...body].join(eol);
// }
/// TS `dropped` defaults to `[]`; pass `&[]`.
pub fn guard_commands(text: &str, missing: &[String], dropped: &[String]) -> String {
    todo!()
}
