// SPDX-License-Identifier: AGPL-3.0-only
// One slate drawn by the real renderer with fixed values, for the #2439 shots: ?slate=<name>&theme=graphite|paper.
// The slate's text comes from slates/ here or the glance test's own files, its values from the glance test's.
import { createRoot } from "react-dom/client";
import { parseSlate, slateStartValues, type SlateJson } from "@wsp/protocol/slate";
import "../../src/index.css";
import "../../src/themes/index.js";
import { ActionRunner, StateSender, type SlateLink } from "../../src/slate/actions.js";
import { SlateEngine, type Scheduler } from "../../src/slate/engine.js";
import { SLATE_VIEWS } from "../../src/slate/pieces/index.js";
import { SlateView } from "../../src/slate/SlateView.js";
import data from "../../../../packages/protocol/test/slate/glance/values.json";

const texts = {
  ...import.meta.glob("../../../../packages/protocol/test/slate/glance/*.slate", { query: "?raw", import: "default", eager: true }),
  ...import.meta.glob("./slates/*.slate", { query: "?raw", import: "default", eager: true }),
} as Record<string, string>;
const at = new URLSearchParams(location.search);
const name = at.get("slate") ?? "before";
const theme = at.get("theme") ?? "graphite";
document.documentElement.dataset["theme"] = theme;
document.documentElement.classList.toggle("dark", theme === "graphite");

const text = Object.entries(texts).find(([path]) => path.endsWith(`/${name}.slate`))?.[1];
const parsed = text === undefined ? undefined : parseSlate(text);
const root = createRoot(document.getElementById("root")!);
if (parsed?.document === undefined) {
  root.render(<pre data-glance-error>{text === undefined ? `no slate ${name}` : JSON.stringify(parsed?.errors, null, 1)}</pre>);
} else {
  const doc = parsed.document;
  const now = (data as { now: number }).now;
  const scheduler: Scheduler = {
    frame: fn => { const id = requestAnimationFrame(() => fn()); return () => cancelAnimationFrame(id); },
    later: (fn, ms) => { const id = setTimeout(fn, ms); return () => clearTimeout(id); },
    now: () => now,
  };
  const set = JSON.parse(at.get("set") ?? "{}") as Record<string, SlateJson>;
  const sources: Record<string, SlateJson> = { "time.now": now, "thread.cost.usd": 4.82 };
  const engine = new SlateEngine("glance", path => sources[path], scheduler);
  engine.setRecord(doc, { ...slateStartValues(doc), ...(data as { values: Record<string, SlateJson> }).values, ...set }, 25, 25);
  const link: SlateLink = {
    event: async () => ({ outcome: "started" as const, said: "Sent to the agent" }),
    writeState: async () => ({ version: 26 }),
    approve: async () => ({ ok: true }),
    cancel: async () => ({ ok: true }),
    revoke: async () => ({ ok: true }),
    consent: () => {},
    fill: () => {},
  };
  const sender = new StateSender(engine, () => link);
  const runner = new ActionRunner(engine, () => link);
  // The slate panel's own column: its title muted over the slate, 16 px in, as SlateSurface draws it.
  root.render(
    <div data-glance={name} className="mx-auto w-full max-w-[760px] px-4 pt-3 pb-8">
      <h2 className="mb-4 flex h-7 items-center text-[13px] leading-5 font-normal text-muted-foreground">{doc.title ?? "Slate"}</h2>
      <SlateView engine={engine} views={SLATE_VIEWS} runner={runner} sender={sender} />
    </div>,
  );
}
