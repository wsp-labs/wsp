// SPDX-License-Identifier: AGPL-3.0-only
// A turn its computer's threads at once holds back, in the composer's place, as
// the usage limit's bar stands: the computer it waits on as the title, what runs
// there and how the wait ends under it, then the setting that lets it start now.
// The bar goes with the hold, as the drawer's row does.
import { HourglassIcon } from "lucide-react";
import { capBarWords, capRaiseAct, type ThreadCapWait } from "@wsp/protocol";
import { cn } from "../../../lib/utils";
import { NOTE } from "../../../settings/layout";
import { openComputerSettings } from "../../../settings/openAt";
import { Button } from "../../ui/button";
import { DRAWER_WORDS } from "../ComposerDrawer";
import { Dock, DockBack } from "../Dock";
import { foldBar, useBarRoot } from "../composerBar";

export function CapBar({ wait, threadKey, workspaceId }: { wait: ThreadCapWait; threadKey: string; workspaceId: string }) {
  const { title, note } = capBarWords(wait);
  const raise = capRaiseAct(wait);
  const barRoot = useBarRoot(threadKey, workspaceId);
  return (
    <Dock
      {...barRoot}
      shell={{ "data-composer-bar": "capped" }}
      mark={<HourglassIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />}
      title={<span data-cap-title>{title}</span>}
      under={
        <p data-cap-note className={cn(NOTE, "break-words")}>
          {note}
        </p>
      }
      back={<DockBack word={DRAWER_WORDS.write} onClick={() => foldBar(threadKey, workspaceId)} />}
      acts={
        raise === undefined ? null : (
          <Button data-cap-act onClick={() => openComputerSettings(wait.placeId)}>
            {raise}
          </Button>
        )
      }
    />
  );
}
