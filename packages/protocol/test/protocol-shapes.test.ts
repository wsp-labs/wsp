// SPDX-License-Identifier: AGPL-3.0-only
// The bump rule, kept: a reply shape that loses, renames or retypes a field while the protocol number stands is the
// change an older client misreads, so it fails here until the number moves and the record is written again. A field,
// a variant or a shape added does not fail: an older client never read it.
import { readFileSync, writeFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { z, type ZodTypeAny } from "zod";
import * as protocol from "../src/index.js";
import { ProtocolRange, RUNTIME_OPS, WIRE_PROTOCOL, protocolMismatchHalves } from "../src/index.js";
import { WIRE_SHAPE_NAMES } from "../src/wire/protocol-shapes.js";

const RECORD = new URL("../src/wire/protocol-shapes.json", import.meta.url);

type Fields = Record<string, string>;

/** The fields of a shape by path, each with the type a reader holds it to: `.name` for a key, `[]` an array's element,
 * `{}` a record's value, `|n` a union's nth option (or a discriminated union's own value), then the base type with
 * `|null` where null reads and `?` where the field may be absent. */
function fieldsOf(schema: ZodTypeAny, path = "", stack: ZodTypeAny[] = []): Fields {
  let node = schema;
  let optional = false;
  let nullable = false;
  for (;;) {
    const def = node._def as { typeName: string; innerType?: ZodTypeAny; schema?: ZodTypeAny; type?: ZodTypeAny; in?: ZodTypeAny; getter?: () => ZodTypeAny };
    if (def.typeName === "ZodLazy") {
      if (stack.includes(node)) return { [path]: "recursive" };
      stack = [...stack, node];
      node = def.getter!();
      continue;
    }
    if (def.typeName === "ZodOptional" || def.typeName === "ZodDefault") optional = true;
    else if (def.typeName === "ZodNullable") nullable = true;
    else if (!["ZodCatch", "ZodReadonly", "ZodEffects", "ZodBranded", "ZodPipeline"].includes(def.typeName)) break;
    node = def.innerType ?? def.schema ?? def.type ?? def.in!;
  }
  const def = node._def as Record<string, unknown> & { typeName: string };
  const mark = (base: string): string => `${base}${nullable ? "|null" : ""}${optional ? "?" : ""}`;
  const out: Fields = {};
  const into = (sub: string, child: ZodTypeAny): void => void Object.assign(out, fieldsOf(child, `${path}${sub}`, stack));
  switch (def["typeName"]) {
    case "ZodObject":
      out[path] = mark("object");
      for (const [key, child] of Object.entries((node as z.AnyZodObject).shape)) into(`.${key}`, child as ZodTypeAny);
      break;
    case "ZodArray":
    case "ZodSet":
      out[path] = mark("array");
      into("[]", def["type"] as ZodTypeAny);
      break;
    case "ZodRecord":
      out[path] = mark("record");
      into("{}", def["valueType"] as ZodTypeAny);
      break;
    case "ZodTuple":
      out[path] = mark("tuple");
      (def["items"] as ZodTypeAny[]).forEach((child, i) => into(`[${i}]`, child));
      break;
    case "ZodUnion":
    case "ZodDiscriminatedUnion": {
      out[path] = mark("union");
      const discriminator = def["discriminator"] as string | undefined;
      [...(def["options"] as Iterable<ZodTypeAny>)].forEach((child, i) => {
        const shape = (child as z.AnyZodObject).shape as Record<string, ZodTypeAny> | undefined;
        const tag = [discriminator, "type", "op"].map(key => (key === undefined ? undefined : shape?.[key]?._def?.value)).find(value => value !== undefined);
        into(`|${tag === undefined ? i : String(tag)}`, child);
      });
      break;
    }
    case "ZodIntersection":
      out[path] = mark("object");
      into("", def["left"] as ZodTypeAny);
      into("", def["right"] as ZodTypeAny);
      break;
    case "ZodEnum":
      out[path] = mark(`enum(${[...(def["values"] as string[])].sort().join("|")})`);
      break;
    case "ZodNativeEnum":
      out[path] = mark(`enum(${Object.values(def["values"] as object).map(String).sort().join("|")})`);
      break;
    case "ZodLiteral":
      out[path] = mark(`literal(${JSON.stringify(def["value"])})`);
      break;
    default:
      out[path] = mark(String(def["typeName"]).replace(/^Zod/, "").toLowerCase());
  }
  return out;
}

const read = (type: string): { base: string; nullable: boolean; optional: boolean } => {
  const optional = type.endsWith("?");
  const bare = optional ? type.slice(0, -1) : type;
  const nullable = bare.endsWith("|null");
  return { base: nullable ? bare.slice(0, -"|null".length) : bare, nullable, optional };
};

const enumValues = (base: string): string[] | undefined => (base.startsWith("enum(") ? base.slice(5, -1).split("|") : undefined);

/** What an older client reading `recorded` would no longer find in `current`: a field gone, or held to another type,
 * an enum short of a value it read, a field that may now be absent or null where it never was. */
function breaks(recorded: Record<string, Fields>, current: Record<string, Fields>): string[] {
  const out: string[] = [];
  for (const [name, fields] of Object.entries(recorded)) {
    const now = current[name];
    if (now === undefined) {
      out.push(`${name}: the shape is gone`);
      continue;
    }
    for (const [path, type] of Object.entries(fields)) {
      const have = now[path];
      if (have === undefined) {
        out.push(`${name}${path}: lost or renamed`);
        continue;
      }
      const [was, is] = [read(type), read(have)];
      const [wasValues, isValues] = [enumValues(was.base), enumValues(is.base)];
      const sameBase = wasValues !== undefined && isValues !== undefined ? wasValues.every(v => isValues.includes(v)) : was.base === is.base;
      if (!sameBase) out.push(`${name}${path}: ${was.base} became ${is.base}`);
      else if (is.optional && !was.optional) out.push(`${name}${path}: now may be absent`);
      else if (is.nullable && !was.nullable) out.push(`${name}${path}: now may be null`);
    }
  }
  return out;
}

const exported = protocol as unknown as Record<string, ZodTypeAny | undefined>;
const WIRE_REPLY_SHAPES = Object.fromEntries(WIRE_SHAPE_NAMES.map(name => [name, exported[name]!]));

const current = (): Record<string, Fields> => Object.fromEntries(Object.entries(WIRE_REPLY_SHAPES).map(([name, shape]) => [name, fieldsOf(shape)]));

/** The ops an older client sent that this host no longer answers by that name: removed, or renamed. */
const lostOps = (recorded: readonly string[], now: readonly string[]): string[] => recorded.filter(op => !now.includes(op));

const WRITE = "WSP_WRITE_SHAPES=1 pnpm exec vitest run --minWorkers=1 --maxWorkers=2 packages/protocol/test/protocol-shapes.test.ts";

describe("the reply shapes held to the protocol number", () => {
  if (process.env["WSP_WRITE_SHAPES"] === "1") writeFileSync(RECORD, `${JSON.stringify({ protocol: WIRE_PROTOCOL, ops: [...RUNTIME_OPS].sort(), shapes: current() }, null, 2)}\n`);
  const recorded = JSON.parse(readFileSync(RECORD, "utf8")) as { protocol: number; ops: string[]; shapes: Record<string, Fields> };

  it("are recorded at the number this host speaks", () => {
    expect(recorded.protocol, `the number moved: write the shapes again with ${WRITE}`).toBe(WIRE_PROTOCOL);
  });

  it("lose, rename and retype no field while the number stands", () => {
    expect(breaks(recorded.shapes, current()), `a client reading protocol ${recorded.protocol} would misread these: undo the change, or move the number in WIRE_PROTOCOL and write the shapes again with ${WRITE}`).toEqual([]);
  });

  it("are all exported zod schemas", () => {
    expect(WIRE_SHAPE_NAMES.filter(name => typeof exported[name]?.safeParse !== "function")).toEqual([]);
  });

  it("lose and rename no op while the number stands", () => {
    expect(lostOps(recorded.ops, RUNTIME_OPS), `a client reading protocol ${recorded.protocol} sends these and the host no longer answers them: undo the change, or move the number in WIRE_PROTOCOL and write the record again with ${WRITE}`).toEqual([]);
  });

  it("name every shape the record holds, so a shape taken off the list is not a quiet removal", () => {
    expect(Object.keys(WIRE_REPLY_SHAPES).sort()).toEqual(Object.keys(recorded.shapes).sort());
  });
});

describe("what counts as a break", () => {
  const same = { id: z.string(), size: z.number(), kind: z.enum(["a", "b"]), inner: z.object({ x: z.string() }), more: z.array(z.object({ y: z.number() })).optional() };
  const held = { Row: fieldsOf(z.object(same)) };
  const after = (shape: ZodTypeAny): string[] => breaks(held, { Row: fieldsOf(shape) });

  it("passes the shape unchanged, with fields, enum values and nested fields added", () => {
    expect(after(z.object(same))).toEqual([]);
    expect(after(z.object({ ...same, extra: z.string(), kind: z.enum(["a", "b", "c"]), inner: z.object({ x: z.string(), z: z.number() }) }))).toEqual([]);
  });

  it("fails a field removed", () => {
    const { size: _, ...rest } = same;
    expect(after(z.object(rest))).toEqual(["Row.size: lost or renamed"]);
  });

  it("fails a field renamed", () => {
    const { size: _, ...rest } = same;
    expect(after(z.object({ ...rest, length: z.number() }))).toEqual(["Row.size: lost or renamed"]);
  });

  it("fails a field retyped, a nested one removed, an enum value removed, a field made optional or nullable", () => {
    expect(after(z.object({ ...same, size: z.string() }))).toEqual(["Row.size: number became string"]);
    expect(after(z.object({ ...same, inner: z.object({}) }))).toEqual(["Row.inner.x: lost or renamed"]);
    expect(after(z.object({ ...same, kind: z.enum(["a"]) }))).toEqual(["Row.kind: enum(a|b) became enum(a)"]);
    expect(after(z.object({ ...same, id: z.string().optional() }))).toEqual(["Row.id: now may be absent"]);
    expect(after(z.object({ ...same, id: z.string().nullable() }))).toEqual(["Row.id: now may be null"]);
  });

  it("fails a field inside an array's element removed", () => {
    expect(after(z.object({ ...same, more: z.array(z.object({})).optional() }))).toEqual(["Row.more[].y: lost or renamed"]);
  });
});

/** Every zod node a listed shape reaches, so a schema nested in one is held through it. */
function reached(node: unknown, seen = new Set<unknown>()): Set<unknown> {
  if (node === null || typeof node !== "object" || seen.has(node)) return seen;
  seen.add(node);
  const def = (node as { _def?: Record<string, unknown> })._def;
  if (def === undefined) return seen;
  if (def["typeName"] === "ZodObject") for (const child of Object.values((node as z.AnyZodObject).shape)) reached(child, seen);
  if (def["typeName"] === "ZodLazy") reached((def["getter"] as () => unknown)(), seen);
  for (const key of ["innerType", "schema", "type", "in", "valueType", "left", "right"]) reached(def[key], seen);
  for (const key of ["items", "options"]) for (const child of (def[key] as Iterable<unknown> | undefined) ?? []) reached(child, seen);
  return seen;
}

describe("the shapes the GPUI app reads", () => {
  it("are all held, reply, view and event, by the record its host client is generated from", () => {
    const record = JSON.parse(readFileSync(new URL("../../../apps/desktop-gpui/host-client/record/protocol.json", import.meta.url), "utf8")) as { definitions: Record<string, unknown> };
    const held = new Set<unknown>();
    for (const shape of Object.values(WIRE_REPLY_SHAPES)) reached(shape, held);
    const read = Object.keys(record.definitions).filter(name => !name.includes("Request") && typeof exported[name] === "object" && exported[name] !== null && "_def" in exported[name]!);
    expect(read.filter(name => !held.has(exported[name]))).toEqual([]);
  });
});

describe("what counts as a lost op", () => {
  it("fails an op removed or renamed, and passes one added", () => {
    expect(lostOps(["a.get", "a.set"], ["a.get", "a.put"])).toEqual(["a.set"]);
    expect(lostOps(["a.get"], ["a.get", "a.put"])).toEqual([]);
  });
});

describe("the range a client sends", () => {
  it("is refused at the schema when its lowest is above its highest", () => {
    expect(ProtocolRange.safeParse([2, 1]).success).toBe(false);
  });
});

describe("the sentence to a person with an older host", () => {
  it("sends them to a line that updates wsp, not to the verb that merges a thread's base", () => {
    expect(protocolMismatchHalves(1, [2, 2]).fix).not.toMatch(/\bwsp update\b/);
  });
});
