# Parity spec: frame, Settings, Usage, sidebar, thread view, composer

What today's Electron app draws and does on six screens, which the GPUI app must match: the frame and layout,
Settings, Usage, the sidebar, the thread view and the composer. Read from `main` at 1516a0c (v0.3.8), `apps/web` and
`apps/desktop`, on 2026-10-18. Each item has an id that stays fixed; every state listed is required in the GPUI app.
`more.md` holds the items that did not fit this file, in the same id space; they count as listed.

How to read an item:

- `W1-<screen>-<n> what. path:line`. Paths are on main: bare ones under `apps/web/src/`, `desktop/` for
  `apps/desktop/src/`, `protocol/` for `apps/web/src/protocol/`, `packages/` for `packages/`.
- A state names its baselines, `<id>-paper.png` and `<id>-graphite.png`, which `parity/baseline.mjs --wave 1` writes,
  and its drive: the data it adds through the throwaway host, then the route and the clicks or keys. `png none` is a
  state the driver cannot reach, with the reason. A state may name another state's file when one view shows both.
  Every drive starts from the New thread view at 1280x800, Transparency off (a page capture holds no native glass),
  at the display's real scale.
- Keys: `mod` is Cmd on macOS and Ctrl elsewhere; the line is in `keybindingDefaults.ts`. The dispatcher
  (`shell/KeybindingDispatcher.tsx`) stands a rule down while a terminal, a text field or a browser tab owns the key.
- Host operations use the names in `apps/desktop-gpui/host-client/src/ops/`. `gap` is a call the host client lacks;
  `window` is an Electron preload call the GPUI app answers itself.
- Design: which look wins (today's app first, the locked designs only where it has no piece), the piece in
  `app/src/ui/` that draws it or "none yet", and what may be copied. gpui-kit and Zed may be read and copied; zui and
  Zeron are never copied.
- A deliberate difference is a place where the GPUI app may differ from today's on purpose; the port notes at the
  end name each one.

## Frame and layout

### Elements

- W1-FRM-001 Window 1280x800, title wsp, no minimum size, ground #09090b off Mac. `desktop/window.ts:130`
- W1-FRM-002 Mac: hiddenInset, traffic lights x 16 y 19 on the 52 px header, vibrancy sidebar. `desktop/window.ts:34`
- W1-FRM-003 Linux, listed desktops: no title bar; min, max, close over the header's right end. `desktop/window.ts:51`
- W1-FRM-004 Mac menu: wsp, File (Close window), Edit, View, Hosts, Window; none on Linux. `desktop/menu.ts:22`
- W1-FRM-005 Context menu: group separators, held rows dim with refusal tooltip, checkbox rows. `desktop/context-menu.ts:39`
- W1-FRM-006 Inset: banners, then centre column (header, notices slot, view) beside the panel. `shell/AppShell.tsx:150`
- W1-FRM-007 Sidebar slot 256 px, drag 220 to 480, right border. `shell/AppShell.tsx:139`
- W1-FRM-008 Rail: 16 px strip, 2 px line on hover, tooltip "Drag to resize sidebar". `components/ui/sidebar.tsx:616`
- W1-FRM-009 Header 52 px; sidebar away: toggle, New thread glyph; Mac left pad 85 px. `shell/HeaderRow.tsx:23`
- W1-FRM-010 Header right: thread slots, then drawer and panel toggles. `shell/AppShell.tsx:164`
- W1-FRM-011 Panel 540 px, 360 min, max 70% and row less 360, left border. `components/preview/PreviewPanelShell.tsx:82`
- W1-FRM-012 Sheet at 980 px and under: min(42vw, 448), 320 min; under 760 min(88vw, 384); 60% scrim. `rightPanelLayout.ts:7`
- W1-FRM-013 Tabs 24 px tall to 144 px, glyph, title; Add a panel, pane slot, toggles. `components/RightPanelTabs.tsx:375`
- W1-FRM-014 Launcher "Open a panel", 54 px tile per pane, held tiles dim. `components/RightPanelTabs.tsx:253`
- W1-FRM-015 Banners 40 px: not running (closed: Reload); sign-in bar. `shell/DisconnectedBanner.tsx:11`
- W1-FRM-016 First run: mark, title, Add a project, PostHog note. `shell/FirstRun.tsx:20`

### States

- W1-FRM-017 Home. png `W1-FRM-017-<theme>.png`; drive: none; opens on bench. `App.tsx:123`
- W1-FRM-018 Launcher. png `W1-FRM-018-<theme>.png`; drive: none; key mod+alt+b. `components/RightPanelTabs.tsx:450`
- W1-FRM-019 Tabs. png `W1-FRM-019-<theme>.png`; drive: none; open short-1, key mod+alt+b, key m, click Add a panel, key a. `components/RightPanelTabs.tsx:375`
- W1-FRM-020 Add menu. png `W1-FRM-020-<theme>.png`; drive: as Tabs; click Add a panel. `components/RightPanelTabs.tsx:418`
- W1-FRM-021 Sidebar away. png `W1-FRM-021-<theme>.png`; drive: none; key mod+b. `shell/HeaderRow.tsx:23`
- W1-FRM-022 Settings. png `W1-FRM-022-<theme>.png`; drive: none; settings general. `shell/AppShell.tsx:141`
- W1-FRM-023 Sheet. png `W1-FRM-023-<theme>.png`; drive: width 980; open short-1, key mod+alt+b. `shell/RightPanel.tsx:196`
- W1-FRM-024 Sidebar sheet. png `W1-FRM-024-<theme>.png`; drive: width 760; click Toggle main sidebar. `components/ui/sidebar.tsx:240`
- W1-FRM-025 Narrowest. png `W1-FRM-025-<theme>.png`; drive: width 360; key mod+alt+b. `rightPanelLayout.ts:7`
- W1-FRM-026 Sidebar yields. png `W1-FRM-026-<theme>.png`; drive: +pref sidebarWidth=480, width 1100; open short-1, key mod+alt+b. `rightPanelLayout.ts:18`
- W1-FRM-027 Reconnecting. png `W1-FRM-027-<theme>.png`; drive: the page's socket to the host cut (DevTools offline), never the host itself. `shell/AppShell.tsx:153`
- W1-FRM-028 Switcher overlay. png `W1-FRM-028-<theme>.png`; drive: none; open short-1, open short-2, ctrl down, key tab, hold 400 ms, shoot, ctrl up. `components/switcher/WorkspaceSwitcher.tsx:79`
- W1-FRM-029 Subagent page: no context ring, splits or toggles; panel and drawer hidden. png `W1-FRM-029-<theme>.png`; drive: `+working long`, `+reply Task`; open long, click the subagent row. `shell/AppShell.tsx:104`
- W1-FRM-030 Drawer open beside the inline panel. png `W1-FRM-030-<theme>.png`; drive: none; open short-1, key mod+j, key mod+alt+b. `App.tsx:139`

### Actions and keys

- W1-FRM-031 Sidebar, key `mod+b` (`keybindingDefaults.ts:44`), `mod+shift+b` (`keybindingDefaults.ts:43`). `shell/shellCommands.ts:331`
- W1-FRM-032 Panel, key `mod+alt+b` (`keybindingDefaults.ts:46`); not under Settings. `shell/shellCommands.ts:347`
- W1-FRM-033 Settings, key `mod+,` (`keybindingDefaults.ts:58`); Escape leaves. `shell/KeybindingDispatcher.tsx:54`
- W1-FRM-034 Panel tabs, key `ctrl+tab` (`keybindingDefaults.ts:80`) with panel focus. `shell/shellCommands.ts:351`
- W1-FRM-035 Close tab, key `mod+w` (`keybindingDefaults.ts:84`), only while focus is in the right panel or a browser page; `mod+shift+w` closes the window. `lib/previewFocus.ts:11`
- W1-FRM-036 Slot, key `mod+1` to `mod+9` (`keybindingDefaults.ts:85`). `shell/shellCommands.ts:318`
- W1-FRM-037 Launcher letters outside fields; arrows, Enter. `components/RightPanelTabs.tsx:166`
- W1-FRM-038 Middle click or X closes a tab. `components/RightPanelTabs.tsx:325`
- W1-FRM-039 Sheet: Escape or backdrop shuts it. `shell/RightPanel.tsx:196`
- W1-FRM-040 Stands down: handled keys, bare keys in fields, terminal Control. `shell/KeybindingDispatcher.tsx:63`
- W1-FRM-041 Mac zoom mod+= - 0; to the terminal while it has focus. `desktop/main.ts:475`

### Host operations

- W1-FRM-042 `events.subscribe` the socket behind the banner. `protocol/client.ts:376`
- W1-FRM-043 `preferences.get` theme, fonts, keys, width. `protocol/client.ts:1248`
- W1-FRM-044 `preferences.set` the sidebar width. `shell/AppShell.tsx:56`
- W1-FRM-045 `projects.list` first run or home. `protocol/client.ts:1219`
- W1-FRM-046 window `hostToken`. `BootGate.tsx:54`
- W1-FRM-047 window `setTheme`, `setTitleBar`, `setGlass`. `settings/theme.ts:51`
- W1-FRM-048 window `setTerminalFocus`, `onShellChord`. `shell/KeybindingDispatcher.tsx:98`
- W1-FRM-049 window `contextMenu`. `actions/contextMenu.ts:94`
- W1-FRM-050 window `setBadge` the dock count of threads waiting on the person; with no window open the menu bar's feed sets it. `notices/hostNotices.ts:377`
- W1-FRM-051 window `sayOutside`, `onNeedsYouOpen` a system notification while the window is not focused; a click raises it and opens the thread. `shell/needsYou.ts:60`
- W1-FRM-052 window gap `here`, `agents`, `install`, `finish` the first launch's computer name, agent scan, tools install and open; the host client has no such calls. `desktop/preload.ts:44`

### Design

- W1-FRM-053 Today's look wins: the 540 px panel, not the locked design's 400.
- W1-FRM-054 `Button`, `shortcut_tooltip`, `spinner`; rail, header, tabs, launcher, sheet: none yet.
- W1-FRM-055 `Palette` has the sidebar's tokens (4422c64) but lacks sidebar-rail, sidebar-row-active, sidebar-row-rest, titlebar, scrollbar, glass-ink and the material tokens.
- W1-FRM-056 gpui-kit `Sheet`, `h_resizable`, `TabBar`, `TitleBar`, `NativeMenu`; GPUI `cx.set_menus`.
- W1-FRM-057 Grain on Linux, solid stand-ins, gpui-kit toasts in the notices slot.

## Settings

### Elements

- W1-SET-001 Settings takes the window: its sidebar in the app sidebar's place, its page in the centre; no right panel or drawer. `shell/AppShell.tsx:141`
- W1-SET-002 Sidebar: lockup row, search field over a hairline, 12 group rows (36 px, glyph, name, mono meta word), Back (esc) at the foot. `settings/SettingsSidebar.tsx:194`
- W1-SET-003 Under the open group its pages with their marks, or its card heads as a trail lit while on screen. `settings/SettingsSidebar.tsx:188`
- W1-SET-004 Crumbs "Settings / group"; under a group a back chevron, the group as a link, the page; Search while searching. `settings/SettingsCrumbs.tsx:41`
- W1-SET-005 Restore defaults, xs ghost at the header's right, only while the open group is off its defaults. `settings/SettingsHeaderActions.tsx:24`
- W1-SET-006 Page: 760 px max, 28 px top, 32 px sides, 30 px between cards. `settings/SettingsPage.tsx:100`
- W1-SET-007 Row: title, mark, reset arrow off default, note, slot (mono word, one control), 60 px floor; Line: label, value or keycaps, 48 px. `settings/rows.tsx:153`
- W1-SET-008 General: Composer, Notifications, Threads, Open in, Startup and quit; Version (glyph, version, state, step, What's new). `settings/general.tsx:73`
- W1-SET-009 Appearance: Mode (3 window tiles, System cut on a slant), the side's theme cards, Glass, Type (sample, fonts, sizes). `settings/appearance.tsx:99`
- W1-SET-010 Keybindings: 4 cards, each command's keycaps as a button, jumps folded with "to". `settings/keybindings.ts:84`
- W1-SET-011 Privacy: one card of 4 switches; two held, reason on hover, when the host says off. `settings/privacy.tsx:21`
- W1-SET-012 Agents head: computer select, tabs Agents, Tool servers, Skills, Plugins, with icons. `settings/agents.tsx:128`
- W1-SET-013 New threads card: Default agent select, a mark per agent, reset arrow while set. `settings/agents.tsx:175`
- W1-SET-014 "On <computer>" card: "checked <ago>" and refresh; row: mark, name, version, update arrow, Sign in or word and dot. `settings/agents.tsx:258`
- W1-SET-015 Kind tab: search and Add (Tool servers and Skills only), a card per group; row: lead, name, marks (4, +n), mono line, step, switch or status. `settings/agentKinds.tsx:418`
- W1-SET-016 Agent page: head (version, sign-in, Update, on switch), New threads (model, effort, access), Models, How it runs. `settings/agentPage.tsx:396`
- W1-SET-017 Project row: glyph, name, computer and source mono, workspace count; page: head, Repository, New threads, Look, Remove. `settings/projects.tsx:86`
- W1-SET-018 Device row: name, paired and seen mono, Revoke red on hover; Account: GitHub row, login, held Sign in. `settings/devices.tsx:85`
- W1-SET-019 Computers list: head Computers, Cores, Memory, Threads; row glyph, name, "default" tag 12 px muted, mono versions, figures, state, chevron. `settings/computers.tsx:242`
- W1-SET-020 Pending list ("Chosen up to <step>", pending or failed mark), Clouds list (image note, "N running"); Add a cloud provider only while init.get offers a cloud. `settings/computers.tsx:177`
- W1-SET-021 Computer page: head row, Behind row, state, Recipe, Setup, Sign-ins, What runs here, Limits, Threads here, Agents, Image, Name, Remove. `settings/computers.tsx:645`
- W1-SET-022 Stepper: minus, number, plus in one 30 px box; shows the value sent until the host answers, min 1, Machines at once also stops at the plan limit. `settings/computerSettings.tsx:75`
- W1-SET-023 Recipes list (glyph, name, summary, followers in mono) or the empty card; Add a computer under. `settings/recipes.tsx:41`
- W1-SET-024 Activity: tabs in the top bar, 3 or 4 tiles with history lines, three selects, Every process, filter. `settings/activity/page.tsx:171`
- W1-SET-025 Activity tree: sortable head; wsp, thread, No thread heads fold onto process, subagent, container rows. `settings/activity/tables.tsx:507`

### States

- W1-SET-026 Search results, unmatched groups at 50%. png `W1-SET-026-<theme>.png`; drive: none; settings general, type "send". `settings/SettingsPage.tsx:28`
- W1-SET-027 Nothing matches. png `W1-SET-027-<theme>.png`; drive: none; settings general, type "zzzz". `settings/SettingsPage.tsx:34`
- W1-SET-028 Off defaults. png `W1-SET-028-<theme>.png`; drive: +pref textSize=15; settings appearance. `settings/rows.tsx:185`
- W1-SET-029 Chord refused. png `W1-SET-029-<theme>.png`; drive: none; settings keybindings, click Search's keycaps, key mod+b. `settings/ChordKeys.tsx:94`
- W1-SET-030 Sidebar as a sheet. png `W1-SET-030-<theme>.png`; drive: width 760; settings general, key mod+b, type "send". `settings/SettingsSidebar.tsx:296`
- W1-SET-031 Slots under words. png `W1-SET-031-<theme>.png`; drive: width 600, +pref textSize=15; settings appearance. `settings/rows.tsx:123`
- W1-SET-032 Desktop rows (When you quit, Start at login, Logs, App and Host lines). png `W1-FRM-022-<theme>.png`; drive: none; settings general. `settings/general.tsx:153`
- W1-SET-033 Agents tab, every agent on PATH needing sign-in. png `W1-SET-033-<theme>.png`; drive: none; settings agents. `settings/agents.tsx:302`
- W1-SET-034 Token field open. png `W1-SET-034-<theme>.png`; drive: none; settings agents, click Sign in on Claude Code. `components/agents/SignInFlowView.tsx:44`
- W1-SET-035 Signed in by key. png `W1-SET-035-<theme>.png`; drive: +op agents.key {agent:claude,key:x,way:key}; settings agents. `settings/agents.tsx:273`
- W1-SET-036 A server's page. png `W1-SET-036-<theme>.png`; drive: +op servers.add {target:{placeId:here},agent:claude,name:x,url}; settings agents, click Tool servers, x. `settings/agentKinds.tsx:575`
- W1-SET-037 Claude's page. png `W1-SET-037-<theme>.png`; drive: none; settings agents, click Claude Code. `settings/agentPage.tsx:394`
- W1-SET-038 Project in use, Remove held. png `W1-SET-038-<theme>.png`; drive: none; settings projects, click bench. `settings/projects.tsx:350`
- W1-SET-039 A paired device. png `W1-SET-039-<theme>.png`; drive: +op pair.issue, pair.redeem {code,name} first on a new socket; settings devices. `settings/devices.tsx:85`
- W1-SET-040 Computers: one row, Clouds head, Add a cloud provider. png `W1-SET-040-<theme>.png`; drive: none; settings computers. `settings/computers.tsx:166`
- W1-SET-041 This computer's page. png `W1-SET-041-<theme>.png`; drive: +working long; settings computers, click this computer. `settings/computers.tsx:632`
- W1-SET-042 Recipes empty. png `W1-SET-042-<theme>.png`; drive: none; settings recipes. `settings/recipes.tsx:80`
- W1-SET-043 12 recipes, one open. png `W1-SET-043-<theme>.png`; drive: +op recipes.save {"name":"rN","file":{"name":"rN"}} x12; settings recipes, click r1. `settings/recipes.tsx:227`
- W1-SET-044 Activity, a working thread open. png `W1-SET-044-<theme>.png`; drive: +working long; settings activity, click long. `settings/activity/tables.tsx:478`
- W1-SET-045 Narrow: columns drop under 768 px, tabs become a select under 640. png `W1-SET-045-<theme>.png`; drive: width 600; settings activity. `settings/activity/page.tsx:69`
- W1-SET-046 Sidebar hidden at full width: the header is the frame row, toggle at its left, no New thread glyph. png `W1-SET-046-<theme>.png`; drive: none; settings general, key mod+b. `shell/HeaderRow.tsx:23`
- W1-SET-047 A chord of the person's own: Reset by its keycaps, Restore defaults in the header. png `W1-SET-047-<theme>.png`; drive: +pref keybindings={"commandPalette.toggle":"mod+shift+k"}; settings keybindings. `settings/ChordKeys.tsx:75`
- W1-SET-048 General off a default: that row's reset arrow, no Restore defaults. png `W1-SET-048-<theme>.png`; drive: +pref sendWith=mod-enter; settings general. `settings/SettingsHeaderActions.tsx:24`

### Actions and keys

- W1-SET-049 Toggle Settings, key `mod+,` (`keybindingDefaults.ts:58`); also the corner gear, palette row, wsp:// link. `shell/shellCommands.ts:342`
- W1-SET-050 Escape closes Settings unless the palette is open; in the field it clears text first. `shell/KeybindingDispatcher.tsx:54`
- W1-SET-051 Page kept in localStorage `wsp:settings-at`, read once at load, so a new window opens where any window last was; never on the host; a move clears the search. `settings/settingsStore.ts:198`
- W1-SET-052 Pointer on a theme card previews it; leaving or blur restores the pick. `settings/ThemePicker.tsx:81`
- W1-SET-053 Keycaps press listens: next chord saved, Esc or a second press stops; Reset while overridden. `settings/ChordKeys.tsx:46`
- W1-SET-054 Changing tab, computer or page closes an item page; so does its item leaving. `settings/agentKinds.tsx:376`
- W1-SET-055 Model grip: drag within its list, key `ArrowUp`/`ArrowDown` moves one; key `Enter` in the id field adds. `settings/agentPage.tsx:191`
- W1-SET-056 Icon row takes a dropped or pasted image; Choose an image opens the file picker, then the Project icon sheet. `settings/projects.tsx:339`
- W1-SET-057 Add a computer, a pending row: AddComputerDialog; Add a cloud provider: AddCloudDialog; Remove: RemoveComputerDialog. `settings/computers.tsx:158`
- W1-SET-058 Activity row menu (right click, ...): Open, Stop turn, End, End all, Open thread, Copy command, Copy PID. `settings/activity/actions.ts:40`

### Host operations

- W1-SET-059 `preferences.set` each pick, reset arrow and Restore defaults. `settings/settingsContext.ts:101`
- W1-SET-060 `init.get` `editor.list` `ssh.include` `account.get` `devices.list` `image.get` on open; `host.terminalConfig` is asked too but nothing draws it. `settings/settingsReads.ts:44`
- W1-SET-061 `release.check` on each General open. `settings/settingsReads.ts:96`
- W1-SET-062 gap host version, off the page's boot line; the host client has no such call. `shell/shellVersion.ts:19`
- W1-SET-063 gap `productUsageOff`, boot line only; the host client has no such call. `settings/privacy.tsx:20`
- W1-SET-064 window gaps `version` `loginStart` `setLoginStart` `openLogs` `fontFamilies` `setTheme` `setGlass`. `desktop/preload.ts:40`
- W1-SET-065 `agents.signIns` lists running sign-ins on open, to reattach their flows. `components/agents/useAgentActs.ts:82`
- W1-SET-066 `window` openExternal: fact links, Open page, repository Open, skills.sh. `settings/agentKinds.tsx:476`
- W1-SET-067 `places.set` each limit, name, login; a reset arrow sends `reset`. `settings/computerSettings.tsx:30`
- W1-SET-068 `places.follow` the Recipe select. `settings/computers.tsx:507`
- W1-SET-069 `daemon.send` proc.watch, scoped to thread cgroups on a joined computer. `machine/procs.ts:145`
- W1-SET-070 `appProcs.watch` names the app's processes here. `machine/appProcs.ts:67`
- W1-SET-071 `activity.end` frees a kept agent. `settings/activity/live.tsx:221`
- W1-SET-072 window `contextMenu` native row menus. `actions/contextMenu.ts:94`
- W1-SET-073 `window` openExternal: What's new, and Get where the shell cannot download. `settings/about.tsx:40`
- W1-SET-074 `sys.unsubscribe` by placeId when a computer row leaves; by workspaceId (`machine/hostLive.ts:48`) for the wsp tile. `machine/computerLive.ts:86`
- W1-SET-075 `daemon.send` proc.unwatch when the last Activity hold on a computer goes. `machine/procs.ts:78`
- W1-SET-076 `daemon.send` ports.watch once the link is live: the Ports view and the port tags. `terminal/wiring.ts:178`
- W1-SET-077 `daemon.close` when nothing holds the box link Activity opened. `terminal/daemon-link.ts:287`
- W1-SET-078 `agents.signInStop` Cancel on a Sign-ins row's or a setup row's sign-in. `components/agents/useAgentActs.ts:119`
- W1-SET-079 `agents.signInCode` a code typed into the sign-in flow under those rows. `components/agents/useAgentActs.ts:216`
- W1-SET-080 `agents.key` a key typed under a setup row's key way, for the host's vault. `components/agents/useAgentActs.ts:229`
- W1-SET-081 `init.keys` Change key in the build box saves the new provider key. `settings/ProviderKey.tsx:37`
- W1-SET-082 window clipboard write: Copy command, Copy PID, Copy name, Copy container ID; no clipboard is an error notice. `actions/clipboard.ts:10`

### Design

- W1-SET-083 Today's look first; locked designs only where the app has no piece.
- W1-SET-084 `Card` (`body`, `locked`), `section_head`, `Row` (its reset arrow), `Line`, `KeyCaps`, `glyph_frame`, `Select`, `Field`, `Button`, `CopyRow`, `Switch`, `Segmented`, `Stepper`, `Grid`, `GridRow`, `Skeleton`, `StepRow` draw the cards.
- W1-SET-085 None yet: the settings sidebar's rows and search field, crumbs, theme tiles; Activity's history lines start from `Sparkline` (D12).
- W1-SET-086 Copy gpui-kit `Switch`, `TabBar` (Segmented), `NumberInput`, `Skeleton`, `Tooltip`, `TextView::markdown`; Zed `canvas` for the lines. Glass card on the Mac only.

## Usage

### Elements

- W1-USE-001 Top bar right: "checked 2 min ago" 12 px, refresh icon-xs (spins while reading), Usage/Limits tabs 36 px. `settings/usage.tsx:57`
- W1-USE-002 Page, data-locked: max 1080 px, top 56 (24 under 640 px), sides 32 (16 under 640 px); sections and pools 40 px apart. `settings/SettingsPage.tsx:100`
- W1-USE-003 Range (Today, 7 days, 30 days) and split (Agent to Source) controls, 36 px, at a row's two ends. `settings/usage.tsx:676`
- W1-USE-004 Totals strip, boxed, hairline cells: Tokens, API estimate, Turns, Cache hit; 26 px mono rolling figures. `settings/usage.tsx:510`
- W1-USE-005 Chart section, unboxed: head "Tokens a day", legend (swatch, agent mark, label). `settings/usage.tsx:730`
- W1-USE-006 Chart: 300 px plot, rolling y figures, dashed grid, Bayer dither under lines, dot on the lead line's end. `settings/usageChart.tsx:206`
- W1-USE-007 "By agent" table, unboxed: glyph, name, share bar, Tokens, Cache hit, Turns, API estimate; models inset. `settings/usage.tsx:758`
- W1-USE-008 "Where the tokens went": 12 px split bar, legend with mono figures; logs line under. `settings/usage.tsx:784`
- W1-USE-009 Limits pool card per agent: identity, computers, burn, checked word, reset line per account. `settings/usage.tsx:288`
- W1-USE-010 Window row: name, "62% left" 24 px, verdict; a segment per account, warning fill from 90%, pace tick. `settings/usage.tsx:190`
- W1-USE-011 "No plan limit" card: account rows with a state word; "Codex reports no plan limit." under. `settings/usage.tsx:362`
- W1-USE-012 Sidebar corner: gauge button opens Usage. `sidebar/SidebarCorner.tsx:28`
- W1-USE-013 Computers list: cloud limit band, "n of N" over bars. `settings/LimitBand.tsx:57`
- W1-USE-014 Composer: "Usage limit reached" bar. `components/chat/bars/UsageBar.tsx:35`
- W1-USE-015 Plan alert notice; Open goes to Usage. `notices/hostNotices.ts:342`

### States

- W1-USE-016 Week by agent: Claude Code, its model, 1.86k today. png `W1-USE-016-<theme>.png`; drive: none; usage. `settings/usage.tsx:721`
- W1-USE-017 Today, hourly. png `W1-USE-017-<theme>.png`; drive: none; usage, click Today. `settings/usage.tsx:414`
- W1-USE-018 Two lines. png `W1-USE-018-<theme>.png`; drive: +project p2, +thread p2 "hi"; usage, click Project. `settings/usage.tsx:706`
- W1-USE-019 Chart hover tooltip. png `W1-USE-019-<theme>.png`; drive: none; usage, hover the chart's right end. `settings/usageChart.tsx:255`
- W1-USE-020 Narrow, long name cut. png `W1-USE-020-<theme>.png`; drive: width 600; usage, click Account. `settings/usage.tsx:547`
- W1-USE-021 Limits, no accounts. png `W1-USE-021-<theme>.png`; drive: none; usage, click Limits. `settings/usage.tsx:351`
- W1-USE-022 One account, 5-hour and Week. png `W1-USE-022-<theme>.png`; drive: `+thread` with `+reply rate-limit` (five_hour 62%, seven_day 30%); usage, click Limits. `settings/usage.tsx:280`
- W1-USE-023 Near and over the limit: 92%, reached. png `W1-USE-023-<theme>.png`; drive: `+reply rate-limit` five_hour 92%, then 100%; usage, click Limits. `settings/usage.tsx:163`

### Actions and keys

- W1-USE-024 Pick Usage or Limits; kept while the window is open. `settings/usage.tsx:69`
- W1-USE-025 Refresh reads both; also every 60 s and on focus after 60 s. `settings/usage.tsx:112`
- W1-USE-026 Pick range or split, arrows too; old figures roll to the new. `settings/usage.tsx:677`
- W1-USE-027 Use reset: dialog asks, spends, notice says the answer; Escape closes unless spending. `settings/usage.tsx:257`

### Host operations

- W1-USE-028 `usage.used` range, split, outside; model too, for the rows under agents. `settings/usage.tsx:94`
- W1-USE-029 `usage.accounts` fresh, held 60 s. `settings/usage.tsx:93`
- W1-USE-030 `usage.reset` an account's key. `settings/usage.tsx:235`
- W1-USE-031 `sessions.mark` resumeAtReset true or false from the limit bar. `components/chat/bars/UsageBar.tsx:30`
- W1-USE-032 window `sayOutside` the plan alert as a system notification while the window lacks focus; a click opens Usage. `notices/hostNotices.ts:346`
- W1-USE-033 `sys.subscribe` and `sys.unsubscribe` placeId: the live band's 2 s `place.sys` samples, asked again after a reconnect. `machine/computerLive.ts:83`

### Design

- W1-USE-034 Today's app wins: agent inks, not primary, draw the lines.
- W1-USE-035 data-locked head 14 px regular, radius 14 px: `Card::locked`, `section_head`.
- W1-USE-036 Totals, pools, No plan limit: `Card`; chart, table, mix: `section_head` over bare content.
- W1-USE-037 `glyph_frame`, agent marks none yet; refresh `Button` icon xs; Use reset `Button` outline xs.
- W1-USE-038 `Segmented`, `Skeleton`, `Sparkline` exist; rolling digits, meters and the dithered chart: none yet; the chart on gpui `canvas`, `PathBuilder`.
- W1-USE-039 Reset dialog: today's 440 px alert, `AlertDialog`; gpui-kit toasts stand in for notices.

## Sidebar

### Elements

- W1-SBR-001 Top row 52 px: sidebar toggle (panel-left-close while shown, panel-left when shut), wsp wordmark 14 px muted. `sidebar/SidebarChrome.tsx:14`
- W1-SBR-002 Fixed head, 8 px sides, 12 px under: New thread, Search, project and computer switchers, 36 px rows. `sidebar/WorkspaceSidebar.tsx:693`
- W1-SBR-003 New thread row: square-pen, words, chord in 11 px mono at the right; 50% while no project. `sidebar/NewThreadRow.tsx:30`
- W1-SBR-004 Switcher head: folder or monitor (All) or the pick's glyph, name, a remote project's computer, chevron. `sidebar/NounSwitcher.tsx:168`
- W1-SBR-005 Tile 52 px: row one 11 px muted (glyph, "bench @ computer", slot); row two (mark, title, PR, crab, fold). `sidebar/ThreadTile.tsx:116`
- W1-SBR-006 Title muted when working, read or resting unless open, and for Resuming, Stopped, Released, Expired, Settled, Finished; foreground when it calls; open at 500. `sidebar/ThreadTile.tsx:361`
- W1-SBR-007 Open tile: `--sidebar-row-selected` fill, 1 px inset ring `--sidebar-row-edge`; hover `--sidebar-row-hover`. `sidebar/rowGrammar.ts:23`
- W1-SBR-008 Tile card at 450 ms, right, 288 px: title, status row, reason, glyph lines project to notes. `sidebar/ThreadTile.tsx:71`
- W1-SBR-009 Thirteen slot kinds, first match wins, needs-you first, resting last. `components/status/kinds/index.ts:24`
- W1-SBR-010 Slot: 12 px glyph, word on hover; working: `42s`, `1h 5m`, crab on row two; resting: age. `components/status/ThreadStatus.tsx:63`
- W1-SBR-011 Hover act in the slot: archive (Settle) on a quiet tile, square (Stop) while its tree works. `sidebar/ThreadTile.tsx:403`
- W1-SBR-012 Fold control: shut, row count and chevron-right; open, chevron-down on hover only. `sidebar/ThreadTile.tsx:154`
- W1-SBR-013 Section head 28 px: "Needs you (2)" 12 px muted, `--sidebar-border` hairline, 14 px chevron. `sidebar/SectionRow.tsx:39`
- W1-SBR-014 Body: sends in flight, Pinned, Needs you, bare list, Settled, 12 px apart; empty parts not drawn. `sidebar/WorkspaceSidebar.tsx:727`
- W1-SBR-015 Settled row 36 px: glyph at 40% grey, muted title, age; "n more" per 25. `sidebar/ThreadTile.tsx:429`
- W1-SBR-016 Subagent row 36 px: bot glyph, muted title, slot; Finished row: check, count, chevron. `sidebar/SubagentRow.tsx:69`
- W1-SBR-017 Foot: Setting up cards, update or cloud card, corner (Settings, Usage, Activity). `sidebar/WorkspaceSidebar.tsx:808`

### States

- W1-SBR-018 No threads. png `W1-SBR-018-<theme>.png`; drive: `+project e`; click "All projects", click "e". `sidebar/WorkspaceSidebar.tsx:775`
- W1-SBR-019 9th tile scrolls. png `W1-SBR-019-<theme>.png`; drive: `+thread` x5; scroll sidebar. `sidebar/WorkspaceSidebar.tsx:724`
- W1-SBR-020 Long names. png `W1-SBR-020-<theme>.png`; drive: 60-char `+project`, 200-char `+thread`. `sidebar/ThreadTile.tsx:129`
- W1-SBR-021 Working, crab on the tile. png `W1-CMP-023-<theme>.png`; drive: `+working short-1`. `sidebar/ThreadTile.tsx:452`
- W1-SBR-022 Needs you. png `W1-SBR-022-<theme>.png`; drive: `+working short-2` with `+reply question`. `components/status/kinds/needs-you.ts:11`
- W1-SBR-023 Failed. png `W1-SBR-023-<theme>.png`; drive: `+thread` with `+reply error`. `components/status/kinds/failed.ts:9`
- W1-SBR-024 Unread. png `W1-SBR-024-<theme>.png`; drive: `+thread bench "hi"`. `components/status/kinds/done.ts:9`
- W1-SBR-025 Selected. png `W1-THR-022-<theme>.png`; drive: open short-1. `sidebar/ThreadTile.tsx:290`
- W1-SBR-026 Hover. png `W1-SBR-026-<theme>.png`; drive: hover short-2, wait 500 ms. `sidebar/ThreadTile.tsx:105`
- W1-SBR-027 Renaming. png `W1-SBR-027-<theme>.png`; drive: double-click short-1's title. `sidebar/RowNameInput.tsx:36`
- W1-SBR-028 Settled shut. png `W1-SBR-028-<theme>.png`; drive: open short-3, key mod+shift+e. `sidebar/WorkspaceSidebar.tsx:754`
- W1-SBR-029 Settled open. png `W1-SBR-029-<theme>.png`; drive: as W1-SBR-028; click "Settled". `sidebar/WorkspaceSidebar.tsx:767`
- W1-SBR-030 Drag over the Pinned head. png `W1-SBR-030-<theme>.png`; drive: drag short-2 by the pointer onto the Pinned head and hold. `sidebar/treeDrag.ts:129`
- W1-SBR-031 Narrow. png `W1-FRM-024-<theme>.png`; drive: width 760; key mod+b. `sidebar/rowGrammar.ts:40`
- W1-SBR-032 Collapsed. png `W1-FRM-021-<theme>.png`; drive: key mod+b. `sidebar/NewThreadRow.tsx:43`
- W1-SBR-033 Subagents. png `W1-SBR-033-<theme>.png`; drive: `+working long`, `+reply Task`. `sidebar/LeadTree.tsx:95`
- W1-SBR-034 One computer. png `W1-SBR-034-<theme>.png`; drive: click "All computers". `sidebar/ComputerSwitcher.tsx:15`
- W1-SBR-035 Keyboard focus. png `W1-SBR-035-<theme>.png`; drive: click short-1; key ArrowDown (2 px ring; Settle shows). `sidebar/rowGrammar.ts:43`
- W1-SBR-036 Stopped. png `W1-SBR-036-<theme>.png`; drive: `+working short-1`; hover short-1; click "Stop thread"; click "Press again to stop". `components/status/kinds/stopped.ts:8`
- W1-SBR-037 Subagent page open: its row selected, its lead's tile not. png `W1-FRM-029-<theme>.png`; drive: `+working long`, `+reply Task`; click the subagent row. `sidebar/ThreadTile.tsx:294`

### Actions and keys

- W1-SBR-038 Click opens the thread; it holds its place until another opens. `sidebar/WorkspaceSidebar.tsx:518`
- W1-SBR-039 Right-click: the thread's verbs, then its copy's. `sidebar/WorkspaceSidebar.tsx:519`
- W1-SBR-040 Double-click title renames; Enter saves; blank, same, Escape or blur cancel. `sidebar/RowNameInput.tsx:36`
- W1-SBR-041 Up, Down, Home, End walk rows; Left and Right fold a tree. `sidebar/WorkspaceSidebar.tsx:660`
- W1-SBR-042 Drag a root onto a head (pin, place), onto Settled, or between roots. `sidebar/WorkspaceSidebar.tsx:611`
- W1-SBR-043 Heads fold, kept per window; Settled starts shut. `sidebar/WorkspaceSidebar.tsx:238`
- W1-SBR-044 Settle: toast with Undo; Stop: two presses in 2 s. `sidebar/ThreadTile.tsx:407`
- W1-SBR-045 New thread, key `mod+t` (`keybindingDefaults.ts:59`). `sidebar/NewThreadRow.tsx:30`
- W1-SBR-046 Search, palette, key `mod+k` (`keybindingDefaults.ts:55`). `sidebar/SearchRow.tsx:22`
- W1-SBR-047 Toggle, key `mod+b`, `mod+shift+b` (`keybindingDefaults.ts:43`). `shell/shellCommands.ts:331`
- W1-SBR-048 Thread walk, key `mod+alt+arrowup`/`down` (`keybindingDefaults.ts:65`): the open workspace's threads, most recently opened first, under the switcher overlay, committed on key-up. `shell/shellCommands.ts:227`
- W1-SBR-049 Settle open tree, key `mod+shift+e` (`keybindingDefaults.ts:67`). `shell/shellCommands.ts:262`
- W1-SBR-050 Next needs you, key `mod+alt+u` (`keybindingDefaults.ts:68`). `shell/shellCommands.ts:295`
- W1-SBR-051 Workspace n, store order, key `mod+1`-`9` (`keybindingDefaults.ts:85`). `shell/shellCommands.ts:318`
- W1-SBR-052 Opens AddProject, Snooze, DeleteThread, ForgetWorkspace, ExportProject. `sidebar/WorkspaceSidebar.tsx:815`

### Host operations

- W1-SBR-053 `sessions.mark` pin, snooze, fold, order. `sidebar/WorkspaceSidebar.tsx:607`
- W1-SBR-054 `sessions.settle`, `sessions.restore`. `actions/threadActions.ts:387`
- W1-SBR-055 `sessions.rename` after `workspaces.wake`. `sidebar/WorkspaceSidebar.tsx:520`
- W1-SBR-056 `sessions.interrupt` for Stop. `actions/verbs.ts:75`
- W1-SBR-057 `preferences.set` project order, dismissals. `sidebar/WorkspaceSidebar.tsx:698`
- W1-SBR-058 window `contextMenu`, the native menu. `actions/contextMenu.ts:94`
- W1-SBR-059 `workspaces.nap`, `workspaces.wake`, `workspaces.stopWake` for a copy's Pause, Wake and Stop waking. `protocol/store/useStore.ts:241`
- W1-SBR-060 `workspaces.restartDaemon` (Reconnect) and `workspaces.bringBack` (Bring back) from a copy's menu. `actions/verbs.ts:36`

### Design

- W1-SBR-061 Today's app first; wsp-design only where it has no piece.
- W1-SBR-062 `Crab`, `StatusMark`; the tile moves the crab off the slot; 8 kinds missing.
- W1-SBR-063 None yet: tile, one-line row, sidebar head, rail, switcher, rename box.
- W1-SBR-064 Corner, cards: `Button` icon, outline xs, text.
- W1-SBR-065 gpui-kit `Popover`, `Input`, `HoverCard`, `PopupMenu`; GPUI `list`, `on_drag`.
- W1-SBR-066 Solid stand-in for glass, grain on Linux, gpui-kit toasts for Undo.

## Thread view

### Elements

- W1-THR-001 Crumbs: project glyph and name, slash, opener link, title (medium, max 70%), Needs-you glyph, tooltip. `shell/ThreadBreadcrumb.tsx:127`
- W1-THR-002 Title row right: context ring (popover, Compact), Git split, Open split, panel toggles. `shell/AppShell.tsx:164`
- W1-THR-003 Transcript: virtual list, 768 px column, sides 12/20 px, end spacer at composer height. `components/chat/timeline/list.tsx:685`
- W1-THR-004 Person bubble: right, max 80%, radius 16; 56 px image thumbs; clamps past 600 chars or 8 lines. `components/chat/timeline/rows.tsx:110`
- W1-THR-005 Reply: markdown 14 px (textSize), line 1.625, 80% ink; Insight note; "(empty response)". `components/chat/timeline/rows.tsx:225`
- W1-THR-006 Hover meta on a turn's last reply: time, Copy, Fork from here; Rewind to here on earlier turns that kept a checkpoint, off while working. `components/chat/RewindDialog.tsx:138`
- W1-THR-007 Settled turn folds behind "Worked for 1m 4s"; a stopped turn keeps its rows and "You stopped after 9s". `adapt/timeline-rows.ts:357`
- W1-THR-008 Call group: kind glyph, summary ("Ran 2 commands"); open, a list capped min(18rem, 50dvh), 12 px in. `components/chat/timeline/workGroup.tsx:272`
- W1-THR-009 Plain call row: glyph, label, chevron; open, mono body max 256 px behind a rule. `components/chat/timeline/workEntry.tsx:206`
- W1-THR-010 Command row: spinner or glyph, colored command, exit words, duration; open, output band, +N lines. `components/chat/timeline/callBlock.tsx:175`
- W1-THR-011 Edit row: Wrote, Edited, Moved or Deleted and path, +/- counts or N lines, Open diff; open, hunks cut at 16. `components/chat/timeline/callBlock.tsx:283`
- W1-THR-012 Working: "Working for 12s" or "Waiting for you 12s"; newest call live with shimmer; Thinking row. `components/chat/timeline/working.tsx:36`
- W1-THR-013 Row tones; a runtime error wraps in destructive ink, alert glyph, never folded. `components/chat/timeline/workEntry.tsx:126`
- W1-THR-014 Prompt row: open (says, command, file fold, options, refusal) or record (glyph, says, outcome). `components/chat/PermissionPromptRow.tsx:101`
- W1-THR-015 Question row: header, question, option buttons with notes; or checkboxes and Answer. `components/chat/QuestionPrompt.tsx:34`
- W1-THR-016 Code block: language or file name, Run, wrap toggle (on), Copy code. `components/markdown/codeBlocks.tsx:207`
- W1-THR-017 Markdown: h1-h4 20/18/16/14 px, bordered inline code, info links with globe and URL tooltip. `index.css:940`
- W1-THR-018 Table: cells wrap at 384 px (toggle cuts them), scroll with fade, Copy table menu. `components/markdown/blocks.tsx:165`
- W1-THR-019 Scroll to end pill, 28 px glass, over the composer off the end. `components/chat/ChatView.tsx:536`
- W1-THR-020 Find bar: 420 px glass card top right; field, 4 toggles, count, Older, Newer, Close. `components/chat/find/FindBar.tsx:111`
- W1-THR-021 Minimap: 72 px left rail, a tick per person message, preview card; shown at a 48 px gutter, on hover below. `components/chat/timeline/minimap.tsx:150`

### States

- W1-THR-022 Short thread. png `W1-THR-022-<theme>.png`; drive: fixture; open short-1. `components/chat/timeline/list.tsx:692`
- W1-THR-023 Long thread at its top, no loading row. png `W1-THR-023-<theme>.png`; drive: fixture; open long, scroll transcript top. `components/chat/timeline/list.tsx:418`
- W1-THR-024 Working turn. png `W1-THR-024-<theme>.png`; drive: +working short-2 (stub: text, Bash tool_use, no result); open short-2. `components/chat/timeline/working.tsx:60`
- W1-THR-025 Error result: a tool's error muted with the alert glyph; a runtime error in destructive ink. png `W1-THR-025-<theme>.png`; drive: `+reply error`; open it. `components/chat/timeline/workEntry.tsx:183`
- W1-THR-026 Long message, clamped. png `W1-THR-026-<theme>.png`; drive: +thread "<2,000 chars>"; open it. `components/chat/timeline/rows.tsx:420`
- W1-THR-027 400-line code, wrap off. png `W1-THR-027-<theme>.png`; drive: +reply code; open it, click Disable line wrap. `components/markdown/codeBlocks.tsx:247`
- W1-THR-028 Wide table. png `W1-THR-028-<theme>.png`; drive: +reply table (12 columns); open it. `components/markdown/blocks.tsx:170`
- W1-THR-029 40 calls, fold shut. png `W1-THR-029-<theme>.png`; drive: +reply 40 of Bash, Read, Edit, Grep; open it. `components/chat/timeline/rows.tsx:197`
- W1-THR-030 Fold open. png `W1-THR-030-<theme>.png`; drive: as W1-THR-029; click Worked for, the group, a command, an edit. `components/chat/timeline/callBlock.tsx:74`
- W1-THR-031 Question waiting: its row, the crumb's glyph. png `W1-CMP-025-<theme>.png`; drive: `+working` with `+reply question`; open it. `shell/ThreadBreadcrumb.tsx:165`
- W1-THR-032 Find, matches. png `W1-THR-032-<theme>.png`; drive: fixture; open long, key mod+f, type "scrollTop". `components/chat/find/FindBar.tsx:139`
- W1-THR-033 Find, none. png `W1-THR-033-<theme>.png`; drive: fixture; open long, key mod+f, type "qqzz". `components/chat/find/ThreadFind.tsx:270`
- W1-THR-034 Minimap preview. png `W1-THR-034-<theme>.png`; drive: fixture; open short-1, hover a gutter tick. `components/chat/timeline/minimap.tsx:248`
- W1-THR-035 Quote. png `W1-THR-035-<theme>.png`; drive: fixture; open short-1, drag over a reply. `components/chat/AssistantSelectionToolbar.tsx:90`
- W1-THR-036 Hover meta. png `W1-THR-036-<theme>.png`; drive: fixture; open short-1, hover the last reply. `components/chat/timeline/rows.tsx:251`
- W1-THR-037 Panel as sheet, no minimap. png `W1-FRM-023-<theme>.png`; drive: width 980; open short-1, key mod+alt+b. `components/chat/timeline/list.tsx:463`
- W1-THR-038 Toggles on own row. png `W1-THR-038-<theme>.png`; drive: width 600; open short-1, key mod+f. `components/chat/find/FindBar.tsx:158`
- W1-THR-039 Retrying. png `W1-THR-039-<theme>.png`; drive: +working short-2, stub sends {"type":"system","subtype":"api_retry","attempt":2,"max_retries":10,"error":"overloaded","error_status":529} and holds; open short-2. `components/chat/ChatView.tsx:184`
- W1-THR-040 Context card. png `W1-THR-040-<theme>.png`; drive: fixture; open short-1, hover the context ring. `components/chat/ContextMeter.tsx:95`
- W1-THR-041 Git menu. png `W1-THR-041-<theme>.png`; drive: fixture; open short-1, click More git actions. `pull-request/GitSplit.tsx:95`

### Actions and keys

- W1-THR-042 Rename: double-click the title or its menu's Rename; Enter saves, Esc cancels. `shell/ThreadBreadcrumb.tsx:66`
- W1-THR-043 Find, key `mod+f` (`keybindingDefaults.ts:73`); older `mod+g`, `f3` (:74, :75); newer `mod+shift+g`, `shift+f3` (:76, :77). `shell/shellCommands.ts:404`
- W1-THR-044 Find bar: Enter older, Shift+Enter newer, Esc closes, returns focus; Alt (Mac Cmd+Alt) C/W/R. `components/chat/find/FindBar.tsx:88`
- W1-THR-045 Fold, group and row toggles; a plain row takes Enter and Space. `components/chat/timeline/workEntry.tsx:196`
- W1-THR-046 Minimap: click jumps 24 px under the top; focused, arrows, Home, End, Enter, Space. `components/chat/timeline/minimap.tsx:182`
- W1-THR-047 Select all outside a field selects the transcript. `components/chat/timeline/list.tsx:500`

### Host operations

- W1-THR-048 `sessions.head` the newest window on open. `components/chat/transcripts.ts:271`
- W1-THR-049 `sessions.history` 200 older events near the top; all while find is open. `components/chat/transcripts.ts:275`
- W1-THR-050 `events.subscribe` once per socket by the client; the view listens for the thread's live events. `components/chat/useChatThread.ts:941`
- W1-THR-051 `sessions.read` stamps the thread read. `components/chat/useReadStamp.ts:17`
- W1-THR-052 `sessions.rename`, after `workspaces.wake` when napping. `protocol/store/useStore.ts:711`
- W1-THR-053 `sessions.settle` on the settle key. `protocol/store/useStore.ts:742`
- W1-THR-054 window `contextMenu` the title menu; links open outside. `actions/contextMenu.ts:94`
- W1-THR-055 `editor.list` once per host, `preferences.set` editor on a pick, `ssh.include` on consent: the Open split. `files/OpenSplit.tsx:44`
- W1-THR-056 `workspaces.commitDraft`, `workspaces.commit`, `workspaces.bringBack`: the Git split's commit, push and pull request. `pull-request/GitSplit.tsx:60`
- W1-THR-057 `sessions.start`, `sessions.interrupt`, `sessions.settle`, `sessions.restore` from child rows and the start line. `actions/verbs.ts:109`

### Design

- W1-THR-058 Today's app wins over the locked design (message text 14 px, not 15/22).
- W1-THR-059 ui/: `Button` (options, Wake, icon acts), `shortcut_tooltip` (find), `spinner`.
- W1-THR-060 None yet: crumbs, list, bubble, call rows, find bar, minimap, Quote, noise field.
- W1-THR-061 gpui-kit `MessageScroller`, `TextView`, highlighter, `Diff`, `Collapsible`, `ShimmerText`; GPUI `list`, `canvas`.
- W1-THR-062 Deliberate differences: grain on Linux, solid find bar and pill without glass, gpui-kit toasts for copy failures.

## Composer

### Elements

- W1-CMP-001 Shell: max 768 px, 22 px glass (card 55% Paper, ink 3% Graphite), 10% hairline, Paper shadow. `components/chat/ComposerSurface.tsx:20`
- W1-CMP-002 Box: full (no messages yet: pickers under), one line (pickers, send beside), tall. `components/chat/ChatComposer.tsx:1244`
- W1-CMP-003 Editor: plain text at the text size (13 to 17 px, 14 by default), 70 to 200 px then scrolls; one-line box 1 to 8 lines; no focus ring. `components/ComposerPromptEditor.tsx:154`
- W1-CMP-004 Placeholder "Ask anything, or / for commands"; "Ask anything" with no commands or under 240 px. `components/ComposerPromptEditor.tsx:163`
- W1-CMP-005 Chips, one caret step: @file (icon, basename, path tip), $skill, quote, PR, issue (body tip), terminal. `components/composerInlineChip.ts:5`
- W1-CMP-006 File tray: 56 px image thumbs (open full size), file tiles, refused tile, x on hover. `components/chat/ChatComposer.tsx:1225`
- W1-CMP-007 Send: 32 px round foreground, arrow; 30% empty; outline, muted when held; spinner sending. `components/chat/ComposerPrimaryActions.tsx:233`
- W1-CMP-008 Stop: 32 px round destructive, square, in send's place while a turn runs; spinner while out. `components/chat/ComposerPrimaryActions.tsx:100`
- W1-CMP-009 Model button; menu 400 px: agent rail, search, rows (mod+1-9, star), Legacy fold, foot. `components/chat/ComposerModelPicker.tsx:254`
- W1-CMP-010 Access button: in a full box over 640 px wide, else the tray. `components/chat/ComposerOptionPickers.tsx:303`
- W1-CMP-011 Tray, 32 px: computer, access, Stashed N, branch (read-only note). `components/chat/ComposerCheckoutRow.tsx:172`
- W1-CMP-012 Drawer on the top edge, 28 px rows: question, usage limit, slot wait, tasks, queue. `components/chat/ComposerDrawer.tsx:96`
- W1-CMP-013 / @ # $ menu over the box, 22 px insets, max 288 px: group heads, rows, foot line. `components/chat/ComposerCommandMenu.tsx:69`
- W1-CMP-014 Drop frame over the chat: 80% ground, card, "Drop files to attach", caps. `components/chat/ChatDropZone.tsx:78`
- W1-CMP-015 Question dock in its place: question, n of N, pick rows, field, Answer, Write a message instead. `components/chat/PromptDock.tsx:342`
- W1-CMP-016 New thread: ASCII hero field, project glyph, "What should we build in <project>?" menu. `shell/ProjectHome.tsx:179`

### States

- W1-CMP-017 Empty, focused. png `W1-THR-022-<theme>.png`; drive: none; open short-1. `components/ComposerPromptEditor.tsx:163`
- W1-CMP-018 Max height, 200 px. png `W1-CMP-018-<theme>.png`; drive: none; new thread, type 12 lines, key shift+enter between. `components/ComposerPromptEditor.tsx:154`
- W1-CMP-019 Restored. png `W1-CMP-019-<theme>.png`; drive: none; open short-1, type "x", open short-2, open short-1. `components/chat/composerDraftStore.ts:191`
- W1-CMP-020 Slash menu. png `W1-CMP-020-<theme>.png`; drive: none; open short-1, type "/". `components/chat/ChatComposer.tsx:1101`
- W1-CMP-021 @ menu, chip. png `W1-CMP-021-<theme>.png`; drive: none; open short-1, type "@", key enter. `components/chat/ChatComposer.tsx:526`
- W1-CMP-022 Model menu. png `W1-CMP-022-<theme>.png`; drive: none; new thread, click the model button. `components/chat/ComposerModelPicker.tsx:271`
- W1-CMP-023 Working. png `W1-CMP-023-<theme>.png`; drive: +working short-1; open short-1, type "a", key enter. `components/chat/ChatComposer.tsx:859`
- W1-CMP-024 Steer. png `W1-CMP-024-<theme>.png`; drive: +working short-1, +pref midTurn=steer; as W1-CMP-023. `components/chat/ChatComposer.tsx:773`
- W1-CMP-025 Question dock. png `W1-CMP-025-<theme>.png`; drive: +reply question (control_request can_use_tool, AskUserQuestion), +working; open the thread. `shell/WorkspaceThread.tsx:113`
- W1-CMP-026 Send held. png `W1-CMP-026-<theme>.png`; drive: none; open short-1, type "/", key escape, hover send. `components/chat/ComposerPrimaryActions.tsx:291`
- W1-CMP-027 Narrow. png `W1-CMP-027-<theme>.png`; drive: width 600; new thread. `components/chat/composerMotion.ts:83`
- W1-CMP-028 New thread. png `W1-FRM-017-<theme>.png`; drive: none; new thread. `shell/ProjectHome.tsx:182`
- W1-CMP-029 Text size 17: editor, placeholder and chips (em) grow. png `W1-CMP-029-<theme>.png`; drive: +pref textSize=17; open short-1, type "@", key enter. `settings/theme.ts:94`
- W1-CMP-030 Drawer, Tasks row. png `W1-CMP-030-<theme>.png`; drive: +reply TodoWrite, +working short-1; open short-1. `components/chat/ComposerDrawer.tsx:104`
- W1-CMP-031 New thread asks for a project. png `W1-CMP-031-<theme>.png`; drive: +project second, +pref newThreadIn=ask; key mod+t. `shell/NewThreadPicks.tsx:44`
- W1-CMP-032 Creation room. png `W1-CMP-032-<theme>.png`; drive: none; new thread, type "x", key enter; shoot while "Setting up" stands (short on a local project). `components/chat/SetupCard.tsx:32`

### Actions and keys

- W1-CMP-033 Send, key `enter` or `mod+enter` by sendWith, the other and `shift+enter` a newline. `components/chat/ChatComposer.tsx:1050`
- W1-CMP-034 Enter while an IME composes does nothing. `components/composer-editor/plugins.ts:51`
- W1-CMP-035 `up` walks back this thread's prompts from an empty box or an unedited recall; `down` walks forward while recalling and empties the box past the newest. `components/chat/composerPromptHistory.ts:67`
- W1-CMP-036 Menu: arrows wrap, `enter`/`tab` pick, `escape` closes. `components/chat/ChatComposer.tsx:1025`
- W1-CMP-037 `mod+s` stashes; on an empty box brings back the one stashed prompt, or opens the list when there are two or more. `components/chat/ChatComposer.tsx:997`
- W1-CMP-038 Chips: left/right step over, backspace removes whole. `components/composer-editor/plugins.ts:149`
- W1-CMP-039 Paste: files take the caps; 200 lines or 32 KiB is pasted-text-N.txt. `components/chat/ChatComposer.tsx:608`
- W1-CMP-040 A printable key outside any field lands in the box. `components/chat/composerTypeToFocus.ts:27`
- W1-CMP-041 Stop; flyout if refused or running after 8 s. `components/chat/ChatComposer.tsx:740`
- W1-CMP-042 Mid-turn send queues; the head starts at turn end. `components/chat/ChatComposer.tsx:870`
- W1-CMP-043 Dock: digits, arrows, `space`, `enter`, `escape` folds. `components/chat/PromptDock.tsx:255`

### Host operations

- W1-CMP-044 `sessions.start` send. `components/chat/ChatComposer.tsx:667`
- W1-CMP-045 `sessions.interrupt` stop. `components/chat/ChatComposer.tsx:756`
- W1-CMP-046 `sessions.steer` steer. `components/chat/ChatComposer.tsx:786`
- W1-CMP-047 `sessions.aside` /btw. `components/chat/ChatComposer.tsx:719`
- W1-CMP-048 `sessions.warm` on focus. `components/chat/ChatComposer.tsx:491`
- W1-CMP-049 `sessions.access` access pick. `components/chat/ComposerOptionPickers.tsx:368`
- W1-CMP-050 `preferences.set` access pick. `components/chat/ComposerOptionPickers.tsx:362`
- W1-CMP-051 `sessions.answer` dock. `components/chat/answerPrompt.ts:39`
- W1-CMP-052 `workspaces.wake` paused send. `components/chat/ChatComposer.tsx:665`
- W1-CMP-053 `harnesses.list` picker lists. `components/chat/ComposerOptionPickers.tsx:138`
- W1-CMP-054 `daemon.send` fs.files for the @ menu (`components/chat/ChatComposer.tsx:526`), git.prList, git.status for the tray's branch. `components/chat/ComposerCheckoutRow.tsx:155`
- W1-CMP-055 `workspaces.checkout` branch. `components/chat/ComposerCheckoutRow.tsx:159`
- W1-CMP-056 `workspaces.create` New thread send. `shell/ProjectHome.tsx:129`
- W1-CMP-057 `conversations.list` the resume picker's list, read for every listable project under All projects. `shell/conversations.ts:100`
- W1-CMP-058 `workspaces.create` Retry and `workspaces.delete` Dismiss on a failed create in the creation room. `components/chat/SetupCard.tsx:26`

### Design

- W1-CMP-059 Today's look; the one pane that frosts on the Mac too: our patch's backdrop blur, grain on Linux, solid where glass is off.
- W1-CMP-060 Shared: drop `Card`, `glyph_frame`; dock `Choice`, `PickRow`, `Field`, `CopyRow`, `Button`; queue `Row`; `spinner`.
- W1-CMP-061 None yet: shell, tray, send, stop, chips, tiles, drawer, menus, banner.
- W1-CMP-062 Copy gpui-kit `Textarea` (tokens, on_paste), `Popover`, `PopupMenu`, `Tooltip`, toasts; Zed `anchored`.

## Port notes

Where GPUI cannot draw or behave as the browser does: how the GPUI app matches today's app, and the deliberate
differences. Each note keeps its D number, which code and tests cite; `more.md` continues from D23.

- D1 Host version: `HostClient::host_release()` returns the `version` the auth answer already carries
  (`packages/runtime/src/serve.ts:889`, set at `packages/host/src/server.ts:642`), which wsp-mcp's dial parses and
  keeps on its Host (`host-client/src/lib.rs:250`, read in `app/src/screens/settings/reads.rs:159`). The page reads
  the same value off its boot line today (`shell/shellVersion.ts:19`). The mark is the host's version, else the app's
  release, else "unknown", as `settings/about.tsx:101` does (`app/src/screens/settings/pages/about.rs:241`). The GPUI
  binary's release stands in for `window.wsp.version` (`about.rs:78`); App and Host lines show when the two differ
  (`about.rs:256`).
- D2 Settings draws as the desktop shell (`window.wsp` set); Electron is that shell, so the baselines show it. Its
  rows: When you quit; Start at login (this computer's host with a service); Logs; App and Host lines; the update
  step, where Get downloads the release through the app's own updater and the releases page opens only when the app
  cannot take it here (`app/src/update/mod.rs:145`), and Restart host stays `host.restart`; Glass on the Mac only;
  installed fonts as a select; the chords a tab keeps plus Close the window, `mod+shift+w`
  (`app/src/screens/settings/pages/general.rs`, `about.rs`, `appearance.rs`, `keybindings.rs`).
- D3 The minimap holds the rows the view holds, not the whole thread, as today
  (`components/chat/timeline/list.tsx:359`), gaining ticks as older pages load (`app/src/thread_view/render.rs`). The
  whole thread would mean reading 4,900 events on open, a prompts-only host op that does not exist, and 900 ticks 0.57
  px apart on a 512 px rail.
- D4 The composer's store: capped at 5 MB, written off the UI thread. Today localStorage `wsp:composer-drafts:v1` is
  rewritten whole on every keystroke and caret move (`components/chat/composerDraftStore.ts:182`), capped only by the
  origin quota. The app keeps the same key in its local store, one JSON file in the app's data folder
  (`app/src/local_store.rs`), written 300 ms after the last edit and at once on send, stash, switch and quit, through
  a temp file and a rename; the 5 MB cap is the whole file's, and past it the least recently edited drafts are
  dropped, never a queued message or a stash entry (`app/src/screens/composer/store.rs`). No import of Electron's
  drafts.
- D5 Deliberate differences: grain may be left out on Linux only where it measurably costs frames, and solid stand-ins
  stand where glass is off (today's Linux app is already solid; `app/src/screens/app_shell/material.rs`). The app
  draws grain on Linux too. The app must draw grain as today's noise (`index.css:855`) on body, sidebar and inset;
  Settings tiles it as a PNG (`app/src/ui/grain.rs`), but the main window now draws one flat grey at the tile's mean
  (`app/src/screens/app_shell/frame.rs`). The composer's backdrop blur is on unless `DESKTOP_GPUI_COMPOSER_GLASS=off`
  (`app/src/glass.rs`). Notices are the app's own toasts, a copy of Sonner's stack drawn as a gpui-kit Root plugin, at
  today's corner with today's words (`app/src/ui/toast/`).
- D6 Motion does not use gpui-ce-motion (1bye/gpui-motion f20749b, MIT or Apache-2.0, its adapter on `gpui-ce =0.2.2`,
  and its README says a retargeted tween restarts at full length). Hovers, switches and other retargeted values use
  gpui-kit 14f5933's own `base::motion` through our `ui/motion.rs`: `transition` keeps its state with
  `use_keyed_state`, starts a new target from the value sampled at that instant, and when the new target is the old
  start scales the duration by CSS's reversing shortening factor (`crates/base/src/motion.rs:268`, `:326`), so a hover
  that ends halfway back takes about half the time, as in Chromium. `Easing::CubicBezier` takes the curves as
  constants in `ui/motion.rs`; `Keyframes` and `Timing` run Settings' glide and flash
  (`app/src/screens/settings/mod.rs`), `Presence` the sheets (`app/src/screens/app_shell/frame.rs`); dialogs fade in
  through a hand-timed `open_fade` (`ui/motion.rs`); `App::reduce_motion` is honoured (`app/src/ui/status.rs`). GPUI's
  `with_animation` (time based, restarts per element id, no retarget; `gpui-pre-0.3.9/src/elements/animation.rs:84`)
  still runs one-shot fades and loops that never retarget: a slate piece's fade in, the minimap strip, the toolbar
  pill, onboarding and the setup steps. Colors ease in sRGB with premultiplied alpha, as `ui/motion.rs` does; the end
  states match Chromium's, and only frames mid-transition may differ.
- D7 Window minimum: none today (`desktop/window.ts:129`) and none in the GPUI app, whose window options set no
  `window_min_size` (`app/src/screens/dialogs/startup.rs`). Narrow states use 980, 760, 600 and 360.
- D8 Keys: one dispatcher reading `DEFAULT_KEYBINDINGS` and `preferences.keybindings` with today's when-words, not
  GPUI keymap contexts; chords a field must see are actions in its key context; bare keys only when no field has focus.
- D9 Per-window state (panel width and tabs, sidebar folds and picks, Settings page, drafts): a file in the app's data
  folder, never `preferences.set`.
- D10 Context menus: every right-click menu goes through the app's `ContextMenu` (`app/src/ui/context_menu.rs`). On
  macOS it pops an AppKit `NSMenu` at the pointer, as Electron's `Menu.popup` does (`app/src/ui/context_menu/mac.rs`).
  The app must show the OS menu on Linux too, as Electron pops it on every platform (`desktop/main.ts:156`); off the
  Mac the app now draws its own port of the web's in-app menu (`actions/ContextMenuHost.tsx`). gpui-kit's `PopupMenu`
  is used only for button dropdowns (`app/src/ui/menu.rs`).
- D11 A reason in an HTML `title` today: a gpui-kit Tooltip in wsp's skin, same words.
- D12 Shared pieces: since 4422c64 `ui/` has Switch, Segmented, Stepper, Grid and GridRow, Skeleton, Sparkline, grain,
  AlertDialog, row menus, Chip, `Card::body` and `locked`, and Row's reset arrow. Still missing: the sidebar's section
  head, StatusMark's kinds (Limited, Resuming, Stopped, Released, Expired, Not pushed, Starting, Snoozed, Finished),
  FileTile, Sheet, DigitRoll. The first screen that needs one adds it to `ui/`.
- D13 Sheets: our own overlay; gpui-kit `Sheet` draws a title and close button and slides 100 px in 150 ms, no fade.
- D14 The app menu on macOS only, as today; View keeps zoom and full screen, drops Reload and devtools; Edit
  dispatches to the focused field. Zoom is a window-wide UI scale.
- D15 Mac: transparent titlebar, traffic lights at (16, 19), `Blurred` background with each region's material share;
  checked against Electron by screenshot.
- D16 Reduced motion stops what today stops; a theme switch draws one frame with transitions at 0 (`index.css:689`).
- D17 Keep today's held marks (`protocol/store/useStore.ts:749`), so a moved or renamed thread does not jump back.
- D18 Repeated rows get computed widths and `flex_none`; every ticking thing (crab, timers, shimmer, noise field) is
  its own entity.
- D19 Lists: the sidebar on GPUI `list` over flat rows, Settled's 25 and Finished's 20 per page kept; the transcript on
  gpui-kit `MessageScroller`, not `VirtualList`, which needs every height up front.
- D20 Markdown on gpui-kit `TextView`, one entity per reply segment, code on its tree-sitter highlighter; math and
  mermaid show their source.
- D21 The composer on gpui-kit `Textarea` with `InlineToken` chips; send waits while the IME composes; no
  `submit_on_enter`.
- D22 The GPUI app has the thread view, the dialogs and the palette: a tile click opens the thread and marks it read
  with `sessions.read` (`app/src/thread_view.rs`); the sidebar's menu entries open their dialogs (`app/src/shell.rs`);
  the Search row opens the palette (`app/src/shell.rs`).

## Motion

Every transition and keyframe animation on the six screens, grouped; per-element lines with sources are in
`more.md`. Std is cubic-bezier(0.4, 0, 0.2, 1), Tailwind's default, which `index.css` does not override; ease-out is
(0, 0, 0.2, 1); snap is (0.23, 1, 0.32, 1).

| What | Property | Time | Curve |
|---|---|---|---|
| Hover and pick fills on rows, buttons, tabs, segments, selects, fields, links (about 100) | color, background, border, box-shadow | 150 ms | std |
| Acts shown on row hover; message meta rows | opacity | 150 ms; meta 200 ms | std |
| Chevrons | rotate 90, 180 or -90 deg | 150 ms; call rows 200 | std |
| Collapsibles | height 0 to auto | 200 ms | std |
| Switch | track 200 ms; thumb 150 ms; press scale 100 ms after 100 | | std; thumb ease |
| Tooltips, cards, popovers | opacity, 2 px slide, size | 150 ms after 600 (cards 450, chart 0) | std |
| Switcher menu | opacity, -2 px y; closes at once | 150 ms | std |
| Sidebar collapse, header pad | width, left, padding-left | 200 ms | linear |
| Sheets; dialogs | opacity, 32 px slide; scale 0.98 to 1 | 200 ms | ease-in-out |
| Settings sub list | height, opacity | 140 ms | snap |
| settle-in (Usage, Activity) | opacity, 3 px rise; table rows 30 ms apart | 180 ms | snap |
| Meters, context ring | width; stroke-dashoffset | 200 ms; share bar 500 | snap |
| Rolling digits | a 0 to 9 column | 500 ms | snap |
| Usage chart | tween from the last frame | 520 ms | 1-(1-k)^3 |
| Build progress | width | 300 ms | (0.16, 1, 0.3, 1) |
| Composer box; layout FLIP | height; translate | 180 ms | ease-out; (0.215, 0.61, 0.355, 1) |
| Composer dock on first message; glass | bottom; background | 300 ms; 200 ms | ease-out |
| Send and stop | all, hover scale 1.05 | 150 ms | std |
| Scroll to end pill | opacity, 4 px rise | 200 ms | ease-out |
| Minimap rail, ticks | opacity; width 8 to 24 px | 150 ms | std |
| Scrollbars | opacity in 100, out 150 after 300 | | std |
| Settings sheet search hit | background out | 900 ms | ease-out |
| Spinner; skeleton; shimmer | rotate; band sweep | 1 s; 2 s; 2.2 s loops | linear |
| Crab; noise field | canvas frames; repaint | per frame; 120 ms | |

No motion: Settings opening, banners, the panel opening inline, a thread switch, menus, the drop frame. Smooth
scrolls (scroll to end, minimap jump) are the browser's own.
