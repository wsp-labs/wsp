// SPDX-License-Identifier: AGPL-3.0-only
// Writes tests/fixtures/toml_parse.json: TOML texts and what smol-toml 1.9.0's parse makes of each, the tree as JSON
// (a date as {"$date":true}, a number that is not finite as a string) or its error's whole message. Needs smol-toml
// beside this file's package: `npm i smol-toml@1.9.0` in a scratch folder, then
// `SMOL_TOML=<that folder>/node_modules/smol-toml/dist/index.js node oracle/toml_parse.oracle.mjs`.
import { writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const { parse } = await import(pathToFileURL(process.env.SMOL_TOML ?? "smol-toml").href);
let seed = 20261026;
const next = n => {
  seed = (Math.imul(seed, 1103515245) + 12345) >>> 0;
  return (seed >>> 8) % n;
};

const seeds = [
  'model = "gpt-5"\n[mcp_servers.a]\ncommand = "x"\nargs = ["--y", \'z\']\nenv = { A = "1", B = "2" }\nstartup_timeout_sec = 20.5\n',
  '[mcp_servers.b]\nurl = "https://h/m"\nbearer_token_env_var = "T"\n[mcp_servers.b.http_headers]\nX = "y"\n[mcp_servers.b.env_http_headers]\nZ = "W"\n',
  '[projects."/work/a"]\ntrust_level = "trusted"\n\n[projects."/work/b"]\ntrust_level = "untrusted"\n',
  'a.b.c = 1\na.b.d = [1, 2, 3]\n[x]\ny = true\n[[z]]\nn = 1\n[[z]]\nn = 2\n',
  's = """\nmulti\\\n   line "quoted" \\u00e9\\U0001F600\n"""\nl = \'\'\'raw \\n\'\'\'\n',
  'i = [1_000, 0x1F, 0o17, 0b101, +5, -0, 1e3, 2.5e-2, inf, -inf, nan]\n',
  'n = { a = 1, b = { c = [1, { d = 2 }] } }\n# comment\n"quoted key" = "v" # tail\n',
  '[mcp_servers."1"]\ncommand = "a"\n[mcp_servers."0"]\ncommand = "b"\n[mcp_servers.z]\ncommand = "c"\n',
];
// V8's legacy date reader takes dates no one writes ("1979-05-,2"); the dates here are the shapes TOML has.
const curated = [
  "d = [1979-05-27T07:32:00Z, 1979-05-27, 07:32:00, 1979-05-27 07:32:00.5-08:00]\n", "d = [1979-05-27T07:32:00+05:30, 1979-05-27t07:32, 00:00:00.999999999, 23:59:59]", "a = 1979-05-27T07:32:00+23:59", "a = 1979-05-27T07:32:00+00:60", "a = 1979-05-27T07:32:00-24:00", "a = 2021-02-31", "a = 2020-02-30T10:00:00", "a = 1979-05-27 x", "a = 1979-05-27T", "a = 1979-05-27T07", "a = 1979-05-27T07:3", "a = 07:32:0", "a = 07:3", "a = 07:32:00.", "a = 1979-05-27 07:32:00.",
  "", "\n", "# only a comment\n", "a = 1", "a = 1\r\nb = 2\r\n", "\ufeffa = 1\n", "a =", "a", "a = ", "= 1", "a = 1 b = 2", "a = 1\na = 2", "[a]\n[a]", "[a]\nb=1\n[a.b]", "a.b = 1\n[a]", "[a.b]\n[a]\nc=1", "[[a]]\n[a]", "[a]\n[[a]]",
  "a = {\n x = 1,\n y = 2,\n}", "a = { x = 1, }", "a = {,}", "a = { x = 1 y = 2 }", "a = { x = 1,, }", "a = [1,,2]", "a = [,]", "a = [1 2]", "a = [\n1,\n2,\n]",
  'a = "\\e"', 'a = "\\x41\\xe9"', 'a = "\\x4"', 'a = "\\u12"', 'a = "\\ud800"', 'a = "\\U00110000"', 'a = "\\U0010FFFF"', 'a = "\\q"', 'a = "abc', "a = 'abc", 'a = """abc', 'a = "a\nb"', 'a = "a\u0001b"', 'a = "\\u0000"',
  "a = 07:32", "a = 07:32:00", "a = 24:00:00", "a = 12:60", "a = 12:00:60", "a = 1979-05-27", "a = 1979-02-30", "a = 1979-13-01", "a = 1979-05-27T07:32", "a = 1979-05-27t07:32:00z", "a = 1979-05-27 07:32:00+24:00", "a = 1979-05-27T07:32:00.Z", "a = 1979-05-27T25:00:00Z", "a = 1979-5-27",
  "a = 9007199254740991", "a = 9007199254740992", "a = -9007199254740992", "a = 0xFFFFFFFFFFFFFFFF", "a = 1__0", "a = _1", "a = 1_", "a = 01", "a = 00", "a = +0", "a = -0.0", "a = 1e", "a = 1.e5", "a = .5", "a = 1.5.5", "a = 0x", "a = 0b2", "a = 0o8", "a = +0x1", "a = 1e400", "a = inf", "a = +inf", "a = nan", "a = infx", "a = tru", "a = true1", "a = truee",
  "a.b.c.d = 1\na.b.c.e = 2", "a.'b'.\"c\" = 1", "'' = 1", "\"\" = 1", "a. b . c = 1", "a..b = 1", ".a = 1", "a. = 1", "a b = 1", "[a b]", "[ a . b ]", "[a.]", "[]", "[a", "[[a]", "[[a]] x", "a = 1 # c\n# d\n  \n\t\nb = 2", "a = 1\x7f", "# c\u0001\n", "key = \"v\"\n[table]\nkey = \"v2\"\n",
  "__proto__ = 1", "[__proto__]\na = 1", "constructor.x = 1", "a = [[1,2],[3,[4]]]", "a = [ ]", "a = { }", "a = []\n[a.b]", "a = {}\n[a.b]", "a = { b = 1 }\na.c = 2", "a = { b = { c = 1 }, b.d = 2 }",
];
const edits = ["[", "]", "{", "}", "=", ".", ",", '"', "'", "\\", "#", "\n", "\r", " ", "\t", "-", "+", "_", ":", "e", "E", "0", "1", "x", "t", "f", "u", "\u00e9", "\u{1F600}", "\u0001", "\u007f", "Z", "T", "i", "n"];
const texts = [...curated, ...seeds];
for (let i = 0; i < 6000; i++) {
  let t = seeds[next(seeds.length)];
  for (let k = 1 + next(3); k > 0; k--) {
    const chars = [...t];
    const at = next(chars.length + 1);
    switch (next(4)) {
      case 0: chars.splice(at, 1); break;
      case 1: chars.splice(at, 0, edits[next(edits.length)]); break;
      case 2: chars.splice(at, 1, edits[next(edits.length)]); break;
      default: chars.length = at; break;
    }
    t = chars.join("");
  }
  texts.push(t);
}
const encode = v => {
  if (v instanceof Date) return { $date: true };
  if (typeof v === "number") return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(encode);
  if (v !== null && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, encode(x)]));
  return v;
};
const cases = [];
for (const text of [...new Set(texts)]) {
  try {
    cases.push({ text, ok: JSON.stringify(encode(parse(text))) });
  } catch (e) {
    if (e instanceof RangeError) continue;
    cases.push({ text, error: e.message });
  }
}
writeFileSync(new URL("../tests/fixtures/toml_parse.json", import.meta.url), JSON.stringify(cases));
console.log(`${cases.length} cases, ${cases.filter(c => c.error).length} errors`);
