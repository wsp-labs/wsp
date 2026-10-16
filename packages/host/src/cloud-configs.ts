// SPDX-License-Identifier: AGPL-3.0-only
// What a copy to a provider's machine cuts out of the configs it carries: the
// Claude account fields, the secret names of an env block, a secret a hook, an
// MCP argument or an address holds inline, git's credential helpers and url
// rewrites, and every value shaped like a token. What a config holds besides
// lands as it is, so a config with no secret lands byte for byte.
import { KEYLIKE_SEGMENT, hidesValue, plainArg, readArgs, secretNamed, type ArgRead } from "@wsp/collect";
import { CATALOG_AGENTS, MCP_AGENTS, editJson, readJsonc, rewriteTomlLine, tomlOpenArray, tomlStrings, uncommentToml, type JsonEdit } from "@wsp/catalog";
import { CLOUD_GIT_CUTS, CONFIG_PATHS, FISH_VARIABLES, gitCut } from "./recipe-configs.js";
import { blankTokens, holdsPrivateKey } from "./token-shapes.js";

const CLAUDE_JSON = "~/.claude.json";
/** The account signed in here, and the key a /login keeps in the file where there is no Keychain (2.1.296). */
const CLAUDE_ACCOUNT_KEYS = ["oauthAccount", "customApiKeyResponses", "primaryApiKey"];
/** Every config file an agent keeps its MCP servers or its hooks in, home-relative. */
const AGENT_CONFIGS = new Set([CLAUDE_JSON, ...MCP_AGENTS.flatMap(a => a.mcp.files), ...CATALOG_AGENTS.flatMap(a => (a.hooks !== undefined ? [a.hooks.file] : []))]);
const GIT_CONFIGS = new Set(CONFIG_PATHS.git.filter(p => p.endsWith("config")).map(p => `~/${p}`));

export const TOKEN_NAME = "a value shaped like a token";

/** The line a copy notes for a file it cut, naming what stayed. */
export const stayedLine = (names: readonly string[]): string => `${[...new Set(names)].join(", ")} blanked or left out; the values stay on this computer`;
export const SIGN_INS_STAY = "mcp-remote's sign-ins stay on this computer; sign in on the machine";
export const FISH_VARIABLES_STAY = "fish keeps its universal variables, secrets among them, here; they stay on this computer";
export const PRIVATE_KEY_STAYS = "holds a private key, which stays on this computer";

/** Every reference to a variable an agent expands: `${X}` with or without a default, a bare `$X`, OpenCode's `{env:X}`. */
const REFS = /\$\{[A-Za-z_]\w*(?::-[^}]*)?\}|\$[A-Za-z_]\w*|\{env:[A-Za-z_]\w*\}/g;
/** A command substitution: the shell runs it and its output is the value, so the text holds none. */
const COMMAND = /\$\(|`/;
const ADDRESS_RUN = /[A-Za-z][A-Za-z0-9+.-]*:\/\/[^\s"'<>`]+/g;
const VALUE = "(\\$\\([^)]*\\)|`[^`]*`|\"[^\"]*\"|'[^']*'|[^\\s\"';&|]*)";
const ASSIGN = new RegExp(`(?<![\\w-])([A-Za-z_]\\w*)=${VALUE}`, "g");
const FLAG_ASSIGN = new RegExp(`(?<![\\w-])(--?[A-Za-z][\\w-]*)=${VALUE}`, "g");
const unquoted = (v: string): string => (/^(["']).*\1$/.test(v) ? v.slice(1, -1) : v);

/** Whether a value holds something of the person's: not empty, not references alone, not a command. */
const carries = (value: string): boolean => {
  const v = unquoted(value);
  return !COMMAND.test(v) && v.replace(REFS, "").trim() !== "";
};
/** A flag's value written as a header line holds only what follows the name and an Authorization header's scheme. */
const flagCarries = (value: string): boolean => carries(/^[A-Za-z0-9-]+:\s*(?:(?:Bearer|Basic|Token)\s+)?(.*)$/i.exec(value)?.[1] ?? value);
/** Words that name a flag taking a mode or a file, as in `--auth oauth` or `--cert none`, rather than the secret. */
const MODE_WORDS = new Set(["AUTH", "CERT"]);
/** A lowercase word after a flag that only auth or cert names a secret is a mode; a flag named by any other secret word
 * (key, token, secret, password, passwd, credential) takes the secret itself. */
const modeWord = (flag: string, value: string): boolean =>
  /^[a-z]+$/.test(value) && !secretNamed(flag.replace(/-/g, "_").toUpperCase().split("_").filter(w => !MODE_WORDS.has(w)).join("_"));
/** Whether a flag's value is a secret to blank: not a path or a plain address, holding something, and not a mode. */
const secretValue = (flag: string, value: string): boolean => !plainArg(value) && flagCarries(value) && !modeWord(flag, value);

interface Cut {
  text: string;
  names: string[];
}

/** An address with its login, its secret-named query values and, for a server's own address, the path segments the
 * collector reads as a key taken out. */
function blankAddress(url: string, own: boolean, names: string[]): string {
  let out = url.replace(/^([^:]+:\/\/)[^/@?#]+@/, (_, scheme: string) => {
    names.push("a login in an address");
    return scheme;
  });
  out = out.replace(/([?&])([^=&#]+)=([^&#]*)/g, (m, sep: string, name: string, value: string) => {
    if (!carries(value) || !secretNamed(name.replace(/-/g, "_"))) return m;
    names.push(name);
    return `${sep}${name}=`;
  });
  if (!own) return out;
  const path = /^([^:]+:\/\/[^/?#]*)([^?#]*)(.*)$/.exec(out);
  if (path === null) return out;
  const segments = path[2]!.split("/").map(seg => (KEYLIKE_SEGMENT.test(seg) ? (names.push("a key in an address"), "") : seg));
  return `${path[1]!}${segments.join("/")}${path[3]!}`;
}

/** One string outside a server's command line with every secret it holds inline blanked; `own` is a server's address. */
function blankString(s: string, own: boolean): Cut {
  const names: string[] = [];
  let out = s.replace(ADDRESS_RUN, url => blankAddress(url, own, names));
  out = out.replace(ASSIGN, (m, name: string, value: string) => (secretNamed(name) && carries(value) ? (names.push(name), `${name}=`) : m));
  out = out.replace(FLAG_ASSIGN, (m, flag: string, value: string) => (hidesValue(flag) && secretValue(flag, unquoted(value)) ? (names.push(flag), `${flag}=`) : m));
  const text = blankTokens(out, "");
  if (text !== out) names.push(TOKEN_NAME);
  return { text, names };
}

/** Whether an address holds anything a copy to a provider's machine blanks out of one: a login, a value under a
 * secret's name in its query, a path segment shaped like a key, or a token's shape. */
export const carriesSecret = (address: string): boolean => blankString(address, true).text !== address;

/** One argument as the collector read it: a flag's or an assignment's secret value goes, an assignment that holds an
 * address loses what the address carries, and every argument loses its tokens. */
function argCut(arg: string, read: ArgRead, names: string[]): string {
  const key = read.key ?? "";
  if (read.kind === "flag" || read.kind === "assign") {
    const at = arg.indexOf(`${key}=`);
    const head = at === -1 ? "" : arg.slice(0, at + key.length + 1);
    const value = arg.slice(head.length);
    if (read.kind === "assign" && !secretNamed(key)) return head + blankAddress(value, true, names);
    if (read.kind === "flag" ? !secretValue(key, value) : plainArg(value) || !carries(value)) return arg;
    names.push(key);
    return head;
  }
  return arg.replace(ADDRESS_RUN, url => blankAddress(url, true, names));
}

/** A server's arguments cut by collect's own reading of them, so the question the scan asked and the cut agree. */
function argsCut(command: string, args: readonly string[], home: string): { args: string[]; names: string[] } {
  const names: string[] = [];
  const reads = readArgs({ command, args }, home);
  const cut = args.map((arg, i) => {
    const out = argCut(arg, reads[i] ?? {}, names);
    const text = blankTokens(out, "");
    if (text !== out) names.push(TOKEN_NAME);
    return text;
  });
  return { args: cut, names };
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const strings = (v: unknown): v is string[] => Array.isArray(v) && v.every(a => typeof a === "string");
const ADDRESS_KEYS = new Set(["url", "httpUrl", "serverUrl"]);
const ENV_KEYS = new Set(["env", "environment"]);
const SERVER_TABLES = new Set(["mcpServers", "mcp", "mcp_servers"]);

/** Names a cut under an MCP server by that server, so the note says whose argument it was. */
function serverNamed(path: readonly (string | number)[], names: readonly string[]): string[] {
  const at = path.findIndex(p => typeof p === "string" && SERVER_TABLES.has(p));
  const server = at === -1 ? undefined : path[at + 1];
  return server === undefined ? [...names] : names.map(n => `${n} of the MCP server ${server}`);
}

/** A server's command line in a JSON entry: `args` beside a `command`, or OpenCode's `command` array, program first. */
function commandLine(entry: Record<string, unknown>, key: string, value: unknown): { command: string; args: string[]; from: number } | undefined {
  if (key === "args" && strings(value)) return { command: typeof entry["command"] === "string" ? entry["command"] : "", args: value, from: 0 };
  if (key === "command" && strings(value) && value.length > 0) return { command: value[0]!, args: value.slice(1), from: 1 };
  return undefined;
}

/** A JSON config with the keys named dropped, the secret names of every env block left out, every server's command
 * line cut, and every other string's inline secrets blanked, each edit made in place so comments and layout stand. */
function jsonCut(text: string, drop: readonly string[], home: string): Cut {
  let root: unknown;
  try {
    root = readJsonc(text).value;
  } catch {
    return { text, names: [] };
  }
  if (!isRecord(root)) return { text, names: [] };
  const edits: JsonEdit[] = [];
  const names: string[] = [];
  for (const key of drop) {
    if (!Object.hasOwn(root, key)) continue;
    edits.push([[key], undefined]);
    names.push(key);
  }
  const walk = (v: unknown, path: (string | number)[], key: string | number | undefined): void => {
    if (typeof v === "string") {
      const cut = blankString(v, typeof key === "string" && ADDRESS_KEYS.has(key));
      if (cut.text !== v) edits.push([path, cut.text]);
      names.push(...serverNamed(path, cut.names));
    } else if (Array.isArray(v)) {
      v.forEach((item, i) => walk(item, [...path, i], undefined));
    } else if (isRecord(v)) {
      for (const [name, value] of Object.entries(v)) {
        if (path.length === 0 && drop.includes(name)) continue;
        const line = commandLine(v, name, value);
        if (line !== undefined) {
          const cut = argsCut(line.command, line.args, home);
          cut.args.forEach((arg, i) => {
            if (arg !== line.args[i]) edits.push([[...path, name, i + line.from], arg]);
          });
          names.push(...serverNamed(path, cut.names));
        } else if (typeof key === "string" && ENV_KEYS.has(key) && typeof value === "string" && secretNamed(name) && carries(value)) {
          edits.push([[...path, name], undefined]);
          names.push(...serverNamed(path, [name]));
        } else walk(value, [...path, name], name);
      }
    }
  };
  walk(root, [], undefined);
  return { text: edits.length === 0 ? text : editJson(text, edits), names };
}

const TOML_KEY = /^("[^"]+"|'[^']+'|[A-Za-z0-9_-]+)\s*=\s*(.*)$/;
const TOML_SERVER = /^\[\s*mcp_servers\s*\.\s*(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))\s*(\.\s*env\s*)?\]$/;
/** Codex's tables of variables, the TOML twin of an env block: a server's env and the shell's set. */
const TOML_ENV_TABLE = /^\[\s*shell_environment_policy\s*\.\s*set\s*\]$/;

/** An inline table of variables with every secret-named member that holds something taken out, the others as written. */
function inlineEnvCut(line: string, names: string[]): string {
  const code = uncommentToml(line);
  const table = /^(\s*[^=]+=\s*)\{(.*)\}(\s*)$/.exec(code);
  if (table === null) return line;
  const members = [...table[2]!.matchAll(/("(?:[^"\\]|\\.)*"|'[^']*'|[A-Za-z0-9_-]+)\s*=\s*("(?:[^"\\]|\\.)*"|'[^']*')/g)];
  const kept = members.filter(m => {
    const name = unquoted(m[1]!);
    if (!secretNamed(name) || !carries(tomlStrings(m[2]!)[0] ?? "")) return true;
    names.push(name);
    return false;
  });
  if (kept.length === members.length) return line;
  return `${table[1]!}${kept.length === 0 ? "{}" : `{ ${kept.map(m => m[0]).join(", ")} }`}${table[3]!}${line.slice(code.length)}`;
}

/** A TOML config cut line by line: a server's `args` array by collect's reading across the lines it spans, the secret
 * names of a table of variables left out, and every other string's inline secrets blanked. */
function tomlCut(text: string, home: string): Cut {
  const lines = text.split("\n");
  const codes = lines.map(l => uncommentToml(l).trim());
  let header = "";
  const tables = codes.map(c => (c.startsWith("[") ? (header = c) : header));
  const commands = new Map<string, string>();
  codes.forEach((c, i) => {
    const m = TOML_KEY.exec(c);
    if (m?.[1] === "command") commands.set(tables[i]!, tomlStrings(m[2]!)[0] ?? "");
  });
  const names: string[] = [];
  const out: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const table = tables[i]!;
    const server = TOML_SERVER.exec(table);
    const path = server === null ? [] : ["mcp_servers", (server[1] ?? server[2] ?? server[3])!];
    const key = TOML_KEY.exec(codes[i]!);
    const name = key === null ? undefined : unquoted(key[1]!);
    if (name === "args" && server !== null && server[4] === undefined) {
      let end = i;
      let value = key![2]!;
      while (tomlOpenArray(value) && end + 1 < lines.length) value += codes[++end]!;
      const args = tomlStrings(value);
      const cut = argsCut(commands.get(table) ?? "", args, home);
      names.push(...serverNamed(path, cut.names));
      let at = 0;
      for (; i <= end; i++) out.push(rewriteTomlLine(lines[i]!, v => cut.args[at++] ?? v));
      i = end;
      continue;
    }
    const inEnv = TOML_ENV_TABLE.test(table) || server?.[4] !== undefined;
    if (inEnv && name !== undefined && secretNamed(name) && carries(tomlStrings(key![2]!)[0] ?? "")) {
      names.push(...serverNamed(path, [name]));
      continue;
    }
    const inline = (name === "set" && /^\[\s*shell_environment_policy\s*\]$/.test(table)) || (name === "env" && server !== null && server[4] === undefined);
    if (inline) {
      const found: string[] = [];
      out.push(inlineEnvCut(lines[i]!, found));
      names.push(...serverNamed(path, found));
      continue;
    }
    out.push(
      rewriteTomlLine(lines[i]!, v => {
        const cut = blankString(v, name === "url");
        names.push(...serverNamed(path, cut.names));
        return cut.text;
      }),
    );
  }
  return { text: out.join("\n"), names };
}

/** Why a staged file stays on this computer whole, or nothing: fish's universal variables, or a private key. */
export function leftWhole(name: string, text: string | undefined): string | undefined {
  if (name === FISH_VARIABLES) return FISH_VARIABLES_STAY;
  return text !== undefined && holdsPrivateKey(text) ? PRIVATE_KEY_STAYS : undefined;
}

/** A config as a provider's machine gets it, by the home-relative path it is read from here, with the names of
 * what stayed; every file loses its values shaped like a token. `home` is this computer's, which the collector
 * reads a command's program against. */
export function cloudCut(path: string, text: string, home: string): Cut {
  const structured = AGENT_CONFIGS.has(path)
    ? path.endsWith(".toml")
      ? tomlCut(text, home)
      : jsonCut(text, path === CLAUDE_JSON ? CLAUDE_ACCOUNT_KEYS : [], home)
    : GIT_CONFIGS.has(path)
      ? gitConfigCut(text, path === "~/.gitconfig")
      : { text, names: [] };
  const blanked = blankTokens(structured.text, "");
  return { text: blanked, names: blanked === structured.text ? structured.names : [...structured.names, TOKEN_NAME] };
}

function gitConfigCut(text: string, main: boolean): Cut {
  const cut = gitCut(text, main, CLOUD_GIT_CUTS);
  if (cut === text) return { text, names: [] };
  const sections = ["credential", "url"].filter(s => new RegExp(`^\\s*\\[\\s*${s}\\b`, "im").test(text)).map(s => `its [${s}] sections`);
  return { text: cut, names: sections.length > 0 ? sections : ["its secret values"] };
}
