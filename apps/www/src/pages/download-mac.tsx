// SPDX-License-Identifier: AGPL-3.0-only
// Where "download the app" lands on a Mac: it starts the download and walks through the first open. The app is not
// signed by Apple yet, so macOS refuses a downloaded copy until the person presses Open Anyway (wsp-map#2004).
import { useEffect } from "react";
import { CopyCommand } from "@/components/copy-command";
import { Section } from "@/components/section";
import { DOWNLOADS } from "@/downloads";
import { INSTALL } from "@/links";

type Step = { title: string; body: string; shot?: { src: string; alt: string } };

export const MAC_STEPS: readonly Step[] = [
  { title: "Drag wsp to Applications", body: "Open wsp-mac.dmg from your Downloads and drag wsp into the Applications folder." },
  { title: "Open it, then click Done", body: 'Open wsp from Applications. macOS shows "wsp" Not Opened. Click Done.' },
  { title: "Click Open Anyway", body: "In System Settings, open Privacy & Security. Next to the line about wsp, click Open Anyway." },
  { title: "Confirm once", body: "Click Open Anyway again and enter your password. From now on wsp opens like any other app." },
];

export function MacDownload() {
  useEffect(() => {
    window.location.href = DOWNLOADS.mac.href;
  }, []);
  return (
    <main>
      <Section className="pt-28 sm:pt-40">
        <div className="mx-auto max-w-[600px] text-center">
          <h1 className="heading text-[34px] sm:text-[48px]">Your download has started.</h1>
          <p className="lede mt-5 text-[17px] sm:text-[18px]">
            wsp is not signed by Apple yet, so macOS asks once before it opens it. Four steps, about a minute.{" "}
            <a href={DOWNLOADS.mac.href} className="text-foreground underline decoration-faint underline-offset-4 transition-colors duration-150 hover:decoration-foreground">
              Download again
            </a>{" "}
            if nothing started.
          </p>
        </div>
        <ol className="mx-auto mt-14 grid max-w-[1100px] gap-x-6 gap-y-10 sm:mt-20 sm:grid-cols-2 lg:grid-cols-4">
          {MAC_STEPS.map((step, i) => (
            <li key={step.title} className="flex flex-col">
              {step.shot !== undefined && <img src={step.shot.src} alt={step.shot.alt} loading="lazy" className="mb-5 aspect-[4/3] w-full rounded-[12px] border border-border object-cover" />}
              <span className="font-mono text-[13px] tabular-nums text-faint">{String(i + 1).padStart(2, "0")}</span>
              <h2 className="mt-3 border-t border-rule pt-4 text-[17px] font-medium text-foreground">{step.title}</h2>
              <p className="mt-2 text-[15px] leading-relaxed text-muted-foreground">{step.body}</p>
            </li>
          ))}
        </ol>
        <div className="mx-auto mt-20 flex max-w-[600px] flex-col items-center text-center sm:mt-24">
          <h2 className="text-[17px] font-medium text-foreground">Prefer the terminal?</h2>
          <p className="mt-2 text-[15px] leading-relaxed text-muted-foreground">The install line puts the same app in Applications, adds the wsp command, and skips every step above.</p>
          <CopyCommand command={INSTALL} shown="curl -fsSL usewsp.com/install | sh" className="mt-5 text-[13px] sm:text-[14px]" />
        </div>
      </Section>
    </main>
  );
}
