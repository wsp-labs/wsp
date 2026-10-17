// SPDX-License-Identifier: AGPL-3.0-only
// Writes record/protocol.json, the JSON Schema of the protocol's own zod schemas for what this client reads: a
// project, a session row, a transcript event and a page of a thread's transcript. The crate generates its Rust types
// from that file at compile time, so a field the host adds or renames reaches Rust only through here. It reads the
// protocol package's build.
//
//   node record.mjs           write the record again
//   node record.mjs --check   exit 1 when the record is not what the protocol says now
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const RECORD = fileURLToPath(new URL("./record/protocol.json", import.meta.url));
const PROTOCOL = new URL("../../../packages/protocol/dist/index.js", import.meta.url);
const HOST_PACKAGE = fileURLToPath(new URL("../../../packages/host/package.json", import.meta.url));
const RECORDED = ["ProjectView", "SessionView", "SessionEvent", "HistoryPage"];

const protocol = await import(PROTOCOL.href);
// zod-to-json-schema is the MCP SDK's own dependency, which is how the host already turns zod into JSON Schema.
const sdk = createRequire(HOST_PACKAGE).resolve("@modelcontextprotocol/sdk/package.json");
const { zodToJsonSchema } = createRequire(sdk)("zod-to-json-schema");

/** typify reads a one-value enum as a tag, not a const, and an object open to more keys as no tagged variant. */
function forTypify(node) {
  if (Array.isArray(node)) return node.map(forTypify);
  if (node === null || typeof node !== "object") return node;
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === "const") out.enum = [value];
    else if (key === "additionalProperties" && value === true) continue;
    else out[key] = forTypify(value);
  }
  return out;
}

/** A schema the record holds by name is referenced by name wherever another one holds it, so it is one Rust type. */
function byName(node, named, self) {
  if (node === null || typeof node !== "object") return node;
  const held = named.find(([name, text]) => name !== self && text === JSON.stringify(node));
  if (held !== undefined) return { $ref: `#/definitions/${held[0]}` };
  if (Array.isArray(node)) return node.map(n => byName(n, named, self));
  return Object.fromEntries(Object.entries(node).map(([key, value]) => [key, byName(value, named, self)]));
}

const inline = {};
for (const name of RECORDED) {
  const schema = zodToJsonSchema(protocol[name], { name, removeAdditionalStrategy: "strict", $refStrategy: "none" });
  inline[name] = forTypify(schema.definitions[name]);
}
const named = Object.entries(inline).map(([name, schema]) => [name, JSON.stringify(schema)]);
const definitions = Object.fromEntries(RECORDED.map(name => [name, byName(inline[name], named, name)]));
const text = `${JSON.stringify({ $schema: "http://json-schema.org/draft-07/schema#", definitions }, null, 1)}\n`;

if (process.argv.includes("--check")) {
  let held = "";
  try {
    held = readFileSync(RECORD, "utf8");
  } catch {}
  if (held !== text) {
    console.error(`${RECORD} is behind packages/protocol; run node apps/desktop-gpui/host-client/record.mjs and commit it`);
    process.exit(1);
  }
} else {
  writeFileSync(RECORD, text);
}
