// fixtures.mjs <outdir>: for every exported zod schema of views/ and the top-level files, sample inputs and what zod's
// parse answers for each (the output JSON, or the first issue), as the Rust shape tests read them.
import M from "./bundle.mjs";
import fs from "node:fs";
import path from "node:path";

const OUT = process.argv[2];
const ROOT = `${process.env.WSP_REPO}/packages/protocol/src`;
const SKIP_TOP = new Set(["adapter-port", "bundles", "semver"]);
const mine = mod => mod.startsWith("views/") || (!mod.includes("/") && !SKIP_TOP.has(mod));
const definedIn = (mod, name) => new RegExp(`^export const ${name}\\b`, "m").test(fs.readFileSync(path.join(ROOT, `${mod}.ts`), "utf8"));

const STRINGS = ["text é", "a", "abc", "/abs/path", "x", "2026-10-25T10:00:00.000Z", "0123456789ab", "deadbeef", "owner/repo", "https://example.com", "a-b", "v1.2.3", "1.2.3", "#ff8800", "user@host", "a.b", "ab_cd", "A", "aaaaaaaa", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "main", "claude", "sk-ant-abc", "12345678"];
const NUMBERS_F = [2.5, 0.5, 7, 3, 1, 0, 100, 1e21, 65535, 1700000000000];
const NUMBERS_I = [7, 3, 1, 0, 2, 100, 4400, 65535, 1700000000000, -1];

const ok = (s, v) => s.safeParse(v).success;

function sample(s, mode, depth = 0) {
  const d = s._def;
  switch (d.typeName) {
    case "ZodString": {
      const min = (d.checks ?? []).find(c => c.kind === "min" || c.kind === "length");
      const base = mode === "full" ? STRINGS : ["s", ...STRINGS];
      for (const c of base) if (ok(s, c)) return c;
      if (min) return "a".repeat(min.value);
      return "s";
    }
    case "ZodNumber": {
      const int = (d.checks ?? []).some(c => c.kind === "int");
      const bounds = (d.checks ?? []).filter(c => c.kind === "min" || c.kind === "max").flatMap(c => [c.value, c.value + 1, c.value - 1]);
      const list = [...(int ? NUMBERS_I : mode === "full" ? NUMBERS_F : [3, ...NUMBERS_F]), ...bounds];
      for (const n of list) if (ok(s, n)) return n;
      return 1;
    }
    case "ZodBoolean": return mode === "full";
    case "ZodLiteral": return d.value;
    case "ZodEnum": return mode === "full" ? d.values[d.values.length - 1] : d.values[0];
    case "ZodNativeEnum": return Object.values(d.values)[0];
    case "ZodUnknown": case "ZodAny": return mode === "full" ? { a: 1, b: [true, null] } : "x";
    case "ZodNull": return null;
    case "ZodArray": {
      const n = d.exactLength?.value ?? d.minLength?.value ?? (mode === "full" && depth < 4 ? 1 : 0);
      return Array.from({ length: n }, () => sample(d.type, mode, depth + 1));
    }
    case "ZodRecord": {
      if (mode !== "full" || depth >= 4) return {};
      const k = d.keyType._def.typeName === "ZodEnum" ? d.keyType._def.values[0] : "k";
      return { [k]: sample(d.valueType, mode, depth + 1) };
    }
    case "ZodObject": {
      const o = {};
      for (const [k, raw] of Object.entries(d.shape())) {
        let t = raw, opt = false, nul = false, def = false;
        for (;;) {
          const tn = t._def.typeName;
          if (tn === "ZodOptional") opt = true;
          else if (tn === "ZodNullable") nul = true;
          else if (tn === "ZodDefault") def = true;
          else break;
          t = t._def.innerType;
        }
        if (mode === "minimal" && (opt || def)) continue;
        if (mode === "nulls" && nul) { o[k] = null; continue; }
        if (mode === "nulls" && (opt || def) && depth > 0) continue;
        if (depth >= 4 && (opt || def)) continue;
        o[k] = sample(t, mode, depth + 1);
      }
      if (d.unknownKeys === "passthrough" && mode === "full") o.extraKey = "kept";
      return o;
    }
    case "ZodOptional": case "ZodNullable": case "ZodDefault": case "ZodCatch": case "ZodReadonly": case "ZodBranded":
      return sample(d.innerType ?? d.type, mode, depth);
    case "ZodLazy": return sample(d.getter(), mode, depth + 1);
    case "ZodEffects": {
      const first = sample(d.schema, mode, depth);
      if (ok(s, first)) return first;
      for (const c of [...STRINGS, ...NUMBERS_I, 13, 14, 15, 16, 17, 12, 11]) if (ok(s, c)) return c;
      return first;
    }
    case "ZodUnion": case "ZodDiscriminatedUnion": {
      const opts = d.options instanceof Map ? [...d.options.values()] : d.options;
      const o = mode === "full" ? opts[opts.length - 1] : opts[0];
      return sample(o, mode, depth + 1);
    }
    case "ZodTuple": return d.items.map(it => sample(it, mode, depth + 1));
    default: return null;
  }
}

/** One mutation per rule a direct field carries, each with zod's first issue. */
let violationsName;
function violations(s, full) {
  const d = s._def;
  const out = [];
  if (d.typeName === "ZodEnum") return [{ case: "not one of the values", input: "not-a-value" }];
  if (d.typeName !== "ZodObject" || typeof full !== "object" || full === null) return out;
  if (d.unknownKeys === "strict") out.push({ case: "an unknown key", input: { ...full, unexpectedKey: 1 } });
  const tag = tagOf(s, violationsName);
  for (const [k, v] of Object.entries(d.shape())) {
    if (k === tag || v.isOptional() || v._def.typeName !== "ZodNullable") continue;
    const { [k]: _, ...rest } = full;
    out.push({ case: `missing ${k}, which is null or a value but never absent`, input: rest });
  }
  const firstRequired = Object.entries(d.shape()).find(([k, v]) => !v.isOptional() && k !== tag);
  if (firstRequired) {
    const { [firstRequired[0]]: _, ...rest } = full;
    out.push({ case: `missing ${firstRequired[0]}`, input: rest });
  }
  for (const [k, raw] of Object.entries(d.shape())) {
    let t = raw;
    while (["ZodOptional", "ZodNullable", "ZodDefault"].includes(t._def.typeName)) t = t._def.innerType;
    const td = t._def;
    const put = (what, v) => out.push({ case: `${k}: ${what}`, input: { ...full, [k]: v } });
    if (td.typeName === "ZodString") for (const c of td.checks ?? []) {
      if (c.kind === "max") put(`longer than ${c.value}`, "a".repeat(c.value + 1));
      if (c.kind === "min" && c.value > 0) put(`shorter than ${c.value}`, "a".repeat(c.value - 1));
      if (c.kind === "length") put(`not ${c.value} long`, "a".repeat(c.value + 1));
      if (c.kind === "regex") put(`not matching /${c.regex.source}/`, "\u0000 no match \u0000");
    }
    if (td.typeName === "ZodNumber") for (const c of td.checks ?? []) {
      if (c.kind === "int") put("not an integer", 2.5);
      if (c.kind === "min") put(`under ${c.value}`, c.inclusive ? c.value - 1 : c.value);
      if (c.kind === "max") put(`over ${c.value}`, c.inclusive ? c.value + 1 : c.value);
    }
    if (td.typeName === "ZodArray") {
      const item = Array.isArray(full[k]) && full[k].length ? full[k][0] : sample(td.type, "full", 1);
      if (td.maxLength) put(`more than ${td.maxLength.value} items`, Array.from({ length: td.maxLength.value + 1 }, () => item));
      if (td.minLength && td.minLength.value > 0) put(`fewer than ${td.minLength.value} items`, []);
      if (td.exactLength) put(`not ${td.exactLength.value} items`, Array.from({ length: td.exactLength.value + 1 }, () => item));
      if (td.type._def.typeName === "ZodString" && (td.type._def.checks ?? []).some(c => c.kind === "max")) {
        const max = td.type._def.checks.find(c => c.kind === "max").value;
        put(`an item longer than ${max}`, ["a".repeat(max + 1)]);
      }
    }
    if (td.typeName === "ZodEffects" && td.effect.type === "refinement") put("failing its refinement", "relative/path");
    if (td.typeName === "ZodEnum") put("not one of its values", "not-a-value");
    if (td.typeName === "ZodBoolean") put("not a boolean", "yes");
  }
  return out;
}

function literalTagged(d) {
  return d.options.every(o => o._def.shape()[d.discriminator]?._def?.typeName === "ZodLiteral");
}
const members = new Map();
{
  const seen = new Set();
  const walk = s => {
    if (!s?._def || seen.has(s)) return;
    seen.add(s);
    const d = s._def;
    if (d.typeName === "ZodDiscriminatedUnion" && literalTagged(d)) for (const o of d.options) members.set(o, d.discriminator);
    if (d.typeName === "ZodObject") for (const v of Object.values(d.shape())) walk(v);
    for (const k of ["innerType", "schema", "type", "valueType"]) if (d[k]) walk(d[k]);
    if (d.typeName === "ZodLazy") walk(d.getter());
    for (const o of d.options instanceof Map ? [...d.options.values()] : d.options ?? []) walk(o);
  };
  for (const ns of Object.values(M)) for (const v of Object.values(ns)) walk(v);
}
const tagOf = (s, name) => {
  if (s._def.typeName !== "ZodObject") return undefined;
  if (members.has(s)) return members.get(s);
  const first = Object.entries(s._def.shape())[0];
  if (name?.endsWith("Event") && first?.[0] === "type" && first[1]._def.typeName === "ZodLiteral") return "type";
  return undefined;
};

// Types whose Rust struct follows the order the TS code writes (gen.mjs ORDER), not zod's: compared as values.
const ANY_ORDER = new Set(["views/preferences:Preferences", "views/preferences:PreferencesChangedEvent", "views/preferences:PreferencesStored"]);

const SERDE_LEVEL = new Set(["invalid_type", "unrecognized_keys", "invalid_enum_value", "invalid_literal", "invalid_union_discriminator", "invalid_union"]);

fs.mkdirSync(OUT, { recursive: true });
const counts = [];
const skipped = [];
for (const [mod, ns] of Object.entries(M)) {
  if (!mine(mod)) continue;
  const entries = [];
  const extra = mod === "views/preferences" ? [["PreferencesStored", ns.Preferences.partial()]] : [];
  for (const [name, s] of [...Object.entries(ns), ...extra]) {
    if (!s?._def?.typeName || !(definedIn(mod, name) || extra.some(([n]) => n === name))) continue;
    const modes = ["full", "minimal", "nulls"];
    const inputs = [];
    for (const m of modes) {
      let v;
      try { v = sample(s, m); } catch (e) { skipped.push(`${mod}:${name} ${m}: ${e.message}`); continue; }
      if (inputs.some(x => JSON.stringify(x.input) === JSON.stringify(v))) continue;
      inputs.push({ case: m, input: v });
    }
    const t = s._def.typeName;
    if (t === "ZodDiscriminatedUnion" || t === "ZodUnion") {
      const opts = s._def.options instanceof Map ? [...s._def.options.values()] : s._def.options;
      opts.forEach((o, i) => { const v = sample(o, "full", 1); if (!inputs.some(x => JSON.stringify(x.input) === JSON.stringify(v))) inputs.push({ case: `variant ${i}`, input: v }); });
    }
    if (t === "ZodEnum") for (const v of s._def.values) if (!inputs.some(x => x.input === v)) inputs.push({ case: `value ${v}`, input: v });
    for (const x of inputs) {
      const r = s.safeParse(x.input);
      if (r.success) entries.push({ type: name, case: x.case, input: x.input, output: JSON.stringify(r.data), ...(ANY_ORDER.has(`${mod}:${name}`) ? { anyOrder: true } : {}) });
      else skipped.push(`${mod}:${name} ${x.case}: sample refused (${r.error.issues[0].code} at ${JSON.stringify(r.error.issues[0].path)}: ${r.error.issues[0].message})`);
    }
    const full = inputs.find(x => x.case === "full")?.input;
    violationsName = name;
    const tag = tagOf(s, name);
    if (tag) for (const e of entries) if (e.type === name && e.output) { const o = JSON.parse(e.output); e.tag = { key: tag, value: o[tag] }; delete o[tag]; e.output = JSON.stringify(o); }
    if (full !== undefined && s.safeParse(full).success) for (const v of violations(s, full)) {
      const r = s.safeParse(v.input);
      if (r.success) { skipped.push(`${mod}:${name} violation "${v.case}" parsed`); continue; }
      const issue = r.error.issues[0];
      entries.push({ type: name, case: v.case, input: v.input, issue: { code: issue.code, path: issue.path, serdeLevel: SERDE_LEVEL.has(issue.code) } });
    }
  }
  if (entries.length === 0) continue;
  const file = mod.startsWith("views/") ? `views_${mod.slice(6).replace(/-/g, "_")}.json` : `types_${mod.replace(/-/g, "_")}.json`;
  fs.writeFileSync(path.join(OUT, file), JSON.stringify(entries, null, 1) + "\n");
  counts.push(`${file}: ${new Set(entries.map(e => e.type)).size} types, ${entries.length} cases`);
}
fs.writeFileSync(path.join(OUT, "SKIPPED.txt"), skipped.join("\n") + "\n");
console.log(counts.join("\n"));
console.log("skipped", skipped.length);
