// SPDX-License-Identifier: AGPL-3.0-only
//! The rc files as the pack and the logins rung read them: which files they are, which variable names mean a secret,
//! the simple commands of a line and the variables each assigns, and the files a line sources. Values never leave
//! here. Port of packages/collect/src/detect/shell-rc.ts:1-205 and :525-563 at a3b870a6e; the spec is
//! spec/shell_rc.md. `stripExports` is the unit shell_rc_strip.
//!
//! Every regex source below is generated from the TS by evaluating its own definitions. Each is a JavaScript regex:
//! `\s` is JavaScript's set (wsp_words::js::is_js_whitespace), `[^]` is any character, and GUARD needs lookbehind.
pub use crate::detect::shell_rc_strip::{strip_exports, Stripped};

pub const FISH_CONF_D: &str = ".config/fish/conf.d";
pub const RC_PATHS: [&str; 11] = [".zshrc", ".zshenv", ".zprofile", ".zlogin", ".bashrc", ".bash_profile", ".profile", ".inputrc", ".aliases", ".zsh_aliases", ".config/fish/config.fish"];
pub const RC_NAMES: [&str; 11] = [".zshrc", ".zshenv", ".zprofile", ".zlogin", ".bashrc", ".bash_profile", ".profile", ".inputrc", ".aliases", ".zsh_aliases", "config.fish"];
pub const SECRET_WORDS: [&str; 8] = ["KEY", "APIKEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PASS", "PASSPHRASE"];
/// JavaScript regex source.
pub const SECRET_END: &str = r#"(?:APIKEY|TOKEN|SECRET|PASSWORD|PASSWD|PASSPHRASE)$"#;
/// JavaScript regex source.
pub const PLAIN_PASS: &str = r#"^(?:ASK|BY|ANTI|DE|OUT|OVER|SUB|SUR|TO|UN|UNDER|(?:BE|EN|GYRO|OUT|PRE|RE|UN)?COM|(?:COUNTER|NON)?TRES)PASS$"#;
pub const SECRET_LAST: [&str; 3] = ["PWD", "AUTH", "PAT"];
/// JavaScript regex source.
pub const SETTING: &str = r#"^(?:FISH_KEY_BINDINGS|FISH_SEQUENCE_KEY_DELAY_MS|DISABLE_FZF_KEY_BINDINGS|TIKTOKEN_CACHE_DIR|CLAUDE_CODE_SKIP_\w+_AUTH|PASSWORD_STORE_(?:DIR|KEY|SIGNING_KEY|GPG_OPTS|X_SELECTION|CLIP_TIME|UMASK|GENERATED_LENGTH|CHARACTER_SET|CHARACTER_SET_NO_SYMBOLS|ENABLE_EXTENSIONS|EXTENSIONS_DIR))$"#;
pub const NAME: &str = "[A-Za-z_][A-Za-z0-9_]*";
/// JavaScript regex source.
pub const EXPORTED: &str = r#"^\s*\\?(?:export|typeset|declare|readonly|local|integer|float|private|env|sudo|doas|nohup|nice)(?:\s+-\w+)*\s+(.*)$"#;
/// JavaScript regex source.
pub const FISH_SET: &str = r#"^\s*set((?:\s+--?[\w-]+)*)\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s|$)"#;
/// JavaScript regex source.
pub const FISH_READS: &str = r#"\s(?:-[A-Za-z]*[eqnS][A-Za-z]*|--(?:erase|query|names|show))(?=\s|$)"#;
/// JavaScript regex source.
pub const ASSIGNED: &str = r#"^([A-Za-z_][A-Za-z0-9_]*)\+?="#;
pub const PRECOMMAND: &str = "\\\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\\s+\\S+|time(?:\\s+-p)?|command(?:\\s+-p)?|exec(?:\\s+-[cl]*a\\s+\\S+|\\s+-[cl]+)*)";
/// JavaScript regex source.
pub const KEYWORDS: &str = r#"^\s*(?:then|do|else|elif|if|while|until|\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\s+\S+|time(?:\s+-p)?|command(?:\s+-p)?|exec(?:\s+-[cl]*a\s+\S+|\s+-[cl]+)*))\s+"#;
/// JavaScript regex source.
pub const ASSIGNS: &str = r#"^(?:(?:export|typeset|declare|readonly|local|set)\s|[A-Za-z_][A-Za-z0-9_]*\+?=(?:"[^"$`]*"|'[^']*'|[^\s"'$`])*$)"#;
/// JavaScript regex source.
pub const GUARD: &str = r#"(?<=^|[\s;&|({])(if|elif|while|until)\s+(?=(?:\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\s+\S+|time(?:\s+-p)?|command(?:\s+-p)?|exec(?:\s+-[cl]*a\s+\S+|\s+-[cl]+)*)\s+)*$)"#;
/// JavaScript regex source.
pub const ABBR_VALUE: &str = r#"^(?:-[^-prcf]*[prcf]|--(?:position|regex|command|function))$"#;
/// JavaScript regex source, flags `g`.
pub const QUOTED: &str = r#"\$'(?:[^'\\]|\\[^])*'?|\\[^]|"(?:[^"\\]|\\[^])*"?|'[^']*'?"#;
/// JavaScript regex source.
pub const HEREDOC: &str = r#"^<<-?\s*(?:"([A-Za-z_]\w*)"|'([A-Za-z_]\w*)'|\\([A-Za-z_]\w*)|([A-Za-z_]\w*))"#;
/// JavaScript regex source.
pub const SOURCE: &str = r#"^([ \t]*)(source|\\?\.)[ \t]+("[^"]*"|'[^']*'|[^\s;&|<>()"'\\`]+)(?=[\s;&|<>()]|$)(.*)$"#;

// shell-rc.ts:527
// export interface SourceCommand {
//   indent: string;
//   word: string;
//   /** As written, quotes included. */
//   token: string;
//   /** Everything after the token. */
//   rest: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCommand {
    pub indent: String,
    pub word: String,
    /// As written, quotes included.
    pub token: String,
    /// Everything after the token.
    pub rest: String,
}

// shell-rc.ts:19
// /** Whether a `~`-relative path is an rc file: one of RC_PATHS, or a .fish file directly in conf.d. */
// export function isRcPath(rel: string): boolean {
//   if ((RC_PATHS as readonly string[]).includes(rel)) return true;
//   return rel.slice(0, rel.lastIndexOf("/")) === FISH_CONF_D && rel.endsWith(".fish");
// }
pub fn is_rc_path(rel: &str) -> bool {
    todo!()
}

// shell-rc.ts:25
// /** The rc files as `~`-relative paths: RC_PATHS and the .fish files in conf.d's listing. */
// export function rcFiles(confD: readonly string[]): string[] {
//   return [...RC_PATHS, ...confD.filter(n => n.endsWith(".fish")).map(n => `${FISH_CONF_D}/${n}`)];
// }
pub fn rc_files(conf_d: &[String]) -> Vec<String> {
    todo!()
}

// shell-rc.ts:45
// /** A secret's word alone between underscores, any case; `KEYTIMEOUT` and `HISTKEY` are no key. A glued word, a PASS
//  * ending, or a last `PWD`, `AUTH` or `PAT` counts only in capitals, as a program reads `PGPASSWORD`, `SSHPASS` and
//  * `GITHUB_PAT`: in shell code `ctoken` is a word of the command line, `old_pwd` a folder and `_target_opt_pat` a pattern. */
// export function isSecretName(name: string): boolean {
//   const upper = name.toUpperCase();
//   if (SETTING.test(upper)) return false;
//   const parts = upper.split("_").filter(w => w !== "");
//   if (parts.some(w => SECRET_WORDS.has(w))) return true;
//   if (name !== upper) return false;
//   const last = parts.at(-1) ?? "";
//   return parts.some(w => SECRET_END.test(w)) || (last.endsWith("PASS") && !PLAIN_PASS.test(last)) || (parts.length > 1 && SECRET_LAST.has(last));
// }
pub fn is_secret_name(name: &str) -> bool {
    todo!()
}

// shell-rc.ts:74
// /** The simple commands of a line: split outside quotes on `;`, `&&`, `||`, `|`, `&` and braces or parentheses, comment dropped. */
// export function simpleCommands(line: string): string[] {
//   const out: string[] = [];
//   let quote: "" | "'" | '"' = "";
//   let cur = "";
//   for (let i = 0; i < line.length; i += 1) {
//     const c = line[i] ?? "";
//     if (c === "\\" && quote !== "'") {
//       cur += c + (line[i + 1] ?? "");
//       i += 1;
//       continue;
//     }
//     if (quote !== "") {
//       cur += c;
//       if (c === quote) quote = "";
//       continue;
//     }
//     if (c === "'" || c === '"') {
//       quote = c;
//       cur += c;
//       continue;
//     }
//     if (commentAt(line, i)) break;
//     if (";|&{}()".includes(c)) {
//       out.push(cur);
//       cur = "";
//       continue;
//     }
//     cur += c;
//   }
//   out.push(cur);
//   return out.map(skipKeywords).filter(s => s.trim() !== "");
// }
pub fn simple_commands(line: &str) -> Vec<String> {
    todo!()
}

// shell-rc.ts:107
// function skipKeywords(command: string): string {
//   let t = command;
//   for (let m = KEYWORDS.exec(t); m !== null; m = KEYWORDS.exec(t)) t = t.slice(m[0].length);
//   return t;
// }
pub(crate) fn skip_keywords(command: &str) -> String {
    todo!()
}

// shell-rc.ts:114
// /** A simple command's words as the shell hands them over: split on blanks outside quotes, each quote taken off. */
// function shellWords(command: string): string[] {
//   const words: string[] = [];
//   let cur = "";
//   let inWord = false;
//   let quote: "" | "'" | '"' | "$'" = "";
//   for (let i = 0; i < command.length; i += 1) {
//     const c = command[i]!;
//     const next = command[i + 1] ?? "";
//     if (quote === "'" || (quote === "$'" && c !== "\\")) {
//       if (c === "'") quote = "";
//       else cur += c;
//     } else if (c === "\\") {
//       cur += quote === '"' && !'"\\$`'.includes(next) ? c + next : next;
//       inWord = true;
//       i += 1;
//     } else if (quote === '"') {
//       if (c === '"') quote = "";
//       else cur += c;
//     } else if (/\s/.test(c)) {
//       if (inWord) words.push(cur);
//       cur = "";
//       inWord = false;
//     } else {
//       inWord = true;
//       if (c === "$" && next === "'") {
//         quote = "$'";
//         i += 1;
//       } else if (c === "'" || c === '"') quote = c;
//       else cur += c;
//     }
//   }
//   if (inWord) words.push(cur);
//   return words;
// }
pub(crate) fn shell_words(command: &str) -> Vec<String> {
    todo!()
}

// shell-rc.ts:153
// /** The text an `alias` (bash, zsh, fish) or fish `abbr` would run: each `name=body`'s body, and fish's `name body...`. */
// function aliasBodies(words: string[]): string[] {
//   const args: string[] = [];
//   for (let i = 1; i < words.length; i += 1) {
//     const w = words[i]!;
//     if (w === "--") {
//       args.push(...words.slice(i + 1));
//       break;
//     }
//     if (/^[-+]./.test(w)) {
//       if (words[0] === "abbr" && ABBR_VALUE.test(w)) i += 1;
//     } else args.push(w);
//   }
//   const bodies = args.filter(a => a.includes("=")).map(a => a.slice(a.indexOf("=") + 1));
//   if (args.length > 1 && !args[0]!.includes("=")) bodies.push(args.slice(1).join(" "));
//   return bodies;
// }
pub(crate) fn alias_bodies(words: &[String]) -> Vec<String> {
    todo!()
}

// shell-rc.ts:171
// /** A simple command's words as the shell hands them over, split where its leading assignments end. */
// function commandWords(command: string): [string[], string[]] {
//   const words = shellWords(command);
//   const at = words.findIndex(w => !ASSIGNED.test(w));
//   return at < 0 ? [words, []] : [words.slice(0, at), words.slice(at)];
// }
/// The leading assignments, then the rest (TS returns the pair as a tuple).
pub(crate) fn command_words(command: &str) -> (Vec<String>, Vec<String>) {
    todo!()
}

// shell-rc.ts:179
// /** The variables a simple command assigns, each of its leading assignments among them. A quoted builtin or name is read
//  * as the bare one, and eval's arguments, joined by blanks as eval joins them, and an alias or abbr body are shell again. */
// function assignedIn(command: string): string[] {
//   const [lead, run] = commandWords(command);
//   const leading = lead.map(w => ASSIGNED.exec(w)![1]!);
//   if (run[0] === "eval") return [...leading, ...assignedNames(run.slice(1).join(" "))];
//   if (run[0] === "alias" || run[0] === "abbr") return [...leading, ...aliasBodies(run).flatMap(assignedNames)];
//   const words = shellWords(command.replace(/\\?\n/g, " "));
//   const bare = skipKeywords(words.join(" "));
//   const fish = FISH_SET.exec(bare);
//   if (fish?.[2] !== undefined) return FISH_READS.test(fish[1]!) ? [] : [fish[2]];
//   const exported = EXPORTED.exec(bare);
//   if (exported?.[1] !== undefined) {
//     return exported[1].split(/\s+/).map(t => ASSIGNED.exec(t)?.[1]).filter((n): n is string => n !== undefined);
//   }
//   const names: string[] = [];
//   for (let i = words.findIndex((_, k) => words.slice(k).join(" ") === bare); i >= 0 && i < words.length; i += 1) {
//     const m = ASSIGNED.exec(words[i]!);
//     if (m === null) break;
//     names.push(m[1]!);
//   }
//   return names;
// }
pub(crate) fn assigned_in(command: &str) -> Vec<String> {
    todo!()
}

// shell-rc.ts:201
// function assignedNames(text: string): string[] {
//   return text.split("\n").flatMap(line => simpleCommands(line).flatMap(assignedIn));
// }
pub(crate) fn assigned_names(text: &str) -> Vec<String> {
    todo!()
}

// shell-rc.ts:238
// /** A `#` outside quotes at the start of a word begins a comment; nothing after it is shell. After `(` or `{` it is a zsh glob flag, a brace expansion or `${#var}`, never a comment, but right after an array's `=(` bash and zsh both read one. */
// function commentAt(line: string, i: number): boolean {
//   return line[i] === "#" && (i === 0 || /[\s;&|]/.test(line[i - 1] ?? "") || line.slice(i - 2, i) === "=(");
// }
/// `i` indexes the line's characters (`line.chars()`); every character it compares is ASCII but `\s`, which is
/// JavaScript's set, so a `#` after a no-break space starts a comment.
pub(crate) fn comment_at(line: &[char], i: usize) -> bool {
    todo!()
}

// shell-rc.ts:537
// /** A command that reads a file: its `source` or `.` word (nvm writes `\.` to dodge an alias on the dot), the one token after it and what follows. */
// export function sourceCommand(command: string): SourceCommand | undefined {
//   const m = SOURCE.exec(command);
//   return m === null ? undefined : { indent: m[1]!, word: m[2]!, token: m[3]!, rest: m[4]! };
// }
pub fn source_command(command: &str) -> Option<SourceCommand> {
    todo!()
}

// shell-rc.ts:544
// /** The file a source token names, absolute, when the token is literal: `~/`, `$HOME/` and `${HOME}/` fold to home; a
//  * variable elsewhere, a glob, a substitution, an escape or an empty, `.` or `..` segment is left to a running shell. */
// export function sourcedPath(token: string, home: string): string | undefined {
//   const raw = /^(["']).*\1$/.test(token) ? token.slice(1, -1) : token;
//   const path = raw.replace(/^(?:~|\$HOME|\$\{HOME\})(?=\/)/, () => home);
//   if (!path.startsWith("/") || /[$`*?[\]\\]/.test(path) || path.split("/").slice(1).some(seg => seg === "" || seg === "." || seg === "..")) return undefined;
//   return path;
// }
pub fn sourced_path(token: &str, home: &str) -> Option<String> {
    todo!()
}

// shell-rc.ts:552
// /** The files an rc file reads with `source` or `.` by a literal path, anywhere on the line, in order, once each. */
// export function sourcedPaths(text: string, home: string): string[] {
//   const out: string[] = [];
//   for (const line of text.split(/\r?\n/)) {
//     // simpleCommands splits on braces, so `${HOME}` is folded before the split.
//     for (const cmd of simpleCommands(line.replace(/\$\{HOME\}/g, "$HOME"))) {
//       const src = sourceCommand(cmd);
//       const p = src === undefined ? undefined : sourcedPath(src.token, home);
//       if (p !== undefined && !out.includes(p)) out.push(p);
//     }
//   }
//   return out;
// }
pub fn sourced_paths(text: &str, home: &str) -> Vec<String> {
    todo!()
}
