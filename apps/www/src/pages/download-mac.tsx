// SPDX-License-Identifier: AGPL-3.0-only
// Where "download the app" lands on a Mac: it starts the download and walks through the first open. The app is not
// signed by Apple yet, so macOS refuses a downloaded copy until the person presses Open Anyway (wsp-map#2004). The
// pictures are cut from a recording of that first open on macOS 26.
import { useEffect } from "react";
import { ArrowUpRight } from "lucide-react";
import { CopyCommand } from "@/components/copy-command";
import { Section } from "@/components/section";
import { DOWNLOADS } from "@/downloads";
import { INSTALL } from "@/links";

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

function Picture({ shot }: { shot: Shot }) {
  const media = { width: shot.width, height: shot.height, className: "block h-auto w-full" };
  return (
    <div className="relative mx-auto overflow-hidden rounded-[12px] border border-border" style={{ maxWidth: shot.width / 2 }}>
      {shot.clip === undefined ? (
        <img src={shot.src} alt={shot.alt} loading="lazy" {...media} />
      ) : (
        <video src={shot.clip} poster={shot.src} autoPlay muted loop playsInline aria-label={shot.alt} {...media} />
      )}
      {shot.ring !== undefined && (
        <span
          aria-hidden
          className="absolute rounded-full ring-2 ring-amber"
          style={{ left: `${shot.ring.x}%`, top: `${shot.ring.y}%`, width: `${shot.ring.w}%`, height: `${shot.ring.h}%` }}
        />
      )}
    </div>
  );
}

export function MacDownload() {
  useEffect(() => {
    window.location.href = DOWNLOADS.mac.href;
  }, []);
  return (
    <main>
      <Section className="pt-28 sm:pt-40">
        <div className="mx-auto max-w-[620px] text-center">
          <h1 className="heading text-[34px] sm:text-[48px]">Your download has started.</h1>
          <p className="lede mt-5 text-[17px] sm:text-[18px]">
            wsp is not signed by Apple yet, so the first time you open it, macOS asks you to allow it. Five steps, about a minute.{" "}
            <a href={DOWNLOADS.mac.href} className="text-foreground underline decoration-faint underline-offset-4 transition-colors duration-150 hover:decoration-foreground">
              Download again
            </a>{" "}
            if nothing started.
          </p>
        </div>
        <ol className="mx-auto mt-14 max-w-[1000px] sm:mt-20">
          {MAC_STEPS.map((step, i) => (
            <li key={step.title} className="grid items-center gap-6 border-t border-rule py-10 md:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] md:gap-12 md:py-14">
              <div>
                <span className="font-mono text-[13px] tabular-nums text-faint">
                  Step {i + 1} of {MAC_STEPS.length}
                </span>
                <h2 className="mt-2 text-[20px] font-medium text-foreground sm:text-[22px]">{step.title}</h2>
                <p className="mt-2 text-[16px] leading-relaxed text-muted-foreground">{step.body}</p>
                {step.settings === true && (
                  <a href={PRIVACY} className="keycap mt-4 inline-flex h-9 items-center gap-1.5 rounded-[8px] px-3 text-[14px] text-foreground transition-colors duration-150 hover:bg-foreground/[0.07]">
                    Open Privacy & Security <ArrowUpRight className="size-3.5 text-faint" />
                  </a>
                )}
              </div>
              <Picture shot={step.shot} />
            </li>
          ))}
        </ol>
        <div className="mx-auto mt-20 flex max-w-[620px] flex-col items-center text-center sm:mt-24">
          <h2 className="text-[17px] font-medium text-foreground">Rather skip all this?</h2>
          <p className="mt-2 text-[15px] leading-relaxed text-muted-foreground">Paste this line into Terminal. It installs the same app with no prompts, and adds the wsp command.</p>
          <CopyCommand command={INSTALL} shown="curl -fsSL usewsp.com/install | sh" className="mt-5 text-[13px] sm:text-[14px]" />
        </div>
      </Section>
    </main>
  );
}
