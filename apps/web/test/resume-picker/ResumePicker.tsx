// SPDX-License-Identifier: AGPL-3.0-only
// Design round for #2054, never shipped: the Resume picker composed from the palette's own pieces (its dialog, input,
// panel, rows, group labels and footer), in two variants. "plan" is the ticket's plan as written: the palette's 576 px
// dialog, three rows of scope chips drawn as the app's segmented controls, a mono second line with a message count, and
// the preview at the right. "rec" is the recommendation: a wider dialog so the preview has room, the scopes as three
// quiet menu triggers in one line, no message count. Enter or a click picks the highlighted row, ⇧Enter picks it on a
// copy, Esc closes.
import { useMemo, useState } from "react";
import { ArrowDownIcon, ArrowUpIcon, ChevronDownIcon } from "lucide-react";
import { agentName } from "@wsp/catalog";
import { fmtBytes, toolActivityLine, toolDoneLine, type ConversationsHeld } from "@wsp/protocol";
import { HarnessMark } from "../../src/components/chat/HarnessMark";
import { Facts } from "../../src/components/Facts";
import { Button } from "../../src/components/ui/button";
import { Command, CommandCollection, CommandDialog, CommandDialogPopup, CommandFooter, CommandGroup, CommandGroupLabel, CommandInput, CommandItem, CommandList, CommandPanel } from "../../src/components/ui/command";
import { Kbd, KbdGroup } from "../../src/components/ui/kbd";
import { Menu, MenuPopup, MenuRadioGroup, MenuRadioItem, MenuTrigger } from "../../src/components/ui/menu";
import { SegmentedControl } from "../../src/components/ui/segmented-control";
import { Spinner } from "../../src/components/ui/spinner";
import { LINE_SLOT_CLASS } from "../../src/components/status/ThreadStatus";
import { cn } from "../../src/lib/utils";
import { compactTimeLabel } from "../../src/sidebar/workspaceRows";
import { previewOf, type PickRow } from "./rows";

export type Variant = "plan" | "rec";
type AgentScope = "all" | "claude" | "codex";
type BranchScope = "all" | "this";
type ProjectScope = "this" | "all";

export interface PickerProject {
  readonly id: string;
  readonly name: string;
  readonly root: string;
  /** The computer it is on, where that is not the one the picker opened on. */
  readonly on?: string;
}

export interface PickerProps {
  readonly variant: Variant;
  readonly rows: ReadonlyArray<PickRow>;
  readonly projects: ReadonlyArray<PickerProject>;
  readonly project: string;
  readonly thisBranch: string;
  readonly projectScope?: ProjectScope;
  readonly held?: ReadonlyArray<ConversationsHeld>;
  readonly reading?: boolean;
  /** The row whose preview is still being read, drawn with the spinner. */
  readonly previewReading?: string;
  readonly open: boolean;
  readonly onClose: () => void;
  /** Enter or a click (copy false), ⇧Enter (copy true). */
  readonly onPick: (row: PickRow, copy: boolean) => void;
}

const ROW_CLASS =
  "cursor-pointer gap-2 hover:bg-transparent hover:text-inherit data-highlighted:bg-transparent data-highlighted:text-inherit data-selected:bg-transparent data-selected:text-inherit [&[data-highlighted][data-selected]]:bg-transparent [&[data-highlighted][data-selected]]:text-inherit";
/** Fake counts for the plan's row, which asks for one; nothing on the list can read it without reading the whole file. */
const PLAN_COUNTS: Record<string, number> = { "8f1c2a90-1": 66, "8f1c2a90-2": 43, "codex-1735-r2": 12, "8f1c2a90-4": 188, "8f1c2a90-5": 21, "codex-docs-search": 8, "897699a0": 22, "73df5281": 417 };

const folderTail = (cwd: string, root: string): string | undefined => (cwd === root ? undefined : cwd.split("/").slice(-2).join("/"));

export function ResumePicker(props: PickerProps) {
  const plan = props.variant === "plan";
  const [query, setQuery] = useState("");
  const [agent, setAgent] = useState<AgentScope>("all");
  const [branch, setBranch] = useState<BranchScope>("all");
  const [scope, setScope] = useState<ProjectScope>(props.projectScope ?? "this");
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const inScope = props.rows.filter(r => scope === "all" || r.project === props.project);
  const agents = new Set(inScope.map(r => r.agent));
  const shown = useMemo(() => {
    const words = query.trim().toLowerCase();
    return inScope.filter(
      r =>
        (agent === "all" || r.agent === agent) &&
        (branch === "all" || r.branch === props.thisBranch) &&
        (words === "" || [r.title, r.firstPrompt ?? "", r.branch ?? "", agentName(r.agent)].some(t => t.toLowerCase().includes(words))),
    );
  }, [inScope, agent, branch, query, props.thisBranch]);
  const groups = useMemo(() => {
    if (scope === "all" && plan) return props.projects.map(p => ({ project: p, rows: shown.filter(r => r.project === p.id) })).filter(g => g.rows.length > 0);
    return [{ project: props.projects.find(p => p.id === props.project)!, rows: [...shown].sort((a, b) => b.lastAt - a.lastAt) }].filter(g => g.rows.length > 0);
  }, [shown, scope, props.projects, props.project]);
  const empty = props.reading !== true && shown.length === 0;
  const ordered = groups.flatMap(g => g.rows);
  const current = ordered.find(r => `c:${r.id}` === highlighted) ?? ordered[0];
  const here = props.projects.find(p => p.id === props.project)!;
  const scopes = plan ? (
    <div data-resume-scopes className="flex flex-wrap items-center gap-2 px-[var(--command-shell-inset)] pb-2">
      <SegmentedControl aria-label="Agent" value={agent} onChange={setAgent} className="h-7" segmentClassName="px-2 text-xs" segments={[{ value: "all", label: "All" }, { value: "claude", label: "Claude Code" }, { value: "codex", label: "Codex" }]} />
      <SegmentedControl aria-label="Branch" value={branch} onChange={setBranch} className="h-7" segmentClassName="px-2 text-xs" segments={[{ value: "this", label: "This branch" }, { value: "all", label: "All branches" }]} />
      <SegmentedControl aria-label="Project" value={scope} onChange={setScope} className="h-7" segmentClassName="px-2 text-xs" segments={[{ value: "this", label: "This project" }, { value: "all", label: "All projects" }]} />
    </div>
  ) : (
    <div data-resume-scopes className="flex flex-wrap items-center gap-1 px-[calc(var(--command-shell-inset)+0.25rem)] pb-1.5">
      {agents.size > 1 && !empty ? (
        <ScopeMenu label={agent === "all" ? "All agents" : agentName(agent)} value={agent} onChange={setAgent} options={[{ value: "all", label: "All agents" }, { value: "claude", label: "Claude Code" }, { value: "codex", label: "Codex" }]} />
      ) : null}
      {empty ? null : <ScopeMenu label={branch === "all" ? "All branches" : props.thisBranch} value={branch} onChange={setBranch} options={[{ value: "all", label: "All branches" }, { value: "this", label: `On ${props.thisBranch}` }]} />}
      <ScopeMenu label={scope === "all" ? "All projects" : here.name} value={scope} onChange={setScope} options={[{ value: "this", label: here.name }, { value: "all", label: "All projects" }]} />
    </div>
  );

  return (
    <CommandDialog open={props.open} onOpenChange={next => (next ? undefined : props.onClose())}>
      <CommandDialogPopup
        aria-label="Resume a conversation"
        data-resume-picker={props.variant}
        className={cn("overflow-hidden p-0", plan ? "" : "max-h-[min(38rem,84vh)] max-w-[55rem]")}
      >
        <Command mode="none" value={query} onValueChange={value => setQuery(value)} onItemHighlighted={value => setHighlighted(typeof value === "string" ? value : null)}>
          <div className="relative">
            <CommandInput
              placeholder="Search conversations..."
              onKeyDown={e => {
                if (e.key !== "Enter" || current === undefined) return;
                e.preventDefault();
                props.onPick(current, e.shiftKey);
              }}
            />
          </div>
          {scopes}
          {(props.held ?? []).map(h => (
            <p key={h.agent} data-resume-held className="flex min-w-0 items-start gap-2 px-[calc(var(--command-shell-inset)+0.75rem)] pb-2 text-xs leading-4 text-muted-foreground">
              <HarnessMark harness={h.agent} label={agentName(h.agent)} className="mt-px size-3.5 shrink-0" />
              <span className="min-w-0">{`${agentName(h.agent)}'s conversations could not be read: ${h.said}`}</span>
            </p>
          ))}
          <CommandPanel className={cn("flex min-h-0", empty ? "" : plan ? "h-[min(15rem,50vh)]" : "h-[min(26rem,56vh)] max-md:h-auto max-md:max-h-[min(26rem,56vh)]")}>
            <div className="min-h-0 min-w-0 flex-1">
              {props.reading === true ? (
                <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
                  <Spinner className="size-4" />
                  Reading conversations on zingzy's MacBook Pro
                </div>
              ) : empty ? (
                <div data-resume-empty className="py-10 text-center text-sm text-muted-foreground">
                  {plan ? "No conversations in this folder yet" : `No conversations in ${here.name} yet`}
                </div>
              ) : (
                <CommandList className="h-full">
                  {groups.map(g => (
                    <CommandGroup items={g.rows.map(r => `c:${r.id}`)} key={g.project.id}>
                      <CommandGroupLabel className="ps-[9px]">{scope === "all" ? (plan ? (g.project.on !== undefined ? `${g.project.name} on ${g.project.on}` : g.project.name) : "Conversations in every project") : `Conversations in ${g.project.name}`}</CommandGroupLabel>
                      <CommandCollection>
                        {(value: string) => {
                          const r = g.rows.find(x => `c:${x.id}` === value)!;
                          return <PickerRow key={value} row={r} plan={plan} onPick={props.onPick} active={(current !== undefined && `c:${current.id}`) === value} project={scope === "all" && !plan ? props.projects.find(p => p.id === r.project) : undefined} />;
                        }}
                      </CommandCollection>
                    </CommandGroup>
                  ))}
                </CommandList>
              )}
            </div>
            {current !== undefined && !empty && props.reading !== true ? <PreviewPane row={current} plan={plan} root={props.projects.find(p => p.id === current.project)?.root ?? here.root} reading={props.previewReading === current.id} /> : null}
          </CommandPanel>
          <CommandFooter className="gap-3 max-sm:flex-col max-sm:items-start">
            <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
              <KbdGroup className="items-center gap-1.5">
                <Kbd>
                  <ArrowUpIcon />
                </Kbd>
                <Kbd>
                  <ArrowDownIcon />
                </Kbd>
                <span>Navigate</span>
              </KbdGroup>
              {empty ? null : (
                <>
                  <KbdGroup className="items-center gap-1.5">
                    <Kbd>Enter</Kbd>
                    <span>Continue</span>
                  </KbdGroup>
                  <KbdGroup className="items-center gap-1.5">
                    <Kbd>⇧ Enter</Kbd>
                    <span>Continue in a copy</span>
                  </KbdGroup>
                </>
              )}
              <KbdGroup className="items-center gap-1.5">
                <Kbd>Esc</Kbd>
                <span>Close</span>
              </KbdGroup>
            </div>
          </CommandFooter>
        </Command>
      </CommandDialogPopup>
    </CommandDialog>
  );
}

function ScopeMenu<T extends string>({ label, value, onChange, options }: { label: string; value: T; onChange: (v: T) => void; options: { value: T; label: string }[] }) {
  return (
    <Menu>
      <MenuTrigger render={<Button type="button" variant="ghost" size="xs" />} className="gap-1 px-2 text-xs text-muted-foreground hover:text-foreground" data-resume-scope={label}>
        <span className="truncate">{label}</span>
        <ChevronDownIcon className="size-3.5 shrink-0 opacity-60" />
      </MenuTrigger>
      <MenuPopup align="start" className="w-48">
        <MenuRadioGroup value={value} onValueChange={next => onChange(next as T)}>
          {options.map(o => (
            <MenuRadioItem key={o.value} value={o.value}>
              {o.label}
            </MenuRadioItem>
          ))}
        </MenuRadioGroup>
      </MenuPopup>
    </Menu>
  );
}

function PickerRow({ row, plan, active, project, onPick }: { row: PickRow; plan: boolean; active: boolean; project?: PickerProject; onPick: (row: PickRow, copy: boolean) => void }) {
  const name = agentName(row.agent);
  const mark = row.thread !== undefined ? "Already a thread" : row.live ? (plan ? "Open in Terminal" : "Open in another app") : undefined;
  const age = compactTimeLabel(new Date(row.lastAt).toISOString());
  const size = row.bytes !== undefined ? fmtBytes(row.bytes) : undefined;
  return (
    <CommandItem value={`c:${row.id}`} data-resume-row={row.id} className={cn(ROW_CLASS, active && "bg-accent! text-accent-foreground!")} onMouseDown={e => e.preventDefault()} onClick={e => onPick(row, e.shiftKey)}>
      <HarnessMark harness={row.agent} label={name} className="size-4" />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="min-w-0 truncate text-foreground text-sm">{row.title}</span>
        {plan ? (
          <span data-item-description className="min-w-0 font-mono text-[11px] leading-4 text-muted-foreground">
            <Facts parts={[mark, row.branch, `${PLAN_COUNTS[row.id] ?? 6} messages`, size, age]} />
          </span>
        ) : (
          <span data-item-description className="min-w-0 text-muted-foreground text-xs">
            <Facts parts={[project !== undefined ? (project.on !== undefined ? `${project.name} on ${project.on}` : project.name) : undefined, mark, row.branch, size]} />
          </span>
        )}
      </span>
      {plan ? null : <span className={cn("inline-flex shrink-0 items-center self-start whitespace-nowrap pt-0.5 tabular-nums", LINE_SLOT_CLASS)}>{age}</span>}
    </CommandItem>
  );
}

function PreviewPane({ row, plan, root, reading }: { row: PickRow; plan: boolean; root: string; reading: boolean }) {
  const all = previewOf(row).lines;
  const lines = plan ? all.filter(l => l.who !== "tool").slice(-2) : all.slice(-4);
  const folder = folderTail(row.cwd, root);
  const home = row.cwd.replace(/^\/Users\/[^/]+/, "~");
  return (
    <aside data-resume-preview className={cn("hidden shrink-0 flex-col border-l border-border/60 md:flex", plan ? "w-[45%]" : "w-[25rem]")}>
      {plan ? (
        <div className="px-4 pt-3 pb-2">
          <div className="truncate text-sm font-medium text-foreground">{row.title}</div>
          <Facts className="mt-0.5 text-xs text-muted-foreground" parts={[agentName(row.agent), folder !== undefined ? <span className="font-mono">{folder}</span> : undefined]} />
        </div>
      ) : (
        <p className="truncate px-4 pt-3.5 pb-3 text-xs text-muted-foreground">
          Runs in <span className="font-mono">{home}</span>
        </p>
      )}
      {reading ? (
        <div className="flex flex-1 items-center justify-center gap-2 pb-6 text-xs text-muted-foreground">
          <Spinner className="size-3.5" />
          Reading the last messages
        </div>
      ) : (
        <div className={cn("flex min-h-0 flex-1 flex-col gap-2.5 overflow-hidden px-4 pb-3", plan && "justify-end")}>
          {lines.map((l, i) =>
            l.who === "tool" ? (
              <p key={i} className="truncate text-xs text-muted-foreground">
                {toolDoneLine(l.tool, l.text) ?? toolActivityLine(l.tool, l.text)}
              </p>
            ) : l.who === "person" ? (
              <p key={i} className="line-clamp-3 self-end rounded-xl bg-foreground/[0.06] px-2.5 py-1.5 text-[13px] leading-5 text-foreground">
                {l.text}
              </p>
            ) : (
              <p key={i} className="line-clamp-4 text-[13px] leading-5 text-foreground/90">
                {l.text}
              </p>
            ),
          )}
        </div>
      )}
    </aside>
  );
}
