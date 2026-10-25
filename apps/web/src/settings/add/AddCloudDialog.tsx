// SPDX-License-Identifier: AGPL-3.0-only
// Add a cloud provider as one dialog, Add a computer's own: its width, head,
// body of cards and foot. The key step says where the key is made, what it
// needs and what the machines cost, and checks it with the provider before
// Continue saves it.
import { CheckIcon, CircleAlertIcon, CloudIcon, ExternalLinkIcon, KeyRoundIcon } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
import { PROVIDER_KEY_WORDS } from "@wsp/protocol";
import { Button } from "../../components/ui/button.js";
import { Dialog, DialogDescription, DialogHeader, DialogPanel, DialogPopup, DialogTitle } from "../../components/ui/dialog.js";
import { Input } from "../../components/ui/input.js";
import { RadioGroup } from "../../components/ui/radio-group.js";
import { Spinner } from "../../components/ui/spinner.js";
import { keyTakenAt } from "../../keyOwners.js";
import { cn } from "../../lib/utils.js";
import { failureOf, type Failure } from "../../protocol/failure.js";
import { useStore } from "../../protocol/store.js";
import { FACT } from "../format.js";
import { CARD_INSET, GLYPH, NOTE } from "../layout.js";
import { INPUT } from "../ProviderKey.js";
import { Card, Row } from "../rows.js";
import { cloudsOffered } from "../providers.js";
import { useSettingsStore } from "../settingsStore.js";
import { CopyRow, RefusalSlot } from "../sheetParts.js";
import { CLOUD_KEY_STEPS } from "./cloudKey.js";
import { checkKey, closeCloudAdd, goCloud, keyWorksLine, useCloudFlow, type CloudStep, type KeyCheck } from "./cloudFlow.js";
import { Choice } from "./PickLists.js";
import { STEP_BODY, STEP_FRAME, STEP_HEAD, StepFoot } from "./StepDialog.js";

/** The key check's line under the field: nothing while typing, the spinner while it asks, the check where it works,
 * the alert where it was refused, and Try again where the provider never answered. */
function CheckLine({ check, name, onAgain }: { check: KeyCheck; name: string; onAgain: () => void }) {
  if (check.state === "idle") return null;
  const mark = check.state === "checking" ? <Spinner className="size-3.5 text-muted-foreground" /> : check.state === "works" ? <CheckIcon aria-hidden className="size-3.5 text-foreground/80" /> : <CircleAlertIcon aria-hidden className="size-3.5 text-destructive-foreground" />;
  const words = check.state === "checking" ? `Checking the key with ${name}` : check.state === "works" ? keyWorksLine(check.facts) : check.failure.said;
  return (
    <div data-k="key-check" data-state={check.state} className="flex items-start gap-2">
      <span className="flex h-5 w-4 shrink-0 items-center justify-center">{mark}</span>
      <span className="flex min-w-0 flex-1 flex-col items-start gap-2">
        <span className={cn(NOTE, (check.state === "refused" || check.state === "unreachable") && "text-destructive-foreground")}>{words}</span>
        {check.state === "unreachable" ? (
          <Button size="xs" variant="outline" data-k="check-again" onClick={onAgain}>
            Try again
          </Button>
        ) : null}
      </span>
    </div>
  );
}

/** The provider step, where the host offers more than one: each by its name and its cheapest machine. */
function ProviderStep({ offered, picked, onPick }: { offered: readonly string[]; picked: string; onPick: (id: string) => void }) {
  return (
    <RadioGroup value={picked} onValueChange={next => onPick(String(next))} className="gap-0">
      <Card id="providers">
        {offered.map(id => {
          const from = CLOUD_KEY_STEPS[id]?.from;
          return <Choice key={id} id={id} picked={picked === id} glyph={<CloudIcon aria-hidden className={GLYPH} />} name={PROVIDER_KEY_WORDS[id]?.name ?? id} {...(from === undefined ? {} : { fact: from })} />;
        })}
      </Card>
    </RadioGroup>
  );
}

/** The key step: how to make the key, the field, and the check under it. */
function KeyStep({ provider, name, held, keyText, onKey, check, refused }: { provider: string; name: string; held: boolean; keyText: string; onKey: (next: string, now: boolean) => void; check: KeyCheck; refused: Failure | null }) {
  const facts = CLOUD_KEY_STEPS[provider];
  return (
    <>
      {facts === undefined ? null : (
        <Card id="make-key" head="Make a key">
          <Row
            id="keys-page"
            title={`Open ${name}'s API keys page`}
            description={facts.keysPage}
            mono
            control={
              <Button size="xs" variant="outline" data-k="open-keys-page" onClick={() => void window.open(`https://${facts.keysPage}`, "_blank", "noopener,noreferrer")}>
                <ExternalLinkIcon aria-hidden className="size-3.5" />
                Open
              </Button>
            }
          />
          <div data-k="key-kind" className="flex flex-col">
            <Row id="admin-key" title="Make an admin key" description={`Pick the admin preset. Or make a new key with the ${facts.preset} preset, which holds every action wsp needs:`} />
            <div className={cn(CARD_INSET, "-mt-1 pb-4")}>
              <CopyRow k="key-actions" value={facts.actions.join(", ")} />
            </div>
          </div>
          <Row id="key-cost" title="What it costs" description={facts.cost} word={facts.from} wordClass="fact" />
        </Card>
      )}
      <Card id="key" head="Key">
        <div className={cn(CARD_INSET, "flex flex-col gap-2 py-4")}>
          <span className="relative block min-w-0">
            <KeyRoundIcon aria-hidden className="pointer-events-none absolute top-1/2 left-3 z-10 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              data-k="cloud-key"
              nativeInput
              type="password"
              autoFocus
              autoComplete="off"
              spellCheck={false}
              value={keyText}
              placeholder={held ? "Paste a new key to replace the saved one" : (PROVIDER_KEY_WORDS[provider]?.keyName ?? "API key")}
              aria-label={PROVIDER_KEY_WORDS[provider]?.keyName ?? "API key"}
              {...(check.state === "refused" ? { "aria-invalid": true } : {})}
              onChange={e => onKey(e.target.value, (e.nativeEvent as InputEvent).inputType === "insertFromPaste")}
              onKeyDown={e => (e.key === "Enter" ? onKey(keyText, true) : undefined)}
              className={INPUT}
            />
          </span>
          {held && keyText === "" ? (
            <div data-k="key-check" data-state="held" className="flex items-start gap-2">
              <span className="flex h-5 w-4 shrink-0 items-center justify-center">
                <CheckIcon aria-hidden className="size-3.5 text-foreground/80" />
              </span>
              <span className={NOTE}>A key is saved. Continue uses it.</span>
            </div>
          ) : (
            <CheckLine check={check} name={name} onAgain={() => onKey(keyText, true)} />
          )}
        </div>
      </Card>
      {refused === null ? null : <RefusalSlot k="key-refused" said={refused.said} {...(refused.fix === undefined ? {} : { fix: refused.fix })} />}
    </>
  );
}

export function AddCloudDialog() {
  const flow = useCloudFlow();
  const api = useStore(s => s.api);
  const saveKeys = useStore(s => s.saveKeys);
  const held = useSettingsStore(s => s.reads.setup?.keys[flow.provider] === true);
  const setup = useSettingsStore(s => s.reads.setup);
  const [keyText, setKeyText] = useState("");
  const keyNow = useRef("");
  const [refused, setRefused] = useState<Failure | null>(null);
  const [busy, setBusy] = useState(false);
  const cloud = PROVIDER_KEY_WORDS[flow.provider]?.name ?? flow.provider;

  if (!flow.open) return null;

  const offered = cloudsOffered(setup);
  const close = (): void => {
    setKeyText("");
    keyNow.current = "";
    closeCloudAdd();
  };

  const onKey = (next: string, now: boolean): void => {
    setKeyText(next);
    keyNow.current = next;
    setRefused(null);
    if (now) void checkKey(api, flow.provider, next, () => keyNow.current);
    else useCloudFlow.setState({ check: { state: "idle" } });
  };

  /** Continue from the key: the key saved where one was typed, then the dialog closes on the cloud added. */
  const keyOn = async (): Promise<void> => {
    setBusy(true);
    setRefused(null);
    try {
      if (keyText.trim() !== "") await saveKeys({ provider: flow.provider, key: keyText.trim() });
      close();
    } catch (e) {
      setRefused(failureOf(e));
    } finally {
      setBusy(false);
    }
  };

  const keyReady = keyText.trim() === "" ? held : flow.check.state === "works";
  const steps: CloudStep[] = offered.length > 1 ? ["provider", "key"] : ["key"];
  const head = flow.step === "provider" ? { title: "Add a cloud provider", line: "Where your sub-threads run." } : { title: `Add ${cloud}`, line: `wsp runs your sub-threads on your ${cloud} account with this key.` };

  const body = (): ReactNode =>
    flow.step === "provider" ? (
      <ProviderStep offered={offered} picked={flow.provider} onPick={id => useCloudFlow.setState({ provider: id, check: { state: "idle" } })} />
    ) : (
      <KeyStep provider={flow.provider} name={cloud} held={held} keyText={keyText} onKey={onKey} check={flow.check} refused={refused} />
    );

  const cancel = (
    <Button variant="outline" data-k="cancel" onClick={close}>
      Cancel
    </Button>
  );
  const foot: ReactNode =
    flow.step === "provider" ? (
      <>
        {cancel}
        <Button data-k="continue" onClick={() => goCloud("key")}>
          Continue
        </Button>
      </>
    ) : (
      <>
        {offered.length > 1 ? (
          <Button variant="outline" data-k="back" onClick={() => goCloud("provider")}>
            Back
          </Button>
        ) : (
          cancel
        )}
        <Button data-k="continue" held={!keyReady || busy} onClick={() => void keyOn()}>
          Continue
        </Button>
      </>
    );

  return (
    <Dialog open onOpenChange={(open, how) => (open || (how.reason === "escape-key" && keyTakenAt(how.event.target, "Escape")) ? undefined : close())}>
      <DialogPopup data-add-cloud={flow.step} initialFocus={flow.step === "key" ? undefined : false} className={STEP_FRAME}>
        <div className="flex min-h-0 flex-1 flex-col">
          <DialogHeader className={STEP_HEAD}>
            <div className="flex min-w-0 flex-col gap-1">
              <DialogTitle>{head.title}</DialogTitle>
              <DialogDescription>{head.line}</DialogDescription>
            </div>
            <span data-k="progress" className={cn(FACT, "shrink-0 pt-0.5")}>
              {`${steps.indexOf(flow.step) + 1} of ${steps.length}`}
            </span>
          </DialogHeader>
          <DialogPanel className={STEP_BODY}>{body()}</DialogPanel>
          <StepFoot left={null}>{foot}</StepFoot>
        </div>
      </DialogPopup>
    </Dialog>
  );
}
