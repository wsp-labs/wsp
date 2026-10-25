// SPDX-License-Identifier: AGPL-3.0-only
// Writes tests/fixtures/json_parse.json: texts and what node's JSON.parse makes of each, a value as JSON.stringify
// writes it or the error's message. Plain node, no packages: `node oracle/json_parse.oracle.mjs` from this folder.
import { writeFileSync } from "node:fs";

let seed = 20261025;
const next = n => {
  seed = (Math.imul(seed, 1103515245) + 12345) >>> 0;
  return (seed >>> 8) % n;
};

const seeds = [
  '{"a":1,"b":[true,false,null],"c":{"d":"e"}}',
  '[1,2.5,-3,1e5,0.5e-3,"x\\n\\u00e9\\ud83d\\ude00"]',
  '{"mcpServers":{"a":{"command":"x","args":["--y"],"env":{"K":"v"}}},"n":-0}',
  '"a\\"b\\\\c\\/d\\b\\f\\r\\t"',
  '  {\n  "k" : [ 1 , 2 ] ,\r\n "z" : "€\u{1F600}" }  ',
  '{"1":1,"b":2,"0":3,"4294967294":4,"4294967295":5,"01":6,"-1":7}',
  '{"a":"\\ud83d","b":"\\ude00","c":"\\ud83d\\ud83d\\ude00\\ude00x"}',
  '{"big":1e999,"neg":-1e999,"huge":123456789012345678901234567890,"tiny":1e-999}',
];
const edits = ["{", "}", "[", "]", '"', ",", ":", "\\", " ", "-", "+", ".", "e", "E", "0", "1", "9", "t", "f", "n", "u", "\n", "\r", "\t", "x", "é", "€", "\u{1F600}", " ", "﻿", "\u0001", "N", "I"];
const curated = [
  "", " ", "not json", "{", "[", "[1,]", '{"a":1,}', '{"a"}', "{a:1}", '{"a":1 "b":2}', "[1 2]", "1 2", "{}x", '"abc', '"a\nb"', '"\\x"', '"\\u12G4"', "-", "-x", "1.", "1.x", "1e", "1e+", "01", "tru", "nul", "undefined", "NaN", "Infinity", "[object Object]", '{"a":', "[1,", '{"a":1', "'a'", "{'a':1}", "﻿1", '["a" x]', "x".repeat(70), '{"a":1}}', "[1]]", '"\\', '"\\u', '"\\u12', '{"a":01}', "[-]", "[1.e1]", "[.5]", "[+1]", '{"a" 1}', "{,}", "[,]", "tr1e", 'tr"e', "[tru]", "nulx", "-0", "-01", "0e", "00", "1\r\n\r\n2", "[\r\n1\r\n 2]", '{"a":\u{1F600}}', '"\\€"', '"\\é"',
  "[".repeat(900) + "]".repeat(900),
];
const texts = [...curated, ...seeds];
for (let i = 0; i < 4000; i++) {
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
const cases = [];
for (const text of [...new Set(texts)]) {
  try {
    const value = JSON.parse(text);
    cases.push({ text, ok: JSON.stringify(value) });
  } catch (e) {
    if (e instanceof RangeError) continue;
    cases.push({ text, error: e.message.toWellFormed() });
  }
}
writeFileSync(new URL("../tests/fixtures/json_parse.json", import.meta.url), JSON.stringify(cases));
console.log(`${cases.length} cases, ${cases.filter(c => c.error).length} errors`);
