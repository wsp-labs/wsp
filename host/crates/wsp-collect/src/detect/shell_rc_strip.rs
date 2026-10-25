// SPDX-License-Identifier: AGPL-3.0-only
//! Cutting secrets out of a carried rc file: every line that assigns a secret-named variable goes, with its
//! continuation and any open quote, substitution, array or heredoc; in a script each cut command stands as `true` so
//! the file still parses. Port of packages/collect/src/detect/shell-rc.ts:205-523 at a3b870a6e; the spec is
//! spec/shell_rc_strip.md. The regex sources it reads (QUOTED, HEREDOC, ASSIGNS, GUARD, KEYWORDS) are in shell_rc.

/// What `strip_exports` hands back (TS `{ names, carried }`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stripped {
    /// Each variable a cut line assigns a secret to, once, in first-cut order.
    pub names: Vec<String>,
    /// The file as it travels, its line endings the file's own.
    pub carried: String,
}

// shell-rc.ts:217
// /** What is still open at the end of a line: a quote, a backtick, `$(`, `${` and array groups, `((` arithmetic, a heredoc waiting for its word, or a trailing backslash. */
// interface Open {
//   quote: "" | "'" | '"' | "`" | "$'";
//   /** The open `$(`, `${`, array `=(` and bare `(` groups, innermost last, each with the quote it opened inside: a
//    * `$(` reads its text unquoted and gives that quote back at its `)`, and a `)` or `}` closes only the group on top. */
//   groups: { close: ")" | "}"; quote: "" | '"' }[];
//   /** Depth of `((` and `$((`; while open, `<<` is a shift and never a heredoc. */
//   arith: number;
//   heredoc?: string;
//   continues: boolean;
// }
/// What is still open at the end of a line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Open {
    /// `""`, `'`, `"`, a backtick or `$'`.
    pub quote: &'static str,
    pub groups: Vec<OpenGroup>,
    pub arith: u32,
    pub heredoc: Option<String>,
    pub continues: bool,
}

/// One open `$(`, `${`, array `=(` or bare `(` group: its closer and the quote it gives back there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenGroup {
    /// `)` or `}`.
    pub close: char,
    /// `""` or `"`.
    pub quote: &'static str,
}

// Private helpers the port keeps (names in snake case):
// shell-rc.ts:211
// /** Whether a line leaves open the quoted argument of an `eval`, `alias` or `abbr`, whose text is shell: the lines up to
//  * the quote's close are read as one command. A single-quoted string ending in `\'` is fish's escaped quote, which this
//  * reading takes for a close, so such a line is read alone. */
// function opensShell(line: string, arith: number): boolean {
//   if ((line.match(QUOTED) ?? []).some(t => /^'[^']*\\'$/.test(t)) || !["'", '"', "$'"].includes(scanLine(line, { ...CLOSED, arith }).quote)) return false;
//   return ["eval", "alias", "abbr"].includes(commandWords(simpleCommands(line).at(-1) ?? "")[1][0] ?? "");
// }
// shell-rc.ts:228
// const CLOSED: Open = { quote: "", groups: [], arith: 0, continues: false };
// shell-rc.ts:230
// function isOpen(o: Open): boolean {
//   return o.quote !== "" || o.groups.length > 0 || o.heredoc !== undefined || o.continues;
// }
// shell-rc.ts:336
// /** What a kept line may carry forward: a heredoc it opens and the depth of an arithmetic block it left open, and nothing else, since a comment's stray quote must never hide what follows. An open `((` only stops `<<` from being read as a heredoc until its `))`, which over-cuts a heredoc body and never leaks. */
// function carried(line: string, arith: number): Pick<Open, "heredoc" | "arith"> {
//   const o = scanLine(line, { ...CLOSED, arith });
//   return { heredoc: o.heredoc, arith: o.arith };
// }
// shell-rc.ts:342
// /** How far a command's leading blanks and keywords run. */
// function leadOf(segment: string): number {
//   let at = /^(?:\s|\\\n)*/.exec(segment)![0].length;
//   for (let m = KEYWORDS.exec(segment.slice(at)); m !== null; m = KEYWORDS.exec(segment.slice(at))) at += m[0].length;
//   return at;
// }
// shell-rc.ts:352
// /** The top-level simple commands and comments of closed shell text, keywords skipped, as [start, end) offsets. They split
//  * on `;`, `|`, a newline, an `&` that is no redirect's, a bare `{` or `}`, and a `(` or `)` opening a subshell or closing a
//  * case pattern; quotes, substitutions, `[[ ]]` and a `(` inside a command (an array, a fish substitution, `f()`) hold together.
//  * With `lists`, `&&`, `||` and `|` do not split, so each span is a whole and-or list. */
// function commandSpans(text: string, lists = false): [number, number][] {
//   const spans: [number, number][] = [];
//   const stack: string[] = [];
//   let start = 0;
//   const end = (i: number): void => {
//     const s = start + leadOf(text.slice(start, i));
//     const e = start + text.slice(start, i).replace(/(?:\s|\\\n)*$/, "").length;
//     if (e > s) spans.push([s, e]);
//   };
//   for (let i = 0; i < text.length; i += 1) {
//     const c = text[i]!;
//     const top = stack.at(-1);
//     if (c === "\\" && top !== "'") i += 1;
//     else if (top === "'" || top === "$'" || top === "`") {
//       if (c === (top === "`" ? "`" : "'")) stack.pop();
//     } else if (c === "$" && (text[i + 1] === "(" || text[i + 1] === "{")) {
//       stack.push(text[i + 1]!);
//       i += 1;
//     } else if (c === "`" || (top !== '"' && (c === "'" || c === '"'))) stack.push(c);
//     else if (top === '"') {
//       if (c === '"') stack.pop();
//     } else if (c === "$" && text[i + 1] === "'") {
//       stack.push("$'");
//       i += 1;
//     } else if (top !== undefined) {
//       if (top === "(" && commentAt(text, i)) while (i + 1 < text.length && text[i + 1] !== "\n") i += 1;
//       else if (c === "(") stack.push(c);
//       else if ((c === ")" && top === "(") || (c === "}" && top === "{")) stack.pop();
//       else if (top === "[[" && text.startsWith("]]", i)) {
//         stack.pop();
//         i += 1;
//       }
//     } else if (commentAt(text, i)) {
//       end(i);
//       const from = i;
//       while (i + 1 < text.length && text[i + 1] !== "\n") i += 1;
//       spans.push([from, i + 1]);
//       start = i + 1;
//     } else if (/^\[\[\s/.test(text.slice(i, i + 3)) && leadOf(text.slice(start, i)) === i - start) {
//       stack.push("[[");
//       i += 1;
//     } else if (c === "(" && leadOf(text.slice(start, i)) < i - start) stack.push(c);
//     else if (
//       ";\n()".includes(c) ||
//       (c === "|" && text[i - 1] !== ">" && !lists) ||
//       (c === "&" && !/[<>]/.test(text[i - 1] ?? "") && text[i + 1] !== ">" && !(lists && /[&|]/.test(`${text[i - 1] ?? ""}${text[i + 1] ?? ""}`))) ||
//       ((c === "{" || c === "}") && /^\s?$/.test(text[i + 1] ?? "") && /^[\s;&]?$/.test(text[i - 1] ?? ""))
//     ) {
//       end(i);
//       start = i + 1;
//     }
//   }
//   end(text.length);
//   return spans;
// }

// shell-rc.ts:243
// /** Carries the shell's quoting state across a line. Inside a heredoc only the terminator word matters. */
// function scanLine(line: string, start: Open): Open {
//   const o: Open = { ...start, groups: [...start.groups], continues: false };
//   if (o.heredoc !== undefined) {
//     if (line.trim() === o.heredoc) delete o.heredoc;
//     return o;
//   }
//   for (let i = 0; i < line.length; i += 1) {
//     const c = line[i];
//     const top = o.groups.at(-1);
//     o.continues = false;
//     if (c === "\\" && o.quote !== "'") {
//       if (i === line.length - 1) o.continues = true;
//       i += 1;
//       continue;
//     }
//     if (o.quote === "$'") {
//       if (c === "'") o.quote = "";
//       continue;
//     }
//     if (o.quote === "'" || o.quote === "`") {
//       if (c === o.quote) o.quote = "";
//       continue;
//     }
//     if (o.quote === "" && commentAt(line, i)) break;
//     if (o.quote === "" && c === "$" && line[i + 1] === "'") {
//       o.quote = "$'";
//       i += 1;
//       continue;
//     }
//     if (o.quote === "" && (c === "'" || c === "`")) {
//       o.quote = c;
//       continue;
//     }
//     if (c === '"') {
//       o.quote = o.quote === '"' ? "" : '"';
//       continue;
//     }
//     if (o.quote === "" && c === "$" && line[i + 1] === "(" && line[i + 2] === "(") {
//       o.arith += 1;
//       i += 2;
//       continue;
//     }
//     if (c === "$" && line[i + 1] === "(") {
//       o.groups.push({ close: ")", quote: o.quote === '"' ? '"' : "" });
//       o.quote = "";
//       i += 1;
//       continue;
//     }
//     if (c === "$" && line[i + 1] === "{") {
//       o.groups.push({ close: "}", quote: "" });
//       i += 1;
//       continue;
//     }
//     if (o.quote === "" && c === "(" && line[i + 1] === "(") {
//       o.arith += 1;
//       i += 1;
//       continue;
//     }
//     if (o.quote === "" && c === ")" && line[i + 1] === ")" && o.arith > 0) {
//       o.arith -= 1;
//       i += 1;
//       continue;
//     }
//     if (o.quote === "" && c === "(" && (top?.close === ")" || line[i - 1] === "=")) {
//       o.groups.push({ close: ")", quote: "" });
//       continue;
//     }
//     if (o.quote === "" && c === ")" && top?.close === ")") {
//       o.groups.pop();
//       o.quote = top.quote;
//       continue;
//     }
//     if (c === "}" && top?.close === "}") {
//       o.groups.pop();
//       continue;
//     }
//     if (c === "<" && line[i + 1] === "<" && line[i + 2] === "<") {
//       i += 2;
//       continue;
//     }
//     if (o.quote === "" && o.arith === 0 && c === "<") {
//       const m = HEREDOC.exec(line.slice(i));
//       const word = m?.[1] ?? m?.[2] ?? m?.[3] ?? m?.[4];
//       if (word !== undefined) {
//         o.heredoc = word;
//         break;
//       }
//     }
//   }
//   return o;
// }
/// Carries the shell's quoting state across one line.
pub fn scan_line(line: &str, start: &Open) -> Open {
    todo!()
}

// shell-rc.ts:418
// /** The arithmetic depth is one stream over the file: a cut line reads it and writes it back like a kept line does. A
//  * line `secretIn` finds a secret in goes too, a heredoc's included, and a heredoc's line that assigns a secret goes with
//  * what it leaves open, up to the heredoc's word. An `eval`, `alias` or `abbr` whose quote runs over lines is one
//  * command. `names` holds only variables, each one a cut line assigns a secret to, since init asks the person for each;
//  * `held` gets what `secretIn` names for a cut line that assigns none. In a `script`, each cut command stands as `true`,
//  * a cut comment goes, and the rest of its line stays, so a block it opened, closed or was the only line of still
//  * parses. What a cut command that does more than assign decided never runs: an `if`, `elif` or `while` condition
//  * holding one stands as false and an `until` condition as true, and a list where one leads into `&&`, `||` or `|`
//  * stands as one `true`. A cut that cannot split (a heredoc, the head of a for, select or case) stands as one `true` at
//  * its indent, or as nothing when it is a comment or holds a case arm's `;;`, `;&` or `;|`. */
// export function stripExports(text: string, secretIn?: (line: string) => string | undefined, held: string[] = [], script = false): { names: string[]; carried: string } {
//   const names: string[] = [];
//   const kept: string[] = [];
//   const eol = text.includes("\r\n") ? "\r\n" : "\n";
//   const holds = (command: string): boolean => assignedIn(command).some(isSecretName) || secretIn?.(command) !== undefined;
//   const leave = (lines: string[], whole: boolean): void => {
//     if (!script) return;
//     const cut = lines.join("\n");
//     let rest = "";
//     let at = 0;
//     const spans = (whole ? commandSpans(cut) : []).filter(([s, e]) => holds(cut.slice(s, e)));
//     const lists = commandSpans(cut, true);
//     const stands = (s: number, e: number): [number, number, string] => {
//       if (cut[s] === "#") return [s, e, ""];
//       if (ASSIGNS.test(cut.slice(s, e))) return [s, e, "true"];
//       const [ls, le] = lists.find(([a, b]) => a <= s && e <= b) ?? [s, e];
//       const guard = GUARD.exec(cut.slice(0, ls));
//       if (guard !== null) {
//         const until = guard[1] === "until";
//         // zsh's `if [[ ]] { }` reads its condition only up to a closing `]]`.
//         const word = /^\s*\{/.test(cut.slice(le)) ? `[[ ${until ? "-z" : "-n"} '' ]]` : until ? "true" : "false";
//         return [guard.index + guard[0].length, le, word];
//       }
//       return /^\s*[&|]/.test(cut.slice(e, le)) ? [ls, le, "true"] : [s, e, "true"];
//     };
//     const ranges = spans.some(([s]) => /^(?:for|select|case)\s/.test(cut.slice(s))) ? [] : spans.map(([s, e]) => stands(s, e));
//     for (const [s, e, word] of ranges.sort(([a, b], [c, d]) => a - c || d - b)) {
//       if (s < at) continue;
//       rest += word === "" ? cut.slice(at, s).replace(/[ \t]+$/, "") : `${cut.slice(at, s)}${word}`;
//       at = e;
//     }
//     rest += cut.slice(at);
//     const first = lines[0]!;
//     if (rest !== cut && rest.trim() !== "" && !rest.split("\n").some(l => secretIn?.(l) !== undefined || simpleCommands(l).some(holds))) kept.push(...rest.split("\n"));
//     else if (!first.trimStart().startsWith("#") && !/;;|;&|;\|/.test(first)) kept.push(`${/^[ \t]*/.exec(first)?.[0] ?? ""}true`);
//   };
//   let cutting: { open: Open; lines: string[]; whole: boolean } | undefined;
//   /** With a heredoc open, `value` is what a cut body line left open: the body lines up to its close go with it. */
//   let passing: Pick<Open, "heredoc" | "arith"> & { value?: Open } = { arith: 0 };
//   let joining: { open: Open; lines: string[] } | undefined;
//   const take = (line: string): void => {
//     const found = secretIn?.(line);
//     const hits = simpleCommands(line).flatMap(c => assignedIn(c).filter(n => isSecretName(n) || (found !== undefined && secretIn?.(c) !== undefined)));
//     if (hits.length === 0 && found === undefined) {
//       kept.push(...line.split("\n"));
//       passing = carried(line, passing.arith);
//       return;
//     }
//     for (const h of hits) if (!names.includes(h)) names.push(h);
//     if (hits.length === 0 && found !== undefined && !held.includes(found)) held.push(found);
//     const o = scanLine(line, { ...CLOSED, arith: passing.arith });
//     if (isOpen(o)) cutting = { open: o, lines: line.split("\n"), whole: o.heredoc === undefined };
//     else {
//       leave(line.split("\n"), true);
//       passing = { arith: o.arith };
//     }
//   };
//   for (const line of text.split(/\r?\n/)) {
//     if (cutting !== undefined) {
//       const o = scanLine(line, cutting.open);
//       cutting.lines.push(line);
//       if (isOpen(o)) cutting = { ...cutting, open: o, whole: cutting.whole && o.heredoc === undefined };
//       else {
//         leave(cutting.lines, cutting.whole);
//         cutting = undefined;
//         passing = { arith: o.arith };
//       }
//       continue;
//     }
//     if (joining !== undefined) {
//       const o = scanLine(line, joining.open);
//       joining.lines.push(line);
//       if (o.quote !== "" || o.groups.length > 0 || o.continues) joining.open = o;
//       else {
//         take(joining.lines.join("\n"));
//         joining = undefined;
//       }
//       continue;
//     }
//     if (passing.heredoc !== undefined) {
//       if (line.trim() === passing.heredoc) {
//         kept.push(line);
//         passing = { arith: passing.arith };
//       } else if (passing.value !== undefined) {
//         const o = scanLine(line, passing.value);
//         passing.value = isOpen(o) ? o : undefined;
//       } else {
//         const found = secretIn?.(line);
//         const named = simpleCommands(line).flatMap(assignedIn).filter(isSecretName);
//         for (const n of named) if (!names.includes(n)) names.push(n);
//         if (named.length === 0 && found !== undefined && !held.includes(found)) held.push(found);
//         if (named.length === 0 && found === undefined) kept.push(line);
//         // In an unquoted heredoc a body line ending in a backslash joins the next: a blank one stands for the cut line.
//         else if (/(?:^|[^\\])(?:\\\\)*\\$/.test(kept.at(-1) ?? "")) kept.push("");
//         const o = scanLine(line, CLOSED);
//         if (named.length > 0 && isOpen(o) && o.heredoc === undefined) passing.value = o;
//       }
//       continue;
//     }
//     if (opensShell(line, passing.arith)) joining = { open: scanLine(line, { ...CLOSED, arith: passing.arith }), lines: [line] };
//     else take(line);
//   }
//   if (joining !== undefined) take(joining.lines.join("\n"));
//   if (cutting !== undefined) leave(cutting.lines, false);
//   return { names, carried: kept.join(eol) };
// }
/// `secret_in` names a secret a line holds by a rule of the caller's; `held` collects what it names for a cut line
/// that assigns none (TS defaults: none, a fresh list, `script` false).
pub fn strip_exports(text: &str, secret_in: Option<&dyn Fn(&str) -> Option<String>>, held: &mut Vec<String>, script: bool) -> Stripped {
    todo!()
}
