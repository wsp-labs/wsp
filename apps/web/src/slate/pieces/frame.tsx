// SPDX-License-Identifier: AGPL-3.0-only
// An HTML page the agent wrote, read by the host from the thread's computer and drawn sealed (frame-seal.ts): it sits
// on the panel with no ground of its own, in the theme's colours, as tall as it says it is up to a cap, and opens
// large in the diagram's dialog. Nothing in it reaches the slate: the panel hears its height and nothing else.
import type { SlatesPageAnswer } from "@wsp/protocol";
import { Maximize2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { Button } from "../../components/ui/button.js";
import { Dialog, DialogClose, DialogPopup, DialogTitle } from "../../components/ui/dialog.js";
import { Spinner } from "../../components/ui/spinner.js";
import { Tooltip, TooltipPopup, TooltipTrigger } from "../../components/ui/tooltip.js";
import { cn } from "../../lib/utils.js";
import { DOC, RUNS, type SlateEngine } from "../engine.js";
import type { PieceView } from "../SlateView.js";
import { frameShell, frameTheme, sealPage, type FrameTheme } from "./frame-seal.js";
import { runsEnded } from "./image.js";
import { str } from "./look.js";
import { EXPANDED_POPUP } from "./zoom.js";

/** A page's tallest inline; past it the page scrolls inside its frame and the dialog shows it whole. */
const INLINE_CAP = 560;
/** The height a frame holds before its page says its own. */
const FIRST_HEIGHT = 160;

type Page = { state: "loading" } | { state: "shown"; html: string } | { state: "refused"; why: string };
type Read = ((src: string, have?: string) => Promise<SlatesPageAnswer>) | undefined;

const sentence = (s: string): string => (/^[a-z]+ /.test(s) ? s[0]!.toUpperCase() + s.slice(1) : s);

function usePage(slate: SlateEngine, src: string, read: Read): Page {
  const [page, setPage] = useState<Page>({ state: "loading" });
  const held = useRef<{ src: string; version?: string }>({ src: "" });
  // Read again on each write and each run's end, either of which may have rewritten the file, as an image is.
  const written = useSyncExternalStore(listen => slate.subscribe(DOC, listen), () => slate.version);
  const ended = useSyncExternalStore(listen => slate.subscribe(RUNS, listen), () => runsEnded(slate));
  useEffect(() => {
    if (read === undefined) return void setPage({ state: "refused", why: "This window reads no pages" });
    let live = true;
    const have = held.current.src === src ? held.current.version : undefined;
    void read(src, have).then(
      answer => {
        if (!live) return;
        if ("unchanged" in answer) return;
        if ("problem" in answer) return void setPage({ state: "refused", why: sentence(answer.problem.message) });
        held.current = { src, ...(answer.version !== undefined ? { version: answer.version } : {}) };
        setPage({ state: "shown", html: answer.html });
      },
      (e: unknown) => live && setPage({ state: "refused", why: e instanceof Error ? e.message : String(e) }),
    );
    return () => void (live = false);
  }, [src, read, written, ended]);
  return page;
}

/** The window's theme as the page is handed it, read again whenever the theme marks on the root change. */
function useFrameTheme(): FrameTheme {
  const [mark, setMark] = useState(() => `${document.documentElement.className}|${document.documentElement.dataset["theme"] ?? ""}`);
  useEffect(() => {
    const root = document.documentElement;
    const watch = new MutationObserver(() => setMark(`${root.className}|${root.dataset["theme"] ?? ""}`));
    watch.observe(root, { attributes: true, attributeFilter: ["class", "data-theme", "style"] });
    return () => watch.disconnect();
  }, []);
  return useMemo(() => frameTheme(), [mark]);
}

/** One sealed frame: as tall as its page says, up to cap where there is one, else the box it is given. */
function Sealed({ html, theme, cap, title, className }: { html: string; theme: FrameTheme; cap?: number; title: string; className?: string }) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(FIRST_HEIGHT);
  const doc = useMemo(() => frameShell(sealPage(html, theme), theme.scheme), [html, theme]);
  useEffect(() => {
    if (cap === undefined) return;
    const heard = (e: MessageEvent) => {
      const d = e.data as { wspFrame?: unknown; height?: unknown } | null;
      if (e.source !== frame.current?.contentWindow || d?.wspFrame !== "height" || typeof d.height !== "number") return;
      setHeight(Math.max(24, Math.min(cap, Math.ceil(d.height))));
    };
    addEventListener("message", heard);
    return () => removeEventListener("message", heard);
  }, [cap]);
  return <iframe ref={frame} data-slate-frame-page title={title} sandbox="allow-scripts" srcDoc={doc} className={cn("block w-full border-0 bg-transparent", className)} style={{ colorScheme: theme.scheme, ...(cap !== undefined ? { height } : {}) }} />;
}

function Expand({ onClick }: { onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-xs" aria-label="Expand" onClick={onClick} className="text-muted-foreground" />}><Maximize2 aria-hidden className="size-3.5" /></TooltipTrigger>
      <TooltipPopup side="top">Expand</TooltipPopup>
    </Tooltip>
  );
}

export const frame: PieceView = {
  type: "frame",
  card: false,
  component: function FramePiece({ props, slate, page: read }) {
    const src = str(props["src"]) ?? "";
    const caption = str(props["caption"]);
    const page = usePage(slate, src, read);
    const theme = useFrameTheme();
    const [open, setOpen] = useState(false);
    return (
      <figure data-slate-frame className="flex min-w-0 flex-col gap-2">
        {page.state === "shown" ? (
          <Sealed html={page.html} theme={theme} cap={INLINE_CAP} title={caption ?? src} />
        ) : (
          <div data-slate-frame-waiting className="relative flex items-center rounded-xl border border-border px-4 py-3 text-xs leading-4 text-pretty text-muted-foreground" style={{ minHeight: FIRST_HEIGHT }}>
            {page.state === "loading" ? <Spinner className="mx-auto size-4" /> : page.why}
          </div>
        )}
        <div className="flex min-h-6 items-center gap-3">
          {caption !== undefined && caption !== "" ? <figcaption className="min-w-0 flex-1 truncate text-xs leading-4 text-muted-foreground">{caption}</figcaption> : null}
          {page.state === "shown" ? <span className="ml-auto"><Expand onClick={() => setOpen(true)} /></span> : null}
        </div>
        {page.state === "shown" ? (
          <Dialog open={open} onOpenChange={setOpen}>
            <DialogPopup data-slate-frame-expanded className={EXPANDED_POPUP} bottomStickOnMobile={false}>
              <div className="flex items-center gap-1 px-5 pt-4 pb-3">
                <DialogTitle className="min-w-0 flex-1 truncate">{caption ?? src}</DialogTitle>
                <DialogClose render={<Button variant="ghost" size="icon-xs" aria-label="Close" className="text-muted-foreground" />}><X aria-hidden className="size-3.5" /></DialogClose>
              </div>
              <div className="mx-5 mb-5 min-h-0 flex-1">
                {open ? <Sealed html={page.html} theme={theme} title={caption ?? src} className="h-full" /> : null}
              </div>
            </DialogPopup>
          </Dialog>
        ) : null}
      </figure>
    );
  },
};
