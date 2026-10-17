// SPDX-License-Identifier: AGPL-3.0-only
// The rc files as the pack and the logins rung read them. Exported variables
// whose name says KEY, TOKEN, SECRET or PASSWORD are listed by name and cut
// from a carried copy of the rc file, wherever on the line the assignment
// sits. Values never leave here, so a cut line takes its continuation and any
// open quote, substitution, array or heredoc with it.
import { RC_FILES as SHELL_RUNG_RC } from "./shell.js";

/** fish reads every .fish file directly in this directory at startup, before config.fish. */
export const FISH_CONF_D = ".config/fish/conf.d";

/** The rc files the shell rung carries, `~`-relative, plus fish's config; fish's conf.d files come through rcFiles and isRcPath. */
export const RC_PATHS = [...SHELL_RUNG_RC.map(n => `.${n}`), ".config/fish/config.fish"] as const;

/** The names an rc file goes by, for a copy of one found under another directory. */
export const RC_NAMES: ReadonlySet<string> = new Set(RC_PATHS.map(p => p.slice(p.lastIndexOf("/") + 1)));

/** Whether a `~`-relative path is an rc file: one of RC_PATHS, or a .fish file directly in conf.d. */
export function isRcPath(rel: string): boolean {
  if ((RC_PATHS as readonly string[]).includes(rel)) return true;
  return rel.slice(0, rel.lastIndexOf("/")) === FISH_CONF_D && rel.endsWith(".fish");
}

/** The rc files as `~`-relative paths: RC_PATHS and the .fish files in conf.d's listing. */
export function rcFiles(confD: readonly string[]): string[] {
  return [...RC_PATHS, ...confD.filter(n => n.endsWith(".fish")).map(n => `${FISH_CONF_D}/${n}`)];
}

const SECRET_WORDS = new Set(["KEY", "APIKEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PASS", "PASSPHRASE"]);
/** A part of a name in capitals that ends in one of these is a secret's even glued: `PGPASSWORD`, `GHTOKEN`, `BORG_PASSPHRASE`. */
const SECRET_END = /(?:APIKEY|TOKEN|SECRET|PASSWORD|PASSWD|PASSPHRASE)$/;
/** The words /usr/share/dict/words lists ending in PASS less REPASS, a retyped password's usual name, and ssh's, sudo's
 * and git's ASKPASS, a program's path. */
const PLAIN_PASS = /^(?:ASK|BY|ANTI|DE|OUT|OVER|SUB|SUR|TO|UN|UNDER|(?:BE|EN|GYRO|OUT|PRE|RE|UN)?COM|(?:COUNTER|NON)?TRES)PASS$/;
/** A last part after another that means a secret in capitals: `MYSQL_PWD`, `REDISCLI_AUTH`, `GITHUB_PAT`. */
const SECRET_LAST = new Set(["PWD", "AUTH", "PAT"]);
/** fish's, oh-my-zsh's, pass's, tiktoken's and Claude Code's own settings that hold a secret's word: a keyboard's key, a
 * store's path or gpg key ids, a cache folder, a switch that skips a cloud's sign-in. pass's are the twelve its man page
 * names, so a name past them is read as any other. */
const SETTING = /^(?:FISH_KEY_BINDINGS|FISH_SEQUENCE_KEY_DELAY_MS|DISABLE_FZF_KEY_BINDINGS|TIKTOKEN_CACHE_DIR|CLAUDE_CODE_SKIP_\w+_AUTH|PASSWORD_STORE_(?:DIR|KEY|SIGNING_KEY|GPG_OPTS|X_SELECTION|CLIP_TIME|UMASK|GENERATED_LENGTH|CHARACTER_SET|CHARACTER_SET_NO_SYMBOLS|ENABLE_EXTENSIONS|EXTENSIONS_DIR))$/;

/** A secret's word alone between underscores, any case; `KEYTIMEOUT` and `HISTKEY` are no key. A glued word, a PASS
 * ending, or a last `PWD`, `AUTH` or `PAT` counts only in capitals, as a program reads `PGPASSWORD`, `SSHPASS` and
 * `GITHUB_PAT`: in shell code `ctoken` is a word of the command line, `old_pwd` a folder and `_target_opt_pat` a pattern. */
export function isSecretName(name: string): boolean {
  const upper = name.toUpperCase();
  if (SETTING.test(upper)) return false;
  const parts = upper.split("_").filter(w => w !== "");
  if (parts.some(w => SECRET_WORDS.has(w))) return true;
  if (name !== upper) return false;
  const last = parts.at(-1) ?? "";
  return parts.some(w => SECRET_END.test(w)) || (last.endsWith("PASS") && !PLAIN_PASS.test(last)) || (parts.length > 1 && SECRET_LAST.has(last));
}

const NAME = "[A-Za-z_][A-Za-z0-9_]*";
const EXPORTED = new RegExp(`^\\s*\\\\?(?:export|typeset|declare|readonly|local|integer|float|private|env|sudo|doas|nohup|nice)(?:\\s+-\\w+)*\\s+(.*)$`);
const FISH_SET = new RegExp(`^\\s*set((?:\\s+--?[\\w-]+)*)\\s+(${NAME})(?:\\s|$)`);
/** fish's `set` flags that query, erase, show or list a variable: such a `set` assigns nothing. */
const FISH_READS = /\s(?:-[A-Za-z]*[eqnS][A-Za-z]*|--(?:erase|query|names|show))(?=\s|$)/;
const ASSIGNED = new RegExp(`^(${NAME})\\+?=`);
/** The precommand words of bash, zsh and fish (fish's `and`, `or` and `not` among them), zsh's `repeat N` and `coproc`,
 * with their flags and an alias-dodging backslash: what follows is a simple command of its own. */
const PRECOMMAND = String.raw`\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\s+\S+|time(?:\s+-p)?|command(?:\s+-p)?|exec(?:\s+-[cl]*a\s+\S+|\s+-[cl]+)*)`;
/** Words that open a compound command's body, and the precommand words. */
const KEYWORDS = new RegExp(String.raw`^\s*(?:then|do|else|elif|if|while|until|${PRECOMMAND})\s+`);
/** A command whose status is 0 whatever it held, so `true` stands for it anywhere: an export-like builtin, or a bare
 * assignment with no substitution (a bare `X=$(cmd)` answers with cmd's status). */
const ASSIGNS = new RegExp(`^(?:(?:export|typeset|declare|readonly|local|set)\\s|${NAME}\\+?=(?:"[^"$\`]*"|'[^']*'|[^\\s"'$\`])*$)`);
/** The keyword whose condition a list is, `!` and the precommand words left to the condition: what follows an `if`,
 * `elif`, `while` or `until`. */
const GUARD = new RegExp(String.raw`(?<=^|[\s;&|({])(if|elif|while|until)\s+(?=(?:${PRECOMMAND}\s+)*$)`);

/** The simple commands of a line: split outside quotes on `;`, `&&`, `||`, `|`, `&` and braces or parentheses, comment dropped. */
export function simpleCommands(line: string): string[] {
  const out: string[] = [];
  let quote: "" | "'" | '"' = "";
  let cur = "";
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i] ?? "";
    if (c === "\\" && quote !== "'") {
      cur += c + (line[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (quote !== "") {
      cur += c;
      if (c === quote) quote = "";
      continue;
    }
    if (c === "'" || c === '"') {
      quote = c;
      cur += c;
      continue;
    }
    if (commentAt(line, i)) break;
    if (";|&{}()".includes(c)) {
      out.push(cur);
      cur = "";
      continue;
    }
    cur += c;
  }
  out.push(cur);
  return out.map(skipKeywords).filter(s => s.trim() !== "");
}

function skipKeywords(command: string): string {
  let t = command;
  for (let m = KEYWORDS.exec(t); m !== null; m = KEYWORDS.exec(t)) t = t.slice(m[0].length);
  return t;
}

/** A simple command's words as the shell hands them over: split on blanks outside quotes, each quote taken off. */
function shellWords(command: string): string[] {
  const words: string[] = [];
  let cur = "";
  let inWord = false;
  let quote: "" | "'" | '"' | "$'" = "";
  for (let i = 0; i < command.length; i += 1) {
    const c = command[i]!;
    const next = command[i + 1] ?? "";
    if (quote === "'" || (quote === "$'" && c !== "\\")) {
      if (c === "'") quote = "";
      else cur += c;
    } else if (c === "\\") {
      cur += quote === '"' && !'"\\$`'.includes(next) ? c + next : next;
      inWord = true;
      i += 1;
    } else if (quote === '"') {
      if (c === '"') quote = "";
      else cur += c;
    } else if (/\s/.test(c)) {
      if (inWord) words.push(cur);
      cur = "";
      inWord = false;
    } else {
      inWord = true;
      if (c === "$" && next === "'") {
        quote = "$'";
        i += 1;
      } else if (c === "'" || c === '"') quote = c;
      else cur += c;
    }
  }
  if (inWord) words.push(cur);
  return words;
}

/** fish's abbr options that take the next word as their value: a long one, or a short one that ends its cluster (`-ac git`). */
const ABBR_VALUE = /^(?:-[^-prcf]*[prcf]|--(?:position|regex|command|function))$/;

/** The text an `alias` (bash, zsh, fish) or fish `abbr` would run: each `name=body`'s body, and fish's `name body...`. */
function aliasBodies(words: string[]): string[] {
  const args: string[] = [];
  for (let i = 1; i < words.length; i += 1) {
    const w = words[i]!;
    if (w === "--") {
      args.push(...words.slice(i + 1));
      break;
    }
    if (/^[-+]./.test(w)) {
      if (words[0] === "abbr" && ABBR_VALUE.test(w)) i += 1;
    } else args.push(w);
  }
  const bodies = args.filter(a => a.includes("=")).map(a => a.slice(a.indexOf("=") + 1));
  if (args.length > 1 && !args[0]!.includes("=")) bodies.push(args.slice(1).join(" "));
  return bodies;
}

/** A simple command's words as the shell hands them over, split where its leading assignments end. */
function commandWords(command: string): [string[], string[]] {
  const words = shellWords(command);
  const at = words.findIndex(w => !ASSIGNED.test(w));
  return at < 0 ? [words, []] : [words.slice(0, at), words.slice(at)];
}

/** The variables a simple command assigns, each of its leading assignments among them. A quoted builtin or name is read
 * as the bare one, and eval's arguments, joined by blanks as eval joins them, and an alias or abbr body are shell again. */
function assignedIn(command: string): string[] {
  const [lead, run] = commandWords(command);
  const leading = lead.map(w => ASSIGNED.exec(w)![1]!);
  if (run[0] === "eval") return [...leading, ...assignedNames(run.slice(1).join(" "))];
  if (run[0] === "alias" || run[0] === "abbr") return [...leading, ...aliasBodies(run).flatMap(assignedNames)];
  const words = shellWords(command.replace(/\\?\n/g, " "));
  const bare = skipKeywords(words.join(" "));
  const fish = FISH_SET.exec(bare);
  if (fish?.[2] !== undefined) return FISH_READS.test(fish[1]!) ? [] : [fish[2]];
  const exported = EXPORTED.exec(bare);
  if (exported?.[1] !== undefined) {
    return exported[1].split(/\s+/).map(t => ASSIGNED.exec(t)?.[1]).filter((n): n is string => n !== undefined);
  }
  const names: string[] = [];
  for (let i = words.findIndex((_, k) => words.slice(k).join(" ") === bare); i >= 0 && i < words.length; i += 1) {
    const m = ASSIGNED.exec(words[i]!);
    if (m === null) break;
    names.push(m[1]!);
  }
  return names;
}

function assignedNames(text: string): string[] {
  return text.split("\n").flatMap(line => simpleCommands(line).flatMap(assignedIn));
}

/** A line's ANSI-C strings, escapes and quoted strings, as bash and zsh read them. */
const QUOTED = /\$'(?:[^'\\]|\\[^])*'?|\\[^]|"(?:[^"\\]|\\[^])*"?|'[^']*'?/g;

/** Whether a line leaves open the quoted argument of an `eval`, `alias` or `abbr`, whose text is shell: the lines up to
 * the quote's close are read as one command. A single-quoted string ending in `\'` is fish's escaped quote, which this
 * reading takes for a close, so such a line is read alone. */
function opensShell(line: string, arith: number): boolean {
  if ((line.match(QUOTED) ?? []).some(t => /^'[^']*\\'$/.test(t)) || !["'", '"', "$'"].includes(scanLine(line, { ...CLOSED, arith }).quote)) return false;
  return ["eval", "alias", "abbr"].includes(commandWords(simpleCommands(line).at(-1) ?? "")[1][0] ?? "");
}

/** What is still open at the end of a line: a quote, a backtick, `$(`, `${` and array groups, `((` arithmetic, a heredoc waiting for its word, or a trailing backslash. */
interface Open {
  quote: "" | "'" | '"' | "`" | "$'";
  /** The open `$(`, `${`, array `=(` and bare `(` groups, innermost last, each with the quote it opened inside: a
   * `$(` reads its text unquoted and gives that quote back at its `)`, and a `)` or `}` closes only the group on top. */
  groups: { close: ")" | "}"; quote: "" | '"' }[];
  /** Depth of `((` and `$((`; while open, `<<` is a shift and never a heredoc. */
  arith: number;
  heredoc?: string;
  continues: boolean;
}

const CLOSED: Open = { quote: "", groups: [], arith: 0, continues: false };

function isOpen(o: Open): boolean {
  return o.quote !== "" || o.groups.length > 0 || o.heredoc !== undefined || o.continues;
}

/** `<<WORD`, `<<-WORD`, `<<"WORD"`, `<<\WORD`: the word starts with a letter or underscore, so `x << 2` is arithmetic. The caller skips `<<<` (a here-string) and `<<` inside `(( ))` before this runs. */
const HEREDOC = /^<<-?\s*(?:"([A-Za-z_]\w*)"|'([A-Za-z_]\w*)'|\\([A-Za-z_]\w*)|([A-Za-z_]\w*))/;

/** A `#` outside quotes at the start of a word begins a comment; nothing after it is shell. After `(` or `{` it is a zsh glob flag, a brace expansion or `${#var}`, never a comment, but right after an array's `=(` bash and zsh both read one. */
function commentAt(line: string, i: number): boolean {
  return line[i] === "#" && (i === 0 || /[\s;&|]/.test(line[i - 1] ?? "") || line.slice(i - 2, i) === "=(");
}

/** Carries the shell's quoting state across a line. Inside a heredoc only the terminator word matters. */
function scanLine(line: string, start: Open): Open {
  const o: Open = { ...start, groups: [...start.groups], continues: false };
  if (o.heredoc !== undefined) {
    if (line.trim() === o.heredoc) delete o.heredoc;
    return o;
  }
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    const top = o.groups.at(-1);
    o.continues = false;
    if (c === "\\" && o.quote !== "'") {
      if (i === line.length - 1) o.continues = true;
      i += 1;
      continue;
    }
    if (o.quote === "$'") {
      if (c === "'") o.quote = "";
      continue;
    }
    if (o.quote === "'" || o.quote === "`") {
      if (c === o.quote) o.quote = "";
      continue;
    }
    if (o.quote === "" && commentAt(line, i)) break;
    if (o.quote === "" && c === "$" && line[i + 1] === "'") {
      o.quote = "$'";
      i += 1;
      continue;
    }
    if (o.quote === "" && (c === "'" || c === "`")) {
      o.quote = c;
      continue;
    }
    if (c === '"') {
      o.quote = o.quote === '"' ? "" : '"';
      continue;
    }
    if (o.quote === "" && c === "$" && line[i + 1] === "(" && line[i + 2] === "(") {
      o.arith += 1;
      i += 2;
      continue;
    }
    if (c === "$" && line[i + 1] === "(") {
      o.groups.push({ close: ")", quote: o.quote === '"' ? '"' : "" });
      o.quote = "";
      i += 1;
      continue;
    }
    if (c === "$" && line[i + 1] === "{") {
      o.groups.push({ close: "}", quote: "" });
      i += 1;
      continue;
    }
    if (o.quote === "" && c === "(" && line[i + 1] === "(") {
      o.arith += 1;
      i += 1;
      continue;
    }
    if (o.quote === "" && c === ")" && line[i + 1] === ")" && o.arith > 0) {
      o.arith -= 1;
      i += 1;
      continue;
    }
    if (o.quote === "" && c === "(" && (top?.close === ")" || line[i - 1] === "=")) {
      o.groups.push({ close: ")", quote: "" });
      continue;
    }
    if (o.quote === "" && c === ")" && top?.close === ")") {
      o.groups.pop();
      o.quote = top.quote;
      continue;
    }
    if (c === "}" && top?.close === "}") {
      o.groups.pop();
      continue;
    }
    if (c === "<" && line[i + 1] === "<" && line[i + 2] === "<") {
      i += 2;
      continue;
    }
    if (o.quote === "" && o.arith === 0 && c === "<") {
      const m = HEREDOC.exec(line.slice(i));
      const word = m?.[1] ?? m?.[2] ?? m?.[3] ?? m?.[4];
      if (word !== undefined) {
        o.heredoc = word;
        break;
      }
    }
  }
  return o;
}

/** What a kept line may carry forward: a heredoc it opens and the depth of an arithmetic block it left open, and nothing else, since a comment's stray quote must never hide what follows. An open `((` only stops `<<` from being read as a heredoc until its `))`, which over-cuts a heredoc body and never leaks. */
function carried(line: string, arith: number): Pick<Open, "heredoc" | "arith"> {
  const o = scanLine(line, { ...CLOSED, arith });
  return { heredoc: o.heredoc, arith: o.arith };
}

/** How far a command's leading blanks and keywords run. */
function leadOf(segment: string): number {
  let at = /^(?:\s|\\\n)*/.exec(segment)![0].length;
  for (let m = KEYWORDS.exec(segment.slice(at)); m !== null; m = KEYWORDS.exec(segment.slice(at))) at += m[0].length;
  return at;
}

/** The top-level simple commands and comments of closed shell text, keywords skipped, as [start, end) offsets. They split
 * on `;`, `|`, a newline, an `&` that is no redirect's, a bare `{` or `}`, and a `(` or `)` opening a subshell or closing a
 * case pattern; quotes, substitutions, `[[ ]]` and a `(` inside a command (an array, a fish substitution, `f()`) hold together.
 * With `lists`, `&&`, `||` and `|` do not split, so each span is a whole and-or list. */
function commandSpans(text: string, lists = false): [number, number][] {
  const spans: [number, number][] = [];
  const stack: string[] = [];
  let start = 0;
  const end = (i: number): void => {
    const s = start + leadOf(text.slice(start, i));
    const e = start + text.slice(start, i).replace(/(?:\s|\\\n)*$/, "").length;
    if (e > s) spans.push([s, e]);
  };
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i]!;
    const top = stack.at(-1);
    if (c === "\\" && top !== "'") i += 1;
    else if (top === "'" || top === "$'" || top === "`") {
      if (c === (top === "`" ? "`" : "'")) stack.pop();
    } else if (c === "$" && (text[i + 1] === "(" || text[i + 1] === "{")) {
      stack.push(text[i + 1]!);
      i += 1;
    } else if (c === "`" || (top !== '"' && (c === "'" || c === '"'))) stack.push(c);
    else if (top === '"') {
      if (c === '"') stack.pop();
    } else if (c === "$" && text[i + 1] === "'") {
      stack.push("$'");
      i += 1;
    } else if (top !== undefined) {
      if (top === "(" && commentAt(text, i)) while (i + 1 < text.length && text[i + 1] !== "\n") i += 1;
      else if (c === "(") stack.push(c);
      else if ((c === ")" && top === "(") || (c === "}" && top === "{")) stack.pop();
      else if (top === "[[" && text.startsWith("]]", i)) {
        stack.pop();
        i += 1;
      }
    } else if (commentAt(text, i)) {
      end(i);
      const from = i;
      while (i + 1 < text.length && text[i + 1] !== "\n") i += 1;
      spans.push([from, i + 1]);
      start = i + 1;
    } else if (/^\[\[\s/.test(text.slice(i, i + 3)) && leadOf(text.slice(start, i)) === i - start) {
      stack.push("[[");
      i += 1;
    } else if (c === "(" && leadOf(text.slice(start, i)) < i - start) stack.push(c);
    else if (
      ";\n()".includes(c) ||
      (c === "|" && text[i - 1] !== ">" && !lists) ||
      (c === "&" && !/[<>]/.test(text[i - 1] ?? "") && text[i + 1] !== ">" && !(lists && /[&|]/.test(`${text[i - 1] ?? ""}${text[i + 1] ?? ""}`))) ||
      ((c === "{" || c === "}") && /^\s?$/.test(text[i + 1] ?? "") && /^[\s;&]?$/.test(text[i - 1] ?? ""))
    ) {
      end(i);
      start = i + 1;
    }
  }
  end(text.length);
  return spans;
}

/** The arithmetic depth is one stream over the file: a cut line reads it and writes it back like a kept line does. A
 * line `secretIn` finds a secret in goes too, a heredoc's included, and a heredoc's line that assigns a secret goes with
 * what it leaves open, up to the heredoc's word. An `eval`, `alias` or `abbr` whose quote runs over lines is one
 * command. `names` holds only variables, each one a cut line assigns a secret to, since init asks the person for each;
 * `held` gets what `secretIn` names for a cut line that assigns none. In a `script`, each cut command stands as `true`,
 * a cut comment goes, and the rest of its line stays, so a block it opened, closed or was the only line of still
 * parses. What a cut command that does more than assign decided never runs: an `if`, `elif` or `while` condition
 * holding one stands as false and an `until` condition as true, and a list where one leads into `&&`, `||` or `|`
 * stands as one `true`. A cut that cannot split (a heredoc, the head of a for, select or case) stands as one `true` at
 * its indent, or as nothing when it is a comment or holds a case arm's `;;`, `;&` or `;|`. */
export function stripExports(text: string, secretIn?: (line: string) => string | undefined, held: string[] = [], script = false): { names: string[]; carried: string } {
  const names: string[] = [];
  const kept: string[] = [];
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const holds = (command: string): boolean => assignedIn(command).some(isSecretName) || secretIn?.(command) !== undefined;
  const leave = (lines: string[], whole: boolean): void => {
    if (!script) return;
    const cut = lines.join("\n");
    let rest = "";
    let at = 0;
    const spans = (whole ? commandSpans(cut) : []).filter(([s, e]) => holds(cut.slice(s, e)));
    const lists = commandSpans(cut, true);
    const stands = (s: number, e: number): [number, number, string] => {
      if (cut[s] === "#") return [s, e, ""];
      if (ASSIGNS.test(cut.slice(s, e))) return [s, e, "true"];
      const [ls, le] = lists.find(([a, b]) => a <= s && e <= b) ?? [s, e];
      const guard = GUARD.exec(cut.slice(0, ls));
      if (guard !== null) {
        const until = guard[1] === "until";
        // zsh's `if [[ ]] { }` reads its condition only up to a closing `]]`.
        const word = /^\s*\{/.test(cut.slice(le)) ? `[[ ${until ? "-z" : "-n"} '' ]]` : until ? "true" : "false";
        return [guard.index + guard[0].length, le, word];
      }
      return /^\s*[&|]/.test(cut.slice(e, le)) ? [ls, le, "true"] : [s, e, "true"];
    };
    const ranges = spans.some(([s]) => /^(?:for|select|case)\s/.test(cut.slice(s))) ? [] : spans.map(([s, e]) => stands(s, e));
    for (const [s, e, word] of ranges.sort(([a, b], [c, d]) => a - c || d - b)) {
      if (s < at) continue;
      rest += word === "" ? cut.slice(at, s).replace(/[ \t]+$/, "") : `${cut.slice(at, s)}${word}`;
      at = e;
    }
    rest += cut.slice(at);
    const first = lines[0]!;
    if (rest !== cut && rest.trim() !== "" && !rest.split("\n").some(l => secretIn?.(l) !== undefined || simpleCommands(l).some(holds))) kept.push(...rest.split("\n"));
    else if (!first.trimStart().startsWith("#") && !/;;|;&|;\|/.test(first)) kept.push(`${/^[ \t]*/.exec(first)?.[0] ?? ""}true`);
  };
  let cutting: { open: Open; lines: string[]; whole: boolean } | undefined;
  /** With a heredoc open, `value` is what a cut body line left open: the body lines up to its close go with it. */
  let passing: Pick<Open, "heredoc" | "arith"> & { value?: Open } = { arith: 0 };
  let joining: { open: Open; lines: string[] } | undefined;
  const take = (line: string): void => {
    const found = secretIn?.(line);
    const hits = simpleCommands(line).flatMap(c => assignedIn(c).filter(n => isSecretName(n) || (found !== undefined && secretIn?.(c) !== undefined)));
    if (hits.length === 0 && found === undefined) {
      kept.push(...line.split("\n"));
      passing = carried(line, passing.arith);
      return;
    }
    for (const h of hits) if (!names.includes(h)) names.push(h);
    if (hits.length === 0 && found !== undefined && !held.includes(found)) held.push(found);
    const o = scanLine(line, { ...CLOSED, arith: passing.arith });
    if (isOpen(o)) cutting = { open: o, lines: line.split("\n"), whole: o.heredoc === undefined };
    else {
      leave(line.split("\n"), true);
      passing = { arith: o.arith };
    }
  };
  for (const line of text.split(/\r?\n/)) {
    if (cutting !== undefined) {
      const o = scanLine(line, cutting.open);
      cutting.lines.push(line);
      if (isOpen(o)) cutting = { ...cutting, open: o, whole: cutting.whole && o.heredoc === undefined };
      else {
        leave(cutting.lines, cutting.whole);
        cutting = undefined;
        passing = { arith: o.arith };
      }
      continue;
    }
    if (joining !== undefined) {
      const o = scanLine(line, joining.open);
      joining.lines.push(line);
      if (o.quote !== "" || o.groups.length > 0 || o.continues) joining.open = o;
      else {
        take(joining.lines.join("\n"));
        joining = undefined;
      }
      continue;
    }
    if (passing.heredoc !== undefined) {
      if (line.trim() === passing.heredoc) {
        kept.push(line);
        passing = { arith: passing.arith };
      } else if (passing.value !== undefined) {
        const o = scanLine(line, passing.value);
        passing.value = isOpen(o) ? o : undefined;
      } else {
        const found = secretIn?.(line);
        const named = simpleCommands(line).flatMap(assignedIn).filter(isSecretName);
        for (const n of named) if (!names.includes(n)) names.push(n);
        if (named.length === 0 && found !== undefined && !held.includes(found)) held.push(found);
        if (named.length === 0 && found === undefined) kept.push(line);
        // In an unquoted heredoc a body line ending in a backslash joins the next: a blank one stands for the cut line.
        else if (/(?:^|[^\\])(?:\\\\)*\\$/.test(kept.at(-1) ?? "")) kept.push("");
        const o = scanLine(line, CLOSED);
        if (named.length > 0 && isOpen(o) && o.heredoc === undefined) passing.value = o;
      }
      continue;
    }
    if (opensShell(line, passing.arith)) joining = { open: scanLine(line, { ...CLOSED, arith: passing.arith }), lines: [line] };
    else take(line);
  }
  if (joining !== undefined) take(joining.lines.join("\n"));
  if (cutting !== undefined) leave(cutting.lines, false);
  return { names, carried: kept.join(eol) };
}

const SOURCE = /^([ \t]*)(source|\\?\.)[ \t]+("[^"]*"|'[^']*'|[^\s;&|<>()"'\\`]+)(?=[\s;&|<>()]|$)(.*)$/;

export interface SourceCommand {
  indent: string;
  word: string;
  /** As written, quotes included. */
  token: string;
  /** Everything after the token. */
  rest: string;
}

/** A command that reads a file: its `source` or `.` word (nvm writes `\.` to dodge an alias on the dot), the one token after it and what follows. */
export function sourceCommand(command: string): SourceCommand | undefined {
  const m = SOURCE.exec(command);
  return m === null ? undefined : { indent: m[1]!, word: m[2]!, token: m[3]!, rest: m[4]! };
}

/** The file a source token names, absolute, when the token is literal: `~/`, `$HOME/` and `${HOME}/` fold to home; a
 * variable elsewhere, a glob, a substitution, an escape or an empty, `.` or `..` segment is left to a running shell. */
export function sourcedPath(token: string, home: string): string | undefined {
  const raw = /^(["']).*\1$/.test(token) ? token.slice(1, -1) : token;
  const path = raw.replace(/^(?:~|\$HOME|\$\{HOME\})(?=\/)/, () => home);
  if (!path.startsWith("/") || /[$`*?[\]\\]/.test(path) || path.split("/").slice(1).some(seg => seg === "" || seg === "." || seg === "..")) return undefined;
  return path;
}

/** The files an rc file reads with `source` or `.` by a literal path, anywhere on the line, in order, once each. */
export function sourcedPaths(text: string, home: string): string[] {
  const out: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    // simpleCommands splits on braces, so `${HOME}` is folded before the split.
    for (const cmd of simpleCommands(line.replace(/\$\{HOME\}/g, "$HOME"))) {
      const src = sourceCommand(cmd);
      const p = src === undefined ? undefined : sourcedPath(src.token, home);
      if (p !== undefined && !out.includes(p)) out.push(p);
    }
  }
  return out;
}
