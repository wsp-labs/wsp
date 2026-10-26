// gen.mjs <outdir>: the zod schemas and plain constants of the protocol's views/ and top-level files as Rust.
// Shapes only: every body that does work is todo!(), every check is todo!() with the rules it must enforce beside it.
import M from "./bundle.mjs";
import fs from "node:fs";
import path from "node:path";

const ROOT = `${process.env.WSP_REPO}/packages/protocol/src`;
const COMMIT = "a3b870a6e";
const OUT = process.argv[2];
const SKIP_TOP = new Set(["adapter-port", "bundles", "semver"]);
const mine = mod => mod.startsWith("views/") || (!mod.includes("/") && !SKIP_TOP.has(mod));

const rustMod = mod => {
  if (mod.startsWith("views/")) return `crate::views::${snake(mod.slice(6))}`;
  if (mod.startsWith("wire/")) return `crate::wire::${snake(mod.slice(5))}`;
  if (mod.startsWith("slate/")) return `crate::slate_pending::${snake(mod.slice(6))}`;
  return `crate::types::${snake(mod)}`;
};
const fileOf = mod => path.join(ROOT, `${mod}.ts`);
const srcCache = new Map();
const src = mod => {
  if (!srcCache.has(mod)) srcCache.set(mod, fs.existsSync(fileOf(mod)) ? fs.readFileSync(fileOf(mod), "utf8") : "");
  return srcCache.get(mod);
};
const lineOf = (mod, name) => {
  const lines = src(mod).split("\n");
  const re = new RegExp(`^export (const|function|async function|type|interface|enum) ${name}\\b`);
  const i = lines.findIndex(l => re.test(l));
  return i === -1 ? undefined : i + 1;
};
const definedIn = (mod, name) => lineOf(mod, name) !== undefined;

function snake(s) {
  return s
    .replace(/[-./ ]+/g, "_")
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .toLowerCase();
}
const camelOfSnake = s => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
const pascal = s => {
  const p = s.split(/[^A-Za-z0-9]+/).filter(Boolean).map(w => w[0].toUpperCase() + w.slice(1)).join("");
  return p === "" ? "Empty" : /^[0-9]/.test(p) ? `V${p}` : p;
};
const KEYWORDS = new Set("as break const continue crate else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while async await dyn abstract become box do final macro override priv typeof unsized virtual yield try gen".split(" "));
const ident = s => {
  const sn = snake(s).replace(/[^a-z0-9_]/g, "_");
  return KEYWORDS.has(sn) ? `r#${sn}` : /^[0-9]/.test(sn) ? `n_${sn}` : sn;
};

// --- every exported schema, by identity, where it is defined
const named = new Map();
for (const [mod, ns] of Object.entries(M)) {
  for (const [name, v] of Object.entries(ns)) {
    if (!v?._def?.typeName) continue;
    if (!definedIn(mod, name)) continue;
    if (!named.has(v)) named.set(v, { mod, name });
  }
}
const enumByValues = new Map();
for (const [s, at] of named) if (s._def.typeName === "ZodEnum") {
  const key = JSON.stringify(s._def.values);
  if (!enumByValues.has(key)) enumByValues.set(key, at);
}

/** Whether every member names its tag with one literal, which serde's internal tag can read. */
function literalTagged(d) {
  return d.options.every(o => o._def.shape()[d.discriminator]?._def?.typeName === "ZodLiteral");
}
// Every object a zod discriminatedUnion holds, with its discriminator: such a struct leaves that key to the enum's tag.
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
  if (members.has(s)) return members.get(s);
  const first = Object.entries(s._def.shape())[0];
  if (name?.endsWith("Event") && first?.[0] === "type" && first[1]._def.typeName === "ZodLiteral") return "type";
  return undefined;
};

// Anonymous shapes already declared, by identity: a schema two fields share is one type.
const anon = new Map();

// Types the TS code writes in another order than its schema's: the struct follows the code (rule 4.26).
const ORDER = {
  // views/preferences.ts:218 applyPreferencesPatch, which preferencesFrom and every preferences.set write through.
  "views/preferences:Preferences": "theme lightTheme darkTheme sidebarMode terminalSize terminalZoom access projectLook projectIcon computerLook serverIcons agentVersions usageLogs productUsage keepAwake transparency projectOrder keybindings appFont codeFont agentDefaults projectDefaults sendWith midTurn notifyNeeds notifyDone planAlerts settleAfter askDelete onQuit newThreadIn labs sidebarWidth target editor updateDismissed cloudCardDismissed textSize codeSize defaultAgent recipeLook threadsShut".split(" "),
  // views/session.ts foldThreads, and for the facts boot.ts threadFacts (see WRITTEN in fixtures.mjs).
  "views/session:ThreadView": "id threadId workspaceId harness startedBy status title sessionId claudeSessionId startedAt endedAt cwd asking waitingOn setupRefusal sub diskKeptUntil diskGone capped limit resumeAt lastLine failure pid agentPid readAt settledAt pinnedAt order foldedAt replaces replacedBy snoozedUntil wokeAt section rewoundAt forkedFrom permissionMode fast subagents turns ran parentThreadId rootThreadId attempt costUsd".split(" "),
  "views/preferences:ThreadFacts": "id threadId workspaceId harness startedBy status title sessionId claudeSessionId startedAt endedAt cwd asking waitingOn setupRefusal capped limit resumeAt lastLine failure pid agentPid readAt settledAt pinnedAt order foldedAt replaces replacedBy snoozedUntil wokeAt section rewoundAt forkedFrom permissionMode fast turns ran parentThreadId rootThreadId attempt costUsd sub diskGone diskKeptUntil model effort contextWindow turnId".split(" "),
};

// --- per-module output
const files = new Map();
const out = mod => {
  if (!files.has(mod)) files.set(mod, { items: [], uses: new Set(), lits: new Map(), names: new Set(), notes: [], consts: [], skipped: [], frames: [] });
  return files.get(mod);
};

// The shapes wsp-frames already declares (its ts-rs output is src/generated), by their keys or values in order.
const GEN = path.join(ROOT, "generated");
const frameShape = name => {
  const file = path.join(GEN, `${name}.ts`);
  if (!fs.existsSync(file)) return undefined;
  const text = fs.readFileSync(file, "utf8");
  const m = text.match(new RegExp(`export type ${name} = ([\\s\\S]*?);\\s*$`, "m"));
  if (!m) return undefined;
  const body = m[1].trim();
  if (body.startsWith("{")) {
    const keys = [];
    let depth = 0, tok = "";
    for (const ch of body.slice(1, -1)) {
      if ("{[(<".includes(ch)) depth++;
      if ("}])>".includes(ch)) depth--;
      if (depth === 0 && ch === ":") { const k = tok.trim().replace(/\?$/, "").replace(/^"|"$/g, ""); keys.push(k); tok = ""; continue; }
      if (depth === 0 && ch === ",") { tok = ""; continue; }
      tok += ch;
    }
    return { keys };
  }
  if (/^"/.test(body)) return { values: [...body.matchAll(/"([^"]*)"/g)].map(x => x[1]) };
  return { other: body };
};
const fromFrames = (name, s) => {
  const f = frameShape(name);
  if (!f) return false;
  if (s._def.typeName === "ZodObject" && f.keys) return JSON.stringify(Object.keys(s._def.shape())) === JSON.stringify(f.keys);
  if (s._def.typeName === "ZodEnum" && f.values) return JSON.stringify(s._def.values) === JSON.stringify(f.values);
  return f.other !== undefined && !["ZodObject", "ZodEnum"].includes(s._def.typeName);
};
const pendingWire = new Map();
const ref = (from, at) => {
  if (at.mod === from) return at.name;
  if (!mine(at.mod)) {
    const s = [...named].find(([, a]) => a === at)[0];
    if (fromFrames(at.name, s)) return (out(from).uses.add(`wsp_frames::${at.name}`), at.name);
    pendingWire.set(at.name, { at, s });
    return (out(from).uses.add(`crate::types::pending_wire::${at.name}`), at.name);
  }
  out(from).uses.add(`${rustMod(at.mod)}::${at.name}`);
  return at.name;
};

function litType(mod, value) {
  const f = out(mod);
  if (value === true) return (f.uses.add("wsp_frames::True"), "True");
  if (value === false) return (f.uses.add("wsp_frames::False"), "False");
  if (typeof value === "number") {
    if (!f.lits.has(`n:${value}`)) f.lits.set(`n:${value}`, { name: `Lit${pascal(String(value).replace("-", "minus"))}`, value, num: true });
    return f.lits.get(`n:${value}`).name;
  }
  if (!f.lits.has(`s:${value}`)) {
    let name = `${pascal(value)}Lit`;
    while ([...f.lits.values()].some(l => l.name === name)) name += "X";
    f.lits.set(`s:${value}`, { name, value });
  }
  return f.lits.get(`s:${value}`).name;
}

const fresh = (mod, want) => {
  const f = out(mod);
  let name = want;
  for (let i = 2; f.names.has(name); i++) name = `${want}${i}`;
  f.names.add(name);
  return name;
};

const strRules = (d, rules) => {
  for (const c of d.checks ?? []) {
    if (c.kind === "max") rules.push(`at most ${c.value} UTF-16 units`);
    else if (c.kind === "min") rules.push(`at least ${c.value} UTF-16 units`);
    else if (c.kind === "length") rules.push(`exactly ${c.value} UTF-16 units`);
    else if (c.kind === "regex") rules.push(`matches /${c.regex.source}/${c.regex.flags}`);
    else rules.push(`zod string check ${c.kind}${c.value !== undefined ? ` ${c.value}` : ""}`);
  }
};
const numRules = (d, rules) => {
  for (const c of d.checks ?? []) {
    if (c.kind === "int") continue;
    if (c.kind === "min") rules.push(`${c.inclusive ? ">=" : ">"} ${c.value}`);
    else if (c.kind === "max") rules.push(`${c.inclusive ? "<=" : "<"} ${c.value}`);
    else rules.push(`zod number check ${c.kind} ${c.value ?? ""}`);
  }
};
const isInt = d => (d.checks ?? []).some(c => c.kind === "int");
const intType = d => "i64";

// The Rust type of a schema, declaring what it needs. rules collects what check() must enforce, keyed by field path.
function typeOf(mod, s, hint, rules, rpath) {
  const at = named.get(s);
  if (at) {
    if (!mine(at.mod) && !at.mod.startsWith("wire/") && !at.mod.startsWith("slate/")) {}
    rules.push([rpath, `check of ${at.name}`]);
    return ref(mod, at);
  }
  const d = s._def;
  const key = `${mod}`;
  if (anon.has(s) && anon.get(s).mod === mod) return anon.get(s).name;
  const t0 = d.typeName;
  if (["ZodObject", "ZodEnum", "ZodUnion", "ZodDiscriminatedUnion"].includes(t0) && hint) {
    if (M[mod]?.[hint] !== s) {
      let n = hint;
      for (let i = 2; out(mod).names.has(n) || (M[mod] && definedIn(mod, n)) || [...anon.values()].some(a => a.mod === mod && a.name === n); i++) n = `${hint}${i}`;
      hint = n;
    }
    anon.set(s, { mod, name: hint });
  }
  switch (d.typeName) {
    case "ZodString": {
      const r = [];
      strRules(d, r);
      for (const x of r) rules.push([rpath, x]);
      return "String";
    }
    case "ZodNumber": {
      const r = [];
      numRules(d, r);
      for (const x of r) rules.push([rpath, x]);
      if (isInt(d)) return intType(d);
      out(mod).uses.add("crate::types::js_number::JsNumber");
      return "JsNumber";
    }
    case "ZodBoolean": return "bool";
    case "ZodUnknown": case "ZodAny": return "serde_json::Value";
    case "ZodNull": return "()";
    case "ZodLiteral": return litType(mod, d.value);
    case "ZodEnum": {
      const hit = enumByValues.get(JSON.stringify(d.values));
      if (hit) return ref(mod, hit);
      const name = hint ?? fresh(mod, "Enum");
      declareEnum(mod, name, d.values, rpath);
      return name;
    }
    case "ZodArray": {
      const inner = typeOf(mod, d.type, `${hint ?? "Item"}Item`.replace(/ItemItem$/, "Item"), rules, `${rpath}[]`);
      if (d.minLength) rules.push([rpath, `at least ${d.minLength.value} items`]);
      if (d.maxLength) rules.push([rpath, `at most ${d.maxLength.value} items`]);
      if (d.exactLength) rules.push([rpath, `exactly ${d.exactLength.value} items`]);
      return `Vec<${inner}>`;
    }
    case "ZodRecord": {
      out(mod).uses.add("indexmap::IndexMap");
      const k = d.keyType._def.typeName === "ZodString" ? "String" : typeOf(mod, d.keyType, `${hint}Key`, rules, `${rpath}{key}`);
      const v = typeOf(mod, d.valueType, `${hint}Value`, rules, `${rpath}{}`);
      return `IndexMap<${k}, ${v}>`;
    }
    case "ZodObject": {
      const name = hint ?? fresh(mod, "Object");
      declareStruct(mod, name, s);
      rules.push([rpath, `check of ${name}`]);
      return name;
    }
    case "ZodLazy": return typeOf(mod, d.getter(), hint, rules, rpath);
    // A field's own optional and nullable are read by declareStruct; one met here is inside a record, an array or a
    // union, where null is a value the Rust side must hold.
    case "ZodNullable":
      return `Option<${typeOf(mod, d.innerType, hint, rules, rpath)}>`;
    case "ZodOptional": case "ZodDefault":
      return typeOf(mod, d.innerType, hint, rules, rpath);
    case "ZodEffects": {
      // The inner schema's rules run first, then the effect: spec/shapes.md rule 7 gives each effect's words.
      const inner = typeOf(mod, d.schema, hint, rules, rpath);
      rules.push([rpath, d.effect.type === "refinement" ? "a refine (spec/shapes.md rule 7)" : `a zod ${d.effect.type} (spec/shapes.md rule 7)`]);
      out(mod).notes.push(`${hint ?? "?"} ${rpath}: zod ${d.effect.type}`);
      return inner;
    }
    case "ZodUnion": case "ZodDiscriminatedUnion": {
      const opts = d.options instanceof Map ? [...d.options.values()] : d.options;
      if (opts.every(o => o._def.typeName === "ZodLiteral" && typeof o._def.value === "string")) {
        const values = opts.map(o => o._def.value);
        const hit = enumByValues.get(JSON.stringify(values));
        if (hit) return ref(mod, hit);
        const name = hint ?? fresh(mod, "Union");
        declareEnum(mod, name, values, rpath);
        return name;
      }
      const name = hint ?? fresh(mod, "Union");
      declareUnion(mod, name, opts, d.discriminator && literalTagged(d) ? d.discriminator : undefined, rpath);
      rules.push([rpath, `check of ${name}`]);
      return name;
    }
    case "ZodTuple": {
      const items = d.items.map((it, i) => typeOf(mod, it, `${hint}${i}`, rules, `${rpath}.${i}`));
      return `(${items.join(", ")})`;
    }
    default:
      out(mod).notes.push(`${hint} ${rpath}: unhandled ${d.typeName}`);
      return `serde_json::Value /* ${d.typeName} */`;
  }
}

const srcLine = (mod, name) => {
  const l = lineOf(mod, name);
  return l === undefined ? "" : `/// TS: packages/protocol/src/${mod}.ts:${l} at ${COMMIT}.\n`;
};

function declareEnum(mod, name, values, rpath) {
  const f = out(mod);
  f.names.add(name);
  const s0 = [...named].find(([, a]) => a.mod === mod && a.name === name)?.[0];
  if (s0 && fromFrames(name, s0)) {
    f.frames.push(name);
    f.items.push(`${srcLine(mod, name)}/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.\npub use wsp_frames::${name};\n\nimpl Check for ${name} {}`);
    return;
  }
  const used = new Set();
  const variants = values.map(v => {
    let n = pascal(v);
    while (used.has(n)) n += "X";
    used.add(n);
    return `    #[serde(rename = ${JSON.stringify(v)})]\n    ${n},`;
  });
  f.items.push(`${srcLine(mod, name)}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]\npub enum ${name} {\n${variants.join("\n")}\n}\n\nimpl Check for ${name} {}`);
}

function unwrap(s) {
  let opt = false, nul = false, def, effects = false;
  for (;;) {
    const t = s._def.typeName;
    if (t === "ZodOptional") opt = true;
    else if (t === "ZodNullable") nul = true;
    // An optional outside a default answers undefined before the default is reached: the key stays absent.
    else if (t === "ZodDefault") { if (!opt) def = s._def.defaultValue(); }
    else break;
    s = s._def.innerType;
  }
  return { s, opt, nul, def };
}

function declareStruct(mod, name, s) {
  const f = out(mod);
  f.names.add(name);
  const reexport = named.get(s)?.name === name && fromFrames(name, s);
  const d = s._def;
  const rules = [];
  const fields = [];
  const defaults = [];
  const tag = tagOf(s, name);
  let tagValue;
  const order = ORDER[`${mod}:${name}`];
  const shapeEntries = Object.entries(d.shape());
  if (order) shapeEntries.sort((a, b) => order.indexOf(a[0]) - order.indexOf(b[0]));
  for (const [key, raw] of shapeEntries) {
    if (key === tag) { tagValue = raw._def.value; continue; }
    const { s: inner, opt, nul, def } = unwrap(raw);
    const fname = ident(key);
    const bare = fname.replace(/^r#/, "");
    const attrs = [];
    if (camelOfSnake(bare) !== key) attrs.push(`rename = ${JSON.stringify(key)}`);
    let ty = typeOf(mod, inner, `${name}${pascal(key)}`, rules, key);
    if (def !== undefined) {
      const fn = `default_${snake(name)}_${bare}`;
      attrs.push(`default = ${JSON.stringify(fn)}`);
      defaults.push(`fn ${fn}() -> ${nul ? `Option<${ty}>` : ty} {\n    from_json(${JSON.stringify(JSON.stringify(def))})\n}`);
      if (nul) ty = `Option<${ty}>`;
    } else if (opt && nul) {
      attrs.push(`default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option"`);
      ty = `Option<Option<${ty}>>`;
    } else if (opt) {
      attrs.push(`default, skip_serializing_if = "Option::is_none"`);
      ty = `Option<${ty}>`;
    } else if (nul) {
      attrs.push(`deserialize_with = "crate::types::check::nullable"`);
      ty = `Option<${ty}>`;
    }
    fields.push(`${attrs.length ? `    #[serde(${attrs.join(", ")})]\n` : ""}    pub ${fname}: ${ty},`);
  }
  const strict = d.unknownKeys === "strict";
  const passthrough = d.unknownKeys === "passthrough";
  if (passthrough) {
    f.uses.add("indexmap::IndexMap");
    fields.push(`    #[serde(flatten)]\n    pub rest: IndexMap<String, serde_json::Value>,`);
  }
  const attrs = [`rename_all = "camelCase"`];
  if (strict) attrs.push("deny_unknown_fields");
  const own = rules.filter(([, r]) => !r.startsWith("check of"));
  const nested = [...new Set(rules.filter(([, r]) => r.startsWith("check of")).map(([p]) => p.split(/[.[{]/)[0]))];
  const doc = [];
  if (own.length) doc.push(`    /// Rules: ${own.map(([p, r]) => `${p}: ${r}`).join("; ")}.`);
  if (nested.length) doc.push(`    /// Nested checks: ${nested.join(", ")}.`);
  if (strict) doc.push(`    /// Strict: an unknown key is refused (zod .strict()).`);
  if (reexport) {
    f.frames.push(name);
    f.items.push(`${srcLine(mod, name)}/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.\npub use wsp_frames::${name};\n\nimpl Check for ${name} {\n${doc.length ? doc.join("\n") + "\n" : ""}    fn check(&self) -> Result<(), SchemaError> {\n        todo!()\n    }\n}`);
    return;
  }
  f.items.push(
    `${srcLine(mod, name)}#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(${attrs.join(", ")})]\npub struct ${name} {\n${fields.join("\n")}\n}\n\n` +
      (tag ? `impl ${name} {\n    /// The \`${tag}\` key's value: the enum holding this struct writes and reads it as its tag.\n    pub const ${snake(tag).toUpperCase()}: &'static str = ${JSON.stringify(tagValue)};\n}\n\n` : "") +
      `impl Check for ${name} {\n${doc.length ? doc.join("\n") + "\n" : ""}    fn check(&self) -> Result<(), SchemaError> {\n        todo!()\n    }\n}` +
      (defaults.length ? `\n\n${defaults.join("\n\n")}` : ""),
  );
  if (defaults.length) f.uses.add("crate::types::check::from_json");
}

function declareUnion(mod, name, opts, disc, rpath) {
  const f = out(mod);
  f.names.add(name);
  const rules = [];
  const used = new Set();
  const variants = opts.map((o, i) => {
    let vname;
    const at = named.get(o);
    if (at) vname = at.name;
    else if (o._def.typeName === "ZodObject" && disc && o._def.shape()[disc]?._def?.value !== undefined) vname = pascal(String(o._def.shape()[disc]._def.value));
    else vname = `V${i}`;
    while (used.has(vname)) vname += "X";
    used.add(vname);
    const ty = at ? (rules.push([`${rpath}|${i}`, `check of ${at.name}`]), ref(mod, at)) : typeOf(mod, o, `${name}${vname}`, rules, `${rpath}|${i}`);
    const tagv = disc ? o._def.shape()[disc]._def.value : undefined;
    return `${disc ? `    #[serde(rename = ${JSON.stringify(tagv)})]\n` : ""}    ${vname}(${ty}),`;
  });
  f.items.push(`${srcLine(mod, name)}/// ${disc ? `zod's discriminatedUnion on \`${disc}\`: each variant's struct leaves that key to this tag.` : "A zod union: the first variant that parses wins, in this order."}\n#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(${disc ? `tag = ${JSON.stringify(disc)}` : "untagged"})]\npub enum ${name} {\n${variants.join("\n")}\n}\n\nimpl Check for ${name} {\n    fn check(&self) -> Result<(), SchemaError> {\n        todo!()\n    }\n}`);
}

// --- constants
function rustConst(mod, name, v) {
  const f = out(mod);
  if (mod === "views/place-setup-words") return (f.skipped.push(`const ${name} (wsp-words)`), true);
  const l = lineOf(mod, name);
  const head = `/// TS: packages/protocol/src/${mod}.ts:${l} at ${COMMIT}.\n`;
  const constName = /^[A-Z0-9_]+$/.test(name) ? name : snake(name).toUpperCase();
  const num = x => (Number.isInteger(x) ? (x < 0 ? ["i64", String(x)] : ["u64", String(x).replace(/\B(?=(\d{3})+(?!\d))/g, "_")]) : ["f64", String(x)]);
  if (typeof v === "string") return f.consts.push(`${head}pub const ${constName}: &str = ${JSON.stringify(v)};`), true;
  if (typeof v === "boolean") return f.consts.push(`${head}pub const ${constName}: bool = ${v};`), true;
  if (typeof v === "number") { const [t, x] = num(v); return f.consts.push(`${head}pub const ${constName}: ${t} = ${x};`), true; }
  if (v instanceof RegExp) return f.consts.push(`${head}/// A JavaScript RegExp, flags "${v.flags}": the pattern source as the TS writes it.\npub const ${constName}: &str = ${JSON.stringify(v.source)};`), true;
  const list = v instanceof Set ? [...v] : Array.isArray(v) ? v : undefined;
  if (list && list.every(x => typeof x === "string")) return f.consts.push(`${head}${v instanceof Set ? "/// A Set in the TS, in insertion order.\n" : ""}pub const ${constName}: &[&str] = &[${list.map(x => JSON.stringify(x)).join(", ")}];`), true;
  if (list && list.every(x => typeof x === "number")) { const t = list.every(Number.isInteger) ? (list.every(x => x >= 0) ? "u64" : "i64") : "f64"; return f.consts.push(`${head}pub const ${constName}: &[${t}] = &[${list.map(x => (t === "f64" && Number.isInteger(x) ? `${x}.0` : String(x))).join(", ")}];`), true; }
  if (v && typeof v === "object" && !Array.isArray(v) && !(v instanceof Map) && !(v instanceof Set) && Object.values(v).every(x => typeof x === "string"))
    return f.consts.push(`${head}/// A Record in the TS: its keys in the TS's order.\npub const ${constName}: &[(&str, &str)] = &[\n${Object.entries(v).map(([k, x]) => `    (${JSON.stringify(k)}, ${JSON.stringify(x)}),`).join("\n")}\n];`), true;
  if (v && typeof v === "object" && !Array.isArray(v) && !(v instanceof Map) && !(v instanceof Set) && Object.values(v).every(x => typeof x === "number"))
    return f.consts.push(`${head}/// A Record in the TS: its keys in the TS's order.\npub const ${constName}: &[(&str, f64)] = &[\n${Object.entries(v).map(([k, x]) => `    (${JSON.stringify(k)}, ${Number.isInteger(x) ? `${x}.0` : x}),`).join("\n")}\n];`), true;
  return false;
}

// Shapes the TS parses with no exported name: given one here so a Rust function can name them.
const EXTRA = [["views/preferences", "PreferencesStored", () => M["views/preferences"].Preferences.partial(), "views/preferences.ts:209 preferencesFrom's Preferences.partial()"]];
for (const [mod, name, make, where] of EXTRA) {
  const s = make();
  out(mod).items.push(`/// ${where}: every key optional, a missing one left absent (no default applies).`);
  declareStruct(mod, name, s);
}

// --- walk my modules
const report = [];
for (const [mod, ns] of Object.entries(M)) {
  if (!mine(mod)) continue;
  const f = out(mod);
  const order = Object.keys(ns).filter(n => definedIn(mod, n)).sort((a, b) => lineOf(mod, a) - lineOf(mod, b));
  for (const name of order) {
    const v = ns[name];
    if (v?._def?.typeName) {
      if (f.names.has(name)) continue;
      const rules = [];
      const t = v._def.typeName;
      if (t === "ZodObject") declareStruct(mod, name, v);
      else if (t === "ZodEnum") declareEnum(mod, name, v._def.values, name);
      else if (t === "ZodUnion" || t === "ZodDiscriminatedUnion") {
        const self = named.get(v);
        named.delete(v);
        typeOf(mod, v, name, rules, name);
        named.set(v, self);
      }
      else {
        const self = named.get(v);
        named.delete(v);
        const ty = typeOf(mod, v, `${name}Item`, rules, name);
        named.set(v, self);
        f.items.push(`${srcLine(mod, name)}pub type ${name} = ${ty};${rules.length ? `\n// Rules: ${rules.map(([p, r]) => `${p}: ${r}`).join("; ")}.` : ""}`);
        f.names.add(name);
      }
    } else if (typeof v === "function") f.skipped.push(`fn ${name}`);
    else if (!rustConst(mod, name, v)) f.skipped.push(`const ${name}: ${v?.constructor?.name ?? typeof v}`);
  }
}

// --- the wire writer's shapes my types name, declared here until the wire branch lands
for (let pass = 0; pass < 3; pass++) for (const [name, { s }] of [...pendingWire]) {
  if (out("pending_wire").names.has(name)) continue;
  const t = s._def.typeName;
  if (t === "ZodObject") declareStruct("pending_wire", name, s);
  else if (t === "ZodEnum") declareEnum("pending_wire", name, s._def.values, name);
  else { const self = named.get(s); named.delete(s); const ty = typeOf("pending_wire", s, `${name}Item`, [], name); named.set(s, self); out("pending_wire").items.push(`pub type ${name} = ${ty};`); out("pending_wire").names.add(name); }
}
SKIP_TOP.delete("pending_wire");

// --- write
fs.mkdirSync(OUT, { recursive: true });
for (const [mod, f] of files) {
  if (!mine(mod)) continue;
  if (f.items.length === 0 && f.consts.length === 0) { report.push(`${mod}: nothing generated; skipped ${f.skipped.join(", ")}`); continue; }
  const lits = [...f.lits.values()].map(l => `literal!(${l.name}, ${l.num ? l.value : JSON.stringify(l.value)});`);
  if (lits.length) f.uses.add("crate::literal");
  const uses = [...f.uses].filter(u => !u.startsWith(`${rustMod(mod)}::`)).sort();
  const needsCheck = f.items.some(i => i.includes("impl Check"));
  const body = [
    `// SPDX-License-Identifier: AGPL-3.0-only`,
    `//! Port of packages/protocol/src/${mod}.ts at ${COMMIT}: its zod shapes and plain constants, written by the generator.`,
    "",
    ...(f.items.some(i => i.includes("Serialize, Deserialize")) ? ["use serde::{Deserialize, Serialize};", ""] : []),
    ...(needsCheck ? [f.items.some(i => i.includes("SchemaError>")) ? "use crate::types::check::{Check, SchemaError};" : "use crate::types::check::Check;"] : []),
    ...uses.map(u => `use ${u};`),
    "",
    ...(lits.length ? [lits.join("\n"), ""] : []),
    ...f.consts.flatMap(c => [c, ""]),
    ...f.items.flatMap(i => [i, ""]),
  ].join("\n");
  const rel = mod.startsWith("views/") ? `views/${snake(mod.slice(6))}.rs` : `types/${snake(mod)}.rs`;
  fs.mkdirSync(path.dirname(path.join(OUT, rel)), { recursive: true });
  const stem = path.basename(rel, ".rs");
  // The repo's size law stops a file at 1,500 lines: a long module's later shapes go to <stem>_more.rs, and the two
  // re-export each other so every path still reads crate::<dir>::<stem>::<Name>.
  if (body.split("\n").length > 1200 && f.items.length > 1) {
    const half = Math.ceil(f.items.length / 2);
    const head = [...f.items.slice(0, half)];
    const rest = f.items.slice(half);
    const first = body.slice(0, body.indexOf(f.items[0])) + head.join("\n\n") + `\n\npub use super::${stem}_more::*;\n`;
    const more = [
      `// SPDX-License-Identifier: AGPL-3.0-only`,
      `//! The later shapes of packages/protocol/src/${mod}.ts at ${COMMIT}, apart only for the repo's file-size law; ${stem}.rs re-exports them.`,
      "",
      "#![allow(unused_imports)]",
      "",
      ...(rest.some(i => i.includes("Serialize, Deserialize")) ? ["use serde::{Deserialize, Serialize};", ""] : []),
      ...(rest.some(i => i.includes("impl Check")) ? [rest.some(i => i.includes("SchemaError>")) ? "use crate::types::check::{Check, SchemaError};" : "use crate::types::check::Check;"] : []),
      ...uses.map(u => `use ${u};`),
      `use super::${stem}::*;`,
      "",
      ...rest.flatMap(i => [i, ""]),
    ].join("\n");
    fs.writeFileSync(path.join(OUT, rel), first.replace(/\n{3,}/g, "\n\n"));
    fs.writeFileSync(path.join(OUT, rel.replace(/\.rs$/, "_more.rs")), more.replace(/\n{3,}/g, "\n\n"));
  } else fs.writeFileSync(path.join(OUT, rel), body.replace(/\n{3,}/g, "\n\n"));
  report.push(`${rel}: ${f.items.length} types (${f.frames.length} from wsp-frames), ${f.consts.length} consts${f.notes.length ? `; notes: ${f.notes.join("; ")}` : ""}${f.skipped.length ? `; not generated: ${f.skipped.join(", ")}` : ""}`);
}
fs.writeFileSync(path.join(OUT, "REPORT.txt"), report.join("\n") + "\n");
console.log(report.length, "modules");
