// SPDX-License-Identifier: AGPL-3.0-only
// What a person sets on one of their own computers, off the row the host keeps for it: the older wsp it runs and
// the update that brings it level, threads at once, how long a quiet workspace waits before it naps, how long one
// turn may run, whether its threads may open threads, and its name and ssh login. Every set goes through places.set
// and the row comes back as the host now reads it; a row off its default carries the arrow that takes it back.
import { useState } from "react";
import { MinusIcon, PlusIcon } from "lucide-react";
import { HERE_PLACE_ID, NAP_AFTER_MAX_MS, TURN_LIMIT_MAX_MS, fmtMemGb, fmtRate, isProviderPlace, placeSettingNamed, sizeFromWord, sizeNameOf, sizeWord, subSizesSold, type PlaceSettingWord, type PlaceSettingsAsk, type PlaceView } from "@wsp/protocol";
import { Button } from "../components/ui/button.js";
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupText } from "../components/ui/input-group.js";
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from "../components/ui/select.js";
import { Switch } from "../components/ui/switch.js";
import { noticeFailure } from "../notices/store.js";
import { useStore } from "../protocol/store.js";
import { COMPUTER_PAGE_WORDS as W, SETTINGS_WORDS } from "./format.js";
import { ROW_FIELD, SELECT_WIDTH } from "./layout.js";
import { cn } from "../lib/utils.js";
import { hereName, placeName } from "./places.js";
import { Card, Row } from "./rows.js";
import { ChangeButton, FieldSheet, sheetWrite, type Written } from "./sheetParts.js";
import type { SettingsContext } from "./settingsContext.js";

const NAP_CHOICES: ReadonlyArray<number | null> = [10, 20, 30, 60, 120, 180].map(m => m * 60_000).filter(ms => ms <= NAP_AFTER_MAX_MS).concat([null] as never);
const TURN_LIMIT_CHOICES: ReadonlyArray<number | null> = [1, 2, 4, 6, 8, 12, 24].map(h => h * 3_600_000).filter(ms => ms <= TURN_LIMIT_MAX_MS).concat([null] as never);
const NEVER = "never";
/** A finished sub-thread's machine goes at once or after one of these; null stands for at once in the select. */
const DELETE_CHOICES: ReadonlyArray<number | null> = [null, ...[10, 30, 60, 120, 180].map(m => m * 60_000)];

const setOn = (place: PlaceView, ask: PlaceSettingsAsk): Promise<void> => useStore.getState().setPlace(place.id, ask).catch(noticeFailure);
const resetOn = (place: PlaceView, word: PlaceSettingWord): (() => void) => () => void useStore.getState().setPlace(place.id, {}, [word]).catch(noticeFailure);

/** The row under a computer's head when it runs an older daemon than this wsp deploys: a button where the host can
 * run the fix there itself, the line to run where only the person can. */
export function BehindRow({ place, ctx }: { place: PlaceView; ctx: SettingsContext }) {
  const update = useStore(s => s.updatePlace);
  const [busy, setBusy] = useState(false);
  const behind = place.behind;
  if (behind === undefined) return null;
  const name = placeName(place);
  const here = place.id === HERE_PLACE_ID;
  const press = (): void => {
    setBusy(true);
    void update(place.id).finally(() => setBusy(false));
  };
  return (
    <Row
      id="behind"
      title={here ? W.behindHereTitle : W.behindTitle(hereName(ctx.places))}
      tone="warning"
      description={behind.act === "update" ? W.behindUpdate(name, hereName(ctx.places)) : W.behindInstall(behind.fix)}
      {...(behind.act === "update"
        ? {
            control: (
              <Button variant="outline" size="xs" data-k="update-wsp" disabled={busy} onClick={press}>
                {busy ? W.updating : W.update(name)}
              </Button>
            ),
          }
        : {})}
      attrs={{ "data-k": "computer-behind" }}
    />
  );
}

/** A setting's number with a step either side, at least one. While a set is on its way the host's row still holds
 * the number before it, so the number shown and the next step go off the last one sent. */
function Stepper({ k, value, label, max, onChange }: { k: string; value: number; label: string; max?: number | undefined; onChange: (next: number) => Promise<void> }) {
  const [sent, setSent] = useState<number | null>(null);
  const shown = sent ?? value;
  const step = (next: number): void => {
    setSent(next);
    void onChange(next).finally(() => setSent(last => (last === next ? null : last)));
  };
  return (
    <span data-k={k} role="group" aria-label={label} className="inline-flex h-[30px] items-center rounded-[7px] border border-border">
      <Button variant="ghost" size="icon-xs" aria-label={W.fewer} disabled={shown <= 1} onClick={() => step(shown - 1)} className="h-full rounded-r-none">
        <MinusIcon aria-hidden className="size-3.5" />
      </Button>
      <span data-k={`${k}-value`} className="min-w-8 px-1 text-center text-[13px] tabular-nums text-foreground">
        {shown}
      </span>
      <Button variant="ghost" size="icon-xs" aria-label={W.more} disabled={max !== undefined && shown >= max} onClick={() => step(shown + 1)} className="h-full rounded-l-none">
        <PlusIcon aria-hidden className="size-3.5" />
      </Button>
    </span>
  );
}

/** A stretch of time in words a select reads, 20 minutes or 1 hour, and the word for none. */
function spanWord(ms: number | null, none: string): string {
  if (ms === null) return none;
  const minutes = Math.round(ms / 60_000);
  if (minutes % 60 !== 0) return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
  const hours = minutes / 60;
  return `${hours} ${hours === 1 ? "hour" : "hours"}`;
}

/** The choices a select offers: its own, with a value set some other way standing among them in order. */
const withValue = (choices: ReadonlyArray<number | null>, now: number | null): ReadonlyArray<number | null> =>
  choices.includes(now) ? choices : [...choices.filter(ms => ms !== null), now, null].sort((a, b) => (a ?? Infinity) - (b ?? Infinity));

/** A select over stretches of time, none among them, which sets the one it is handed. */
function SpanSelect({ k, label, value, choices, none, onChange }: { k: string; label: string; value: number | null; choices: ReadonlyArray<number | null>; none: string; onChange: (ms: number | null) => void }) {
  return (
    <Select value={value === null ? NEVER : String(value)} onValueChange={v => onChange(v === NEVER ? null : Number(v))}>
      <SelectTrigger size="sm" aria-label={label} data-k={k} className={SELECT_WIDTH}>
        <SelectValue>{(v: string) => spanWord(v === NEVER ? null : Number(v), none)}</SelectValue>
      </SelectTrigger>
      <SelectPopup>
        {withValue(choices, value).map(ms => (
          <SelectItem key={ms ?? NEVER} value={ms === null ? NEVER : String(ms)}>
            {spanWord(ms, none)}
          </SelectItem>
        ))}
      </SelectPopup>
    </Select>
  );
}

/** A dollar figure typed into a row, saved on Enter or when the field is left, and put back where it reads no figure. */
function DollarsField({ k, label, value, onChange }: { k: string; label: string; value: number; onChange: (usd: number) => void }) {
  const [typed, setTyped] = useState<string | null>(null);
  const save = (): void => {
    if (typed === null) return;
    const usd = Number(typed);
    setTyped(null);
    if (typed.trim() !== "" && Number.isFinite(usd) && usd >= 0 && usd !== value) onChange(usd);
  };
  return (
    <InputGroup className={cn(SELECT_WIDTH, ROW_FIELD)}>
      <InputGroupAddon>
        <InputGroupText>$</InputGroupText>
      </InputGroupAddon>
      <InputGroupInput data-k={k} aria-label={label} inputMode="decimal" value={typed ?? String(value)} className="tabular-nums" onChange={e => setTyped(e.target.value)} onBlur={save} onKeyDown={e => e.key === "Enter" && save()} />
    </InputGroup>
  );
}

/** The sizes a cloud's plan sells a sub-thread, by class name, figures and rate. */
function SizeSelect({ place }: { place: PlaceView }) {
  const sold = subSizesSold(place.sizes ?? [], place.plan);
  const now = place.size === undefined ? "" : sizeWord(place.size);
  // The trigger is a select's width, so it leaves the rate to the list.
  const word = (key: string, rated = true): string => {
    const offer = sold.find(s => sizeWord(s) === key);
    const size = offer ?? sizeFromWord(key);
    if (size === undefined) return key;
    return W.sizeChoice(sizeNameOf(size), size.cpu, fmtMemGb(size.memMb), offer === undefined || !rated ? undefined : fmtRate(offer.rateUsdPerHour));
  };
  return (
    <Select value={now} onValueChange={v => void setOn(place, { size: sizeFromWord(String(v))! })}>
      <SelectTrigger size="sm" aria-label={W.sizeTitle} data-k="sub-size" className={SELECT_WIDTH}>
        <SelectValue>{(v: string) => word(v, false)}</SelectValue>
      </SelectTrigger>
      <SelectPopup>
        {sold.map(s => (
          <SelectItem key={sizeWord(s)} value={sizeWord(s)}>
            {word(sizeWord(s))}
          </SelectItem>
        ))}
      </SelectPopup>
    </Select>
  );
}

/** A cloud's own limits, ahead of the nap and the turn limit: machines at once against the plan's, spend per day,
 * the size a sub-thread gets, and how long a finished sub-thread's machine stays. */
function CloudLimitRows({ place }: { place: PlaceView }) {
  const name = placeName(place);
  const cap = place.cap !== undefined && "machines" in place.cap ? place.cap : undefined;
  const set = place.settings ?? {};
  if (cap === undefined) return null;
  const planCap = place.plan?.machinesAtOnce;
  const deleteAfter = place.deleteAfterMs ?? null;
  return (
    <>
      <Row
        id="machines-at-once"
        title={W.machinesTitle}
        description={W.machinesLine(name, planCap)}
        control={<Stepper k="machines-at-once" value={cap.machines} label={W.machinesTitle} max={planCap} onChange={n => setOn(place, { machines: n })} />}
        {...(set.machines === undefined ? {} : { reset: resetOn(place, "machines") })}
      />
      <Row
        id="spend-per-day"
        title={W.spendTitle}
        description={W.spendLine(name)}
        control={<DollarsField k="spend-per-day" label={W.spendTitle} value={cap.spendPerDayUsd} onChange={usd => void setOn(place, { spendPerDayUsd: usd })} />}
        {...(set.spendPerDayUsd === undefined ? {} : { reset: resetOn(place, "spend") })}
      />
      {place.size === undefined ? null : (
        <Row id="sub-size" title={W.sizeTitle} description={W.sizeLine} control={<SizeSelect place={place} />} {...(set.size === undefined ? {} : { reset: resetOn(place, "size") })} />
      )}
      {place.deleteAfterMs === undefined ? null : (
        <Row
          id="delete-after"
          title={W.deleteTitle}
          description={W.deleteLine}
          control={<SpanSelect k="delete-after" label={W.deleteTitle} value={deleteAfter === 0 ? null : deleteAfter} choices={DELETE_CHOICES} none={W.deleteAtOnce} onChange={ms => void setOn(place, { deleteAfterMs: ms ?? 0 })} />}
          {...(set.deleteAfterMs === undefined ? {} : { reset: resetOn(place, "delete-after") })}
        />
      )}
    </>
  );
}

/** Limits: threads at once on any computer that has said its shape, the nap window wherever workspaces nap, and the
 * turn limit wherever the host says one. */
export function LimitsCard({ place }: { place: PlaceView }) {
  const name = placeName(place);
  const threads = place.cap !== undefined && "threads" in place.cap ? place.cap.threads : undefined;
  const fallback = place.capDefault !== undefined && "threads" in place.capDefault ? place.capDefault.threads : undefined;
  const set = place.settings ?? {};
  const naps = place.napMs !== undefined;
  const limited = place.turnLimitMs !== undefined;
  const cloud = isProviderPlace(place);
  if (threads === undefined && !naps && !limited && !cloud) return null;
  return (
    <Card id="computer-limits" head={W.limits}>
      {cloud ? <CloudLimitRows place={place} /> : null}
      {threads === undefined ? null : (
        <Row
          id="threads-at-once"
          title={W.threadsAtOnce}
          description={fallback === undefined || place.shape === undefined ? W.threadsLineBare : W.threadsLine(fallback, name, fmtMemGb(place.shape.memMb))}
          control={<Stepper k="threads-at-once" value={threads} label={W.threadsAtOnce} onChange={n => setOn(place, { threads: n })} />}
          {...(set.threads === undefined ? {} : { reset: resetOn(place, "threads") })}
        />
      )}
      {!naps ? null : (
        <Row
          id="nap-after"
          title={W.napTitle}
          description={W.napLine}
          control={<SpanSelect k="nap-after" label={W.napTitle} value={place.napMs ?? null} choices={NAP_CHOICES} none={W.napNever} onChange={ms => void setOn(place, { napMs: ms })} />}
          {...(set.napMs === undefined ? {} : { reset: resetOn(place, "nap") })}
        />
      )}
      {!limited ? null : (
        <Row
          id="turn-limit"
          title={W.turnLimitTitle}
          description={W.turnLimitLine}
          control={<SpanSelect k="turn-limit" label={W.turnLimitTitle} value={place.turnLimitMs ?? null} choices={TURN_LIMIT_CHOICES} none={W.turnLimitOff} onChange={ms => void setOn(place, { turnLimitMs: ms })} />}
          {...(set.turnLimitMs === undefined ? {} : { reset: resetOn(place, "turn-limit") })}
        />
      )}
    </Card>
  );
}

/** Threads here: how many levels deep a thread on this computer may open threads of its own. */
export function SpawnCard({ place }: { place: PlaceView }) {
  const spawn = place.spawn;
  if (spawn === undefined) return null;
  const set = place.settings ?? {};
  return (
    <Card id="computer-spawn" head={W.threadsHere}>
      <Row
        id="levels-deep"
        title={W.levelsTitle}
        description={W.levelsLine}
        control={<Stepper k="levels-deep" value={spawn.maxDepth} label={W.levelsTitle} onChange={n => setOn(place, { spawn: { maxDepth: n } })} />}
        {...(placeSettingNamed(set, "max-depth") ? { reset: resetOn(place, "max-depth") } : {})}
      />
    </Card>
  );
}

/** What a computer the person added is called and the ssh login the host reaches it by, each changed in a sheet
 * through places.set. A new login is saved only once the host reads this same computer over it, so a refusal keeps
 * the old one and stands under the field in the host's words. */
export function NameCard({ place }: { place: PlaceView }) {
  const placesSet = useStore(s => s.api?.placesSet);
  const [sheet, setSheet] = useState<"name" | "ssh" | null>(null);
  if (place.kind !== "computer" || place.id === HERE_PLACE_ID) return null;
  const name = placeName(place);
  const ssh = place.road?.ssh;
  const close = (): void => setSheet(null);
  const write = (field: "name" | "ssh", value: string): Promise<Written> =>
    value === (field === "name" ? place.name : ssh) ? Promise.resolve(null) : sheetWrite(useStore.getState().setPlace(place.id, { [field]: value }));
  return (
    <>
      <Card id="computer-name-login">
        <Row id="computer-name" title={W.nameTitle} description={name} control={<ChangeButton k="computer-name-change" word={SETTINGS_WORDS.change} held={placesSet === undefined} onClick={() => setSheet("name")} />} />
        <Row id="computer-ssh" title={W.sshTitle} description={ssh ?? W.sshNone} mono={ssh !== undefined} control={<ChangeButton k="computer-ssh-change" word={SETTINGS_WORDS.change} held={placesSet === undefined} onClick={() => setSheet("ssh")} />} />
      </Card>
      {sheet === "name" ? <FieldSheet k="computer-name" title={W.nameTitle} line={W.nameSheet(name)} initial={place.name} placeholder={place.name} mono={false} required save={value => write("name", value)} onClose={close} /> : null}
      {sheet === "ssh" ? <FieldSheet k="computer-ssh" title={W.sshTitle} line={W.sshSheet(name)} initial={ssh ?? ""} placeholder="user@host" required checking={login => W.sshChecking(login, name)} save={value => write("ssh", value)} onClose={close} /> : null}
    </>
  );
}
