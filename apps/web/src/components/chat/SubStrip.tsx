// SPDX-License-Identifier: AGPL-3.0-only
// The one row over a sub-thread's composer while its machine holds or blocks
// it, in the usage limit strip's grammar: the composer's own frame
// holding a settings row, the state's glyph in its frame, the state as the
// title with the why under it, and in the row's slot its act, or two acts
// standing under the words. A state with nothing to do draws no slot. The thread says each of these here
// and nowhere else.
import { CloudOffIcon, HourglassIcon, MemoryStickIcon, TimerIcon, UnplugIcon, UploadIcon, type LucideIcon } from "lucide-react";
import { SUB_STRIP, cloudSaid, hereName, isSubOomLine, namesPlace, sizeWord, subNextSize, subSizesSold, subPushAgainPrompt, subStartAgainPrompt, type SessionView, type SubStripWords } from "@wsp/protocol";
import { sendToThread, useWorkspaceVerbs } from "../../actions/verbs";
import { useSidebarProjects, useStore } from "../../protocol/store";
import { openComputerSettings, openSettingsGroup } from "../../settings/openAt";
import { useSettingsStore } from "../../settings/settingsStore";
import { minutesWord, subKept } from "../../sidebar/tileCard";
import { useMinuteClockWhen } from "../status/useMinuteClock";
import { cn } from "../../lib/utils";
import { GlyphFrame } from "../../settings/grid";
import { LIST_TITLE, NOTE } from "../../settings/layout";
import { LED_SLOT_INDENT, SLOT_CLASS, SPLIT_CLASS } from "../../settings/rows";
import { Button } from "../ui/button";
import { ComposerSurface } from "./ComposerSurface";

/** One act in the strip's slot; the last is the primary where there are two. */
export interface SubStripAct {
  readonly k: string;
  readonly word: string;
  readonly run: () => void;
}

export function SubStrip({ k, glyph: Glyph, words, acts = [] }: { k: string; glyph: LucideIcon; words: SubStripWords; acts?: readonly SubStripAct[] }) {
  return (
    <div className="relative w-full px-3 sm:px-5">
      <ComposerSurface.Shell data-sub-strip={k} className="mx-auto w-full min-w-0 max-w-3xl">
        <ComposerSurface.Host>
          <ComposerSurface.Main>
            <div className={cn("min-h-15 px-5 py-3", acts.length === 0 ? "flex items-center" : acts.length > 1 ? "flex flex-col gap-3" : SPLIT_CLASS)}>
              <div className="flex min-w-0 items-center gap-3">
                <GlyphFrame>
                  <Glyph aria-hidden className="size-4 text-foreground/80" />
                </GlyphFrame>
                <span className="flex min-w-0 flex-col gap-0.5">
                  <span data-sub-strip-title className={LIST_TITLE}>
                    {words.title}
                  </span>
                  <span data-sub-strip-note className={cn(NOTE, "break-words")}>
                    {words.note}
                  </span>
                </span>
              </div>
              {acts.length === 0 ? null : (
                <div data-sub-strip-slot className={acts.length > 1 ? "flex flex-wrap items-center gap-2 pl-11" : cn(SLOT_CLASS, LED_SLOT_INDENT)}>
                  {acts.map(act => (
                    <Button key={act.k} size="xs" variant="outline" data-sub-strip-act={act.k} onClick={act.run}>
                      {act.word}
                    </Button>
                  ))}
                </div>
              )}
            </div>
          </ComposerSurface.Main>
        </ComposerSurface.Host>
      </ComposerSurface.Shell>
    </div>
  );
}


/** The strip a sub-thread's latest row calls for, or nothing while it runs or rests with nothing holding it. */
export function SubThreadStrip({ workspaceId, row }: { workspaceId: string; row: SessionView }) {
  const places = useStore(s => s.places);
  const lead = useSidebarProjects()
    .flatMap(p => p.threads)
    .find(t => t.threadId !== null && t.threadId === row.parentThreadId);
  const api = useStore(s => s.api);
  const verbs = useWorkspaceVerbs();
  const sub = row.sub;
  const now = useMinuteClockWhen(sub?.deleteAt !== undefined);
  if (sub === undefined) return null;
  const on = cloudSaid(sub.on);
  const send = (prompt: string): void => sendToThread(api, { workspaceId, threadId: row.threadId ?? null, harness: row.harness, title: row.harnessTitle ?? sub.machine }, prompt);
  const capped = row.capped;
  if (row.status === "running" && capped !== undefined)
    return capped.why === "minute" ? (
      <SubStrip k="waiting-minute" glyph={HourglassIcon} words={SUB_STRIP.waitingMinute(on, capped.running, capped.atOnce)} />
    ) : (
      <SubStrip k="waiting-machines" glyph={HourglassIcon} words={SUB_STRIP.waitingSlot(on, capped.running, capped.atOnce)} acts={[{ k: "raise", word: SUB_STRIP.raise, run: () => openComputerSettings(capped.placeId) }]} />
    );
  if (row.status === "running") return null;
  if (sub.cloudGone === true)
    return (
      <SubStrip
        k="forgotten"
        glyph={UnplugIcon}
        words={SUB_STRIP.cloudGone(on, hereName(places))}
        acts={[
          {
            k: "add-again",
            word: SUB_STRIP.addAgain(on),
            run: () => {
              openSettingsGroup("computers");
              useSettingsStore.getState().askAdd("cloud");
            },
          },
        ]}
      />
    );
  if (row.status === "failed" && row.failure !== undefined && isSubOomLine(row.failure)) {
    // The same machine would be killed again, so the lead, which starts sub-threads, is asked to start it at the size.
    const cloud = places.find(p => namesPlace(p, sub.on));
    const next = cloud === undefined ? undefined : subNextSize(sub.size, subSizesSold(cloud.sizes ?? [], cloud.plan));
    const bigger = next === undefined ? undefined : sizeWord(next);
    const startAgain = bigger === undefined || lead === undefined ? undefined : (): void => sendToThread(api, lead, subStartAgainPrompt(row.harnessTitle ?? sub.machine, on, bigger));
    return <SubStrip k="oom" glyph={MemoryStickIcon} words={SUB_STRIP.oom(row.failure)} acts={startAgain === undefined ? [] : [{ k: "start-again", word: SUB_STRIP.startAgain(bigger!), run: startAgain }]} />;
  }
  // A restart that failed is said by the start line alone, with its Try again.
  if (sub.state === "released" && sub.spawn?.failed !== undefined) return null;
  if (sub.state === "released") return <SubStrip k="released" glyph={CloudOffIcon} words={SUB_STRIP.released(on)} />;
  if (sub.held !== undefined)
    return (
      <SubStrip
        k="not-pushed"
        glyph={UploadIcon}
        words={SUB_STRIP.notPushed(sub.held, sub.branch, sub.result?.pushRefused, subKept(sub, row.diskKeptUntil))}
        acts={[
          { k: "copy-home", word: SUB_STRIP.copyTo(hereName(places)), run: () => void verbs.exportProject?.(workspaceId) },
          { k: "push-again", word: SUB_STRIP.pushAgain, run: () => send(subPushAgainPrompt(sub.branch)) },
        ]}
      />
    );
  if (sub.deleteAt !== undefined) return <SubStrip k="deleting" glyph={TimerIcon} words={SUB_STRIP.deleting(minutesWord(sub.deleteAt - now), sub.branch)} />;
  return null;
}
