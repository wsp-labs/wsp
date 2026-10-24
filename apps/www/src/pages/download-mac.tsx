// SPDX-License-Identifier: AGPL-3.0-only
// Where "download the app" lands on a Mac: it starts the download and walks through the first open. The app is not
// signed by Apple yet, so macOS refuses a downloaded copy until the person presses Open Anyway (wsp-map#2004). The
// pictures are cut from a recording of that first open on macOS 26.
import { useEffect, useState } from "react";
import { ArrowUpRight, ChevronLeft, ChevronRight } from "lucide-react";
import { CopyCommand } from "@/components/copy-command";
import { Section } from "@/components/section";
import { DOWNLOADS } from "@/downloads";
import { INSTALL } from "@/links";
import { cn } from "@/lib/utils";

/** System Settings' own address for Privacy & Security; a browser asks before it opens it. */
const PRIVACY = "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension";

/** A step's picture, its size for layout, and where on it the button to press sits, in percent of the picture. */
type Shot = { src: string; alt: string; width: number; height: number; clip?: string; ring?: { x: number; y: number; w: number; h: number } };
type Step = { title: string; body: string; shot: Shot; settings?: true };

const DIR = "/download/mac";

export const MAC_STEPS: readonly Step[] = [
  {
    title: "Drag wsp to Applications",
    body: "Open wsp-mac.dmg from your Downloads, then drag wsp onto the Applications folder.",
    shot: { src: `${DIR}/1-dmg.webp`, clip: `${DIR}/1-drag.mp4`, alt: "The wsp disk image window: wsp dragged onto the Applications folder", width: 1500, height: 1025 },
  },
  {
    title: "Open wsp, then click Done",
    body: 'Open wsp from Applications. macOS says "wsp" Not Opened. That is expected; click Done.',
    shot: { src: `${DIR}/2-not-opened.webp`, alt: 'The macOS prompt "wsp" Not Opened, with Done and Move to Bin', width: 907, height: 559, ring: { x: 24, y: 75.5, w: 25.8, h: 11.7 } },
  },
  {
    title: "Click Open Anyway in System Settings",
    body: 'Open System Settings, then Privacy & Security, and scroll down to Security. Next to "wsp" was blocked, click Open Anyway.',
    shot: { src: `${DIR}/3-settings.webp`, alt: 'System Settings, Privacy & Security: "wsp" was blocked to protect your Mac, with Open Anyway', width: 1452, height: 605, ring: { x: 80.1, y: 58.9, w: 15.7, h: 10.9 } },
    settings: true,
  },
  {
    title: "Click Open Anyway again",
    body: "macOS asks once more. Click Open Anyway. The blue Move to Bin deletes wsp.",
    shot: { src: `${DIR}/4-open-wsp.webp`, alt: 'The prompt Open "wsp"?, with Move to Bin, Open Anyway and Done', width: 594, height: 756, ring: { x: 11.1, y: 73.9, w: 77.8, h: 8.4 } },
  },
  {
    title: "Confirm with Touch ID or your password",
    body: "Touch the sensor, or click Use Password. wsp opens, and from now on it opens like any other app.",
    shot: { src: `${DIR}/5-password.webp`, alt: "The prompt asking for Touch ID or your password to allow wsp", width: 594, height: 696 },
  },
];

/** How long each step shows before the next, while nobody has picked one. */
const DWELL_MS = 7000;
/** The stage's width over its height; every picture fits inside it, centred, never past its own size. */
const STAGE = 1.5;

function Picture({ shot, shown }: { shot: Shot; shown: boolean }) {
  const ratio = shot.width / shot.height;
  const media = { width: shot.width, height: shot.height, className: "block h-auto w-full" };
  return (
    <div className={cn("col-start-1 row-start-1 flex items-center justify-center p-4 transition-opacity duration-300 sm:p-8", shown ? "opacity-100" : "pointer-events-none opacity-0")} aria-hidden={!shown}>
      <div className="relative overflow-hidden rounded-[10px] border border-white/[0.08]" style={{ width: `min(${Math.min(100, (ratio / STAGE) * 100)}%, ${shot.width / 2}px)` }}>
        {shot.clip === undefined ? (
          <img src={shot.src} alt={shot.alt} {...media} />
        ) : (
          <video src={shot.clip} poster={shot.src} autoPlay muted loop playsInline aria-label={shot.alt} {...media} />
        )}
        {shot.ring !== undefined && (
          <span aria-hidden className="absolute rounded-full ring-2 ring-amber" style={{ left: `${shot.ring.x}%`, top: `${shot.ring.y}%`, width: `${shot.ring.w}%`, height: `${shot.ring.h}%` }} />
        )}
      </div>
    </div>
  );
}

export function MacDownload() {
  const [at, setAt] = useState(0);
  // The steps turn by themselves until a person picks one, like the slate examples on the home page.
  const [auto, setAuto] = useState(true);
  const [held, setHeld] = useState(false);
  useEffect(() => {
    window.location.href = DOWNLOADS.mac.href;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) setAuto(false);
  }, []);
  const go = (i: number) => {
    setAt((i + MAC_STEPS.length) % MAC_STEPS.length);
    setAuto(false);
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") go(at + 1);
      if (e.key === "ArrowLeft") go(at - 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  const step = MAC_STEPS[at]!;
  return (
    <main>
      <Section className="pt-28 sm:pt-36">
        <div className="mx-auto max-w-[620px] text-center">
          <h1 className="heading text-[34px] sm:text-[44px]">Your download has started.</h1>
          <p className="lede mt-4 text-[17px]">
            wsp is not signed by Apple yet, so the first time you open it, macOS asks you to allow it. Five steps, about a minute.{" "}
            <a href={DOWNLOADS.mac.href} className="text-foreground underline decoration-faint underline-offset-4 transition-colors duration-150 hover:decoration-foreground">
              Download again
            </a>{" "}
            if nothing started.
          </p>
        </div>
        <div
          className="mx-auto mt-12 grid max-w-[1100px] gap-6 md:grid-cols-[minmax(0,4fr)_minmax(0,7fr)] md:gap-10"
          onPointerEnter={() => setHeld(true)}
          onPointerLeave={() => setHeld(false)}
        >
          <ol className="order-2 md:order-1">
            {MAC_STEPS.map((s, i) => (
              <li key={s.title} className="relative border-t border-rule first:border-t-0 last:border-b md:first:border-t">
                <button type="button" onClick={() => go(i)} aria-current={i === at ? "step" : undefined} className="group flex w-full items-baseline gap-3 py-3.5 text-left">
                  <span className={cn("font-mono text-[13px] tabular-nums transition-colors duration-150", i === at ? "text-amber" : "text-faint")}>{i + 1}</span>
                  <span className={cn("text-[16px] transition-colors duration-150", i === at ? "font-medium text-foreground" : "text-muted-foreground group-hover:text-foreground")}>{s.title}</span>
                </button>
                {i === at && (
                  <div className="pb-5 pl-5">
                    <p className="text-[15px] leading-relaxed text-muted-foreground">{s.body}</p>
                    {s.settings === true && (
                      <a href={PRIVACY} className="keycap mt-4 inline-flex h-9 items-center gap-1.5 rounded-[8px] px-3 text-[14px] text-foreground transition-colors duration-150 hover:bg-foreground/[0.07]">
                        Open Privacy & Security <ArrowUpRight className="size-3.5 text-faint" />
                      </a>
                    )}
                  </div>
                )}
                {auto && i === at ? (
                  <span
                    key={at}
                    aria-hidden
                    className="absolute bottom-[-1px] left-0 h-px w-full origin-left bg-foreground/60"
                    style={{ animation: `fill ${DWELL_MS}ms linear forwards`, animationPlayState: held ? "paused" : "running" }}
                    onAnimationEnd={() => setAt(n => (n + 1) % MAC_STEPS.length)}
                  />
                ) : null}
              </li>
            ))}
          </ol>
          <div className="order-1 md:order-2">
            <div className="grid overflow-hidden rounded-[14px] border border-border bg-ground" style={{ aspectRatio: STAGE }} aria-live="polite" aria-label={`Step ${at + 1} of ${MAC_STEPS.length}: ${step.title}`}>
              {MAC_STEPS.map((s, i) => (
                <Picture key={s.title} shot={s.shot} shown={i === at} />
              ))}
            </div>
            <div className="mt-3 flex items-center justify-between">
              <span className="font-mono text-[13px] tabular-nums text-faint">
                Step {at + 1} of {MAC_STEPS.length}
              </span>
              <div className="flex gap-2">
                <button type="button" onClick={() => go(at - 1)} aria-label="Previous step" className="keycap inline-flex size-8 items-center justify-center rounded-[8px] text-muted-foreground transition-colors duration-150 hover:text-foreground">
                  <ChevronLeft className="size-4" />
                </button>
                <button type="button" onClick={() => go(at + 1)} aria-label="Next step" className="keycap inline-flex size-8 items-center justify-center rounded-[8px] text-muted-foreground transition-colors duration-150 hover:text-foreground">
                  <ChevronRight className="size-4" />
                </button>
              </div>
            </div>
          </div>
        </div>
        <div className="mx-auto mt-20 flex max-w-[620px] flex-col items-center text-center">
          <h2 className="text-[17px] font-medium text-foreground">Rather skip all this?</h2>
          <p className="mt-2 text-[15px] leading-relaxed text-muted-foreground">Paste this line into Terminal. It installs the same app with no prompts, and adds the wsp command.</p>
          <CopyCommand command={INSTALL} shown="curl -fsSL usewsp.com/install | sh" className="mt-5 text-[13px] sm:text-[14px]" />
        </div>
      </Section>
    </main>
  );
}
