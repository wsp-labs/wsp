// SPDX-License-Identifier: AGPL-3.0-only
// The git button in the thread header, beside Open: the act the checkout calls
// for on the main part, the rest in its menu, as T3 Code's GitActionsControl.
// A commit here takes every changed file with the message the workspace's own
// agent drafts; where no draft comes back, the Changes pane opens on the
// uncommitted files so the person writes one. View PR opens the Pull request
// pane.
import { ChevronDownIcon, GitCommitHorizontalIcon, GitPullRequestIcon, UploadIcon } from "lucide-react";
import { useState } from "react";
import { isPullRequestNamed } from "@wsp/protocol";
import { Button } from "../components/ui/button.js";
import { Menu, MenuItem, MenuPopup, MenuTrigger } from "../components/ui/menu.js";
import { useDiffStore } from "../diffs/store.js";
import { addNotice, noticeFailure } from "../notices/store.js";
import { useAbsentComputer, useShownThreadId, useStatus, useStore, useWorkspace, useWorkspaceState } from "../protocol/store.js";
import { useCheckoutOf } from "../sidebar/tileCheckout.js";
import { useRightPanelStore } from "../rightPanelStore.js";
import { awayLine, GIT_WORDS, gitMenu, gitQuickAction, type GitAct, type GitMenuItem } from "./gitAction.logic.js";

const ICONS: Record<GitMenuItem["id"], typeof GitCommitHorizontalIcon> = { commit: GitCommitHorizontalIcon, push: UploadIcon, pr: GitPullRequestIcon };

export function GitSplit({ workspaceId }: { workspaceId: string }) {
  const workspace = useWorkspace(workspaceId);
  const status = useStatus(workspaceId);
  const machine = useWorkspaceState(workspaceId);
  const absent = useAbsentComputer(workspaceId);
  const base = useStore(s => s.projects.find(p => p.id === workspace?.project.id)?.base);
  const api = useStore(s => s.api);
  const threadId = useShownThreadId(workspaceId);
  const bringBack = useStore(s => s.bringBack);
  const open = useRightPanelStore(s => s.open);
  const setScope = useDiffStore(s => s.setScope);
  const [busy, setBusy] = useState(false);
  useCheckoutOf(workspaceId);
  // A deleted machine leaves no copy to commit or push from, though the host keeps its last checkout read; an open
  // pull request is read on this computer, so View PR stays.
  const prOpen = isPullRequestNamed(status?.pr) && status.pr.state === "open";
  if (workspace === null || machine === null || machine === "released" || (machine === "gone" && !prOpen) || status?.checkout === undefined || api?.bringBack === undefined) return null;
  const away = absent === null ? awayLine(machine) : absent.start !== undefined ? `${absent.said}; git actions open once they start again` : absent.sentence;
  const state = { checkout: status.checkout, pr: status.pr, base, opensPr: workspace.parentThreadId === undefined, busy, away };
  const quick = gitQuickAction(state);
  const shown = quick.act ?? quick.holds;
  const name = workspace.name;

  const openChanges = (): void => {
    setScope(workspaceId, "head");
    open(workspaceId, "diff");
  };
  const running = async (work: () => Promise<void>): Promise<void> => {
    setBusy(true);
    try {
      await work();
    } catch (e) {
      noticeFailure(e, said => said, { where: name });
    } finally {
      setBusy(false);
    }
  };
  const commitAll = async (): Promise<boolean> => {
    const draft = await api.commitDraft?.(workspaceId, undefined, threadId);
    if (draft?.message == null) {
      if (draft?.note !== undefined) addNotice({ kind: "note", text: draft.note, where: name });
      openChanges();
      return false;
    }
    await api.commit?.(workspaceId, draft.message);
    return true;
  };
  const run = (act: GitAct): void => {
    if (act === "view-pr") return open(workspaceId, "pr");
    void running(async () => {
      if ((act === "commit-push-pr" || act === "commit-push") && !(await commitAll())) return;
      await bringBack(workspaceId);
    });
  };
  const pick = (item: GitMenuItem): void => (item.act === "changes" ? openChanges() : run(item.act));

  return (
    <div data-git-split className="flex shrink-0 items-center">
      <Button
        variant="outline"
        data-git-quick={quick.act ?? "held"}
        disabled={quick.act === null}
        title={quick.hint ?? quick.label}
        onClick={() => quick.act !== null && run(quick.act)}
        className="rounded-e-none [-webkit-app-region:no-drag]"
      >
        {shown === "view-pr" || shown === "create-pr" ? <GitPullRequestIcon /> : shown === "push" ? <UploadIcon /> : <GitCommitHorizontalIcon />}
        <span className="max-sm:sr-only">{quick.label}</span>
      </Button>
      <Menu>
        <MenuTrigger render={<Button variant="outline" data-git-more aria-label={GIT_WORDS.menu} className="-ms-px rounded-s-none px-2 [-webkit-app-region:no-drag]" />}>
          <ChevronDownIcon />
        </MenuTrigger>
        <MenuPopup align="end" data-git-menu>
          {gitMenu(state).map(item => {
            const Icon = ICONS[item.id];
            return (
              <MenuItem key={item.id} data-git-item={item.id} disabled={item.disabled} onClick={() => pick(item)}>
                <Icon />
                {item.label}
              </MenuItem>
            );
          })}
        </MenuPopup>
      </Menu>
    </div>
  );
}
