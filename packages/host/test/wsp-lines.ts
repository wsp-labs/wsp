// SPDX-License-Identifier: AGPL-3.0-only
/** The words of a shell line: quotes and `<...>` placeholders group, `[` and `]` are dropped, and a redirect, a
 * pipe, a `&` or a `#` comment ends the line; the comment's text comes back beside the words. */
export function shellWords(line: string): { words: string[]; comment?: string } {
  const words: string[] = [];
  let word = "";
  let quote: string | undefined;
  let angle = 0;
  const flush = (): void => {
    if (word !== "") words.push(word);
    word = "";
  };
  for (let i = 0; i < line.length; i++) {
    const c = line[i]!;
    if (quote !== undefined) {
      word += c;
      if (c === quote) quote = undefined;
      continue;
    }
    if (angle > 0) {
      word += c;
      if (c === "<") angle++;
      if (c === ">") angle--;
      continue;
    }
    if (c === "'" || c === '"') {
      quote = c;
      word += c;
      continue;
    }
    if (c === "<") {
      angle = 1;
      word += c;
      continue;
    }
    if (c === "[" || c === "]") continue;
    if (c === "#" && word === "") return { words, comment: line.slice(i + 1).trim() };
    if (/\s/.test(c)) {
      flush();
      continue;
    }
    if (word === "" && (c === ">" || c === "&" || c === "|" || c === ";" || /^\d>/.test(line.slice(i)))) {
      return { words };
    }
    word += c;
  }
  flush();
  return { words };
}

export const ILLUSTRATIVE = "illustrative";

/** Every `wsp ...` line a reader would copy from the text: each line of a fenced block and each inline code span
 * that opens with wsp (a leading nohup dropped). An output line (`wsp <version>`), an error line (`wsp new: ...`)
 * and a fenced line whose comment says illustrative are not commands. */
export function wspCommands(text: string): string[][] {
  const candidates: string[] = [];
  const lines = text.split("\n");
  let fenced = false;
  for (const line of lines) {
    if (line.trim().startsWith("```")) {
      fenced = !fenced;
      continue;
    }
    if (fenced) candidates.push(line);
    else for (const m of line.matchAll(/`([^`]+)`/g)) candidates.push(m[1]!);
  }
  const commands: string[][] = [];
  for (const candidate of candidates) {
    const { words, comment } = shellWords(candidate.trim());
    if (words[0] === "nohup") words.shift();
    if (words[0] !== "wsp" || words.length < 2) continue;
    if (comment === ILLUSTRATIVE) continue;
    const first = words[1]!;
    if (first.startsWith("<") || first.endsWith(":")) continue;
    commands.push(words);
  }
  return commands;
}
