// SPDX-License-Identifier: AGPL-3.0-only
import { ArrowLeft, ArrowRight, Check, Minus } from "lucide-react";
import type { ReactNode } from "react";
import poster from "@/assets/clips/setup-poster.jpg";
import hero from "@/assets/shots/w-hero.webp";
import hero800 from "@/assets/shots/w-hero-800.webp";
import hero1440 from "@/assets/shots/w-hero-1440.webp";
import { Lockup } from "@/components/brand";
import { CopyCommand } from "@/components/copy-command";
import { Install } from "@/components/download";
import { Head, Section } from "@/components/section";
import { KINDS, READ, ROWS, saidOf, sources, STEPS, strongerOf, TOOLS, VENDORS_BRING, WSP, type Cell, type Said, type Tool } from "@/compare";
import { INSTALL, REPO } from "@/links";
import { ToolIcon, WspIcon } from "@/logos";
import { ToolCards } from "@/sections/compares";
import { WALL } from "@/wall";
import { cn } from "@/lib/utils";

/** The one picture a tool's page shows, from the shots the home page already uses. */
const SHOTS = {
  computer: {
    title: "Add a computer. Pick what comes along.",
    lede: "Point wsp at a Linux computer you can reach over SSH. You pick what it copies there, and it joins your sidebar ready for threads.",
    img: {
      src: poster,
      width: 1000,
      height: 1094,
      sizes: "(min-width: 1024px) 560px, 100vw",
      alt: "The Agents step of Add a computer in wsp: Claude Code, Codex, Gemini CLI, OpenCode and Pi to install on office-pc, each with how it signs in there.",
    },
  },
  subthreads: {
    title: "Agents start each other, and you see every thread.",
    lede: "A lead thread hands work to Claude Code and Codex threads on any of your computers. Each one sits in your sidebar, where you read and answer it.",
    img: {
      src: hero,
      srcSet: `${hero800} 800w, ${hero1440} 1440w, ${hero} 2880w`,
      width: 1440,
      height: 900,
      sizes: "(min-width: 1200px) 1100px, 92vw",
      alt: "The wsp app: a lead thread on a MacBook Pro has handed its tickets to Claude Code and Codex threads on five computers, with its board of the night's landings in the slate beside it.",
    },
  },
} as const;

const shown = (url: string): string => url.replace(/^https:\/\/(www\.)?/, "").replace(/\/blob\/[0-9a-f]{40}\//, "/");

function Mark({ good }: { good: boolean }) {
  return good ? <Check aria-label="yes" className="mt-[3px] size-4 shrink-0 text-success" /> : <Minus aria-label="no" className="mt-[3px] size-4 shrink-0 text-faint" />;
}

/** One answer: its mark and short words, with the sentence and pages it sums as the hover title. */
function Answer({ cell, us }: { cell: Cell; us: boolean }) {
  return (
    <td title={cell.says.map(shown).join("\n")} className={cn("border-l border-rule px-4 py-4 sm:px-5", us ? "bg-raised font-medium text-foreground" : "text-muted-foreground max-sm:border-l-0")}>
      <span className="flex gap-2.5">
        <Mark good={cell.good} />
        {cell.short}
      </span>
    </td>
  );
}

/** The questions down the side, the tool then wsp across. On a phone each question heads its own two answers. */
function Table({ tool }: { tool: Tool }) {
  return (
    <div className="overflow-hidden rounded-[14px] border border-border">
      <table className="w-full border-collapse text-[15px] leading-[1.5] max-sm:block sm:table-fixed">
        <colgroup>
          <col className="w-[36%]" />
          <col />
          <col />
        </colgroup>
        <thead className="max-sm:block">
          <tr className="max-sm:grid max-sm:grid-cols-2">
            <td className="max-sm:hidden" />
            <th scope="col" className="border-l border-rule px-4 py-4 text-left text-[14px] font-medium text-muted-foreground max-sm:border-l-0 sm:px-5">
              {tool.name}
            </th>
            <th scope="col" className="border-l border-rule bg-raised px-4 py-4 text-left text-[14px] font-medium text-foreground sm:px-5">
              <Lockup className="text-[18px]" />
            </th>
          </tr>
        </thead>
        <tbody className="max-sm:block">
          {ROWS.map(r => (
            <tr key={r.key} className="border-t border-rule align-top max-sm:grid max-sm:grid-cols-2">
              <th scope="row" className="px-4 py-4 text-left font-normal max-sm:col-span-2 max-sm:pb-0 sm:px-5">
                <span className="block text-muted-foreground max-sm:text-[14px]">{r.label}</span>
                <span className="mt-1 block text-[13px] leading-snug text-faint">{r.about}</span>
              </th>
              <Answer cell={tool.cells[r.key]} us={false} />
              <Answer cell={WSP[r.key]} us />
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** One side's points, each one sentence with its pages as the hover title; a point from AHEAD carries wsp's own line. */
function Points({ title, items }: { title: string; items: { said: Said; wsp?: Said }[] }) {
  return (
    <div className="border-t border-rule pt-6">
      <h3 className="text-[17px] font-medium tracking-[-0.01em] text-foreground">{title}</h3>
      <ul className="mt-4 flex flex-col gap-3">
        {items.map(({ said, wsp }) => (
          <li key={said[0]} title={[...said.slice(1), ...(wsp?.slice(1) ?? [])].map(shown).join("\n")} className="flex gap-3 text-[15px] leading-relaxed text-muted-foreground">
            <span aria-hidden className="mt-[11px] size-1 shrink-0 rounded-full bg-faint" />
            <span>
              {said[0]}
              {wsp !== undefined && <span className="text-faint">{` ${wsp[0]}`}</span>}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Header({ eyebrow, icons, title, children }: { eyebrow: ReactNode; icons?: ReactNode; title: string; children: ReactNode }) {
  return (
    <header className="mx-auto max-w-[1200px] px-4 pt-32 pb-12 sm:px-6 sm:pt-40 sm:pb-16">
      <div className="font-mono text-[13px] text-faint">{eyebrow}</div>
      {icons !== undefined && <div className="mt-8 flex items-center gap-3">{icons}</div>}
      <h1 className={cn("display text-[40px] sm:text-[64px]", icons === undefined ? "mt-4" : "mt-6")}>{title}</h1>
      <p className="lede mt-6 max-w-[640px] text-[17px] sm:text-[18px]">{children}</p>
    </header>
  );
}

function Shot({ shot }: { shot: (typeof SHOTS)[keyof typeof SHOTS] }) {
  const { alt, ...img } = shot.img;
  return (
    <Section id="shot" className="grid items-center gap-12 pt-0 sm:pt-0 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] lg:gap-14">
      <Head title={shot.title}>{shot.lede}</Head>
      <div className="relative isolate overflow-hidden rounded-[20px] border border-white/[0.08] p-[5%] sm:rounded-[24px]">
        <img {...WALL} sizes="(min-width: 1024px) 700px, 100vw" alt="" aria-hidden loading="lazy" decoding="async" draggable={false} className="absolute inset-0 -z-10 size-full object-cover select-none" />
        <img {...img} alt={alt} loading="lazy" decoding="async" draggable={false} className="block h-auto w-full rounded-[12px] shadow-[0_30px_80px_-20px_rgb(0_0_0/0.7),0_0_0_1px_rgb(0_0_0/0.45)] select-none" />
      </div>
    </Section>
  );
}

function TryIt() {
  return (
    <div className="border-t border-rule">
      <Section className="flex flex-col gap-8 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <h2 className="heading text-[32px] sm:text-[44px]">Try it on a computer you own.</h2>
          <p className="lede mt-4 text-[17px]">
            Something here wrong or out of date?{" "}
            <a href={`${REPO}/issues/new`} className="text-foreground underline decoration-faint underline-offset-4 transition-colors duration-150 hover:decoration-foreground">
              Open an issue
            </a>{" "}
            with the page that says so.
          </p>
        </div>
        <Install />
      </Section>
    </div>
  );
}

export function Compare() {
  return (
    <main>
      <Header eyebrow="Compare" title="Where wsp sits.">
        {`Most of these run agents on the computer you sit at, and leave any second computer for you to set up by hand. The vendors run them on their own VM, from ${VENDORS_BRING}. wsp copies your setup onto computers you own.`}
      </Header>
      <div className="mx-auto flex max-w-[1200px] flex-col gap-14 px-4 pb-20 sm:px-6 sm:pb-28">
        {KINDS.map(kind => (
          <section key={kind.name}>
            <h2 className="font-mono text-[13px] font-normal text-faint">{kind.name}</h2>
            <ul className="mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {kind.tools.map(tool => (
                <li key={tool.slug}>
                  <a href={`/compare/${tool.slug}`} className="group flex h-full flex-col rounded-[14px] border border-border p-6 transition-colors duration-150 hover:bg-raised">
                    <span className="flex items-center gap-3.5 text-[17px] font-medium tracking-[-0.01em] text-foreground">
                      <ToolIcon slug={tool.slug} />
                      <span className="flex-1">{tool.name}</span>
                      <ArrowRight className="size-4 shrink-0 text-faint transition-colors duration-150 group-hover:text-foreground" />
                    </span>
                    <span className="mt-2 text-[15px] leading-relaxed text-muted-foreground">{tool.line}</span>
                  </a>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
      <TryIt />
    </main>
  );
}

export function ToolPage({ tool }: { tool: Tool }) {
  return (
    <main>
      <Header
        eyebrow={
          <a href="/compare" className="inline-flex items-center gap-1.5 transition-colors duration-150 hover:text-foreground">
            <ArrowLeft className="size-3.5" /> Compare
          </a>
        }
        icons={
          <>
            <WspIcon className="size-12 rounded-[12px]" />
            <span className="font-mono text-[13px] text-faint">vs</span>
            <ToolIcon slug={tool.slug} className="size-12 rounded-[12px]" />
          </>
        }
        title={`wsp vs ${tool.name}`}
      >
        {tool.differs}
      </Header>

      <div className="mx-auto max-w-[1200px] px-4 sm:px-6">
        <Table tool={tool} />
      </div>

      <Section id="strengths">
        <h2 className="heading text-[32px] sm:text-[44px]">Where each is stronger.</h2>
        <div className="mt-12 grid gap-x-16 gap-y-10 md:grid-cols-2">
          <Points title={`Where ${tool.name} is stronger`} items={strongerOf(tool)} />
          <Points title="Where wsp is stronger" items={tool.weaker.map(p => ({ said: saidOf(tool, p) }))} />
        </div>
      </Section>

      <Section id="pick" className="pt-0 sm:pt-0">
        <h2 className="heading text-[32px] sm:text-[44px]">Which to pick.</h2>
        <div className="mt-12 grid gap-x-16 gap-y-10 md:grid-cols-2">
          <Points title={`Pick ${tool.name} if`} items={tool.pickThem.map(p => ({ said: saidOf(tool, p) }))} />
          <Points title="Pick wsp if" items={tool.pickWsp.map(p => ({ said: saidOf(tool, p) }))} />
        </div>
      </Section>

      <Section id="steps" className="pt-0 sm:pt-0">
        <h2 className="heading text-[32px] sm:text-[44px]">Switch in three steps.</h2>
        <ol className="mt-12 grid gap-4 lg:grid-cols-[1.3fr_1fr_1fr]">
          {STEPS.map((step, i) => (
            <li key={step.title} title={step.said.slice(1).map(shown).join("\n")} className="flex min-w-0 flex-col rounded-[14px] border border-border p-6">
              <span className="font-mono text-[13px] text-faint">{i + 1}</span>
              <h3 className="mt-3 text-[17px] font-medium tracking-[-0.01em] text-foreground">{step.title}</h3>
              <p className="mt-2 text-[15px] leading-relaxed text-muted-foreground">
                {step.said[0].split("`").map((part, j) =>
                  j % 2 === 1 ? (
                    <code key={part} className="font-mono text-[13.5px] text-foreground">
                      {part}
                    </code>
                  ) : (
                    part
                  ),
                )}
              </p>
              {i === 0 && <CopyCommand command={INSTALL} shown="curl -fsSL usewsp.com/install | sh" className="mt-5 self-start" />}
            </li>
          ))}
        </ol>
      </Section>

      <Shot shot={SHOTS[tool.shot]} />

      <Section id="others" className="pt-0 sm:pt-0">
        <Head title="wsp vs others." />
        <ToolCards tools={TOOLS.filter(t => t.slug !== tool.slug)} className="lg:grid-cols-3" />
      </Section>

      <Section id="sources" className="pt-0 sm:pt-0">
        <details className="group border-t border-rule pt-6">
          <summary className="flex cursor-pointer list-none items-center gap-2 text-[17px] font-medium tracking-[-0.01em] text-foreground [&::-webkit-details-marker]:hidden">
            <ArrowRight className="size-4 text-faint transition-transform duration-150 group-open:rotate-90" />
            Sources, read {READ}
          </summary>
          <ul className="mt-6 gap-x-10 font-mono text-[12.5px] leading-[1.9] text-muted-foreground md:columns-2">
            {sources(tool).map(url => (
              <li key={url} className="break-inside-avoid">
                <a href={url} className="break-all transition-colors duration-150 hover:text-foreground">
                  {shown(url)}
                </a>
              </li>
            ))}
          </ul>
        </details>
      </Section>

      <TryIt />
    </main>
  );
}
