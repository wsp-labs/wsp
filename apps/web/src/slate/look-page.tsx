// SPDX-License-Identifier: AGPL-3.0-only
// The page a slate look draws: the app's own renderer in a hidden window the host's computer opens through the app
// (packages/host/src/slate-look.ts), the slate handed over its bridge with the theme and width the person's window
// last drew it at. The host reads every image and page it names, as it does for the window; nothing here presses,
// writes or sends. Once every picture and frame has drawn, it says its height and the window is captured.
import { createRoot } from "react-dom/client";
import type { SlatesImageAnswer, SlatesPageAnswer, SlateView } from "@wsp/protocol";
import type { SlateDoc, SlateJson } from "@wsp/protocol/slate";
import { ActionRunner, StateSender, type SlateLink } from "./actions.js";
import { SlateEngine } from "./engine.js";
import { SLATE_VIEWS } from "./pieces/index.js";
import { SlateView as SlateDrawing } from "./SlateView.js";
import { themeById } from "../themes/index.js";

interface LookBridge {
  slate(): Promise<{ view: SlateView; theme: string; width: number }>;
  read(kind: "image" | "page", src: string): Promise<SlatesImageAnswer | SlatesPageAnswer>;
  drawn(height: number): Promise<void>;
}

const nothing = async () => ({});

/** Settled once nothing is loading and every frame has said a height, or after the wait runs out. */
async function settled(): Promise<void> {
  const end = Date.now() + 8_000;
  await new Promise(r => setTimeout(r, 400));
  while (Date.now() < end) {
    const loading = document.querySelector("[data-slate-image-loading], [data-slate-frame-waiting] svg") !== null;
    const pictures = [...document.querySelectorAll("img")].every(i => i.complete);
    if (!loading && pictures) break;
    await new Promise(r => setTimeout(r, 100));
  }
  await new Promise(r => setTimeout(r, 600));
}

export async function mountLook(root: HTMLElement): Promise<void> {
  const bridge = (window as unknown as { wspLook?: LookBridge }).wspLook;
  if (bridge === undefined) throw new Error("this page draws a slate for a look and has no bridge to read it from");
  const { view, theme, width } = await bridge.slate();
  document.documentElement.dataset["theme"] = theme;
  document.documentElement.classList.toggle("dark", themeById(theme)?.side === "dark");
  const engine = new SlateEngine(view.threadId);
  engine.setRecord(view.document as unknown as SlateDoc | null, view.values as Record<string, SlateJson>, view.version, view.revision);
  const link = { event: nothing, writeState: nothing, approve: nothing, cancel: nothing, revoke: nothing, consent: () => {}, fill: () => {}, image: (src: string) => bridge.read("image", src), page: (src: string) => bridge.read("page", src) } as unknown as SlateLink;
  createRoot(root).render(
    <div data-panel style={{ width }} className="w-full px-4 py-4">
      <SlateDrawing engine={engine} views={SLATE_VIEWS} runner={new ActionRunner(engine, () => link)} sender={new StateSender(engine, () => link)} />
    </div>,
  );
  await settled();
  await bridge.drawn(Math.ceil(document.querySelector("[data-panel]")!.getBoundingClientRect().height));
}
