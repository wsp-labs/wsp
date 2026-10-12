// SPDX-License-Identifier: AGPL-3.0-only
// The picker of conversations the agents kept outside wsp, the palette's sibling: its dialog, input, rows, group label
// and footer, wider so the newest messages of the highlighted row show beside the list, which they do not below 768
// px. Three menus under the search set the agent, the branch and the project; a row says only what is true of it,
// open in another app or already a thread. Enter continues the highlighted row, ⇧Enter continues it in a copy.
import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { ChevronDownIcon } from "lucide-react";
import { agentName } from "@wsp/catalog";
import { CONVERSATION_WORDS, fmtBytes, refusalLine, type ProjectView } from "@wsp/protocol";
import { HarnessMark } from "../components/chat/HarnessMark.js";
import { useProjectBranch } from "../components/chat/ComposerCheckoutRow.js";
import { Facts } from "../components/Facts.js";
import { LINE_SLOT_CLASS } from "../components/status/ThreadStatus.js";
import { Button } from "../components/ui/button.js";
import { CommandPaletteContent } from "../components/palette/CommandPaletteContent.js";
import { CommandCollection, CommandDialog, CommandDialogPopup, CommandGroup, CommandGroupLabel, CommandItem, CommandList } from "../components/ui/command.js";
import { Menu, MenuPopup, MenuRadioGroup, MenuRadioItem, MenuTrigger } from "../components/ui/menu.js";
import { Spinner } from "../components/ui/spinner.js";
import { tilde } from "../lib/tilde.js";
import { requestComposerFocus } from "./shellRequests.js";
import { cn } from "../lib/utils.js";
import { useSidebarProjects, useStore } from "../protocol/store.js";
import { compactTimeLabel, placeNames } from "../sidebar/workspaceRows.js";
import { isAnswer, isRead, pickConversation, previewKey, useConversationsStore, type PickedRow } from "./conversations.js";

/** How long the highlight rests on a row before its preview is asked for, so arrowing past rows asks for none. */
export const PREVIEW_REST_MS = 150;
/** How many rows the list draws at first, and adds each time the reader nears its end: clearing a search on 400
 * conversations mounted every row in one task of 250 to 320 ms, and 60 rows took 60 to 70 ms, past 100 ms once a
 * garbage collection landed in it (dev build, measured 2026-10-12). */
export const ROWS_DRAWN = 30;

const COPY_KEYS = [{ keys: "⇧ Enter", label: CONVERSATION_WORDS.copy }];

const ROW_CLASS =
  "cursor-pointer gap-2 hover:bg-transparent hover:text-inherit data-selected:bg-transparent data-selected:text-inherit data-highlighted:bg-accent! data-highlighted:text-accent-foreground!";

type AgentScope = string;
type BranchScope = "all" | "this";
type ProjectScope = "this" | "all";


/** The rows a scope and a search leave, newest first: the search matches the title, the first prompt, the branch,
 * the agent's name and, over every project, the project's. */
export function pickerRows(rows: ReadonlyArray<PickedRow>, o: { agent: AgentScope; branch: string | null; query: string; names?: ReadonlyMap<string, string> }): PickedRow[] {
  const words = o.query.trim().toLowerCase();
  return rows
    .filter(
      ({ project, row }) =>
        (o.agent === "all" || row.agent === o.agent) &&
        (o.branch === null || row.branch === o.branch) &&
        (words === "" || [row.title, row.firstPrompt ?? "", row.branch ?? "", agentName(row.agent), o.names?.get(project) ?? ""].some(t => t.toLowerCase().includes(words))),
    )
    .sort((a, b) => b.row.lastAt - a.row.lastAt);
}

export function ResumePicker() {
  const project = useConversationsStore(s => s.picker);
  return project === null ? null : <Picker key={project} project={project} />;
}

const valueOf = (r: PickedRow): string => `c:${r.project}:${r.row.agent}:${r.row.id}`;

function Picker({ project }: { project: string }) {
  const projects = useStore(s => s.projects);
  const places = useStore(s => s.places);
  const answers = useConversationsStore(s => s.answers);
  const here = projects.find(p => p.id === project);
  const branch = useProjectBranch(project)?.branch ?? null;
  const [query, setQuery] = useState("");
  const [agent, setAgent] = useState<AgentScope>("all");
  const [branchScope, setBranchScope] = useState<BranchScope>("all");
  const [scope, setScope] = useState<ProjectScope>("this");
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const names = useMemo(() => placeNames(places), [places]);
  // Every project on a computer the person owns: a provider's machines keep no conversation of the person's.
  const listable = useMemo(() => projects.filter(p => places.find(pl => pl.id === p.computer)?.kind !== "provider"), [places, projects]);
  const listed = useMemo((): ReadonlyArray<ProjectView> => (scope === "all" ? listable : here !== undefined ? [here] : []), [here, listable, scope]);
  useEffect(() => {
    if (scope === "all") for (const p of listable) if (p.id !== project) void useConversationsStore.getState().read(p.id);
  }, [listable, project, scope]);
  const projectNames = useMemo(() => new Map(projects.map(p => [p.id, p.name])), [projects]);
  const inScope = useMemo(
    () =>
      listed.flatMap(p => {
        const read = answers[p.id];
        return isAnswer(read) ? read.rows.map((row): PickedRow => ({ project: p.id, row })) : [];
      }),
    [answers, listed],
  );
  const shown = useMemo(
    () => pickerRows(inScope, { agent, branch: branchScope === "this" ? branch : null, query, ...(scope === "all" ? { names: projectNames } : {}) }),
    [agent, branch, branchScope, inScope, projectNames, query, scope],
  );
  const [drawnFor, setDrawnFor] = useState({ shown, n: ROWS_DRAWN });
  const drawn = drawnFor.shown === shown ? drawnFor.n : ROWS_DRAWN;
  const visible = useMemo(() => shown.slice(0, drawn), [drawn, shown]);
  const visibleValues = useMemo(() => visible.map(valueOf), [visible]);
  const grow = useCallback(() => setDrawnFor(was => (drawn >= shown.length ? was : { shown, n: drawn + ROWS_DRAWN })), [drawn, shown]);
  const own = answers[project];
  const reading = own === undefined;
  const refused = own !== undefined && !isAnswer(own) ? own : null;
  // An agent whose store did not answer says so in one line, and, over every project, a project that did not.
  const held = listed.flatMap((p): { key: string; agent: string | null; said: string }[] => {
    const read = answers[p.id];
    if (read === undefined) return [];
    if (!isAnswer(read)) return p.id === project ? [] : [{ key: p.id, agent: null, said: `${p.name}: ${read.said}` }];
    return read.held.map(h => ({ key: `${p.id}:${h.agent}`, agent: h.agent, said: h.why !== undefined ? h.said : refusalLine(h.said, h.fix) }));
  });
  const agents = [...new Set(inScope.map(r => r.row.agent))];
  const empty = !reading && inScope.length === 0;
  const current = shown.find(r => valueOf(r) === highlighted);
  // A pick hands the focus to where it lands, a draft's composer, a thread's or the confirm; the dialog gives none back.
  const picked = useRef(false);
  // A scope pick hands the focus back to the search, so the arrows and Enter drive the list as before it.
  const search = useRef<HTMLInputElement | null>(null);
  const sidebarProjects = useSidebarProjects();
  const sidebar = useRef(sidebarProjects);
  sidebar.current = sidebarProjects;
  const computer = here === undefined ? "" : (names.get(here.computer) ?? here.computer);
  /** The home of the computer a project is on, as that computer last reported it; empty where it never has. */
  const homeOf = (id: string): string => places.find(pl => pl.id === projects.find(p => p.id === id)?.computer)?.home ?? "";

  /** A row a wsp thread already runs on opens that thread, where the sidebar holds it, on either key: the host resumes
   * no copy of a thread's conversation, and the thread forks from any message. Any other row is the picker's pick. */
  const pick = useCallback((r: PickedRow, copy: boolean): void => {
    picked.current = true;
    const thread = r.row.thread === undefined ? undefined : sidebar.current.flatMap(p => p.threads).find(t => t.threadId === r.row.thread);
    if (thread !== undefined) {
      useConversationsStore.getState().openPicker(null);
      useStore.getState().select(thread.workspaceId, thread.threadId);
      requestComposerFocus(thread.workspaceId);
      return;
    }
    pickConversation(r.project, r.row, copy);
  }, []);

  // The list is drawn again only when what it shows moves, never as the highlight walks it.
  const list = useMemo(
    () =>
      shown.length === 0 ? null : (
        <CommandList className="h-full">
          <CommandGroup items={visibleValues}>
            <CommandGroupLabel className="ps-2.25">{scope === "all" ? CONVERSATION_WORDS.everyProject : CONVERSATION_WORDS.group(here?.name ?? "")}</CommandGroupLabel>
            <CommandCollection>
              {(value: string) => {
                const r = visible.find(x => valueOf(x) === value)!;
                return <PickerRow key={value} value={value} picked={r} project={scope === "all" ? projectNames.get(r.project) : undefined} onPick={pick} />;
              }}
            </CommandCollection>
          </CommandGroup>
        </CommandList>
      ),
    [visible, shown.length, scope, here?.name, projectNames, pick],
  );

  return (
    <CommandDialog open onOpenChange={next => (next ? undefined : useConversationsStore.getState().openPicker(null))}>
      <CommandDialogPopup aria-label={CONVERSATION_WORDS.row} finalFocus={() => !picked.current} data-resume-picker className="max-h-152 max-w-220 overflow-hidden p-0">
        <CommandPaletteContent
          // The rows as base-ui sees them, so its resting highlight lands on the first row when the read does and after
          // every search; Enter takes the row it draws and no other.
          items={visibleValues}
          mode="none"
          value={query}
          onValueChange={value => setQuery(value)}
          onItemHighlighted={value => {
            setHighlighted(typeof value === "string" ? value : null);
            if (typeof value === "string" && visible.findIndex(r => valueOf(r) === value) >= drawn - 5) grow();
          }}
          inputProps={{
            ref: search,
            placeholder: CONVERSATION_WORDS.search,
            onKeyDown: e => {
              if (e.key !== "Enter" || current === undefined) return;
              e.preventDefault();
              pick(current, e.shiftKey);
            },
          }}
          inputAccessory={
            <>
              <div data-resume-scopes className="flex flex-wrap items-center gap-1 px-[calc(var(--command-shell-inset)+0.25rem)] pb-1.5">
                {agents.length > 1 ? (
                  <ScopeMenu k="agent" returnTo={search} label={agent === "all" ? CONVERSATION_WORDS.allAgents : agentName(agent)} value={agent} onChange={setAgent} options={[{ value: "all", label: CONVERSATION_WORDS.allAgents }, ...agents.map(a => ({ value: a, label: agentName(a) }))]} />
                ) : null}
                {!empty && branch !== null ? (
                  <ScopeMenu k="branch" returnTo={search} label={branchScope === "all" ? CONVERSATION_WORDS.allBranches : branch} value={branchScope} onChange={setBranchScope} options={[{ value: "all", label: CONVERSATION_WORDS.allBranches }, { value: "this", label: CONVERSATION_WORDS.onBranch(branch) }]} />
                ) : null}
                {here !== undefined ? (
                  <ScopeMenu k="project" returnTo={search} label={scope === "all" ? CONVERSATION_WORDS.allProjects : here.name} value={scope} onChange={setScope} options={[{ value: "this", label: here.name }, { value: "all", label: CONVERSATION_WORDS.allProjects }]} />
                ) : null}
              </div>
              {held.map(h => (
                <p key={h.key} data-resume-held className="flex min-w-0 items-start gap-2 px-[calc(var(--command-shell-inset)+0.75rem)] pb-2 text-xs leading-4 text-muted-foreground">
                  {h.agent !== null ? <HarnessMark harness={h.agent} label={agentName(h.agent)} className="mt-px size-3.5 shrink-0" /> : null}
                  <span className="min-w-0">{h.said}</span>
                </p>
              ))}
            </>
          }
          panelClassName={cn("flex min-h-0", shown.length > 0 && "h-104 max-md:h-auto max-md:max-h-104")}
          escapeLabel={CONVERSATION_WORDS.close}
          {...(shown.length > 0 ? { footerActionLabel: CONVERSATION_WORDS.send, footerKeys: COPY_KEYS } : {})}
        >
          <div
            className="min-h-0 min-w-0 flex-1"
            onScrollCapture={e => {
              const at = e.target as HTMLElement;
              if (at.scrollTop + at.clientHeight >= at.scrollHeight - 240) grow();
            }}
          >
            {reading ? (
              <div data-conversations-reading className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
                <Spinner className="size-4" />
                {CONVERSATION_WORDS.reading(computer)}
              </div>
            ) : refused !== null && scope === "this" ? (
              <div data-refused className="px-6 py-10 text-center text-sm text-muted-foreground">
                {refused.fix === undefined ? refused.said : refusalLine(refused.said, refused.fix)}
              </div>
            ) : empty ? (
              <div data-resume-empty className="py-10 text-center text-sm text-muted-foreground">
                {CONVERSATION_WORDS.emptyIn(scope === "all" ? CONVERSATION_WORDS.allProjects.toLowerCase() : (here?.name ?? ""))}
              </div>
            ) : shown.length === 0 ? (
              <div className="py-10 text-center text-sm text-muted-foreground">{CONVERSATION_WORDS.noMatch}</div>
            ) : (
              list
            )}
          </div>
          {current !== undefined ? <PreviewPane picked={current} home={homeOf(current.project)} /> : null}
        </CommandPaletteContent>
      </CommandDialogPopup>
    </CommandDialog>
  );
}

function ScopeMenu<T extends string>({ k, label, value, onChange, options, returnTo }: { k: string; label: string; value: T; onChange: (v: T) => void; options: ReadonlyArray<{ value: T; label: string }>; returnTo: RefObject<HTMLInputElement | null> }) {
  return (
    <Menu>
      <MenuTrigger render={<Button type="button" variant="ghost" size="xs" />} className="gap-1 px-2 text-xs text-muted-foreground hover:text-foreground" data-resume-scope={k}>
        <span className="truncate">{label}</span>
        <ChevronDownIcon className="size-3.5 shrink-0 opacity-60" />
      </MenuTrigger>
      <MenuPopup align="start" className="w-48" finalFocus={returnTo}>
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

const PickerRow = memo(function PickerRow({ value, picked, project, onPick }: { value: string; picked: PickedRow; project: string | undefined; onPick: (r: PickedRow, copy: boolean) => void }) {
  const { row } = picked;
  const name = agentName(row.agent);
  const mark = row.thread !== undefined ? CONVERSATION_WORDS.thread : row.live ? CONVERSATION_WORDS.live : undefined;
  return (
    <CommandItem value={value} data-resume-row={row.id} className={ROW_CLASS} onMouseDown={e => e.preventDefault()} onClick={e => onPick(picked, e.shiftKey)}>
      <HarnessMark harness={row.agent} label={name} className="size-4" />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="min-w-0 truncate text-foreground text-sm">{row.title}</span>
        <span data-item-description className="min-w-0 text-muted-foreground text-xs">
          <Facts parts={[project, mark, row.branch, row.bytes !== undefined ? fmtBytes(row.bytes) : undefined]} />
        </span>
      </span>
      <span className={cn("inline-flex shrink-0 items-center self-start whitespace-nowrap pt-0.5 tabular-nums", LINE_SLOT_CLASS)}>{compactTimeLabel(new Date(row.lastAt).toISOString())}</span>
    </CommandItem>
  );
});

/** The highlighted row's newest messages and where it runs, asked for once the highlight rests on it. */
function PreviewPane({ picked, home }: { picked: PickedRow; home: string }) {
  const key = previewKey(picked.project, picked.row);
  const read = useConversationsStore(s => s.previews.get(key));
  useEffect(() => {
    const wait = setTimeout(() => void useConversationsStore.getState().preview(picked.project, picked.row), PREVIEW_REST_MS);
    return () => clearTimeout(wait);
  }, [picked.project, picked.row]);
  const rows = isRead(read) ? read.rows.filter(r => r.who !== "note").slice(-4) : [];
  return (
    <aside data-resume-preview className="hidden w-100 shrink-0 flex-col border-l border-border/60 md:flex">
      <p className="truncate px-4 pt-3.5 pb-3 text-xs text-muted-foreground">
        {CONVERSATION_WORDS.runsIn} <span className="font-mono">{tilde(isRead(read) ? (read.cwd ?? picked.row.cwd) : picked.row.cwd, home)}</span>
      </p>
      {read === undefined ? (
        <div data-preview-reading className="flex flex-1 items-center justify-center gap-2 pb-6 text-xs text-muted-foreground">
          <Spinner className="size-3.5" />
          {CONVERSATION_WORDS.previewReading}
        </div>
      ) : !isRead(read) ? (
        <p data-preview-refused className="px-4 text-xs text-muted-foreground">
          {read.fix === undefined ? read.said : refusalLine(read.said, read.fix)}
        </p>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-hidden px-4 pb-3">
          {rows.map((r, i) =>
            r.who === "tool" ? (
              <p key={i} className="truncate text-xs text-muted-foreground">
                {r.text}
              </p>
            ) : r.who === "person" ? (
              <p key={i} className="line-clamp-3 self-end rounded-xl bg-foreground/[0.06] px-2.5 py-1.5 text-note leading-5 text-foreground">
                {r.text}
              </p>
            ) : (
              <p key={i} className="line-clamp-4 text-note leading-5 text-foreground/90">
                {r.text}
              </p>
            ),
          )}
        </div>
      )}
    </aside>
  );
}

