# Wave 2 parity spec

Map wsp-labs/wsp-map#2364, ticket #2407. What today's app draws and does on Wave 2's screens, read off `main` at
1516a0cd5 (2026-10-17): the terminal (#2396), the pull request, Changes and Files panes (#2397), slates (#2398),
setup dialogs, onboarding and the pair screen (#2399), the palette, switcher, activity and notices (#2400), and the
panes no ticket owns. Builders tick items by id; reviewers cite them.

**Reading an item.** One line: id, what, source `path:line`. Paths are under `apps/web/src/`; `desktop:` is
`apps/desktop/src/`, `protocol:`, `runtime:` and `host:` are `packages/<name>/src/`, `daemon:` is `daemon/`, `sonner:`
is the Sonner 2.0.8 package. A state marked Shot has baselines `<id>-paper.png` and `<id>-graphite.png`, made by
`parity/baseline.mjs --wave 2` from `states.mjs` here; No shot says why. Host operations use the names in
`apps/desktop-gpui/host-client/src/ops/`; Gap marks one the client lacks. Design names the `app/src/ui/` piece that
draws an element, or none yet, and each transition with its property, duration and curve ("default curve" is
Tailwind's `cubic-bezier(0.4, 0, 0.2, 1)`, 150 ms unless given). `more.md` holds the items this file had no room for,
in the same id sequences; builders tick both.

**Design rules for every screen.** Map decision 5: the screen looks exactly like today's Electron app, pixel-compared
per state in Paper and Graphite; the locked designs count only where the app has no such piece. The fork ruling
(#2366): gpui-kit 14f5933 on `gpui-pre` 0.3.9. Blur and rounded clipping are #2373's patch, already on `gpui/main`.
gpui-kit (Apache-2.0) may be used and adapted; Zed's crates (GPL) are read, never pasted; zui and Zeron are never
copied.

## Who owns each pane

| Pane (`panes.ts`) | Owner |
|---|---|
| Browser | #2401, wave 3 |
| Terminal | #2396 |
| Changes, Pull request, Files | #2397 |
| Computer | none: W2-UNO below; D11 recommends #2400 |
| Processes | #2400 |
| Agents | body is Settings' own (#2391); its wrapper none: W2-UNO; D11 recommends #2391 |
| Aside (the side question, `/btw`) | none: W2-UNO; D11 recommends #2394 |
| Slate | #2398 |

Drawn by today's app and named by no ticket, all in W2-UNO: `components/DaemonDown.tsx` (terminal, Changes and
Processes draw it), the context menu (native in Electron, `actions/ContextMenuHost.tsx` in a browser), and the
Electron shell's own tray, Quit question, notifications and dock badge, Move to Applications and older-host prompts.
Claimed twice: `settings/RemoveComputerDialog.tsx` (#2391 and #2399), `settings/activity/` (#2391 and #2400), the
Computer pane's `ChartPlot` (also #2392's LiveBand and #2400's Activity). D11 rules each.

## Terminal (#2396)

Sources: `terminal/`, `components/ThreadTerminalDrawer.tsx`, `WorkspaceTerminalDrawer.tsx`, `WorkspaceTerminalPanel.tsx`. Emulation per D1; the grid, cursor and selection are ours, read off `terminal/ghostty/renderer.ts`; Zed's terminal crates may be read, not pasted.

### Elements

- W2-TRM-001 Drawer under the thread: top border, 6 px drag strip, 280 px, 180 px to 75% window. `components/ThreadTerminalDrawer.tsx:1035`
- W2-TRM-002 Canvas grid, cursor (block, bar, underline, hollow unfocused), selection, hidden IME textarea. `terminal/ghostty/surface/terminalSurface.ts:169`
- W2-TRM-003 Scrollbar, hidden without scrollback; drag, click, arrows, Page, Home, End. `terminal/ghostty/surface/terminalSurface.ts:182`
- W2-TRM-004 Floating toolbar: split, split vertical, new, font, close, dividers. `components/ThreadTerminalDrawer.tsx:1053`
- W2-TRM-005 Splits, 4 per group, side by side or stacked. `components/ThreadTerminalDrawer.tsx:1114`
- W2-TRM-006 Tab list at 2+, 144 px: toolbar, groups "Single", "Side by side", "Stacked" and count, tabs "shell N" (lowest free N), "(exited)", X on hover. `components/ThreadTerminalDrawer.tsx:1157`
- W2-TRM-007 Font card: label, input, hint. `components/TerminalFontButton.tsx:27`
- W2-TRM-008 State overlay: title, "for Ns" ticks 1 s, hints, Wake or Start again, refusal. `components/ThreadTerminalDrawer.tsx:594`
- W2-TRM-009 Shell-gone overlay, New terminal. `components/ThreadTerminalDrawer.tsx:630`
- W2-TRM-010 Empty pane: "No terminals open." and New terminal, or the state's sentence. `components/ThreadTerminalDrawer.tsx:993`
- W2-TRM-011 Right panel terminal, no drag strip. `components/WorkspaceTerminalPanel.tsx:94`

### States

- W2-TRM-012 Shell prompt; spawned on first open only. Shot: Ctrl+J on a fresh thread. `components/WorkspaceTerminalDrawer.tsx:82`
- W2-TRM-013 Coloured output. Shot: printf SGR. `index.css:747`
- W2-TRM-014 Full-screen program. Shot: less. `terminal/ghostty/surface/terminalSurface.ts:1039`
- W2-TRM-015 Ghostty config applied, translucent (macOS glass); theme flip rereads light:,dark:. Shot: a Ghostty config, then the drawer. `terminal/ghosttyConfig.ts:70`
- W2-TRM-016 Ctrl-hovered link underlined. Shot: print a URL, move over it with Ctrl. `terminal/ghostty/renderer.ts:251`
- W2-TRM-017 Tabs and splits. Shot: split vertical, new. `components/ThreadTerminalDrawer.tsx:1202`
- W2-TRM-018 Split limit, "Max 4 per group". No shot: near duplicate of W2-TRM-017. `actions/format.ts:263`
- W2-TRM-019 Zoomed. Shot: Ctrl+= x3. `terminal/fontSetting.ts:61`
- W2-TRM-020 Font card open. Shot: font button. `components/ThreadTerminalDrawer.tsx:1098`
- W2-TRM-021 Right panel terminal. Shot: the launcher's Terminal card. `panes.ts:75`
- W2-TRM-022 Long output scrolled up, 10,000 rows. Shot: 5,000 lines, wheel up. `terminal/ghostty/core.ts:13`
- W2-TRM-023 Narrow window. Shot: 640 px, split. `components/ThreadTerminalDrawer.tsx:1116`
- W2-TRM-024 No terminals open. Shot: close last, reopen. `components/ThreadTerminalDrawer.tsx:996`
- W2-TRM-025 Pty exited. Shot: two tabs, exit. `terminal/link.ts:510`
- W2-TRM-026 Starting. No shot: needs a host with no wsp-daemon staged. `adapt/terminal-pane.ts:117`
- W2-TRM-027 Nothing answered. No shot: as starting. `adapt/terminal-pane.ts:119`
- W2-TRM-028 Daemon down, Start again, typing refused. No shot: no host op or wsp line stops the local daemon. `components/DaemonDown.tsx:55`
- W2-TRM-029 Reconnecting, seconds. No shot: follows a stopped daemon, for seconds. `adapt/terminal-pane.ts:122`
- W2-TRM-030 Shell gone. No shot: follows a stopped daemon's restart. `components/WorkspaceTerminalDrawer.tsx:43`
- W2-TRM-031 Paused, waking, gone, not answering, no daemon, refused, reauth. No shot: cloud or remote computer. `adapt/terminal-pane.ts:112`
- W2-TRM-032 Wasm load error. No shot: needs broken wasm. `components/ThreadTerminalDrawer.tsx:446`

### Actions and keys

- W2-TRM-033 Ctrl+J toggles drawer, also from a focused terminal. `keybindingDefaults.ts:45`
- W2-TRM-034 Ctrl+D split, Ctrl+N new: shell's on Linux, Cmd on Mac. A focused terminal keeps Ctrl chords but Ctrl+J, palette, settings, sidebar, preview, search, zoom, Ctrl+1-9, panel tabs. `keybindings.ts:304`
- W2-TRM-035 Ctrl+=, Ctrl+Shift+=, Ctrl+-, Ctrl+0: zoom, 1 px, 6 to 32. `shell/shellCommands.ts:374`
- W2-TRM-036 Toolbar buttons, tab click, group row, tab X. `components/ThreadTerminalDrawer.tsx:1056`
- W2-TRM-037 Drag top edge resizes; window resize clamps, refits. `components/ThreadTerminalDrawer.tsx:891`
- W2-TRM-038 Ctrl+click: URL to browser, path to Files. `components/ThreadTerminalDrawer.tsx:357`
- W2-TRM-039 Wheel: scrollback, arrows on alt screen; mouse-tracking programs get clicks, wheel, no menu, bar Shift, Ctrl, Cmd. `terminal/ghostty/surface/terminalSurface.ts:1020`

### Host operations

- W2-TRM-040 `daemon.open`: channel. `terminal/daemon-link.ts:219`
- W2-TRM-041 `daemon.send`: every frame. `terminal/daemon-link.ts:123`
- W2-TRM-042 `daemon.close`. `terminal/daemon-link.ts:166`
- W2-TRM-043 `host.terminalConfig`: per scheme; failure once: "Terminal config not read from the host: <said>. The pane keeps its defaults." `components/ThreadTerminalDrawer.tsx:337`
- W2-TRM-044 `workspaces.touch`: per 30 s typing. `terminal/wiring.ts:138`
- W2-TRM-045 `workspaces.wake`: Wake. `terminal/paneWords.ts:74`
- W2-TRM-046 `workspaces.restartDaemon`: Start again. `components/DaemonDown.tsx:37`
- W2-TRM-047 `preferences.set`: zoom. `terminal/fontSetting.ts:65`
- W2-TRM-048 Push `daemon.event`, `daemon.closed`. `terminal/daemon-link.ts:177`
- W2-TRM-049 Gap: frames `pty.create`, `pty.attach`, `pty.write`, `pty.resize`, `pty.kill`, `pty.list`, `pty.tab`, `pty.thread`, `ping`, untyped. `terminal/link.ts:252`
- W2-TRM-050 Gap: events `pty.data`, `pty.exit`, `pty.mode`, untyped. `terminal/link.ts:171`

### Design

- W2-TRM-051 Grid, cursor, selection, link underline: none yet. Port `terminal/ghostty/renderer.ts`.
- W2-TRM-052 Toolbar `ui::Button::icon_xs`, `ui::shortcut_tooltip`; strip none yet.
- W2-TRM-053 Tabs, overlays, scrollbar: none yet. Buttons `ui::Button::outline` xs, input `ui::Field`.
- W2-TRM-054 Motion: scrollbar thumb, background-color, 120 ms, ease-out. `terminal/ghostty/surface/terminalSurface.ts:192`
- W2-TRM-055 Motion: toolbar, outline button colors, font input border, 150 ms, default curve. `components/ThreadTerminalDrawer.tsx:1081`

## Pull request, Changes and Files (#2397)

Sources: `diffs/`, `components/diffs/`, `pull-request/`, `files/`, `components/files/`. Data the driver writes into `bench`: A, a branch off main with one change; B, five changes over committed fixtures (edit, delete, rename with an edit, binary, add); C, one long nested path; D, 300 new files; E, a file over 2 MB. Code and diffs per D3, markdown per D4.

### Elements

- W2-PRP-001 Changes head: scope menu, crumbs, branch, pin; Send to thread, +/- count, Commit, refresh, fold all, stacked or split. `diffs/DiffSurface.tsx:428`
- W2-PRP-002 Commit box: message, drafting spinner, no-draft note or error, "The agent is still working", Cancel, Commit. `diffs/CommitBox.tsx:23`
- W2-PRP-003 Changed files fold: count, "N of M viewed", 224 px tree; row: chevron, glyph, name, +/-, commit tick, controls. `diffs/DiffSurface.tsx:603`
- W2-PRP-004 File controls: edit, save, stop, discard, viewed tick. `diffs/FileControls.tsx:35`
- W2-PRP-005 Code view: sticky 32 px heads, hunks, "N unmodified lines" bars expand, unified or split, wrapped. `components/diffs/StyledDiffCodeView.tsx:77`
- W2-PRP-006 Line comment: field, "⌘/Ctrl Enter to send", Cancel, Comment; kept with delete. `components/diffs/DiffCommentAnnotation.tsx:87`
- W2-PRP-007 Discard dialog: title, "This cannot be undone." or a new file's deletes note, Cancel, red Discard. `diffs/DiscardDialog.tsx:28`
- W2-PRP-008 Not running: Start, link word, or "The thread is not running." over the computer's or pane's line. `diffs/NotRunning.tsx:16`
- W2-PRP-009 Files: crumbs, root chevron, pin, New thread here; refresh, Search files, tree, "… N more entries not shown", folder errors. `files/FilesSurface.tsx:72`
- W2-PRP-010 File tab: crumbs, markdown toggle, reload, Open in editor, 2 MB note, code with a lit line, or markdown. `files/FilePreviewSurface.tsx:94`
- W2-PRP-011 Finder: Find a file or Search in files, hits, Open foot, "more match" note. `files/FileFinder.tsx:150`
- W2-PRP-012 PR head: glyph, title, face, "opened 3 h ago", "#N" link, state word, "base ← branch", labels, "Review asked of", source link, refusal; strip refresh; tabs. `pull-request/PullRequestSurface.tsx:195`
- W2-PRP-013 Status: checks fold (failed with Fix, running for, rest grouped), Review verdicts, agent button, "N unresolved" with Send to agent, behind with Update or Conflicts with Fix, hold line, Merge; settled: merged by, landed sha, Open on GitHub. `pull-request/Status.tsx:88`
- W2-PRP-014 Activity rail: comments, bot notice Show or Hide, reviews, pushes, sent time, hover acts; threads: code tail, Resolve, Reply, resolved folded; reactions; comment box. `pull-request/Conversation.tsx:338`
- W2-PRP-015 PR Files: count, fold all, 30 px rows, a file's diff under its row, threads, Open in Changes; no diff, 2 MB cut. `pull-request/Files.tsx:122`
- W2-PRP-016 Draft review: verdict, summary, rows (tick, path:line, body, Send to thread), Post or "Posted ...". `pull-request/ReviewDraftSection.tsx:115`
- W2-PRP-017 Review with an agent dialog: Codex or Claude, note or refusal, Start. `pull-request/ReviewDialog.tsx:36`
- W2-PRP-018 Header git button: quick act, held with why on hover, menu Commit, Push, PR. `pull-request/GitSplit.tsx:79`

### States

- W2-PRP-019 No PR: no Pull request card; git button "Commit, push and PR". Shot: data A, the launcher open. `panes.ts:102`
- W2-PRP-020 Loading spinner. No shot: one git.diff round trip. `diffs/DiffSurface.tsx:586`
- W2-PRP-021 Empty: "No uncommitted changes at <folder>." Shot: clean. `diffs/DiffSurface.tsx:598`
- W2-PRP-022 Not a git repository. No shot: needs the daemon's home outside any git repo. `diffs/DiffSurface.tsx:590`
- W2-PRP-023 Modified, added, deleted, renamed, binary. Shot: B. `diffs/model.ts:61`
- W2-PRP-024 Long paths. Shot: C. `components/chat/ChangedFilesTree.tsx:258`
- W2-PRP-025 300 files. Shot: D. `diffs/DiffSurface.tsx:621`
- W2-PRP-026 Over 2 MB: cut line, files with no patch; all cut line. Shot: E. `diffs/DiffSurface.tsx:564`
- W2-PRP-027 Split view. Shot: B. `diffs/DiffSurface.tsx:555`
- W2-PRP-028 Comments, draft and kept. No shot: the gutter's + sits in a shadow root the driver cannot click. `components/diffs/AnnotatableCodeView.tsx:307`
- W2-PRP-029 Viewed, "1 of 5 viewed". Shot: B. `diffs/DiffSurface.tsx:615`
- W2-PRP-030 Commit box. Shot: B. `diffs/CommitBox.tsx:32`
- W2-PRP-031 Discard dialog. Shot: B. `diffs/DiscardDialog.tsx:34`
- W2-PRP-032 Editing a file. Shot: B. `diffs/DiffSurface.tsx:407`
- W2-PRP-033 Narrow: head controls hide under 640 px; panel is a sheet. Shot: B. `diffs/DiffSurface.tsx:452`
- W2-PRP-034 Read refused. No shot: git must refuse inside a repo. `diffs/DiffSurface.tsx:574`
- W2-PRP-035 Not running. No shot: nothing stops bench's daemon. `diffs/NotRunning.tsx:33`
- W2-PRP-036 This turn (turn card's Open diff), snapshot gone. No shot: stub turns change no files. `diffs/DiffSurface.tsx:478`
- W2-PRP-037 Files tree, search typed. Shot: search typed in the tree. `components/files/FileBrowserPanel.tsx:188`
- W2-PRP-038 Folder past 10,000 entries. No shot: the tree's rows sit in a shadow root the driver cannot click. `components/files/fileTreeRows.ts:61`
- W2-PRP-039 File at a line. Shot: B. `files/FilePreviewSurface.tsx:155`
- W2-PRP-040 Markdown rendered. Shot: the committed fixtures. `files/FilePreviewSurface.tsx:150`
- W2-PRP-041 Binary file. Shot: the committed fixtures. `files/FilePreviewSurface.tsx:146`
- W2-PRP-042 File over 2 MB. Shot: E. `files/FilePreviewSurface.tsx:135`
- W2-PRP-043 Finder with hits. Shot: the committed fixtures. `files/FileFinder.tsx:150`
- W2-PRP-044 Open PR, checks running and failed. No shot: GitHub. `pull-request/Status.tsx:91`
- W2-PRP-045 PR diff with comments and replies. No shot: GitHub. `pull-request/Files.tsx:64`
- W2-PRP-046 Review draft. No shot: `workspaces.review` reads GitHub. `pull-request/ReviewDraftSection.tsx:64`
- W2-PRP-047 PR why line, skeletons, unread, merged, closed, cut notes. No shot: GitHub. `pull-request/PullRequestSurface.tsx:156`
- W2-PRP-048 Commit in Uncommitted only; no discard in Branch changes or a turn; no edit in Staged, deleted, binary, cut. Seen in W2-PRP-023's shot. `diffs/DiffSurface.tsx:402`

### Actions and keys

- W2-PRP-049 `mod+alt+b` panel; launcher D Changes, R Pull request, F Files; + menu. `components/RightPanelTabs.tsx:166`
- W2-PRP-050 Scope: Branch changes (default), Uncommitted, Working tree, Staged; kept on reload. `diffs/DiffSurface.tsx:441`
- W2-PRP-051 Viewed tick marks the blob and folds; an edit unmarks. `diffs/DiffSurface.tsx:262`
- W2-PRP-052 Discard; a refusal stays in the dialog. `diffs/DiscardDialog.tsx:15`
- W2-PRP-053 Commit: draft asked, ticks, Commit, toast. `diffs/DiffSurface.tsx:313`
- W2-PRP-054 Comment: gutter + or drag lines, mod+Enter, Escape; Send to thread quotes into the composer. `components/diffs/DiffCommentAnnotation.tsx:132`
- W2-PRP-055 `mod+p`, `mod+shift+f` finder, Enter opens at the line; `mod+o` Open. `keybindingDefaults.ts:56`
- W2-PRP-056 PR: refresh, tabs, checks fold, Fix, Update, Merge, method, when checks pass, review Start; toasts. `pull-request/Status.tsx:107`
- W2-PRP-057 Git button: drafted commit then bring back; no draft opens Uncommitted; View PR. `pull-request/GitSplit.tsx:69`
- W2-PRP-058 PR acts only via this computer's gh; reactions undo on refusal. `pull-request/pageActs.ts:33`

### Host operations

- W2-PRP-059 `daemon.send` with `git.diff`, `git.turn`, `git.status`, `fs.write`. `diffs/DiffSurface.tsx:186`
- W2-PRP-060 `daemon.send` with `fs.list` (gitignore on), `fs.read`, `fs.search`. `files/listing.ts:59`
- W2-PRP-061 `workspaces.viewed`, push `workspace.viewed`, `workspaces.discard`. `diffs/DiffSurface.tsx:273`
- W2-PRP-062 `workspaces.commitDraft`, `.commit`, then `.checkout`. `diffs/DiffSurface.tsx:293`
- W2-PRP-063 Push `workspace.status`: PR and checkout. `protocol/store/useStore.ts:1129`
- W2-PRP-064 `workspaces.pullRequestView`, `.pullRequestDiff`. `pull-request/PullRequestSurface.tsx:87`
- W2-PRP-065 `workspaces.pullRequestSend`, `.pullRequestReply`, `.pullRequestResolve`, `.pullRequestReact`. `pull-request/pageActs.ts:43`
- W2-PRP-066 `workspaces.fix`, `.update`, `.merge`. `pull-request/acts.ts:19`
- W2-PRP-067 `workspaces.review`, `.reviewDraft`, `.reviewPost`, push `workspace.review`. `pull-request/ReviewDraftSection.tsx:41`
- W2-PRP-068 `workspaces.bringBack`. `pull-request/GitSplit.tsx:73`
- W2-PRP-069 `editor.open`, `editor.list`, `ssh.include`. `files/OpenInEditor.tsx:41`

### Design

- W2-PRP-070 Status card `ui::Card`, `ui::section_head`; rows `ui::Line`; sub rows none yet.
- W2-PRP-071 PR glyph `ui::glyph_frame`; buttons `ui::Button`; dialogs `ui::Dialog`, `ui::quiet`; `ui::KeyCaps`; `ui::spinner`.
- W2-PRP-072 None yet: code view, trees, crumbs, tabs, checkboxes, text areas, menus, skeleton, faces, chips.
- W2-PRP-073 Motion: file body, opacity in on mount, 200 ms, ease-out; reduced: off. `components/diffs/StyledDiffCodeView.tsx:242`
- W2-PRP-074 Motion: running dot, opacity 1 to 0.35, 1.6 s, ease-in-out, loops; reduced: off. `index.css:1264`
- W2-PRP-075 Motion: skeleton, translateX, sweep then hold, 2 s, linear, loops; reduced: off. `index.css:274`
- W2-PRP-076 Motion: spinners, rotate, 1 s, linear, loops. `diffs/DiffSurface.tsx:516`
- W2-PRP-077 Motion: chevrons, rotate 90°, 150 ms, default curve. `diffs/DiffSurface.tsx:610`
- W2-PRP-078 Motion: hovers, colors, 150 ms, default curve. `components/chat/ChangedFilesTree.tsx:196`

## Slates (#2398)

Sources: `slate/` and the catalog, `protocol:slate/kit.ts:224` (31 pieces). The driver writes `data/*.slate` with `wsp slate write`; `every.slate` holds each catalog piece once. Formulas per D2, charts per D8, diagrams per D9.

### Elements

- W2-SLT-001 Piece box `[data-slate-piece][data-slate-type]`; draw error "This part could not be drawn". `slate/SlateView.tsx:126`
- W2-SLT-002 Layout: column gathers rows into cards; row; grid 2-4 cols; 2+ numbers make a stat strip. `slate/pieces/runs.tsx:99`
- W2-SLT-003 Section: quiet head, note, chevron fold, refresh glyph. `slate/pieces/section.tsx:82`
- W2-SLT-004 Heading: section-head style at every level. `slate/pieces/heading.tsx:15`
- W2-SLT-005 Text: tone, size, mono, placeholder. `slate/pieces/text.tsx:38`
- W2-SLT-006 Markdown: restricted, images as links, no KaTeX. `slate/pieces/markdown.tsx:16`
- W2-SLT-007 Number: label, 26 px mono figure, unit, note with status word. `slate/pieces/number.tsx:49`
- W2-SLT-008 Meter and ring: label, note, 56x4 track, figure. `slate/pieces/meter.tsx:23`
- W2-SLT-009 Bars: head, card rows of name, track, figure; 2+ share a segmented switch. `slate/pieces/bars.tsx:37`
- W2-SLT-010 Sparkline: label, 64x16 line, last figure. `slate/pieces/sparkline.tsx:20`
- W2-SLT-011 Chart: legend, y figures, dashed grid, canvas dither, lines, end dot, end x ticks; stacked bands. `slate/pieces/chart.tsx:77`
- W2-SLT-012 Donut: 5 parts plus Other, whole inside, legend figure and percent. `slate/pieces/donut.tsx:31`
- W2-SLT-013 Timeline: group heads, name, bar on dashed clock, length or state word, ticks. `slate/pieces/timeline.tsx:101`
- W2-SLT-014 Treemap: squarified tiles, name and size, hover title, group legend. `slate/pieces/treemap.tsx:74`
- W2-SLT-015 Diagram: caption, Expand, Mermaid in zoom frame; dialog: zoom out, in, fit, close. `slate/pieces/diagram.tsx:103`
- W2-SLT-016 Image: framed picture at its ratio (cap 420 px), caption, Expand. `slate/pieces/image.tsx:297`
- W2-SLT-017 Images: gallery, sideways strip, before and after slider. `slate/pieces/image.tsx:438`
- W2-SLT-018 Table: header, card rows, figures mono at end, row buttons or chevron row; 3+ text columns fold. `slate/pieces/table.tsx:230`
- W2-SLT-019 Checklist: checkbox, title muted when done, note. `slate/pieces/checklist.tsx:35`
- W2-SLT-020 Facts: wrapping line, or grid of lines. `slate/pieces/facts.tsx:53`
- W2-SLT-021 Status word in tone; chip word. `slate/pieces/status.tsx:18`
- W2-SLT-022 Output: label, run word (crab while running), stale note, Cancel, mono log tail, why. `slate/pieces/output.tsx:79`
- W2-SLT-023 Button: keycap kinds, xs in a row, held tooltip, outcome line. `slate/pieces/button.tsx:65`
- W2-SLT-024 Input: label and field, textarea, password, secret dots, Enter hint, conflict row. `slate/pieces/input.tsx:154`
- W2-SLT-025 Select: label and select; bare in a toolbar. `slate/pieces/select.tsx:39`
- W2-SLT-026 Choices: question row, option rows, check or Right and Wrong. `slate/pieces/choices.tsx:95`
- W2-SLT-027 Toggle: label, note, switch; segmented All/label beside a select. `slate/pieces/toggle.tsx:58`
- W2-SLT-028 Empty: quiet sentence, one button at right. `slate/pieces/empty.tsx:13`
- W2-SLT-029 Pane entry: Slate, PanelsTopLeft, letter S, description, held reason. `panes.ts:156`
- W2-SLT-030 Frame: header, held rows, sheets, scroll, 760 px column, 16 px in. `slate/SlateSurface.tsx:101`
- W2-SLT-031 Header: title (or Slate) 13 px muted, truncated, spinner, "Slate menu" button. `slate/SlateSurface.tsx:141`
- W2-SLT-032 Menu: Undo vN (if undoable), Clear, Stop runs (if runs), Forget secrets (if secrets), Approvals, Copy as text. `slate/SlateSurface.tsx:148`
- W2-SLT-033 Refusal line under the header, error ink. `slate/SlateSurface.tsx:162`
- W2-SLT-034 Held row: "This slate wants to run", command mono, Review, Not now; limit hold: why. `slate/consent.tsx:135`
- W2-SLT-035 Body: a host per piece id; failed piece: quiet line; unknown type: its fallback or "This part needs a newer wsp". `slate/SlateView.tsx:126`
- W2-SLT-036 Empty: "Nothing here yet", lead, ask line, Ask for one, Undo, held note, refusal. `slate/SlateSurface.tsx:304`
- W2-SLT-037 Sheet "Run this command?": command, pipe, files, cadence, env, args, where, author, refusal; Not now, Always in this thread, Run once. `slate/consent.tsx:206`
- W2-SLT-038 Confirm sheet: alert, confirm text, Not now, destructive Run. `slate/consent.tsx:216`
- W2-SLT-039 Batch sheet "Run these N commands?": switch rows (off 60%); Not now, Allow all/N, Run once. `slate/approvals.tsx:50`
- W2-SLT-040 Tool sheet "Let this panel run <tool> on <Server>?": Show code fold; Not now, Allow. `slate/mcp.tsx:143`
- W2-SLT-041 Link prompt "Open <domain> from this slate?": Not now, Always for this domain, Open once. `slate/consent.tsx:27`
- W2-SLT-042 Approvals dialog: card rows (mono, "Refused.", "Run $x"), Revoke, refusal. `slate/standing.tsx:26`
- W2-SLT-043 Forget secrets alert: Cancel, destructive Forget secrets. `slate/SlateSurface.tsx:175`
- W2-SLT-044 Tool result by shape: text, table, facts, heads. `slate/shape.tsx:16`

### States

- W2-SLT-045 Every piece once. Shot: every.slate. `slate/SlateView.tsx:113`
- W2-SLT-046 Empty data, one point, steady chart. Shot: empty.slate. `slate/pieces/chart.tsx:91`
- W2-SLT-047 Long text, folded table. Shot: long.slate. `slate/pieces/table.tsx:165`
- W2-SLT-048 Many rows: 200 drawn and "and 50 more", 400 points. Shot: 250 rows and 400 points, built in states.mjs. `slate/pieces/table.tsx:155`
- W2-SLT-049 Failed images (missing, not an image, domain ask, 61 images, compare of 3) and diagrams (parse, address). Shot: failed.slate. `slate/pieces/image.tsx:210`
- W2-SLT-050 Held controls. Shot: held.slate, hover Deploy. `slate/pieces/button.tsx:86`
- W2-SLT-051 Toolbar, stat strip, bar switch, stack, row open, compare, strip. Shot: variants.slate. `slate/pieces/runs.tsx:95`
- W2-SLT-052 Choices answered wrong. Shot: pick Link. `slate/pieces/choices.tsx:84`
- W2-SLT-053 Chart hover tooltip. Shot: hover last point. `settings/usageChart.tsx:255`
- W2-SLT-054 Diagram dialog. Shot: click Expand. `slate/pieces/diagram.tsx:116`
- W2-SLT-055 Image dialog with "1 of 3". Shot: click a gallery thumb. `slate/pieces/image.tsx:263`
- W2-SLT-056 Section folded. No shot: near duplicate, one chevron and a hidden body. `slate/pieces/section.tsx:85`
- W2-SLT-057 Input conflict row. No shot: the driver's blur races the host's write. `slate/pieces/input.tsx:165`
- W2-SLT-058 Wide panel: chart 260 px, treemap 300, donut 148, timeline names 9rem. No shot: the width is read once per load, and the driver does not reload. `slate/pieces/chart.tsx:35`
- W2-SLT-059 Runs idle. Shot: runs.slate on a fresh thread. `slate/SlateSurface.tsx:108`
- W2-SLT-060 Approval prompt. Shot: Run quick. `slate/SlateSurface.tsx:272`
- W2-SLT-061 Running, streaming. Shot: Run slow. `slate/store.ts:213`
- W2-SLT-062 Done. Shot: Run quick. `slate/engine.ts:168`
- W2-SLT-063 Failed. Shot: Run broken. `slate/engine.ts:168`
- W2-SLT-064 Cancelled. Shot: Stop runs. `slate/SlateSurface.tsx:129`
- W2-SLT-065 Refusal, Not now. No shot: near duplicate of W2-SLT-064; its reason is not drawn. `runtime:slate-runs.ts:922`
- W2-SLT-066 Press said "Sent". Shot. `slate/pieces/press.ts:29`
- W2-SLT-067 Press refused. Shot: ftp link. `slate/actions.ts:176`
- W2-SLT-068 Confirm sheet. Shot: Run danger. `slate/consent.tsx:216`
- W2-SLT-069 Batch sheet. Shot: timers.slate on a fresh thread. `slate/SlateSurface.tsx:263`
- W2-SLT-070 Held row after Escape. Shot. `slate/consent.tsx:147`
- W2-SLT-071 Limit hold. Shot: Start five. `runtime:slate-runs.ts:411`
- W2-SLT-072 Refreshing, long title cut. Shot: timers. `slate/SlateSurface.tsx:143`
- W2-SLT-073 Link prompt. Shot. `slate/SlateSurface.tsx:276`
- W2-SLT-074 Approvals with rows. Shot. `slate/standing.tsx:40`
- W2-SLT-075 Menu open. Shot. `slate/SlateSurface.tsx:148`
- W2-SLT-076 Forget secrets alert. Shot. `slate/SlateSurface.tsx:159`
- W2-SLT-077 No slate. Shot: a fresh thread with no slate. `slate/SlateSurface.tsx:304`
- W2-SLT-078 Cleared. Shot: Clear. `slate/SlateSurface.tsx:317`
- W2-SLT-079 Held note. No shot: stub docks no prompt. `slate/SlateSurface.tsx:331`
- W2-SLT-080 Rewound, newer, unknown type. No shot: host refuses or needs a rewind. `slate/SlateSurface.tsx:295`
- W2-SLT-081 Tool sheet. No shot: no MCP server. `slate/mcp.tsx:143`
- W2-SLT-082 Header refusal, Updated line. No shot: Undo off when refused; stub writes no slate. `slate/SlateSurface.tsx:162`
- W2-SLT-083 Loading or failed get: blank. No shot: too brief. `slate/SlateSurface.tsx:98`

### Actions and keys

- W2-SLT-084 Chart hover: tooltip with x word and each figure, crosshair, dot follows. `settings/usageChart.tsx:251`
- W2-SLT-085 Input: Enter (cmd/ctrl Enter multi-line) submits, not during IME. `slate/pieces/input.tsx:127`
- W2-SLT-086 Select, choices (arrows move and pick), toggle: write value, raise change. `slate/pieces/choices.tsx:66`
- W2-SLT-087 Button, row action, row open: busy until answered, hover lists steps. `slate/pieces/press.ts:21`
- W2-SLT-088 Output Cancel. `slate/pieces/output.tsx:93`
- W2-SLT-089 Open: launcher card, add menu, key S; a never-shown slate opens once on select. `slate/store.ts:176`
- W2-SLT-090 Menu: write undo/clear, cancel running runs, blank secrets, copy sketch. `slate/SlateSurface.tsx:124`
- W2-SLT-091 Review opens the sheet; row Not now refuses. `slate/SlateSurface.tsx:209`
- W2-SLT-092 Batch answers each switched key; late rows start off. `slate/approvals.tsx:38`

### Host operations

- W2-SLT-093 `slates.get`: the record. `slate/wire.ts:56`
- W2-SLT-094 `slates.state`: values, secrets. `slate/wire.ts:57`
- W2-SLT-095 `slates.event`: press, submit, change. `slate/wire.ts:58`
- W2-SLT-096 `slates.approve`: sheets, link domains. `slate/wire.ts:59`
- W2-SLT-097 `slates.cancel`: Stop, Cancel. `slate/wire.ts:60`
- W2-SLT-098 `slates.revoke`: Revoke. `slate/wire.ts:61`
- W2-SLT-099 `slates.shown`: first showing. `slate/wire.ts:62`
- W2-SLT-100 `slates.read`: Copy as text. `slate/wire.ts:63`
- W2-SLT-101 `slates.write`: undo, clear. `slate/wire.ts:64`
- W2-SLT-102 `slates.subscribe`, `slates.unsubscribe`: shown hold, git, pr. `slate/SlateSurface.tsx:95`
- W2-SLT-103 `slates.resolve`: missing paths. `slate/wire.ts:68`
- W2-SLT-104 Push `session.slate`, `slate.values`, `slate.run`, `session.done`: no gap. `protocol/store/useStore.ts:1176`
- W2-SLT-105 `usage.accounts`: usage source. `slate/sources/usage.ts:28`

### Design

- W2-SLT-106 `ui::Card` at 16 px inset, heads `ui::section_head`; rows as `ui::Row`, `ui::Line` but 44 px floor, not 60, 48.
- W2-SLT-107 `ui::Button` (danger none yet), `ui::Select`, `ui::Field`, `ui::spinner`, `ui::Crab`; checklist `ui::PickRow` (it mutes unticked, not done); choices `ui::Choice` (it has a radio, no tick).
- W2-SLT-108 None yet: chart, donut, timeline, treemap, sparkline, meter track, diagram, image, zoom frame, dialogs, switch, segmented control, status word, DigitRoll.
- W2-SLT-109 Motion: chart lines and scale tween from old points, 520 ms, ease-out cubic. `settings/usageChart.tsx:75`
- W2-SLT-110 Motion: number figure, digit roll, 500 ms, cubic-bezier(0.23,1,0.32,1). `components/ui/digit-roll.tsx:56`
- W2-SLT-111 Motion: meter fill, width and color, 200 ms, cubic-bezier(0.23,1,0.32,1). `slate/pieces/meter.tsx:38`
- W2-SLT-112 Motion: table row and choice hover, colors, 150 ms, default curve. `slate/pieces/table.tsx:89`
- W2-SLT-113 Motion: segmented control, colors, 150 ms, default curve. `components/ui/segmented-control.tsx:49`
- W2-SLT-114 Menu, held row, empty state, code block, fold, switch, destructive button: none yet.
- W2-SLT-115 ui::Button, ui::spinner, ui::Card, ui::Row, ui::Dialog (560 px; today 440).
- W2-SLT-116 Motion: dialog scale .98, opacity, 200 ms ease-in-out; backdrop 200 ms default. `components/ui/dialog-styles.ts:6`
- W2-SLT-117 Motion: switch track 200 ms default; thumb .15 s, CSS ease. `components/ui/switch.tsx:12`
- W2-SLT-118 Motion: piece fade-in once, opacity 150 ms ease-out, off for reduced motion. `index.css:1726`

## Setup dialogs, onboarding and the pair screen (#2399)

Sources: `settings/add/`, the confirm dialogs, `desktop:onboarding.html`, `desktop:starting.html`, `PairScreen.tsx`, `BootGate.tsx`, `shell/FirstRun.tsx`. Startup per D5, the confirm frame per D10.

### Elements

- W2-DLG-001 Step dialog: 560 by 640 px; head (title, line, mono "n of N", "n of N done"), cards, foot. `settings/add/AddComputerDialog.tsx:824`
- W2-DLG-002 Where: mono address field, ssh host radios; host key note, copy row, Trust and connect. `settings/add/AddComputerDialog.tsx:55`
- W2-DLG-003 Checks: seven rows; failed row: said, fix, sudo field, Try again. `settings/add/setup.ts:246`
- W2-DLG-004 Step row: mark, name, note, mono time ticking, chevron, opened part (refusal, log, acts), items indented. `settings/add/StepRow.tsx:60`
- W2-DLG-005 Start from radios; pick tools (Filter, Order, All, None, Invert); pick row (checkbox, glyph, name, tag, note, slot, Name, Icon, Colour). `settings/add/PickList.tsx:43`
- W2-DLG-006 Pick steps: Agents, MCP servers (copy or leave keys), CLIs (tick used), Skills, Plugins, GitHub, Import projects (name taken), Other config. `settings/add/PickLists.tsx:80`
- W2-DLG-007 Summary: lines card, disk line, disk short refusal, Save as a recipe. `settings/add/AddComputerDialog.tsx:203`
- W2-DLG-008 Running view: setup rows, sign-in wait (code, Open the tab again, Skip for now; ran out: Retry), log. `settings/add/AddComputerDialog.tsx:342`
- W2-DLG-009 Ready page: hero ink wash, glyph tile, "X is ready", line, primary, Close. `settings/add/AddComputerDialog.tsx:503`
- W2-DLG-010 Cloud key step: Make a key card (keys page, actions copy row, cost), key field, check line. `settings/add/AddCloudDialog.tsx:94`
- W2-DLG-011 Cloud provider radios if several, picks, locked rows, Sign-ins on here, build rows, ready page. `settings/add/AddCloudDialog.tsx:160`
- W2-DLG-012 Confirm frame: 440 px, title, sentence, refusal slot, Cancel, destructive. `components/ui/alert-dialog.tsx:48`
- W2-DLG-013 Remove a computer: holdings sentence, leave line, slot (reading, unsaved), sudo field, Remove, Remove anyway, Forget (key refused), Done. `settings/RemoveComputerDialog.tsx:139`
- W2-DLG-014 Delete thread (processes read on open, Keep running, Stop and delete, standing line); delete or forget workspace (unpushed lines). `components/DeleteThreadDialog.tsx:74`
- W2-DLG-015 Onboarding welcome: accent glow, halo, tilde mark, Get started. `desktop:onboarding.html:142`
- W2-DLG-016 Onboarding agents: heading, sentence, rows (checkbox, glyph, name, state), hint, refusal, Open wsp or Check again. `desktop:onboarding.html:173`
- W2-DLG-017 Starting window: spinner, "Starting wsp", always dark. `desktop:starting.html:30`
- W2-DLG-018 First run: mark, title, sentence, Add a project, usage line if on, Privacy settings. `shell/FirstRun.tsx:20`
- W2-DLG-019 Pair screen: heading, hint, code field, Pair, two-line error; blank booting page. `PairScreen.tsx:28`
- W2-DLG-020 Failed start card "Could not start X", failed row, Open that thread. `shell/FailedStart.tsx:36`

### States

- W2-DLG-021 Where, empty. Shot: Settings, Computers. `settings/add/AddComputerDialog.tsx:664`
- W2-DLG-022 Where, 30 hosts, a long alias. Shot: ~/.ssh/config. `settings/add/AddComputerDialog.tsx:68`
- W2-DLG-023 Where, narrow: bottom sheet. Shot: 600 px window. `settings/add/StepDialog.tsx:17`
- W2-DLG-024 Checks running. Shot: unroutable address. `settings/add/setup.ts:266`
- W2-DLG-025 Checks refused. Shot: unknown host. `settings/add/AddComputerDialog.tsx:82`
- W2-DLG-026 Host key, sudo, picks, summary, disk short, running, sign-in wait, failed, needs you, ready. No shot: real SSH host. `settings/add/AddComputerDialog.tsx:666`
- W2-DLG-027 Cloud key step. Shot: Add a cloud provider. `settings/add/AddCloudDialog.tsx:488`
- W2-DLG-028 Key refused or unreachable. No shot: needs the live provider: the host checks a Boat key against boat.dev directly (packages/host/src/providers-key.ts takes BOX_API_URL from packages/engine/src/box-backend.ts:25, with no override), so no capture can reach this state without calling Boat; the app's own test covers it with a fixture answer. `settings/add/AddCloudDialog.tsx:60`
- W2-DLG-029 Key held, cloud picks, summary, build, ready. No shot: Boat account. `settings/add/AddCloudDialog.tsx:466`
- W2-DLG-030 Remove a computer. No shot: second computer. `settings/RemoveComputerDialog.tsx:144`
- W2-DLG-031 Delete thread. No shot: opens from a native menu. `components/DeleteThreadDialog.tsx:76`
- W2-DLG-032 Onboarding welcome. No shot: needs a second launch on an empty home. `desktop:onboarding.html:142`
- W2-DLG-033 Agents found. No shot: as W2-DLG-032. `desktop:onboarding.html:336`
- W2-DLG-034 None ticked. No shot: as W2-DLG-032. `desktop:onboarding.html:326`
- W2-DLG-035 No agent found. No shot: as W2-DLG-032. `desktop:onboarding.html:320`
- W2-DLG-036 Config refused tools. No shot: as W2-DLG-032. `desktop:onboarding.html:376`
- W2-DLG-037 Agents unread, shell refused. No shot: no fault to inject. `desktop:onboarding.html:337`
- W2-DLG-038 Starting window. No shot: needs a launch while the host is held. `desktop:main.ts:588`
- W2-DLG-039 Start failed box. No shot: native dialog. `desktop:starting.ts:38`
- W2-DLG-040 First run. No shot: needs a home with no project. `App.tsx:93`
- W2-DLG-041 Pair screen. No shot: needs a page with no desktop bridge. `BootGate.tsx:108`
- W2-DLG-042 Pair refused. No shot: as W2-DLG-041. `protocol/pairing.ts:59`
- W2-DLG-043 Failed start. No shot: failed cloud fork. `App.tsx:131`

### Actions and keys

- W2-DLG-044 Connect held until typed, no Enter; host pick fills field; Set up held while empty, starting, disk short. `settings/add/AddComputerDialog.tsx:749`
- W2-DLG-045 Onboarding: Get started or Enter; Open wsp or Enter installs, then opens. `desktop:onboarding.html:391`

### Host operations

- W2-DLG-046 `places.sshHosts`: suggested hosts. `settings/add/AddComputerDialog.tsx:578`
- W2-DLG-047 `places.add`: connect, with key, sudo password. `settings/adds.ts:90`
- W2-DLG-048 `places.choose`, `places.estimate`, `places.setup`, `places.skip`, `places.setupLog`, `places.list`, `recipes.options`, `recipes.save`: picks, setup, log, recipes. `settings/add/addFlow.ts:278`
- W2-DLG-049 `init.get`, `init.keys`, `init.start`, `init.step`, `init.answer`, `init.draft`, `init.build`, `providers.checkKey`, `agents.read`, `agents.signIn` family: cloud, sign-ins. `settings/add/AddCloudDialog.tsx:364`
- W2-DLG-050 `pair.redeem` on its own unauthed socket; `pair.issue` CLI only. `protocol/pairing.ts:66`
- W2-DLG-051 Push: place.stage, .setup, .joined, .changed, .pending, .present, .absent, .removed, .sync; init.job; agents.changed. `protocol/store/useStore.ts:974`
- W2-DLG-052 Gap: `onboarding:here`, `:agents`, `:install`, `:finish`, `titlebar:set`, `hosts:token`: Electron IPC. `desktop:main.ts:515`
- W2-DLG-053 `places.holds`, `places.remove`, `sessions.delete`, `workspaces.checkout`, `workspaces.delete`, `workspaces.forget`: confirms. `settings/RemoveComputerDialog.tsx:63`
- W2-DLG-054 `golden.get`, `workspaces.list`: onboarding check. `desktop:host-lifecycle.ts:472`

### Design

- W2-DLG-055 Step dialog `ui::Dialog`; summary `ui::Card`, `ui::Line`; copy rows `ui::CopyRow`.
- W2-DLG-056 Step rows `ui::StepRow`, lacks note, time, chevron, log, acts, refusal, indent, needs you, warned.
- W2-DLG-057 Radio rows `ui::Choice`; tick rows `ui::PickRow`; tools `ui::Field`, `ui::Select`, `ui::Button`.
- W2-DLG-058 Confirm frame, destructive button, refusal slot, device code, ready page, onboarding, starting, pair, first run: none yet.
- W2-DLG-059 Motion: backdrop opacity, 200 ms, default curve, blur 4 px. `components/ui/dialog-styles.ts:2`
- W2-DLG-060 Motion: popup scale 0.98, opacity, 200 ms, ease-in-out. `components/ui/dialog-styles.ts:5`

## Palette, switcher, activity and notices (#2400)

Sources: `components/palette/`, `components/switcher/`, `components/procs/`, `settings/activity/` (D11 splits it with #2391), `notices/`. Commands per D6.

### Elements

- W2-PAL-001 Palette: blurred backdrop, popup max-w-xl at 10vh, list max-h min(28rem,70vh). `components/palette/CommandPalette.tsx:228`
- W2-PAL-002 Search field, "Search commands, tasks, and threads..."; on a page, Back and its own ghost. `components/palette/CommandPalette.tsx:241`
- W2-PAL-003 Groups: Actions, Tasks, Recent threads (12); typing swaps Recent for Threads, In messages. `components/palette/CommandPalette.logic.ts:188`
- W2-PAL-004 Row: icon, title, description, trailing, shortcut, page chevron; held at 64% with reason. `components/palette/CommandPaletteResults.tsx:84`
- W2-PAL-005 Switcher: blurred backdrop, one row of up to 6 cards. `components/switcher/WorkspaceSwitcher.tsx:81`
- W2-PAL-006 Card: picture well or glyph, title, harness, computer, status. `components/switcher/WorkspaceSwitcher.tsx:121`
- W2-PAL-007 Computer table: filter, count, sortable head, 22 px mono tree rows, tags wsp, app, terminal, agent. `components/procs/ProcessesSurface.tsx:116`
- W2-PAL-008 Lit row: inspect fields; kill, end or stop turn; said line. `components/procs/ProcessesSurface.tsx:237`
- W2-PAL-009 Thread pane: Activity's Threads table, "Open in Activity". `components/procs/ThreadProcesses.tsx:98`
- W2-PAL-010 Activity views Threads, Processes, Kinds, Ports. `settings/activity/page.tsx:67`
- W2-PAL-011 Processes, Kinds, Ports tables. `settings/activity/tables.tsx:575`
- W2-PAL-012 Settle ask: list, Cancel, keep, stop. `components/procs/ProcessesLeft.tsx:94`
- W2-PAL-013 Toast: kind glyph, text, where, detail, action, X. `notices/Notice.tsx:46`
- W2-PAL-014 Toast stack, centre top right, 3 shown. `notices/Notice.tsx:108`
- W2-PAL-015 Actions: New thread (in...), Resume, Add a project, Copy as Markdown, moves, task acts, task and thread steps, sidebar, Add a computer, Settings, Activity, GitHub, docs, bug, right panel. `components/palette/paletteItems.tsx:198`

### States

- W2-PAL-016 Palette at rest. Shot: mod+k. `components/palette/CommandPalette.tsx:78`
- W2-PAL-017 Results. Shot: type "Parity". `components/palette/CommandPalette.logic.ts:146`
- W2-PAL-018 Message hits. Shot: "LegendList". `components/palette/CommandPalette.tsx:99`
- W2-PAL-019 No results; after > No matching actions. Shot: "zzqx". `components/palette/CommandPaletteResults.tsx:40`
- W2-PAL-020 New thread page: project rows, mod+1..9, Add a project; No matching projects. Shot. `components/palette/paletteItems.tsx:155`
- W2-PAL-021 Command running. No shot: closes before run. `components/palette/CommandPalette.tsx:191`
- W2-PAL-022 Switcher. Shot: hold ctrl+tab. `shell/shellCommands.ts:236`
- W2-PAL-023 Computer table. Shot: New thread, then Processes. `components/procs/ProcessesSurface.tsx:72`
- W2-PAL-024 Kill armed. No shot: the arm lasts 2 s, under the driver's settle. `components/procs/ProcessesSurface.tsx:302`
- W2-PAL-025 Thread pane, processes. Shot: a python server in a fresh thread. `components/procs/ThreadProcesses.tsx:98`
- W2-PAL-026 Thread pane empty. No shot: a thread's kept agent always lists. `components/procs/ThreadProcesses.tsx:94`
- W2-PAL-027 Activity Threads. Shot. `settings/activity/page.tsx:186`
- W2-PAL-028 Every process. Shot. `settings/activity/page.tsx:197`
- W2-PAL-029 Ports. No shot: on Boat no port row came within 40 s for a listening server; cause not found. `settings/activity/tables.tsx:648`
- W2-PAL-030 Row menu. No shot: native, off the page. `actions/contextMenu.ts:94`
- W2-PAL-031 Settle ask. Shot: mod+shift+e. `components/procs/ProcessesLeft.tsx:74`
- W2-PAL-032 Pending skeletons, away (pane: Start), refused. No shot: transient, dead daemon. `settings/activity/page.tsx:172`
- W2-PAL-033 Stop, End all asks; pane Stop <title>?. No shot: live turn, slate. `settings/activity/live.tsx:295`
- W2-PAL-034 Note toast. Shot: a --notify run while another thread shows. `notices/hostNotices.ts:348`
- W2-PAL-035 Done toast. Shot. `actions/threadActions.ts:392`
- W2-PAL-036 Error toast. Shot: mod+o. `files/openCopy.ts:23`
- W2-PAL-037 Waiting toast. No shot: permission ask. `notices/hostNotices.ts:309`
- W2-PAL-038 Narrow: under 640 px views a select, tiles 2 across; under 768 px CPU, Power, PID drop, menus shown. Shot: 600 px. `settings/activity/page.tsx:69`

### Actions and keys

- W2-PAL-039 mod+k, mod+shift+p; Search row; mod+t page. `keybindingDefaults.ts:54`
- W2-PAL-040 200 ms then message search, 2+ chars; ">" actions only. `components/palette/CommandPalette.tsx:45`
- W2-PAL-041 Arrows, Enter, click, Esc, backdrop, Backspace back, mod+digit. `components/palette/CommandPalette.tsx:202`
- W2-PAL-042 Switcher: ctrl+tab, mod+alt+arrows; paints after 150 ms; key up commits; Esc, blur cancel. `shell/KeybindingDispatcher.tsx:47`
- W2-PAL-043 Launcher P; row click, Enter, Space; sort; filter. `components/RightPanelTabs.tsx:73`
- W2-PAL-044 Kill arms 2 s, TERM tree, KILL at 5 s. `components/procs/kill.ts:23`
- W2-PAL-045 Toast: action, X, Esc, hover pause, swipe. `notices/Notice.tsx:97`

### Host operations

- W2-PAL-046 `sessions.search`: message hits. `protocol/client.ts:1174`
- W2-PAL-047 `sessions.history`: copy as Markdown. `protocol/client.ts:1145`
- W2-PAL-048 `sessions.settle`, `sessions.restore`. `protocol/client.ts:1170`
- W2-PAL-049 `sessions.interrupt`: stop turn. `protocol/client.ts:1155`
- W2-PAL-050 `activity.end`: kept agent. `protocol/client.ts:1153`
- W2-PAL-051 `appProcs.watch`, `appProcs.unwatch`, push `app.procs`. `protocol/client.ts:1133`
- W2-PAL-052 `daemon.open`, `daemon.send`: proc.*, container.stop, ports.watch. `machine/procs.ts:136`
- W2-PAL-053 `sys.subscribe`, `slates.get`, `editor.open`, `workspaces.restartDaemon`. `protocol/client.ts:1127`
- W2-PAL-054 Notice pushes: place.*, session.*, job.needs-you, init.job, workspace.gone, .woken, usage.alert, host.notice. `notices/hostNotices.ts:221`

### Design

- W2-PAL-055 Palette, switcher, toasts, tiles, table none yet; `ui::` KeyCaps, StatusMark, Crab, Card, glyph_frame, Select, Field, Dialog, spinner.
- W2-PAL-056 Motion: backdrop opacity 200 ms default; popup scale 0.98, opacity 200 ms ease-in-out. `components/ui/dialog-styles.ts:6`
- W2-PAL-057 Motion: toast in from -100% y, transform, opacity, height 400 ms ease, shadow 200 ms; back ones lift, shrink 5% each, hide content; back exit 500/200 ms; swipe 200 ms ease-out; X 150 ms. `notices/Notice.tsx:67`
- W2-PAL-058 Motion: Activity settle-in 180 ms cubic-bezier(0.23,1,0.32,1); skeleton sweep 2 s linear infinite. `index.css:185`

## Panes and pieces no ticket owns

The Computer pane, the Agents pane's wrapper, Aside, `DaemonDown`, the context menu and the Electron shell's own windows. D11 names an owner for each.

### Elements

- W2-UNO-001 Computer: Load, Memory, Disk charts; label, figure (load 2 places, "used of total"); memory, disk caution near full. `components/machine/MachineSurface.tsx:59`
- W2-UNO-002 Computer plot 96px: dashed rules 25/50/75%, solid top, base; 1.5px line, fading fill, newest dot; 60 slots from the right; load to window peak, min 1. `components/machine/MachineSurface.tsx:133`
- W2-UNO-003 Computer: chips (state, where, CPUs, memory; sub-machine: cost, delete timer); path truncated, tooltip. `components/machine/MachineSurface.tsx:196`
- W2-UNO-004 Computer, no thread: name and glyph, OS, cores, memory; no footer. `components/machine/MachineSurface.tsx:213`
- W2-UNO-005 Computer footer, as they apply: pause or wake, start daemon, rebuild, bring back, export, Delete, Forget (red hover); else none. `components/machine/MachineSurface.tsx:241`
- W2-UNO-006 Agents: full-width glyph tabs Agents, Tool servers, Skills, Plugins; on a page "Back to <tab>", slash, page name. `components/agents/AgentsPanel.tsx:79`
- W2-UNO-007 Side question: note line, question bubble at right (max 85%). `components/chat/AsideSurface.tsx:34`
- W2-UNO-008 Side question: crab, "Asking" then "Answering", seconds since asked; streamed words above; then markdown answer. `components/chat/AsideSurface.tsx:52`
- W2-UNO-009 Daemon down: reading's sentence, "Start again" ("Starting…"), red refusal under; terminal drawer, Changes, Processes. `components/DaemonDown.tsx:68`
- W2-UNO-010 Native context menu: rows, chord, group separators, check rows; hover: refusal, else hint. `desktop:context-menu.ts:39`
- W2-UNO-011 Tray (mac): icon, "?" if one waits; title count; count line, thread rows (state on computer; Open, Allow, Deny, Stop), "Open wsp", "Quit wsp"; lost: "wsp is not running", "Start wsp". `desktop:tray.ts:129`
- W2-UNO-012 Quit: "Quit wsp?"; Quit, Quit and stop wsp, Cancel; General's answer skips it; "wsp did not stop" box. `desktop:quit.ts:29`
- W2-UNO-013 Notifications if the window is unfocused or shut, click opens thread; dock badge: waiting threads. `desktop:needs-you.ts:63`
- W2-UNO-014 Move to Applications (mac): "Move to Applications", "Not now". `desktop:move.ts:31`
- W2-UNO-015 Older host: "restart it as <version>?" (Restart as, Quit); serves-again warning. `desktop:host-lifecycle.ts:403`

### States

- W2-UNO-016 Computer live, local thread. Shot: a fresh thread. `components/machine/MachineSurface.tsx:106`
- W2-UNO-017 Computer, this computer. Shot: New thread, then the pane. `components/machine/MachineSurface.tsx:43`
- W2-UNO-018 Computer long path. No shot: needs a second project in a deep folder. `components/machine/MachineSurface.tsx:176`
- W2-UNO-019 Computer pending. No shot: until the first sample, 2 s. `components/machine/MachineSurface.tsx:96`
- W2-UNO-020 Computer word for figures, plot dimmed, no dot: paused, waiting, deleted, unreachable, away word. No shot: cloud, sub-machine. `machine/live.ts:25`
- W2-UNO-021 Computer "unavailable", refusal as tooltip. No shot: daemon-read only. `components/machine/MachineSurface.tsx:104`
- W2-UNO-022 Computer sub-machine: no pause, rebuild, bring back; export "Copy to <here>". No shot: cloud. `components/machine/MachineSurface.tsx:238`
- W2-UNO-023 Agents list and item page. Shot. `components/agents/AgentsPanel.tsx:94`
- W2-UNO-024 Agents "Not answering" after 30 s silence, acts held. No shot: nothing stops a local daemon. `components/agents/AgentsSurface.tsx:39`
- W2-UNO-025 Side question asking. No shot: the stub claude cannot resume, so the host refuses at once. `components/chat/AsideSurface.tsx:52`
- W2-UNO-026 Side question streaming. No shot: as W2-UNO-025. `components/chat/AsideSurface.tsx:48`
- W2-UNO-027 Side question answered. No shot: as W2-UNO-025. `components/chat/AsideSurface.tsx:40`
- W2-UNO-028 Side question refused. Shot: /btw on a fresh thread. `components/chat/AsideSurface.tsx:44`
- W2-UNO-029 Daemon down: here "<name>'s terminals and files stopped", button; elsewhere "<name> is not answering". No shot: no op stops a daemon. `components/DaemonDown.tsx:52`

### Actions and keys

- W2-UNO-030 Agents: tab click; row opens its page; Escape steps back except in a field or an add page's control. `components/agents/AgentsPanel.tsx:67`
- W2-UNO-031 Side question: Esc or tab close removes it, restores prior tab, shuts a panel it opened; leaving the thread closes it; new `/btw` replaces it. `components/chat/asideStore.ts:70`

### Host operations

- W2-UNO-032 `sys.subscribe`, `sys.unsubscribe`: local readings while a pane shows them; push `workspace.sys`. `machine/hostLive.ts:53`
- W2-UNO-033 `daemon.send` with daemon op `sys.watch`; daemon push `sys.sample`: box, cloud, this computer. `terminal/wiring.ts:183`
- W2-UNO-034 `workspaces.nap`, `workspaces.wake`, `workspaces.stopWake`, `workspaces.restartDaemon`, `workspaces.rebuild`, `workspaces.bringBack`, `project.export`, `workspaces.delete`, `workspaces.forget`: footer, Daemon down; push `workspace.cost`. `protocol/client.ts:1083`
- W2-UNO-035 `agents.read`, push `agents.changed`; acts as Settings > Agents. `components/agents/useAgentsReport.ts:50`
- W2-UNO-036 `sessions.aside` with askId; push `aside.text`. `protocol/client.ts:1178`
- W2-UNO-037 Tray: `sessions.answer` (Allow, Deny), `sessions.interrupt` (Stop). `desktop:host-feed.ts:179`

### Design

- W2-UNO-038 Computer: plot, chips none yet (`ChartPlot` also #2392, #2400); footer `ui::Button::outline().xs()`, danger hover none yet.
- W2-UNO-039 Agents tabs: none yet (segmented control); back `ui::Button` Ghost; body #2391's pieces.
- W2-UNO-040 Side question: `ui::Crab`, #2394's markdown and bubble, refusal none yet. Daemon down: `ui::Button::outline().xs()`.

## Decisions for the coordinator to rule

Each has a recommendation and its reasons. No Wave 2 builder starts before these are ruled.

**D1. Terminal emulation: `libghostty-vt`.** Today's pane runs libghostty-vt as a 631 KB wasm vendored from t3code
at Ghostty 9f62873 (`terminal/ghostty/README.md:19`), drawn on a 2D canvas by our `renderer.ts`. The crate 0.2.2 is
MIT OR Apache-2.0 (fine in our AGPL app) and gives the same parser, widths, graphemes, selection, OSC 8 links and key,
mouse and paste encoders. Cost: Zig 0.16 at build (it fetches Ghostty; 0.2.2 pins a887df42, not 9f62873) and a C API
Ghostty calls not yet stable. `alacritty_terminal` 0.26 (Apache-2.0, pure Rust) is the runner-up: no grapheme
clusters, no key encoder, colour replies left to us, and Zed ships a fork, so it drifts from today's pane. Neither
size in our binary was measured. The Ghostty config needs no parser in the app: the host parses it
(`packages/collect/src/ghostty-config.ts`) and sends the fields over `host.terminalConfig`. A test host stages a real
wsp-daemon as CI does (`.github/workflows/flows.yml:66-74`): `musl-tools` and `gperf`, then in `daemon/`
`scripts/libseccomp-archive.sh musl` and `LIBSECCOMP_LIB_PATH=$PWD/target/libseccomp/musl cargo build --locked --release
--target x86_64-unknown-linux-musl -p wsp-daemon-bin`, then `node packages/wspx/scripts/daemon-binary.mjs --triple
x86_64-unknown-linux-musl` (1 min 23 s cold on this 8x16 Boat machine, a 9,037,400-byte binary). The host spawns it on the first
`daemon.open`; a test subscribes first (channel frames go only to the opener), then sends `pty.create`, `pty.attach`,
`pty.write` and waits for `pty.data`.

**D2. Slate formulas: the host evaluates and pushes values.** One TypeScript module is the language
(`protocol:slate/expr.ts`: parser :435, evaluator :963, checker :1074, 51 functions). The host already runs it for
batches, reads and `slates.resolve` (`runtime:slates.ts:1253`, `:1487`) but pushes only plain values; the window
evaluates again to draw. A generated parser covers only the grammar, so the 51 functions would be hand-copied, which
is ruled out. QuickJS keeps one source but has no `Intl`: 1234.5 at 2 decimals reads "1234.5" there, "1,234.50" in
Electron. Cost of the host road: a push of resolved props per piece and row (a protocol Gap), the window's scheduler
(`slate/engine.ts`) moving to the host, a host tick for `time.now`, and a bound input keeping its text while its write
is in flight.

**D3. Diffs and code: gpui-kit's `Diff` with tree-sitter.** Today: `@pierre/diffs` on Shiki 4.4.3 (242 grammars on
demand), themes `pierre-light` and `pierre-dark`, surfaces from app tokens
(`lib/diffRendering.ts:252`). gpui-kit's `Diff` is read-only, virtualised on GPUI's `list`, split or unified, with
annotations and highlighting off the UI thread. Use it with `tree-sitter-languages` (35 grammars plus JSON, already in
`app/Cargo.toml`); an unknown language draws plain, as today. Generate each theme's `highlight` block from the pierre
JSON and check it by a screenshot pair. Large diffs keep today's limits: the daemon's 2 MiB cut and its banner, plain
lines past 1,000 characters, virtual rows, the 2 MB file tab, 500 finder hits. Editing in place takes gpui-kit's code
editor input.

**D4. Markdown: gpui-kit's renderer with our styles.** `TextView::markdown` parses GFM; plugins take code block
chrome, alerts and details; math and Mermaid draw as source. The missing style knobs (lists, blockquote, rules) go
upstream as a small PR, not a fork. `gpui/main`'s thread view already uses it (`app/src/thread_view.rs`), so the PR
pane, slates and the thread view share one renderer and the prose styles of `index.css`'s `.chat-markdown`.

**D5. Startup: the frame (#2390) owns it.** It takes the single-instance lock per home, attaches to or starts the
host, shows the starting window after 800 ms (`desktop:starting.ts:9`) until the next window stands, then asks
`golden.get` and `workspaces.list`: a host holding nothing opens onboarding, else the app. The starting window needs
nothing from the host. Onboarding needs the computer's name, the agent scan and a per-agent install: the install is
`agents.addTools` here, but name and scan have no host op (W2-DLG-052), so #2399 adds two. It follows the system
colour scheme, as no preference exists yet. The pair screen draws only for a host GPUI holds no token for.

**D6. The palette's commands come from `screens/app_shell/keys.rs`.** Today a hand list
(`components/palette/paletteItems.tsx:195`) borrows only shortcut labels from the key table. Give each command id in
the key table a title, search words, icon, group, disabled reason and keep-open flag; the palette's chordless
commands (New thread in, Resume, Add a project, Copy as Markdown, thread moves, Add a computer, Activity, links) join
it with no chord. Threads, projects, message hits and a task's acts stay dynamic. Trap: the palette steps
`workspace.next` and `thread.next` directly; the key's action would open the switcher with a hold no key-up ends.

**D7. Motion: gpui-kit's `motion` module and a `ui::motion` file of our durations and curves.** Today's app has no
springs: `rg` over `apps/web/src` finds 138 `duration-*` classes in 87 files (101 at 150 ms) and 24 `@keyframes` in
`index.css`, and curves `cubic-bezier(0.23, 1, 0.32, 1)` (10 uses), `(0.16, 1, 0.3, 1)`, `(0.215, 0.61, 0.355, 1)`,
`(0.1, 0.5, 0.2, 1)`, and view-transition rules nothing triggers (`index.css:23-80`); each Design list gives its
screen's. GPUI's `with_animation` cannot retarget or reverse. `1bye/gpui-motion`'s core retargets but reruns the full
duration (150 ms reversed at 50 ms ends at 200 ms) and its adapter targets `gpui-ce`. `gpui_kit::base::motion` has a
CSS-like `Transition`, `Easing::cubic_bezier`, keyframes and reduced motion; when a hover ends it starts from the
current value and shortens the duration by CSS's reversing factor (gpui-kit `crates/base/src/motion.rs:325`). A second quick
reversal runs full length (a small upstream fix). An animated hover needs an element id and an `on_hover` that
notifies, since `.hover()` styles snap.

**D8. Charts: port `UsageChart` once.** The slate chart is the Usage page's (`settings/usageChart.tsx`: SVG lines, a
dithered canvas fill, a 520 ms tween from the old points). gpui-kit's charts reveal by a mask, do not tween, have no
dither and no treemap or timeline. #2392 builds the piece; #2398 uses it.

**D9. Slate diagrams: the source as code first**, the catalog's declared fallback, listed as a difference for the
owner's yes; a native Mermaid renderer is ruled separately.

**D10. Shared pieces with none yet.** #2399 adds a 440 px confirm frame (today's `max-w-110`; `ui::Dialog` is the
560 px step dialog) and a destructive button; #2398 a switch, a menu and a segmented control; #2397 a multi-line
field. Context menus are drawn by us, styled as `actions/ContextMenuHost.tsx`. Slate choices draw as today's, a tick
at the right, not as `ui::Choice`.

**D11. Owners for what no ticket names.** Computer pane and `DaemonDown`: #2400. Agents wrapper: #2391. Aside:
#2394 (the `/btw` send stays #2395's). Context menu: #2390. Tray, Quit question, notifications, dock badge, Move to
Applications, older-host prompts: #2402. `RemoveComputerDialog`: #2399, opened by Settings. `settings/activity/`:
#2391 the page frame, #2400 the model, tables and acts. `ChartPlot`: one `ui/` piece from #2392.

**D12. Switcher pictures.** Electron photographs the page (`desktop:previews.ts`); no host op does. Cards without a
picture until a capture road exists, listed as a difference.

## Baselines

`states.mjs` drives today's app to every Shot state for `parity/baseline.mjs --wave 2` (#2406), on the host
`bench/fixture.sh` makes from a `main` checkout with a wsp-daemon staged (D1). A state that changes a thread makes its
own with `wsp run`; data goes in through `wsp`, `preferences.set` and files in `bench`; slate documents are in
`data/`. A state that fits a box names it as `clip` (a pane, the drawer, a dialog), which keeps both themes near 22 MB;
`baseline.mjs` captures the whole window until it reads `clip`.
