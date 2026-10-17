// SPDX-License-Identifier: AGPL-3.0-only
// Writes record/protocol.json, the JSON Schema of the protocol's own zod schemas for what this client says and reads:
// the params of every op the web app sends the host, named <Op>Request, and every schema of the protocol the crate's
// source names as protocol::<Name>, which is the replies, the views and the events it reads. `ops` maps each op to its
// request's name. An op is one the web app sends when its name stands as a string anywhere in apps/web/src outside its
// tests. The crate generates its Rust types from that file at compile time, so a field the host adds or renames, or an
// op the web app starts sending, reaches Rust only through here. It reads the protocol package's build.
//
//   node record.mjs           write the record again
//   node record.mjs --check   exit 1 when the record is not what the protocol and the web app say now
import { readdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const RECORD = fileURLToPath(new URL("./record/protocol.json", import.meta.url));
const PROTOCOL = new URL("../../../packages/protocol/dist/index.js", import.meta.url);
const HOST_PACKAGE = fileURLToPath(new URL("../../../packages/host/package.json", import.meta.url));
const WEB = fileURLToPath(new URL("../../../apps/web/src", import.meta.url));
const SRC = fileURLToPath(new URL("./src", import.meta.url));

const protocol = await import(PROTOCOL.href);
// zod-to-json-schema is the MCP SDK's own dependency, which is how the host already turns zod into JSON Schema.
const sdk = createRequire(HOST_PACKAGE).resolve("@modelcontextprotocol/sdk/package.json");
const { zodToJsonSchema } = createRequire(sdk)("zod-to-json-schema");

/** typify reads a one-value enum as a tag, not a const, an object open to more keys as no tagged variant, and a union
 * of string enums as a struct of options, so that union is one enum here. A string's pattern, format and length are
 * dropped: the host checks them, and typify would need the regress, chrono or uuid crates, or a newtype per string, to
 * check them again. */
function forTypify(node) {
  if (Array.isArray(node)) return node.map(forTypify);
  if (node === null || typeof node !== "object") return node;
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === "properties") out[key] = Object.fromEntries(Object.entries(value).map(([field, s]) => [field, forTypify(s)]));
    else if (key === "const") out.enum = [value];
    else if (key === "additionalProperties" && value === true) continue;
    else if (["pattern", "format", "minLength", "maxLength"].includes(key) && node.type !== "object" && node.type !== "array") continue;
    else out[key] = forTypify(value);
  }
  const words = out.anyOf?.map(member => (member.type === "string" && Array.isArray(member.enum) && Object.keys(member).length === 2 ? member.enum : undefined));
  if (words !== undefined && words.every(Array.isArray)) {
    delete out.anyOf;
    return { type: "string", enum: words.flat(), ...out };
  }
  return out;
}

/** A schema the record holds by name is referenced by name wherever another one holds it, so it is one Rust type. A
 * union's tagged variants stay inline, since typify reads a variant's tag only there. */
function byName(node, named, self) {
  if (node === null || typeof node !== "object") return node;
  const held = named.find(([name, text]) => name !== self && text === JSON.stringify(node));
  if (held !== undefined) return { $ref: `#/definitions/${held[0]}` };
  if (Array.isArray(node)) return node.map(n => byName(n, named, self));
  const inside = n => Object.fromEntries(Object.entries(n).map(([key, value]) => [key, byName(value, named, self)]));
  const variant = n => (n.properties?.type?.enum?.length === 1 ? inside(n) : byName(n, named, self));
  return Object.fromEntries(Object.entries(node).map(([key, value]) => [key, key === "anyOf" ? value.map(variant) : byName(value, named, self)]));
}

/** Puts a schema in `into` under `name`, refusing a name already holding another schema rather than replacing it. */
function hold(into, name, schema) {
  if (into[name] !== undefined && JSON.stringify(into[name]) !== JSON.stringify(schema)) throw new Error(`record.mjs would name two schemas ${name}`);
  into[name] = schema;
  return { $ref: `#/definitions/${name}` };
}

/** The text of every file under `dir` whose name `wanted` takes. */
function sources(dir, wanted) {
  return readdirSync(dir, { withFileTypes: true, recursive: true })
    .filter(entry => entry.isFile() && wanted(entry.name))
    .map(entry => readFileSync(join(entry.parentPath, entry.name), "utf8"));
}

/** Every op of the protocol whose name the web app's source holds as a string. */
function sentByTheWebApp() {
  const ops = new Set(protocol.RUNTIME_OPS);
  const sent = new Set();
  for (const text of sources(WEB, name => /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name))) {
    for (const [, word] of text.matchAll(/["'`]([a-zA-Z]+\.[a-zA-Z.]+)["'`]/g)) if (ops.has(word)) sent.add(word);
  }
  return [...sent].sort();
}

/** Every zod schema the protocol exports that the crate's code names, as protocol::Name or in a use of protocol. */
function namedByTheCrate() {
  const names = new Set();
  for (const text of sources(SRC, name => name.endsWith(".rs")).map(text => text.replace(/\/\/.*$/gm, ""))) {
    for (const [, name] of text.matchAll(/protocol::([A-Z]\w*)/g)) names.add(name);
    for (const [, list] of text.matchAll(/protocol::\{([^}]*)\}/g)) for (const name of list.split(/[\s,]+/)) names.add(name);
  }
  return [...names].filter(name => typeof protocol[name]?.safeParse === "function").sort();
}

const pascal = word => word.split(/[.\-_]/).map(part => part[0].toUpperCase() + part.slice(1)).join("");
const requestName = op => `${pascal(op)}Request`;
const json = (name, zod, io) => forTypify(zodToJsonSchema(zod, { name, removeAdditionalStrategy: "strict", $refStrategy: "none", pipeStrategy: io, effectStrategy: io }).definitions[name]);

const inline = {};
for (const name of namedByTheCrate()) inline[name] = json(name, protocol[name], "output");
const named = Object.entries(inline).map(([name, schema]) => [name, JSON.stringify(schema)]);
const ops = {};
const byOp = new Map(protocol.RuntimeRequest._def.left.options.map(option => [option.shape.op.value, option]));
/** Whether typify makes a type of its own for a field of this schema, rather than a string, number or list of them. */
const ownType = s => s.$ref === undefined && ((Array.isArray(s.enum) && s.enum.length > 1) || s.type === "object" || s.anyOf !== undefined || (s.type === "array" && ownType(s.items ?? {})));

/** typify reads a request's optional field two ways it cannot send: an empty list or map is dropped, where the host
 * reads `[]` apart from no list at all, and a null is dropped, where the host reads null as "clear it" and no field as
 * "keep it". An optional list or map may be null here, which typify sends as given, and an optional field that may be
 * null is held by name as <Request><Field>OrNull, which typify makes a newtype: Some(X(None)) is sent as null. */
function sentAsGiven(schema, name) {
  if (schema === null || typeof schema !== "object") return schema;
  if (Array.isArray(schema)) return schema.map((s, at) => sentAsGiven(s, name + at));
  const out = Object.fromEntries(Object.entries(schema).map(([key, value]) => [key, key === "properties" ? value : sentAsGiven(value, name)]));
  if (out.properties === undefined) return out;
  const required = new Set(out.required ?? []);
  out.properties = Object.fromEntries(
    Object.entries(out.properties).map(([field, s]) => {
      const inner = sentAsGiven(s, name + pascal(field));
      if (required.has(field)) return [field, inner];
      const types = [inner.type].flat();
      const nullable = types.includes("null") || inner.anyOf?.some(member => member.type === "null");
      if (nullable) {
        // What it holds besides null takes its own name too, or typify names it after the newtype and makes two types of one name.
        const held = `${name}${pascal(field)}`;
        const members = inner.anyOf?.map(member => (member.type !== "null" && ownType(member) ? hold(requests, held, member) : member));
        return [field, hold(requests, `${held}OrNull`, members === undefined ? inner : { ...inner, anyOf: members })];
      }
      if (inner.type === "array" || (inner.type === "object" && inner.additionalProperties !== undefined)) return [field, { anyOf: [inner, { type: "null" }] }];
      return [field, inner];
    }),
  );
  return out;
}

const requests = {};
for (const op of sentByTheWebApp()) {
  ops[op] = requestName(op);
  requests[ops[op]] = sentAsGiven(json(ops[op], byOp.get(op).omit({ id: true, op: true }), "input"), ops[op]);
}
Object.assign(inline, requests);
const definitions = Object.fromEntries(Object.keys(inline).map(name => [name, byName(inline[name], named, name)]));

/** typify names a variant's own field types after the union, so two variants holding fields of one name in two shapes
 * would read as one Rust type: each such field is held by name instead, its variant's tag then the field. */
for (const schema of Object.values({ ...definitions })) {
  if (!Array.isArray(schema.anyOf)) continue;
  const shapes = new Map();
  for (const variant of schema.anyOf) {
    for (const [field, s] of Object.entries(variant.properties ?? {})) if (ownType(s)) shapes.set(field, new Set([...(shapes.get(field) ?? []), JSON.stringify(s)]));
  }
  for (const variant of schema.anyOf) {
    const tag = variant.properties?.type?.enum?.[0];
    if (typeof tag !== "string") continue;
    for (const [field, s] of Object.entries(variant.properties)) {
      if (!ownType(s) || shapes.get(field).size < 2) continue;
      variant.properties[field] = hold(definitions, pascal(tag) + pascal(field), s);
    }
  }
}
const text = `${JSON.stringify({ $schema: "http://json-schema.org/draft-07/schema#", ops, definitions }, null, 1)}\n`;

if (process.argv.includes("--check")) {
  let held = "";
  try {
    held = readFileSync(RECORD, "utf8");
  } catch {}
  if (held !== text) {
    console.error(`${RECORD} is behind packages/protocol or apps/web; run node apps/desktop-gpui/host-client/record.mjs and commit it`);
    process.exit(1);
  }
} else {
  // Whole or not at all, since a build may be reading it.
  writeFileSync(`${RECORD}.tmp`, text);
  renameSync(`${RECORD}.tmp`, RECORD);
}
