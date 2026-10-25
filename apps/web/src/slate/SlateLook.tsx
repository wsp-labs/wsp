// SPDX-License-Identifier: AGPL-3.0-only
// The page wsp slate look draws: one thread's slate as the panel's Slate tab draws it, whole, at the panel's width in
// the person's theme, opened by the desktop app in a window nobody sees (#slate-look=<thread>&w=<px>). Once every
// image and frame has come in and the height has held still, it says so on window.__wspSlateLook, which the app
// reads before it takes the picture.
import { useEffect, useRef } from "react";
import { PREVIEW_PANEL_DEFAULT_WIDTH } from "../components/preview/PreviewPanelShell.js";
import { finiteNumber } from "../hooks/useLocalStorage.js";
import { RIGHT_PANEL_WIDTH_STORAGE_KEY } from "../rightPanelStore.js";
import { useStore } from "../protocol/store.js";
import { useSlateStore } from "./store.js";
import { ThreadSlate } from "./SlateSurface.js";
import "./SlateHost.js";

export const SLATE_LOOK_HASH = "#slate-look=";

export const SLATE_LOOK_WORDS = {
  notDrawn: (seconds: number) => `The app did not draw this thread's slate within ${seconds} s; look again in a moment.`,
  newer: (schema: number) => `This slate needs a newer wsp app to draw it (schema ${schema}); update the app and look again.`,
} as const;

/** How long the page waits for its images and frames before it is taken as it stands. */
const SETTLE_CAP_MS = 15_000;
/** Polls the height must hold for before the picture is taken. */
const STILL_POLLS = 4;
const POLL_MS = 100;

export interface SlateLookAsk {
  threadId: string;
  width?: number;
}

export type SlateLookState = { state: "drawing" } | { state: "ready"; width: number; height: number; theme: string } | { state: "failed"; why: string };

/** The look a page's hash asks for, or none. */
export function slateLookOf(hash: string): SlateLookAsk | undefined {
  if (!hash.startsWith(SLATE_LOOK_HASH)) return undefined;
  const params = new URLSearchParams(hash.slice(1));
  const threadId = params.get("slate-look");
  if (threadId === null || threadId === "") return undefined;
  const width = Number(params.get("w"));
  return { threadId, ...(Number.isInteger(width) && width > 0 ? { width } : {}) };
}

/** The panel's left border, inside the width it keeps: the slate draws one pixel narrower. */
const PANEL_BORDER = 1;

/** The width the slate draws at in the panel as this window last left it, else in its default. */
function panelWidth(): number {
  try {
    const raw = window.localStorage.getItem(RIGHT_PANEL_WIDTH_STORAGE_KEY);
    return (raw === null ? PREVIEW_PANEL_DEFAULT_WIDTH : Math.round(finiteNumber.decode(raw))) - PANEL_BORDER;
  } catch {
    return PREVIEW_PANEL_DEFAULT_WIDTH - PANEL_BORDER;
  }
}

/** Whether everything the slate loads has come in: no image or frame still waiting, every picture decoded, every
 * frame's page having said its height. */
export function slateSettled(root: ParentNode): boolean {
  if (root.querySelector("[data-slate-surface]") === null) return false;
  if (root.querySelector("[data-slate-image-loading], [data-slate-frame-waiting] svg") !== null) return false;
  if ([...root.querySelectorAll("img")].some(img => !img.complete)) return false;
  return [...root.querySelectorAll("[data-slate-frame-page]")].every(frame => frame.hasAttribute("data-slate-frame-heard"));
}

const say = (state: SlateLookState): void => void ((window as unknown as { __wspSlateLook?: SlateLookState }).__wspSlateLook = state);

export function SlateLook({ threadId, width }: SlateLookAsk) {
  const drawn = width ?? panelWidth();
  const root = useRef<HTMLDivElement>(null);
  const newer = useSlateStore(s => s.byThread[threadId]?.newer);
  const bound = useStore(s => s.api !== null);
  useEffect(() => {
    say({ state: "drawing" });
    const began = Date.now();
    let still = 0;
    let last = -1;
    const timer = setInterval(() => {
      const el = root.current;
      if (el === null) return;
      const height = Math.ceil(el.getBoundingClientRect().height);
      still = height === last && slateSettled(el) ? still + 1 : 0;
      last = height;
      if (still < STILL_POLLS && Date.now() - began < SETTLE_CAP_MS) return;
      clearInterval(timer);
      if (el.querySelector("[data-slate-surface]") === null) return say({ state: "failed", why: SLATE_LOOK_WORDS.notDrawn(SETTLE_CAP_MS / 1000) });
      say({ state: "ready", width: drawn, height, theme: document.documentElement.dataset["theme"] ?? "" });
    }, POLL_MS);
    return () => clearInterval(timer);
  }, [drawn]);
  useEffect(() => {
    if (newer !== undefined) say({ state: "failed", why: SLATE_LOOK_WORDS.newer(newer) });
  }, [newer]);
  return (
    <div ref={root} data-slate-look={threadId} style={{ width: drawn }} className="flex flex-col bg-background">
      {bound ? <ThreadSlate threadId={threadId} whole /> : null}
    </div>
  );
}
