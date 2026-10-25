import { it } from "vitest";
import { writeFileSync } from "node:fs";
const R = "/root/wsp-vm/wt-2441-cut";
const MINE: Record<string, string> = { workspaces: "WORKSPACE_VERBS", agents: "AGENT_VERBS", computers: "COMPUTER_VERBS", projects: "PROJECT_VERBS", slate: "SLATE_VERBS", activity: "ACTIVITY_VERBS", conversations: "CONVERSATION_VERBS" };
function kindOf(t: any): unknown {
  const d = t._def;
  switch (d.typeName) {
    case "ZodOptional": return { optional: kindOf(d.innerType) };
    case "ZodNullable": return { nullable: kindOf(d.innerType) };
    case "ZodDefault": return kindOf(d.innerType);
    case "ZodString": return d.checks?.some((c: any) => c.kind === "regex") ? { string: { regex: String(d.checks.find((c: any) => c.kind === "regex").regex.source) } } : "string";
    case "ZodNumber": return { number: d.checks.map((c: any) => [c.kind, c.value ?? null, c.inclusive ?? null]) };
    case "ZodBoolean": return "boolean";
    case "ZodArray": return { array: kindOf(d.type), ...(d.minLength ? { min: d.minLength.value } : {}) };
    case "ZodRecord": return { record: kindOf(d.valueType) };
    case "ZodEnum": return { enum: d.values };
    case "ZodUnion": return { union: d.options.map(kindOf) };
    case "ZodLiteral": return { literal: d.value };
    case "ZodUnknown": return "unknown";
    default: return d.typeName;
  }
}
it("dumps", async () => {
  await import(`${R}/packages/host/src/verbs.ts`);
  const out: Record<string, unknown[]> = {};
  for (const [file, name] of Object.entries(MINE)) {
    const m = await import(`${R}/packages/host/src/verbs/${file}.ts`);
    out[file] = (m[name] as any[]).map(v => ({
      name: v.name,
      ...(v.usage !== undefined ? { usage: v.usage, about: v.about, page: v.page } : {}),
      ...(v.options !== undefined ? { options: Object.entries(v.options).map(([k, o]: [string, any]) => ({ name: k, type: o.type, ...(o.multiple ? { multiple: true } : {}), ...(o.short ? { short: o.short } : {}), ...(o.valueWith ? { valueWith: o.valueWith } : {}) })) } : {}),
      ...(v.cloud ? { cloud: true } : {}),
      ...(v.cloudFlags ? { cloudFlags: v.cloudFlags } : {}),
      ...(v.cliOnly ? { cliOnly: v.cliOnly } : {}),
      ...(v.toolOnly ? { toolOnly: v.toolOnly } : {}),
      ...(v.hostSide ? { hostSide: v.hostSide } : {}),
      ...(v.readsHere ? { readsHere: v.readsHere } : {}),
      ...(v.startsNoHost ? { startsNoHost: v.startsNoHost } : {}),
      ...(v.anyRelease ? { anyRelease: v.anyRelease } : {}),
      ...(v.command ? { command: true } : {}),
      ...(v.tool ? { tool: { description: v.tool.description, input: Object.entries(v.tool.input).map(([k, t]: [string, any]) => ({ name: k, kind: kindOf(t), ...(t.description ? { describe: t.description } : t._def.innerType?.description ? { describe: t._def.innerType.description } : {}) })), output: Object.keys(v.tool.output), ...(v.tool.stream ? { stream: v.tool.stream } : {}) } } : {}),
    }));
  }
  writeFileSync("/root/cli-b-probe/table.json", JSON.stringify(out, null, 1));
  console.log(Object.entries(out).map(([k, v]) => `${k} ${v.length}`).join(", "));
});
