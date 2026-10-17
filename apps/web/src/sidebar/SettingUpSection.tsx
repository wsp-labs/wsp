// SPDX-License-Identifier: AGPL-3.0-only
// The sidebar's Setting up section, pinned to its foot above the corner's
// icons while the threads scroll above it, and gone when nothing is set up:
// one card per computer whose setup is running, waits on the person or
// stopped. Each card is the computer's glyph and name with the steps done of
// all at the right, the step under way as one line, and a thin bar of how far
// it got; a stopped one says so in the failed ink, and one that waits on the
// person in its ink with its mark. The bar moves by a width transition when a
// step ends and nothing animates at rest. A card opens Add a computer on its
// running steps; a cloud's image build stands here the same way and opens Add
// a cloud provider on it.
import { INIT_ROW_STATES, SETUP_WORDS, initBuildRows, initJobBuilding, initStageCount, placeWord, setupWord, type InitJob, type PlaceView } from "@wsp/protocol";
import { StateMark } from "../components/status/StateMark.js";
import { SidebarMenuButton } from "../components/ui/sidebar.js";
import { cn } from "../lib/utils.js";
import { useStore, usePlaces } from "../protocol/store.js";
import { openSetup } from "../settings/add/addFlow.js";
import { openCloudAdd } from "../settings/add/cloudFlow.js";
import { setupCount, setupRows } from "../settings/add/setup.js";
import { ComputerGlyph } from "../settings/ComputerGlyph.js";
import { capitalised } from "../settings/format.js";
import { isProviderPlace, placeName } from "../settings/places.js";
import { SectionRow } from "./SectionRow.js";

type CardTone = "working" | "needs-you" | "failed";

/** The words a computer's setup stands at, read by the protocol's one rule, as the tone its card takes. */
const TONES: Partial<Record<string, CardTone>> = { [SETUP_WORDS.settingUp]: "working", [SETUP_WORDS.needsYou]: "needs-you", [SETUP_WORDS.failed]: "failed" };

/** Every computer the section holds, with its tone and its one line. */
export function settingUp(places: readonly PlaceView[]): { place: PlaceView; tone: CardTone; line: string }[] {
  return places.flatMap(place => {
    if (place.setup === undefined) return [];
    const word = placeWord(place, null);
    const tone = TONES[word.word];
    if (tone === undefined) return [];
    const line = tone === "working" ? capitalised(setupWord(place.setup)) : capitalised(word.sentence ?? word.word);
    return [{ place, tone, line }];
  });
}

/** A cloud's image build as a card: the cloud its job is for, while it builds or once it failed. */
export function cloudBuilding(job: InitJob | null, places: readonly PlaceView[]): { place: PlaceView; tone: CardTone; line: string; done: number; of: number } | undefined {
  const place = job?.place === undefined ? undefined : places.find(p => p.id === job.place!.id && isProviderPlace(p));
  if (job === null || place === undefined || (!initJobBuilding(job.phase) && job.phase !== "failed")) return undefined;
  const rows = initBuildRows(job.rows).rows;
  const { done, total } = initStageCount(rows);
  const failed = job.phase === "failed";
  const line = failed ? SETUP_WORDS.failed : (rows.find(r => r.state === INIT_ROW_STATES.running)?.label ?? SETUP_WORDS.settingUp);
  return { place, tone: failed ? "failed" : "working", line, done, of: total };
}

const INK: Record<CardTone, { line: string; bar: string }> = {
  working: { line: "text-[var(--sidebar-prose)]", bar: "bg-sidebar-foreground" },
  "needs-you": { line: "text-status-input", bar: "bg-status-input" },
  failed: { line: "text-status-failed", bar: "bg-status-failed" },
};

function SetupCard({ place, tone, line, count, onOpen }: { place: PlaceView; tone: CardTone; line: string; count?: { done: number; of: number }; onOpen?: () => void }) {
  const { done, of } = count ?? setupCount(setupRows(place));
  const name = placeName(place);
  return (
    <li data-setup-card={place.id} data-tone={tone} className="min-w-0">
      <SidebarMenuButton data-sidebar-row data-row-id={`setup:${place.id}`} className="h-auto flex-col items-stretch gap-1.5 px-2 py-2 data-[active=true]:font-normal" onClick={onOpen ?? (() => openSetup(place.id))}>
        <span className="flex min-w-0 items-center gap-2">
          <ComputerGlyph place={place} className="size-3.5 text-sidebar-muted-foreground" />
          <span className="min-w-0 flex-1 truncate text-sm leading-[18px] text-sidebar-foreground">{name}</span>
          {tone === "failed" ? (
            <span data-k="setup-count" className="shrink-0 font-mono text-[11px] text-status-failed">
              {SETUP_WORDS.failed}
            </span>
          ) : tone === "needs-you" ? (
            <StateMark state="needs-you" why={line} className="[&_svg]:size-3.5" />
          ) : (
            <span data-k="setup-count" className="shrink-0 font-mono text-[11px] text-sidebar-muted-foreground tabular-nums">
              {done}/{of}
            </span>
          )}
        </span>
        <span className={cn("min-w-0 truncate text-xs leading-4", INK[tone].line)}>{line}</span>
        <span aria-hidden className="block h-0.5 w-full overflow-hidden rounded-full bg-sidebar-border">
          <span data-k="setup-bar" className={cn("block h-full rounded-full transition-[width] duration-200 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none", INK[tone].bar)} style={{ width: `${of === 0 ? 0 : Math.round((done / of) * 100)}%` }} />
        </span>
      </SidebarMenuButton>
    </li>
  );
}

/** The section, folded by its head as every sidebar head is, the fold the sidebar's own to remember. */
export function SettingUpSection({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const places = usePlaces();
  const cards = settingUp(places);
  const cloud = cloudBuilding(useStore(s => s.initJob), places);
  const count = cards.length + (cloud === undefined ? 0 : 1);
  if (count === 0) return null;
  return (
    <ul data-sidebar-foot-setup className="flex w-full min-w-0 flex-col">
      <li data-section="setting-up" className="min-w-0">
        <SectionRow label="Setting up" count={count} collapsed={collapsed} onToggle={onToggle} rowId="section:setting-up" head="setting-up" />
        {/* Three 64 px cards and their gaps: past three the list scrolls under a hard edge, so the threads keep room. */}
        {collapsed ? null : (
          <ul data-k="setup-cards" className="flex max-h-49 min-w-0 flex-col gap-0.5 overflow-y-auto">
            {cards.map(card => (
              <SetupCard key={card.place.id} {...card} />
            ))}
            {cloud === undefined ? null : <SetupCard place={cloud.place} tone={cloud.tone} line={cloud.line} count={{ done: cloud.done, of: cloud.of }} onOpen={() => openCloudAdd(cloud.place.id)} />}
          </ul>
        )}
      </li>
    </ul>
  );
}
