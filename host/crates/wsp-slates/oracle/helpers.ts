// Runs the pure helpers of the slates-a slice at 65f6b89c7, copied verbatim, and prints what they answer.
import { createHash } from "node:crypto";
const SLATE_LIMITS = { errorsPerPass: 20, startsPerMinute: 12 } as const;
type SlateJson = any;
type SlateProblem = { code: string; name: string; message: string; piece?: string; prop?: string; line?: number; fix?: string };

// slate-message.ts
function defanged(value: SlateJson): SlateJson {
  if (typeof value === "string") return value.replace(/^(\s*slate)\s*:(?=\s*\{)/gm, "$1：");
  if (Array.isArray(value)) return value.map(defanged);
  if (value !== null && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, defanged(v)]));
  return value;
}
function longest(value: SlateJson, at: (string | number)[] = []): { at: (string | number)[]; length: number } | undefined {
  if (typeof value === "string") return { at, length: value.length };
  const kids = Array.isArray(value) ? value.map((v, i) => longest(v, [...at, i])) : value !== null && typeof value === "object" ? Object.entries(value).map(([k, v]) => longest(v, [...at, k])) : [];
  return kids.reduce<{ at: (string | number)[]; length: number } | undefined>((best, k) => (k !== undefined && (best === undefined || k.length > best.length) ? k : best), undefined);
}
function cutAt(value: SlateJson, at: (string | number)[], keep: number): SlateJson {
  if (at.length === 0) return typeof value === "string" ? `${value.slice(0, keep)} (cut)` : value;
  const [head, ...rest] = at;
  if (Array.isArray(value)) return value.map((v, i) => (i === head ? cutAt(v, rest, keep) : v));
  if (value !== null && typeof value === "object") return { ...value, [head as string]: cutAt(value[head as string]!, rest, keep) };
  return value;
}
// slate-writes.ts
function writeBudget(now: () => number) {
  const writes = new Map<string, { tokens: number; at: number; hour: number[] }>();
  return {
    spend(threadId: string): number | undefined {
      const at = now();
      const b = writes.get(threadId) ?? { tokens: 20, at, hour: [] };
      b.tokens = Math.min(20, b.tokens + ((at - b.at) / 1000) * 5);
      b.at = at;
      b.hour = b.hour.filter(t => at - t < 3_600_000);
      writes.set(threadId, b);
      if (b.tokens < 1 || b.hour.length >= 600) return b.tokens < 1 ? Math.ceil((1 - b.tokens) / 5) : Math.ceil((3_600_000 - (at - b.hour[0]!)) / 1000);
      b.tokens -= 1;
      b.hour.push(at);
      return undefined;
    },
    delete: (threadId: string): boolean => writes.delete(threadId),
  };
}
// slate-refused.ts
function invalid(errors: SlateProblem[], warnings: SlateProblem[]): any {
  const said = (p: SlateProblem): string => {
    const where = [p.line !== undefined ? `line ${p.line}` : undefined, p.piece !== undefined ? (p.prop !== undefined ? `${p.piece}.${p.prop}` : p.piece) : undefined].filter(w => w !== undefined).join(", ");
    return `${where === "" ? "" : `${where}: `}${p.code} ${p.name} ${p.message}${p.fix !== undefined ? `. Did you mean ${p.fix}?` : ""}`;
  };
  const listed = errors.slice(0, 20);
  const more = errors.length > 20 ? ` And ${errors.length - 20} more.` : "";
  const message = `slate refused: ${errors.length} error${errors.length === 1 ? "" : "s"}: ${listed.length === 1 ? said(listed[0]!) : listed.map((p, i) => `${i + 1}) ${said(p)}`).join(" ")}${more}`;
  return { message, kind: "invalid", errors: errors.slice(0, 20).length, warnings: warnings.length };
}
// slate-read.ts runShown
const isRunRecord = (v: any) => typeof v === "object" && v !== null && !Array.isArray(v) && typeof v["state"] === "string" && typeof v["runs"] === "number";
function runShown(rec: SlateJson | undefined, name: string, named: boolean): SlateJson {
  if (!isRunRecord(rec)) return rec ?? null;
  const full = rec;
  if (named) return Object.fromEntries(Object.entries(full).filter(([k]) => ["state", "why", "exit", "ms", "runs", "stale", "refreshing", "endedAt"].includes(k)));
  const cut = (field: string, text: string): string => (text.length <= 4000 ? text : `${text.slice(0, 4000)}… (${text.length} characters; values ["$${name}.${field}"] reads it whole)`);
  const out: Record<string, SlateJson> = {};
  for (const [k, v] of Object.entries(full)) {
    if (k === "out" && full["json"] !== undefined) continue;
    if ((k === "out" || k === "err") && typeof v === "string") out[k] = cut(k, v);
    else if (k === "lines" && Array.isArray(v)) out[k] = v.slice(-50);
    else out[k] = v;
  }
  return out;
}
// slate-runs.ts helpers
function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stableJson(v)}`).join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}
const key = (decl: any): string => {
  const env = Object.entries(decl.env ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(([name, expr]) => [name, stableJson(expr)]);
  const what = [decl.kind, decl.cmd, env, (decl.args ?? []).map(stableJson), decl.stdin === undefined ? null : stableJson(decl.stdin), decl.on ?? "thread", decl.cwd ?? "", decl.stream === true, decl.every ?? null, decl.always === true, decl.timeout ?? 60, decl.once === true, ...(decl.then !== undefined ? [decl.then] : []), ...(decl.files !== undefined ? [stableJson(decl.files)] : []), ...(decl.scripts !== undefined ? [stableJson(decl.scripts)] : [])];
  return createHash("sha256").update(JSON.stringify(what)).digest("hex").slice(0, 32);
};
function asText(value: SlateJson): string {
  if (value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}
function timeoutOf(decl: any): number { return Math.min(Math.max(decl.timeout ?? 60, 1), 600000 / 1_000); }
function spent(times: readonly number[] | undefined, at: number): number[] | undefined {
  const kept = (times ?? []).filter(s => at - s < 60_000);
  return kept.length >= 12 ? undefined : [...kept, at];
}
function secretForms(text: string): string[] {
  const forms = new Set<string>([text]);
  const bytes = Buffer.from(text, "utf8");
  const b64 = (b: Buffer): string[] => { const std = b.toString("base64"); const url = std.replace(/\+/g, "-").replace(/\//g, "_"); return [std, std.replace(/=+$/, ""), url, url.replace(/=+$/, "")]; };
  for (const f of b64(bytes)) forms.add(f);
  for (const shift of [0, 1, 2]) {
    const all = Buffer.concat([Buffer.alloc(shift), bytes]).toString("base64");
    const from = shift === 0 ? 0 : 4;
    const to = Math.floor((shift + bytes.length) / 3) * 4;
    const middle = all.slice(from, to);
    if (middle.length < 8) continue;
    forms.add(middle);
    forms.add(middle.replace(/\+/g, "-").replace(/\//g, "_"));
  }
  const url = encodeURIComponent(text);
  forms.add(url);
  forms.add(url.replace(/%[0-9A-F]{2}/g, m => m.toLowerCase()));
  forms.add(url.replace(/%20/g, "+"));
  forms.delete("");
  return [...forms];
}
const p = (label: string, v: unknown) => console.log(`${label} => ${JSON.stringify(v)}`);
p("defanged.line", defanged("ok\nslate: {\"v\":2}\n  slate :{x}\nslate: not json\nmyslate: {"));
p("defanged.nested", defanged({ a: ["slate:{}", 1, null], b: { c: " slate\t: {" } }));
p("longest.tie", longest({ a: "xx", b: "yy", c: ["zzz", "q"] }));
p("longest.none", longest({ a: 1, b: [null, true] }) ?? "undefined");
p("longest.string", longest("abc"));
p("cutAt.obj", cutAt({ a: "abcdefgh", b: ["x", "yyyyyy"] }, ["b", 1], 2));
p("cutAt.root", cutAt("abcdef", [], 3));
p("cutAt.keep0", cutAt("abcdef", [], 0));
p("cutAt.nonstring", cutAt(5, [], 3));
let t = 0; const wb = writeBudget(() => t);
const out: any[] = []; for (let i = 0; i < 22; i++) out.push(wb.spend("a") ?? null); p("budget.burst22", out);
t = 100; p("budget.after100ms", wb.spend("a") ?? null);
t = 200; p("budget.after200ms", wb.spend("a") ?? null);
t = 10_000; const o2: any[] = []; for (let i = 0; i < 3; i++) o2.push(wb.spend("a") ?? null); p("budget.10s", o2);
{ let tt = 0; const w = writeBudget(() => tt); const res: any[] = []; for (let i = 0; i < 601; i++) { tt = i * 1000; res.push(w.spend("h") ?? null); } p("budget.hour.600th_and_601st", res.slice(598)); tt = 600_000; p("budget.hour.at600s", w.spend("h") ?? null); tt = 3_600_000; p("budget.hour.at3600s", w.spend("h") ?? null); }
p("invalid.one", invalid([{ code: "S501", name: "name-undeclared", message: "$nope is not declared", line: 1, piece: "text-1", prop: "value", fix: "$no" }], []));
p("invalid.two", invalid([{ code: "S501", name: "a", message: "m1" }, { code: "T306", name: "b", message: "m2", piece: "meter-1" }], [{ code: "W1", name: "w", message: "x" }]));
p("invalid.zero", invalid([], []));
p("invalid.22", invalid(Array.from({ length: 22 }, (_, i) => ({ code: `C${i}`, name: "n", message: "m" })), []).message.slice(-40));
p("runShown.named", runShown({ state: "done", why: "w", exit: 0, out: "x", err: "e", json: {}, ms: 3, runs: 1, stale: true, endedAt: 9, startedAt: 2, lines: ["a"] }, "r", true));
p("runShown.json_drops_out", runShown({ state: "done", out: "{}", json: {}, runs: 1 }, "r", false));
p("runShown.cut", (runShown({ state: "done", out: "y".repeat(4001), runs: 1 }, "raw", false) as any).out.slice(3995));
p("runShown.cut4000", (runShown({ state: "done", out: "y".repeat(4000), runs: 1 }, "raw", false) as any).out.length);
p("runShown.lines", (runShown({ state: "done", lines: Array.from({ length: 60 }, (_, i) => String(i)), runs: 1 }, "r", false) as any).lines.slice(0, 2));
p("runShown.notrecord", runShown({ state: "done" }, "r", false));
p("runShown.undefined", runShown(undefined, "r", false));
p("runShown.runsString", runShown({ state: "x", runs: "1" }, "r", true));
p("key.plain", key({ kind: "cmd", cmd: "true" }));
p("key.env", key({ kind: "cmd", cmd: "echo \"$ID\"", env: { ID: { bind: "$id" } } }));
p("key.full", key({ kind: "cmd", cmd: "x", env: { B: "$b", A: { bind: "$a" } }, args: ["$x", { bind: "$y" }], stdin: { format: "${$z}" }, on: "host", cwd: "sub", stream: true, every: 30, always: true, timeout: 20, once: true, then: "jq .", files: { "b.sh": "2", "a.sh": "1" }, scripts: { "x.sh": "abc" } }));
p("key.timeout60_same_as_absent", key({ kind: "cmd", cmd: "true", timeout: 60 }) === key({ kind: "cmd", cmd: "true" }));
p("key.on_thread_same_as_absent", key({ kind: "cmd", cmd: "true", on: "thread" }) === key({ kind: "cmd", cmd: "true" }));
p("key.confirm_not_in_key", key({ kind: "cmd", cmd: "true", confirm: "Sure?" }) === key({ kind: "cmd", cmd: "true" }));
p("stableJson", stableJson({ b: [1, { d: null, c: "x" }], a: undefined, "é": true }));
p("stableJson.undefined", stableJson(undefined));
p("asText", [asText(null), asText("s"), asText(1.5), asText(true), asText([1, "a"]), asText({ b: 1, a: 2 }), asText(1e21), asText(-0)]);
p("timeoutOf", [timeoutOf({}), timeoutOf({ timeout: 0 }), timeoutOf({ timeout: 0.5 }), timeoutOf({ timeout: 601 }), timeoutOf({ timeout: -5 }), timeoutOf({ timeout: 2.5 })]);
p("spent", [spent(undefined, 0), spent(Array.from({ length: 12 }, (_, i) => i), 59_999) ?? "undefined", spent(Array.from({ length: 12 }, (_, i) => i), 60_000)]);
p("secretForms.token", secretForms("vrcl_Ab+9/zz&q=1 ünï_0123456789"));
p("secretForms.short", secretForms("4321"));
p("secretForms.space", secretForms("a b"));
