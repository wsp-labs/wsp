# Parity spec: frame, Settings, Usage, sidebar, thread view, composer, the rest

The items that did not fit `spec.md` (kept under 60,000 characters), in the same id space and the same five lists
per screen; they count as listed, and a state here is as required as one there. After them: each screen's own port
notes (D23 on) and every transition line by screen with its source. Paths and drive words read as in `spec.md`.

## Frame and layout

### Elements

- W1-FRM-058 Quit asks "Quit wsp?": Quit, Quit and stop wsp, Cancel. `desktop/quit.ts:37`
- W1-FRM-059 Panes B, T, D, R, F, M, P, A, S in launcher order. `panes.ts:56`
- W1-FRM-060 First launch window, welcome: the tilde mark at 320x200, a primary glow from the top and a halo behind the mark, 40 px primary "Get started →". `desktop/onboarding.html:142`
- W1-FRM-061 First launch, agents: "Your agents drive wsp", a sentence naming the computer, card of 48 px rows (box, 18 px glyph or initial, 15 px name, mono state), hint, "Open wsp →". `desktop/onboarding.html:173`
- W1-FRM-062 First launch words: states "installed", "not installed", "installs on first run"; three hint lines; refusals in two halves, what happened then the fix. `desktop/onboarding.html:193`
- W1-FRM-063 Mac menu bar item: template icon, a question mark icon while a thread waits; title is the working count, a remote host's name first; tooltip "wsp N". `desktop/main.ts:734`
- W1-FRM-064 Menu bar menu: "No threads working" or "N working, M waiting on you"; a row per live thread, "<state> on <computer>", submenu Open, Allow, Deny, Stop; Open wsp, Quit wsp. `desktop/tray.ts:129`
- W1-FRM-065 Hosts menu on the Mac: this computer's name, then each linked host's alias, a check on the host the window shows. `packages/protocol/src/words/places.ts:514`
- W1-FRM-066 Switcher overlay: dim blurred scrim, one glass row of up to 6 cards; card 224 px: 88 px page picture or project glyph, title, agent mark, computer, status. `components/switcher/WorkspaceSwitcher.tsx:81`
- W1-FRM-067 Terminal drawer slot under the centre column: top border, 280 px tall, 180 min, 75% of the window max, 6 px drag band on top; per thread, shut by default. `components/ThreadTerminalDrawer.tsx:64`
- W1-FRM-068 Overlays the shell mounts that none of these screens lists: the command palette, the file finder, the settle-with-processes question, the browser guests layer. `shell/AppShell.tsx:69`
- W1-FRM-069 First run words: "Add a project to get started", "A project is a folder on <here>. Its threads work in that folder.", "wsp sends anonymous usage counts to PostHog." `sidebar/words.ts:18`
- W1-FRM-070 Banner titles "wsp is not running, reconnecting." and "wsp is not running." with an Unplug glyph; the sign-in bar has an ExternalLink glyph, one bar per page. `shell/DisconnectedBanner.tsx:13`
- W1-FRM-071 Notices slot: top right of the centre column under the header, 8 px down, 20 px in, newest on top, three at most. `notices/Notice.tsx:108`
- W1-FRM-072 Starting window after 800 ms: 16 px spinner, "Starting wsp", centred on --background, 52 px drag strip. `desktop/starting.html:30`
- W1-FRM-073 Failed start dialog: error box, title, the failure's first line, Open log, Quit. `desktop/starting.ts:38`
- W1-FRM-074 Quit detail counts the threads working on this computer; General's standing answer skips the question. `desktop/quit.ts:49`
- W1-FRM-075 Mac menu rows: About, Services, Hide, Hide others, Show all, Quit wsp ⌘Q; File: Close window ⌘⇧W. `desktop/menu.ts:22`
- W1-FRM-076 Page root: GlassGround filter, then a blank --background while BootGate asks the shell for a token. `BootGate.tsx:107`
- W1-FRM-077 Settings: its sidebar in the slot, its page in the centre, Restore defaults in the header; panel and toggles hidden. `shell/AppShell.tsx:141`
- W1-FRM-078 Drawer and panel toggles leave the header for the tab strip while the panel is inline. `shell/AppShell.tsx:168`
- W1-FRM-079 Tab close: 16 px button, the glyph swaps to X on tab hover or focus. `components/ui/panel-tab-close-button.tsx:30`
- W1-FRM-080 Strip slot: the open pane's own control, right of the tabs, hidden when empty. `components/RightPanelTabs.tsx:445`
- W1-FRM-081 Missing active pane: "Nothing open here." and "Pick a panel from the tab strip above." `shell/RightPanel.tsx:186`
- W1-FRM-082 Launcher line: "A browser, a terminal, the changes, the files, the computer or what runs on it." `components/RightPanelTabs.tsx:255`
- W1-FRM-083 Pane words: Browser, Terminal, Changes, Pull request (only with one), Files, Computer, Processes, Agents, Slate; Side question by /btw. `panes.ts:56`
- W1-FRM-084 Banner words: "The runtime socket dropped..." or "Start wsp in a terminal, then reload this page." `shell/DisconnectedBanner.tsx:16`
- W1-FRM-085 Sign-in bar: "A sign-in page for <host> is ready on <workspace>", full URL on hover, Open, Dismiss. `shell/SignInBanner.tsx:24`
- W1-FRM-086 Linux controls: header, inline tab bar and banners end short of them. `index.css:147`
- W1-FRM-087 Connecting: "connecting to runtime…" mono 14 px muted. `App.tsx:153`
- W1-FRM-088 Projects refused: "Projects not read: <said>" centred. `App.tsx:111`
- W1-FRM-089 Project home: hero, thread head, composer, centred, 8vh under. `shell/ProjectHome.tsx:179`
- W1-FRM-090 Failed start: one card, the step rows, Open that thread. `shell/FailedStart.tsx:32`
- W1-FRM-091 Pair screen: "Pair this browser", hint, Pairing code, Pair, two-line error slot. `PairScreen.tsx:29`
- W1-FRM-092 DaemonDown: sentence, Start or "Starting…", refusal line; drawn by the diffs pane. `components/DaemonDown.tsx:70`
- W1-FRM-093 RenderErrorBoundary wraps each slate piece ("This part could not be drawn") and chat markdown's diagrams and code blocks; a renderer crash reloads the page, at most once a minute. `desktop/main.ts:96`
- W1-FRM-094 Under 768 px the sidebar is a left sheet, window less 12 px wide, shut at start. `components/ui/sidebar.tsx:28`
- W1-FRM-095 The rail is gone under 640 px. `components/ui/sidebar.tsx:616`
- W1-FRM-096 The sidebar opens every launch; the sidebar_state cookie it writes is never read. `components/ui/sidebar.tsx:25`
- W1-FRM-097 Panel per thread, shut by default on a thread and on this computer; a workspace's own pane opens empty. `rightPanelStore.ts:111`
- W1-FRM-098 Tab titles: a browser page's title, else its host, else "Browser"; the terminal's label; a file's base name with a file glyph, "Files" with a folder. `panes.ts:64`
- W1-FRM-099 Held hints on launcher tiles and add-menu rows: "Available while this runs.", "Review the changes once it runs.", "Pick a thread to use it.". `panes.ts:62`
- W1-FRM-100 Held reasons on this computer's panel: "Pick a project to review its changes.", "Pick a thread to read its files.", "Pick a thread to see its slate.". `panes.ts:93`
- W1-FRM-101 Panel drag handle: 8 px band over the left border, col-resize cursor; a 1 px line, border on hover, primary at 60% while pressed. `components/preview/RightPanelResizeHandle.tsx:24`
- W1-FRM-102 Sidebar toggle: 28 px ghost button, PanelLeftClose glyph while open, PanelLeft while shut. `components/ui/sidebar.tsx:349`
- W1-FRM-103 Window title "wsp", led by "• " while a prompt or a build's sign-in waits. `notices/hostNotices.ts:374`
- W1-FRM-104 Linux desktops off the list (tiling, Openbox): the desktop's own title bar on a plain window, no drawn controls, no menu. `desktop/window.ts:60`
- W1-FRM-105 Launch dialogs: "Move wsp to your Applications folder?", restart an earlier release's host, the serving-again warning, "wsp did not stop". `desktop/main.ts:849`
- W1-FRM-106 The panel toggle's working-agents badge never draws: the shell passes a count of 0. `shell/AppShell.tsx:125`

### States

- W1-FRM-107 First run, sign-in bar. png none: needs an empty host, a sign-in page. `App.tsx:97`
- W1-FRM-108 First launch, welcome then agents. png none: Electron only, a computer whose host holds nothing yet. `desktop/main.ts:624`
- W1-FRM-109 Menu bar, host down: "wsp is not running" and Start wsp; a remote host "<host> is not answering". png none: a native menu. `desktop/tray.ts:131`
- W1-FRM-110 Tab hover. png `W1-FRM-110-<theme>.png`; drive: as Tabs; hover Computer. `components/ui/panel-tab-close-button.tsx:30`
- W1-FRM-111 Rail hover. png `W1-FRM-111-<theme>.png`; drive: none; hover the sidebar's right edge. `components/ui/sidebar.tsx:638`
- W1-FRM-112 Launcher highlight. png `W1-FRM-112-<theme>.png`; drive: none; key mod+alt+b, key ArrowDown. `components/RightPanelTabs.tsx:186`
- W1-FRM-113 Many tabs, fade. png `W1-FRM-113-<theme>.png`; drive: as Tabs, then Add a panel and key b three times (a 456 px strip at the 540 px panel). `components/RightPanelTabs.tsx:357`
- W1-FRM-114 Closed banner. png none: needs a refused token. `shell/DisconnectedBanner.tsx:19`
- W1-FRM-115 Starting window. png none: Electron only, a start over 800 ms. `desktop/main.ts:612`
- W1-FRM-116 Pair screen. png none: the shell answers no token twice. `BootGate.tsx:108`
- W1-FRM-117 Projects refused, failed start. png none: a refused list, a fork that never stood. `App.tsx:107`
- W1-FRM-118 Window on another computer, socket down: no banner, the sidebar's line says the host sleeps. png none: needs a second computer. `shell/AppShell.tsx:153`
- W1-FRM-119 Theme on System follows the computer's dark mode live. png none: an OS setting. `settings/theme.ts:48`
- W1-FRM-194 No thread selected: the centre is the first project's home, its New thread page (folder mark, "What should we build in bench?", ASCII ground, composer, computer and branch chips); nothing draws a "Pick a thread" page. png `W1-FRM-017-<theme>.png`; drive: none. `App.tsx:87`
- W1-FRM-195 Workspace link: `wsp://workspace/<id>` opens the thread today's app shows for that workspace with none picked, on the fixture the panes thread, the newest. png `W1-FRM-195-<theme>.png`; drive: the fixture's realHost; hash `#open/workspace/<bench's workspace>`, reload. `shell/links.ts:29`

### Actions and keys

- W1-FRM-120 Rail drag sets width on release. `components/ui/sidebar.tsx:406`
- W1-FRM-121 Panel drag, width kept on release. `hooks/useResizableWidth.ts:142`
- W1-FRM-122 Palette, key `mod+shift+p` (`keybindingDefaults.ts:54`); it opens over Settings and over the switcher. `shell/shellCommands.ts:334`
- W1-FRM-123 File finder, key `mod+p` (`keybindingDefaults.ts:56`), text search `mod+shift+f` (`keybindingDefaults.ts:57`); needs a workspace, not under Settings. `shell/shellCommands.ts:338`
- W1-FRM-124 Open in editor, key `mod+o` (`keybindingDefaults.ts:69`); needs a workspace, not under Settings. `shell/shellCommands.ts:413`
- W1-FRM-125 Walk back, key `ctrl+shift+tab` (`keybindingDefaults.ts:79`), `mod+alt+arrowleft` (`keybindingDefaults.ts:60`); previous panel tab `ctrl+shift+tab` (`keybindingDefaults.ts:81`). `shell/shellCommands.ts:389`
- W1-FRM-126 Escape with a notice standing dismisses the newest notice before the dispatcher sees the key, so Settings stays open. `notices/Notice.tsx:98`
- W1-FRM-127 Launcher letters beside a composer answer only to keys pressed inside the panel; the launcher takes focus as it mounts. `components/RightPanelTabs.tsx:174`
- W1-FRM-128 The switcher paints 150 ms into the hold, after a picture of the thread on screen; a quick tap switches with no overlay. `shell/workspaceSwitcher.ts:46`
- W1-FRM-129 Launch opens the last workspace and thread kept per state file, else the address, else the sidebar's top row. `protocol/store/useStore.ts:642`
- W1-FRM-130 Closing the window on the Mac leaves the app in the menu bar, and a Dock click opens a new one; on Linux it quits. `desktop/main.ts:581`
- W1-FRM-131 Menu bar acts: Open opens the thread, Allow and Deny answer its prompt, Stop, Start wsp, Open wsp; Quit wsp asks the quit question. `desktop/main.ts:744`
- W1-FRM-132 A Hosts row moves the window to that host's page, then the menu redraws. `desktop/main.ts:387`
- W1-FRM-133 wsp:// link: open a thread, workspace, project home or Settings group once rows land; else "That link names a <kind> this wsp does not have." `shell/links.ts:20`
- W1-FRM-134 A link raises the window; one before the page is up waits; a window on another host goes home first; a second launch hands over its link or raises. `desktop/deep-link.ts:32`
- W1-FRM-135 First launch: Get started or Enter goes on; Open wsp installs the tools into ticked agents, then opens the app; Check again rescans. `desktop/onboarding.html:356`
- W1-FRM-136 Drawer, key `mod+j` (`keybindingDefaults.ts:45`); browser pane `mod+shift+j` (`keybindingDefaults.ts:53`). `shell/shellCommands.ts:360`
- W1-FRM-137 Switcher, key `ctrl+tab` (`keybindingDefaults.ts:78`) away from panel focus. `shell/shellCommands.ts:386`
- W1-FRM-138 Header toggles, pressed while open, tooltip with the chord. `components/chat/PanelLayoutControls.tsx:38`
- W1-FRM-139 Add menu: a pane's letter picks it. `components/RightPanelTabs.tsx:312`
- W1-FRM-140 Rail: under 2 px travel keeps the width; a collapsed rail takes no pointer and an open rail swallows a click, so no rail click opens the sidebar. `components/ui/sidebar.tsx:616`
- W1-FRM-141 Panel drag cancel reverts. `hooks/useResizableWidth.ts:157`
- W1-FRM-142 A composer focus request shuts the sheet. `shell/RightPanel.tsx:138`
- W1-FRM-143 Commands a focused terminal passes to the app; Control keys a shell keeps. `keybindings.ts:304`
- W1-FRM-144 Switch overlay: Escape cancels, key up commits, window blur cancels. `shell/KeybindingDispatcher.tsx:47`
- W1-FRM-145 Reload on the closed banner; Open, Dismiss on the sign-in bar. `shell/DisconnectedBanner.tsx:21`
- W1-FRM-146 Quit wsp ⌘Q asks the quit question on the Mac. `desktop/main.ts:394`
- W1-FRM-147 A theme switch holds transitions off for one frame. `settings/theme.ts:34`
- W1-FRM-148 http and https links the page opens go to the default browser; navigation off the host's page is refused. `desktop/main.ts:453`
- W1-FRM-149 After a tab closes, focus goes into the tab that takes its place, so the next mod+w closes that one. `shell/RightPanel.tsx:140`
- W1-FRM-150 Settings and a subagent page hide, not drop, what they cover: a long thread and its scroll are there on the way back. `shell/AppShell.tsx:144`
- W1-FRM-151 The active tab scrolls into view when it changes. `components/RightPanelTabs.tsx:335`
- W1-FRM-152 Context menu rows carry their chord as accelerator and a live row's hint as hover text; a failed act is the notice "<title>: <said>". `desktop/context-menu.ts:48`
- W1-FRM-153 Sidebar width follows the record live: a Settings reset or another window's drag moves it; a press with no travel saves nothing. `components/ui/sidebar.tsx:580`
- W1-FRM-154 Pair screen: Enter submits, Pair held while empty, "Pairing" while busy; the shell is asked again each time the window comes forward. `PairScreen.tsx:43`
- W1-FRM-155 The New thread page's panel moves to the thread its first send made; the page itself opens with its panel shut. `rightPanelStore.ts:700`
- W1-FRM-156 Panels of workspaces the host stops listing are dropped; panels of 100 threads are kept. `rightPanelStore.ts:678`
- W1-FRM-157 A panel or sidebar drag sets the col-resize cursor and stops text selection window-wide until release. `hooks/useResizableWidth.ts:102`

### Host operations

- W1-FRM-158 window `capturePreview`, `workspacePreview` the switcher's page pictures, keyed by theme and thread. `shell/workspacePreviews.ts:38`
- W1-FRM-159 `sessions.answer`, `sessions.interrupt` from the menu bar's own connection, which also reads `sessions.list`, `workspaces.list`, `places.list`. `desktop/host-feed.ts:179`
- W1-FRM-160 `pair.redeem` the pair screen's code. `protocol/pairing.ts:66`
- W1-FRM-161 `workspaces.list` with projects.list decides first run. `protocol/client.ts:1079`
- W1-FRM-162 `workspaces.restartDaemon` DaemonDown's Start. `components/DaemonDown.tsx:37`
- W1-FRM-163 window `getBundle` downloads a release in the background. `shell/update.ts:111`
- W1-FRM-164 window `hosts`, `version` for the version notice. `shell/shellVersion.ts:43`
- W1-FRM-165 window `onOpen` wsp:// links. `shell/links.ts:62`
- W1-FRM-166 `status.subscribe`, `forwards.list`, `init.get`, `places.list`, `golden.get`, `harnesses.list`, `capabilities.get` on every connect; a refusal is a notice. `protocol/store/useStore.ts:324`
- W1-FRM-167 window gap keep awake: a power hold while a thread works on this computer and Keep awake is on. `desktop/main.ts:773`
- W1-FRM-168 window `log:renderer` writes page errors nobody caught to the app log. `desktop/preload.ts:104`

### Design

- W1-FRM-169 Inside the sidebar, background, card, accent, muted, input map to sidebar-canvas, -card, -hover, -hover-ink, -fill, -quiet, -input; `Palette` has none. `index.css:800`
- W1-FRM-170 Mac dark, inline panel and Settings page: clear ground, card white 2%, muted 3%, border white 10%, input 16%; muted ink glass-ink 85% everywhere. `index.css:1515`
- W1-FRM-171 Mac dark over glass: the sidebar's quiet inks all take glass-ink; status ink working #fbcfe8, failed #ffc9c9, input #c6d2ff. `index.css:1630`
- W1-FRM-172 Solid when Transparency is off, the computer asks for less transparency, or the window is not the Mac's; the window's glass follows. `settings/theme.ts:64`
- W1-FRM-173 Chord labels: Mac "⌃⌥⇧⌘K", elsewhere "Ctrl+Alt+Shift+K"; keys read Esc, Up, Down, Left, Right, Space. `keybindings.ts:351`
- W1-FRM-174 Banner: no fill, no border, 40 px floor, 16 px sides, 13 px text, 14 px muted glyph; title and line truncate. `components/ui/alert.tsx:10`
- W1-FRM-175 Switcher overlay, drawer frame, first launch screens, menu bar item: none yet in ui/.
- W1-FRM-176 Font stacks `--font-sans-system`, `--font-mono-system`, as theme.rs SANS and MONO. `index.css:159`
- W1-FRM-177 Scrollbar 6 px, thumb radius 3, light rgb 217 and 191 on hover, dark white 8% and 12%. `index.css:904`
- W1-FRM-178 Focus: outline in ring at 50%, kit buttons a 2 px ring. `index.css:616`
- W1-FRM-179 Themes paper, linen, frost set 43 tokens; graphite, denim, tungsten, moss, pitch add glass-ink, material-ground, -raised, -edge. `themes/index.ts:17`
- W1-FRM-180 Grain: 256 px fractal noise tile at 3.5% on body, sidebar and inset. `index.css:855`
- W1-FRM-181 Mac dark material shares: sidebar 40%, centre 67%, panel 74%; light sidebar veil 100%. `index.css:1473`
- W1-FRM-182 The solid class takes every glass and share to 100%. `index.css:1556`
- W1-FRM-183 `Field`, `Button` primary for the pair screen; `Card`, `StepRow` for the failed start.
- W1-FRM-184 Also gpui-kit `WindowBorder`, `Alert`, `Empty`, `Kbd`, `Tooltip`; GPUI `TitlebarOptions`, `WindowBackgroundAppearance`, Zed's `window_shadow` example.
- W1-FRM-185 Off the Mac, glass is the ground at 80% under a 12 px blur, saturate 1.14, dark 16 px and 1.08; only the Chromium reference draws it. `index.css:1435`
- W1-FRM-186 Header: 12 px sides, 20 px from 640 px wide; 16 px between controls; toggles 28 px, 32 px under 640, 16 px glyph at 80%. `components/WorkspacePageHeader.tsx:14`
- W1-FRM-187 Tooltip: hairline, popover ground, small shadow, 13 px, 8 px sides, 4 px top and bottom, radius 8, 4 px off its side. `components/ui/tooltip.tsx:46`
- W1-FRM-188 Add a panel menu: dropdown glass, radius 12, popover shadow, 176 px min, 28 px rows with the letter at the right, 6 px under. `components/ui/menu.tsx:62`
- W1-FRM-189 Switcher scrim: background at 60%, dark 64%, 4 px blur. `index.css:355`
- W1-FRM-190 Tokens: text meta 11, note 13, head 13.5, title 15; radius field 7, card 11, popover 12, composer 22; `ui::size` lacks the last two. `index.css:168`
- W1-FRM-191 Chrome text does not select; fields, pre, code and select-text areas do. `index.css:632`
- W1-FRM-192 Sheet: popover ground, left hairline, 1 px top shade, black 4% light, white 6% dark. `components/ui/sheet.tsx:72`
- W1-FRM-193 Linux window controls placed on the left push the sidebar toggle past them. `index.css:142`

## Settings

### Elements

- W1-SET-087 Sign-in under a row: copy line, key or token field, code and Open, browser wait, 344 px terminal, refusal. `components/agents/SignInFlowView.tsx:24`
- W1-SET-088 Item page: head (acts, status, switch), about, Details (copy on hover, links), tools, SKILL.md, Remove. `settings/agentKinds.tsx:575`
- W1-SET-089 Add a tool server: Agent, Name, Reached by, Command or URL, Where; masked pairs; target file, Add server. `settings/AddServerRows.tsx:147`
- W1-SET-090 Model row: grip when shown, Default mark, id under label, x on added ids, shown switch; id field and Add. `settings/agentPage.tsx:186`
- W1-SET-091 Live band under a computer row: CPU, memory lanes, 15 min, figures right; away: muted, "Last read HH:MM". `settings/LiveBand.tsx:34`
- W1-SET-092 Setup log row: 16 px mark, name, note, ticking mono time, turning chevron; under it refusal, wait block or log lines. `settings/add/StepRow.tsx:93`
- W1-SET-093 Image row: file glyph, Your image, state note, spinner or press; build box (320 px, scrolls) only while it needs you. `settings/ImageCard.tsx:218`
- W1-SET-094 Recipe page: head row and icon select, note, Agents, MCP servers, CLIs, Skills (first 5) picks, Name, Delete. `settings/recipes.tsx:150`
- W1-SET-095 General, Composer: Send with (segments Enter, ⌘ Enter or Ctrl Enter), A message while a thread works (Queue, Steer). `settings/general.tsx:76`
- W1-SET-096 General, Notifications: When a thread needs you, When a thread finishes (Off, Notify, Sound, Notify and sound), When a plan window runs low (switch). `settings/general.tsx:84`
- W1-SET-097 General, Threads: New thread starts in (Current project, Ask every time), Settle a thread after (15 minutes to Never), Ask before deleting (switch). `settings/general.tsx:93`
- W1-SET-098 Keybindings, Window and panels in order: Search, Find a file, Search in files, Settings, Toggle the sidebar, terminal drawer, right panel, panel tabs, preview. `settings/keybindings.ts:22`
- W1-SET-099 Tasks and threads in order: New thread, Next/Previous task, Next/Previous thread, Settle, Next that needs you, Find, older/newer match, Open in editor, Jump. `settings/keybindings.ts:23`
- W1-SET-100 Terminal, while it has focus: Split the terminal, New terminal, Zoom in, Zoom out, Reset the zoom. `settings/keybindings.ts:24`
- W1-SET-101 The theme card's head names the side drawn, "Light themes" or "Dark themes"; under System it follows the computer's side live. `settings/appearance.tsx:83`
- W1-SET-102 Type rows in order: App font, Reading size (Default, 13 to 17 px), Code font, Code size (Default, 11 to 15 px). `settings/appearance.tsx:123`
- W1-SET-103 Type sample's code block wears the renderer's head: the word "ts", Wrap lines and Copy code glyph buttons; the line in syntax colours. `components/markdown/codeBlocks.tsx:212`
- W1-SET-104 The settings sidebar sits in the app's resizable sidebar: its width, drag rail and glass ground; its top row holds the toggle and wordmark. `shell/AppShell.tsx:139`
- W1-SET-105 Kind cards: Tool servers Global, then one per project; Skills System, Global, then projects; Plugins one per agent in catalog order, then projects. `settings/agentKinds.tsx:431`
- W1-SET-106 Kind card heads: the first "<group> on <computer>" with its checked time and refresh; a project's head has its folder after the name, muted 13 px. `settings/agentKinds.tsx:273`
- W1-SET-107 Add words: "Add skill" opens the skills.sh search, "Add a tool server" the form; a held Add carries its reason (away word, not yet) on hover. `settings/agentKinds.tsx:408`
- W1-SET-108 Server status: Connected or Signed in (with N tools) green, Needs sign-in amber, Failed red; Checking, No sign-in needed, Key from the environment, Off, Not checked grey. `components/agents/kinds/servers.ts:108`
- W1-SET-109 Server row's right end: Sign in while it needs one, the Reconnect glyph beside Failed, Check for a command on another computer, else its status word. `components/agents/kinds/servers.ts:220`
- W1-SET-110 Server page Details: Status (why it failed after), Command or URL (copy), Environment or Headers names, Config location per agent, Tools, Recipe. `components/agents/kinds/servers.ts:252`
- W1-SET-111 Skill page Details: Status (System: always on, kept current; project: in the repo), Description as prose, each folder's path, shared or agent mark. `components/agents/kinds/skills.ts:185`
- W1-SET-112 Skill page acts: your own skill a switch and Remove; a project's skill Remove and no switch; a System skill none. `components/agents/kinds/skills.ts:38`
- W1-SET-113 Found skill page: Status, Source, Installs; an Agents card (a switch per installed agent, held "reads the shared skills folder"); Where; Install <name>. `components/agents/kinds/skills.ts:115`
- W1-SET-114 Plugin row's right end: your own plugin a switch; a project's On or Off (where it is set on hover); Missing in amber when the agent loads none of it. `components/agents/kinds/plugins.ts:23`
- W1-SET-115 Plugin page: about (description or why missing); Details Status, Marketplace (GitHub link), Version, Scope, Path, Set in; What it brings, counted. `components/agents/kinds/plugins.ts:71`
- W1-SET-116 Agent row: "v<version>" (or installs on first run, version unreadable) under the name; Signed in, Signed in with a token, Needs sign-in, Not checked. `components/agents/kinds/agents.ts:25`
- W1-SET-117 How it runs: Program (path, Change), Config folder ("<agent>'s own folder", Change), Launch arguments (count, Edit), Environment (count, values hidden, Edit). `settings/agentPage.tsx:442`
- W1-SET-118 Agent page Model row: an effort select, 96 px, beside the model only where that model has efforts; no Model row when the agent lists no models. `settings/agentPage.tsx:276`
- W1-SET-119 State cell, first that holds: Blocked, Failed, Needs you, away word, Setting up, Behind (spinner while syncing), Update or Behind, Sign in, Ready. `settings/places.ts:174`
- W1-SET-120 State marks: failed alert, needs you question, pending dashed ring, offline unplug, working spinner, Ready none; a tooltip says why. `components/status/StateMark.tsx:17`
- W1-SET-121 Computer glyphs: a Mac by model (laptop, desktop, Mac mini and Mac Studio drawn here, tower), this computer desktop, joined server, cloud brand mark. `settings/ComputerGlyph.tsx:44`
- W1-SET-122 Cloud limit band: Machines at once, Starts this minute, hour, today as "N of M" over a 4 px bar, warning from 90%, danger at the limit; counting line. `settings/LimitBand.tsx:57`
- W1-SET-123 Span choices: Nap 10 to 30 minutes, 1 to 3 hours, Never; Turn limit 1 to 24 hours, Off; Delete after At once to 3 hours; odd values slot in. `settings/computerSettings.tsx:23`
- W1-SET-124 Size select reads "Small, 2 vCPU, 4 GB" on its trigger and adds the hourly rate in its list; Spend per day is a field with a $ addon. `settings/computerSettings.tsx:148`
- W1-SET-125 Name and SSH sheets: title, sentence, one field (SSH mono, user@host), Save held while empty; saving says "Checking that <login> reaches <name>". `settings/computerSettings.tsx:298`
- W1-SET-126 Recipe pick rows: checkbox, 32 px frame (agent mark, plug, terminal, scroll), name, mono version, agent marks, note; size, calls or source at right. `settings/add/PickRow.tsx:70`
- W1-SET-127 Recipe Agents lists every agent offered, unticked ones muted, ticked ones naming their sign-in way; MCP servers, CLIs, Skills list only what it holds. `settings/add/PickLists.tsx:84`
- W1-SET-128 Recipe with keyed MCP servers: a radio pair under them, "Copy keys to <computers>" naming each server's keys, or "Leave them on <here>". `settings/add/PickLists.tsx:195`
- W1-SET-129 Activity figure inks: memory warn past 2 GB, danger past 8 GB; CPU 100% and 400%; power 1 W and 3 W; wsp's tile 50% and 75% of the computer. `settings/activity/model.ts:494`
- W1-SET-130 Slate heading row: a thread's slate runs and tool servers folded under "Slate: <title>", figures summed, no tag; End all in its menu. `settings/activity/tables.tsx:223`
- W1-SET-131 Activity under 640 px: tiles in two columns, computer select full width, kind and thread selects side by side, the filter full width. `settings/activity/page.tsx:241`
- W1-SET-132 Group order: General, Appearance, Agents, Computers, Recipes, Projects, Usage, Activity, Keybindings, Privacy, Devices, Account. `settings/groupIds.ts:6`
- W1-SET-133 Search field: 28 px, search glyph 14 px muted, "Search settings" at 13 px, hairline under it. `settings/SettingsSidebar.tsx:161`
- W1-SET-134 Group rows are 36 px (rowGrammar h-9 beats size sm's h-7), not the 28 px the file's comment says. `sidebar/rowGrammar.ts:26`
- W1-SET-135 A group whose sub page is open reads in the foreground ink with no fill; the page's own row is lifted. `settings/SettingsSidebar.tsx:205`
- W1-SET-136 General's meta word: the newer version while this wsp is behind it, mono 11 px at the row's right. `settings/about.tsx:125`
- W1-SET-137 Search page: one card per matching group, the group's name as its head, a link to its page. `settings/SettingsPage.tsx:47`
- W1-SET-138 Usage and Activity frame: 1080 px, 56 px top, 48 px gap, card head 14 px regular, radius 14. `settings/SettingsPage.tsx:97`
- W1-SET-139 Select in a row: 176 x 30 px (144 px under 640), radius 7, 13 px. `settings/layout.ts:25`
- W1-SET-140 Restore defaults is the glyph alone with its tooltip under 640 px. `settings/SettingsHeaderActions.tsx:31`
- W1-SET-141 Version mark: host version, else app version, else "unknown". `settings/about.tsx:101`
- W1-SET-142 Version: App and Host lines in a card of their own once the shell's and the host's versions differ. `settings/about.tsx:108`
- W1-SET-143 Version state line: "<v> is out", checking, checks off, "Up to date, checked <when>", could not reach GitHub. `settings/about.tsx:65`
- W1-SET-144 Version step: Check for updates, or Get <v>, Downloading, Quit and open, Restart to update, Restart host. `settings/about.tsx:90`
- W1-SET-145 Startup and quit: When you quit (desktop), Start wsp at login (own computer's service), Keep <computer> awake. `settings/general.tsx:153`
- W1-SET-146 Logs card, App logs with Open logs, only where the shell offers openLogs. `settings/general.tsx:171`
- W1-SET-147 Editor row: select with editor glyphs; no control before editor.list answers, the no-editor note on an empty list. `settings/general.tsx:105`
- W1-SET-148 Mode tile: 16:10 window picture (sidebar, message, reply, composer, accent dot), label; picked ring 2 px offset 2. `settings/ThemePicker.tsx:61`
- W1-SET-149 Theme card: window picture over a rule, name 13 px medium, description clamped to 2 lines. `settings/ThemePicker.tsx:78`
- W1-SET-150 Type sample: a reply with a code line, drawn by the conversation's renderer in the picked faces and sizes. `settings/appearance.tsx:73`
- W1-SET-151 Font rows: installed families as a select, Default first, each in its face; a typed field where the shell gives none. `settings/FontPicker.tsx:32`
- W1-SET-152 Glass card in a browser tab and the Mac app only, never on a solid desktop window. `settings/appearance.tsx:116`
- W1-SET-153 Fixed card: Send the message (General's send key), Submit a review comment, Leave Settings Esc, Close the window (desktop). `settings/keybindings.ts:76`
- W1-SET-154 Chords a browser tab keeps are drawn only in the desktop shell. `settings/keybindings.ts:46`
- W1-SET-155 Keycaps listening: "Press keys" 12 px muted on the accent fill. `settings/ChordKeys.tsx:90`
- W1-SET-156 Agent versions switch held off while the host's update checks are off, with the reason on hover. `settings/privacy.tsx:40`
- W1-SET-157 Available to install card: mark, name, about line; Install is held today, so its slot draws nothing. `settings/agents.tsx:306`
- W1-SET-158 Not read card: a row per reader the host could not read or recipe row that failed, the reason under it. `settings/agentKinds.tsx:129`
- W1-SET-159 Card head: "checked <ago>" or the stale word with its reason on hover, then refresh, disabled and spinning while reading. `settings/agentKinds.tsx:168`
- W1-SET-160 Status word: 13 px muted word, then an 8 px dot, green, amber, red or grey; its reason on hover where it has one. `settings/agentKinds.tsx:90`
- W1-SET-161 Act button: xs outline with glyph, spinner while busy; a held one sits in a box whose title is the reason. `components/agents/agentsParts.tsx:22`
- W1-SET-162 Row lead: the agent's mark, a server's brand mark or fetched 20 px icon, else the kind's glyph. `components/agents/agentsParts.tsx:90`
- W1-SET-163 Fact lines: prose row, paste line in a CopyRow, value with hover copy, agent mark, status, link with external glyph, act. `settings/agentKinds.tsx:452`
- W1-SET-164 Choice cards: many-pick rows with switches, a held one on with its reason; one-pick select line with its lost note. `settings/agentKinds.tsx:495`
- W1-SET-165 Tools card (a server's tools, a plugin's brings): read time, refresh, rows with a count, empty or refused line. `settings/agentKinds.tsx:544`
- W1-SET-166 SKILL.md: bordered box 168 to 480 px tall that scrolls, restricted markdown, "first N KB" note, skeleton, refusal. `components/agents/SkillPreview.tsx:24`
- W1-SET-167 Add skill page: search, skills.sh link, skeleton while asking, found rows in mono with installs or "installed". `settings/agentKinds.tsx:652`
- W1-SET-168 A refusal of the person's own ask, under its kind row in destructive ink. `settings/agentKinds.tsx:264`
- W1-SET-169 Agent head line: "Signed in with <plan>" and address, the host's whole sentence on hover; else the sign-in word. `settings/agentPage.tsx:400`
- W1-SET-170 Access segments Ask, Auto-edit, Full, Plan; a held one at half ink with its reason as title. `settings/agentPage.tsx:300`
- W1-SET-171 Line for an installed agent wsp runs no threads on, in place of the cards. `settings/agentPage.tsx:436`
- W1-SET-172 Project new threads: Default agent, Model, Access; the unset value "Default (x)" widens its select. `settings/projects.tsx:197`
- W1-SET-173 Look card: Icon select (glyphs in the hue, Choose an image), Colour select with swatches. `settings/projects.tsx:336`
- W1-SET-174 Repository row: remote in mono, Open where it parses as a web address. `settings/projects.tsx:331`
- W1-SET-175 Project Remove: in-use refusal or the landing line, Remove outline, red on hover, held while in use. `settings/projects.tsx:131`
- W1-SET-176 Account sentence ends "Not from the app yet."; the button is held with no title. `settings/account.tsx:36`
- W1-SET-177 The tabs hide while an item or add page stands under them; the computer select stays. `settings/agents.tsx:337`
- W1-SET-178 Settings sidebar sub lists: a page per agent a thread runs on, a page per project; search finds each agent page. `settings/agents.tsx:59`
- W1-SET-179 Behind row under a computer head: warning title, the update or install sentence, "Update wsp on <name>", then "Updating". `settings/computerSettings.tsx:46`
- W1-SET-180 State line under the head: word, mark or act, sentence muted and cut with a hover; "Try now" or "Try over ssh" while away, "Dialling…" while it waits. `settings/computers.tsx:379`
- W1-SET-181 Cloud head: plan line, credit line, starts today in tabular figures, then the limit band. `settings/computers.tsx:487`
- W1-SET-182 Recipe card: Follows a recipe, "Changes to it reach <name> on their own.", select of recipes with their glyphs, and None. `settings/computers.tsx:513`
- W1-SET-183 Setup card: head Setup, one row with Done or Needs you, the rows left or failed, Show setup log or Hide setup log. `settings/computers.tsx:609`
- W1-SET-184 Setup wait block: device code, Open the tab again, Skip for now, a later note; ran out: "The sign-in page ran out." and Retry. `settings/add/AddComputerDialog.tsx:282`
- W1-SET-185 Step log: last lines in 11 px mono, 192 px high at most, pinned to the end; "No output yet." before any. `settings/add/StepRow.tsx:155`
- W1-SET-186 Sign-ins card: head "Sign-ins on <name>", lede; per shared agent: name, road sentence, mono state, Sign in or Cancel, its flow under. `settings/ComputerSignIns.tsx:34`
- W1-SET-187 What runs here row: "N threads running, and wsp's own parts", chevron; opens Activity on this computer. `settings/computers.tsx:579`
- W1-SET-188 Limits card: Machines at once, Spend per day, "Machine size for a sub-thread", "Delete a finished sub-thread's machine after", Threads at once, "Nap a quiet workspace after", Turn limit. `settings/computerSettings.tsx:219`
- W1-SET-189 Threads here card: Agents may start agents switch; Levels deep stepper while it is on. `settings/computerSettings.tsx:259`
- W1-SET-190 Agents row: "Agents, tool servers, skills and plugins on <name>", opens Agents with this computer picked. `settings/computers.tsx:559`
- W1-SET-191 Cloud page Agents and Tool servers lists off the image: mark, name, state note, Version column; no act, no sign-in. `settings/computers.tsx:419`
- W1-SET-192 Name and login card: Name row, SSH login row in mono (or the none word), each with Change. `settings/computerSettings.tsx:294`
- W1-SET-193 Remove line: "Remove <name>", what a remove takes in one sentence, danger outline Remove; never on this computer. `settings/computers.tsx:464`
- W1-SET-194 Image build box: step frame (title, sentence, links, primary, refusal slot) over the stages or the sign-ins alone. `settings/ImageBuild.tsx:50`
- W1-SET-195 Build stage: ring, dot, check or cross, name, elapsed, state word or spinner, chevron; open: up to 8 lines (6 under 700 px high), ANSI colours. `settings/recipe/BuildRows.tsx:204`
- W1-SET-196 Build sign-in: mark column, name, state; action line with the code in 20 px mono, Open, Retry, code field and Submit. `settings/recipe/BuildRows.tsx:134`
- W1-SET-197 Recipe delete confirm: "Delete <name>?", what leaving does, Cancel and Delete. `settings/recipes.tsx:196`
- W1-SET-198 Recipe Name row: a 176 px field, read-only while a computer follows the recipe. `settings/recipes.tsx:183`
- W1-SET-199 Recipe's Skills list: the first 5, then "N more skills on this recipe." `settings/add/PickLists.tsx:265`
- W1-SET-200 Activity tiles: wsp memory "of X in use" tinted by share, CPU "100% is one core", Power "CPU power, last 2 s", Processes "N threads". `settings/activity/page.tsx:235`
- W1-SET-201 Activity group head: glyph frame (harness mark, wsp, dashed circle), name, status, project note, totals, chevron; 20 px nest per lead. `settings/activity/tables.tsx:345`
- W1-SET-202 Process row: indented by depth, kind glyph or harness mark, name, kind tag, ports (":3000 +1"), inked figures, menu on hover. `settings/activity/tables.tsx:213`
- W1-SET-203 Subagent row (state, time, tokens, model as tags), container row (image, Up time), Task manager row (the sampler's cost). `settings/activity/tables.tsx:244`
- W1-SET-204 Processes view: flat rows, PID column, the thread under the name; the rest of the computer muted with Every process. `settings/activity/tables.tsx:542`
- W1-SET-205 Kinds view: a head per kind (Agents, Dev servers, Builds...) with its process count; rows name their thread. `settings/activity/tables.tsx:616`
- W1-SET-206 Ports view: port, process, thread, Local only or Open to the network, Open on hover, menu. `settings/activity/tables.tsx:663`
- W1-SET-207 Activity ask dialog: "Stop the turn in <title>?" or End all, the sentence, Cancel and the red press. `settings/activity/live.tsx:300`
- W1-SET-208 Theme grid: columns of at least 208 px, two under 640 px; Mode grid three columns; gaps 12 px, 8 px under 640. `settings/ThemePicker.tsx:151`
- W1-SET-209 Open in's ssh switch row stands only once the host answers ssh.include; Keep <computer> awake only once the computer's name is known. `settings/general.tsx:131`
- W1-SET-210 Privacy, Devices and Account cards have no head, so those groups draw no trail; Agent versions never carries a reset arrow. `settings/privacy.tsx:23`
- W1-SET-211 The group list scrolls under the fixed search field and draws no scrollbar. `components/ui/sidebar.tsx:712`
- W1-SET-212 Agent page: the computer select alone above its head, no tabs; "Not installed here" as the head's line where that computer lacks the agent. `settings/agentPage.tsx:393`
- W1-SET-213 Models: models off the picker follow the shown ones with no grip; the Model note reads "<agent> here does not run <model>; running <other>". `settings/agentPage.tsx:254`
- W1-SET-214 The computer select lists this computer and those joined to it, never a cloud; a picked one that left falls back to this one. `settings/agents.tsx:48`
- W1-SET-215 Add a tool server where no agent keeps servers in a file: one line "No agent on <computer> keeps tool servers in a file wsp writes." `settings/AddServerRows.tsx:139`
- W1-SET-216 Add a tool server placeholders: notion, an npx line, https://mcp.notion.com/mcp, API_KEY or Authorization, value. `settings/AddServerRows.tsx:167`
- W1-SET-217 Devices: the device wsp init opened reads "this browser"; inside the minute "seen just now"; a held Revoke ends the line "not on this wsp yet". `settings/devices.tsx:23`
- W1-SET-218 Project page head: "<source> on <computer>" under the name, "<n> threads, <m> running" at the right in the value ink. `settings/projects.tsx:330`
- W1-SET-219 Project new threads: no card until the project's computer lists its agents; Model and Access only where the agent has one; Access lists only levels it takes. `settings/projects.tsx:186`
- W1-SET-220 Kind glyphs: agent bot, tool server, dev port, test flask, build hammer, slate panels, terminal, tab globe, app window, wsp cog, other dashed circle. `settings/activity/tables.tsx:51`
- W1-SET-221 Native hover titles: a process's command (or "wsp needs this to run"), container placement, subagent and sampler notes, state sentence, live band. `settings/activity/tables.tsx:216`
- W1-SET-222 Live band: the memory lane and figure take the warning ink when memory is near full; CPU lane sky, memory violet, both muted while away. `settings/LiveBand.tsx:32`
- W1-SET-223 Computer head line: system, cores, memory as last reported ("macOS, 10 cores, 32 GB"); the versions stand as the mono mark after the name. `settings/computers.tsx:479`
- W1-SET-224 Image row note: "built <when>", "<stage>, n of m" or the copy sentence, each closed with a stop, then "On your image: <names>." `settings/ImageCard.tsx:216`
- W1-SET-225 Build stage lines: "pnpm:" prefix at 60%, 16 ANSI colours from the terminal theme, a failed stage's last line in danger, pinned to the newest. `settings/recipe/BuildRows.tsx:272`
- W1-SET-226 Tile lines: one point a frame, 150 kept (5 minutes), dropped on leaving; scaled to the peak times 1.15, never under 1 GB, 100%, 1 W. `settings/activity/page.tsx:271`
- W1-SET-227 Computers' sidebar sub-rows, one per computer, this one first; Settings search finds a computer by name with its state sentence under it. `settings/computers.tsx:114`
- W1-SET-228 Under 768 px a thread head carries a "..." button beside its name, opening its menu (Open thread). `settings/activity/tables.tsx:355`

### States

- W1-SET-229 Slots under the words. png `W1-SET-229-<theme>.png`; drive: width 600; settings agents. `settings/rows.tsx:123`
- W1-SET-230 Update arrow, other computers, plugins, signed-in account. png none; needs a newer release, a second computer, plugin files, cloud sign-in. `settings/agents.tsx:204`
- W1-SET-231 Other computer, pending, cloud, setup log: png none; needs a computer over ssh or a cloud key. `settings/computers.tsx:138`
- W1-SET-232 A read refused on open: error notice "<Setup, Editors, Account, Devices, Newest release> not read: <said>"; its row stays away. png none: needs a refusing host. `settings/settingsReads.ts:48`
- W1-SET-233 Agent missing on the picked computer: head line "Not installed here" and no slot. png none: needs an agent a thread runs on that the read lacks. `settings/agentPage.tsx:418`
- W1-SET-234 A process being ended: its tag reads Ending and a spinner takes its menu until it leaves the frame; a container's stop the same. png none; one frame. `settings/activity/tables.tsx:179`
- W1-SET-235 Activity refused: the daemon's refusal of the process watch in place of the tables. png none; needs a daemon that refuses proc.watch. `settings/activity/page.tsx:180`
- W1-SET-236 A cloud row while its setup runs or after it failed: spinner or failed mark in the state cell. png none; needs a cloud key and a setup. `settings/computers.tsx:279`
- W1-SET-237 First open lands on Appearance. png `W1-SET-237-<theme>.png`; drive: none; settings. `settings/settingsStore.ts:20`
- W1-SET-238 Newer release: meta word, Get. png none: needs a release newer than the host. `settings/about.tsx:125`
- W1-SET-239 Trail lit as the page scrolls. png `W1-SET-239-<theme>.png`; drive: none; settings general, scroll page to the foot. `settings/SettingsSidebar.tsx:216`
- W1-SET-240 Keycaps listening. png `W1-SET-240-<theme>.png`; drive: none; settings keybindings, click Search's keycaps. `settings/ChordKeys.tsx:90`
- W1-SET-241 Theme previewed under the pointer. png `W1-SET-241-<theme>.png`; drive: none; settings appearance, hover a theme card not picked. `settings/ThemePicker.tsx:138`
- W1-SET-242 Narrowest window; window.ts sets no minimum. png `W1-SET-242-<theme>.png`; drive: width 480; settings general. `settings/SettingsPage.tsx:100`
- W1-SET-243 Reading: a skeleton row under "On <computer>". png none; agents.read answers before a capture, needs a held read. `settings/agents.tsx:300`
- W1-SET-244 Copy line sign-in. png `W1-SET-244-<theme>.png`; drive: none (needs OpenCode on PATH); settings agents, click Sign in on OpenCode. `components/agents/SignInFlowView.tsx:29`
- W1-SET-245 Tool servers empty. png `W1-SET-245-<theme>.png`; drive: none; settings agents, click Tool servers. `settings/agentKinds.tsx:428`
- W1-SET-246 Add a tool server form. png `W1-SET-246-<theme>.png`; drive: none; settings agents, click Tool servers, click Add a tool server. `settings/AddServerRows.tsx:147`
- W1-SET-247 Add skill results. png `W1-SET-247-<theme>.png`; drive: none (needs network); settings agents, click Skills, Add skill, type "pdf". `settings/agentKinds.tsx:665`
- W1-SET-248 Skill rows. png `W1-SET-248-<theme>.png`; drive: +op skills.add {target:{placeId:here},skill:<skills.sh id>,agents:[claude]}, needs network; settings agents, click Skills. `settings/agentKinds.tsx:432`
- W1-SET-249 Plugins empty. png `W1-SET-249-<theme>.png`; drive: none; settings agents, click Plugins. `settings/agentKinds.tsx:428`
- W1-SET-250 Remove asks first. png `W1-SET-250-<theme>.png`; drive: +op servers.add as for the server's page; settings agents, Tool servers, x, click Remove; dialog insides are in `parity/wave-2`. `components/agents/agentsParts.tsx:47`
- W1-SET-251 Model select open. png `W1-SET-251-<theme>.png`; drive: none; settings agents, click Claude Code, click Model. `settings/agentPage.tsx:264`
- W1-SET-252 Many projects. png `W1-SET-252-<theme>.png`; drive: +project p01 to p30; settings projects. `settings/projects.tsx:79`
- W1-SET-253 Long project name. png `W1-SET-253-<theme>.png`; drive: +project <80-character name>; settings projects, click it. `settings/projects.tsx:330`
- W1-SET-254 Removable project. png `W1-SET-254-<theme>.png`; drive: +project other; settings projects, click other, click Remove. `settings/projects.tsx:134`
- W1-SET-255 No projects, or projects refused. png none; the fixture's bench always stands, a refusal needs projects.list to fail. `settings/projects.tsx:72`
- W1-SET-256 Nothing paired (the fixture holds thread tokens only). png `W1-SET-256-<theme>.png`; drive: none; settings devices. `settings/devices.tsx:81`
- W1-SET-257 Devices refused. png none; needs the page served on a ticket socket. `settings/devices.tsx:76`
- W1-SET-258 Account signed out. png `W1-SET-258-<theme>.png`; drive: none; settings account. `settings/account.tsx:24`
- W1-SET-259 Computer away, every act held with the away word. png none; needs a second computer that stops answering. `settings/agents.tsx:320`
- W1-SET-260 A recipe named in 64 characters, list and page. png `W1-SET-260-<theme>.png`; drive: +op recipes.save {"name":"A recipe for the build machines that run the nightly release job","file":{"name":"A recipe for the build machines that run the nightly release job"}}; settings recipes, click it. `settings/recipes.tsx:153`
- W1-SET-261 A followed recipe (name read-only, "Followed by"): png none; needs a computer that follows it. `settings/recipes.tsx:183`
- W1-SET-262 Recipes or places refused: png none; needs the host to refuse recipes.list or places.list. `settings/recipes.tsx:230`
- W1-SET-263 Activity loading: skeleton tiles, three skeleton rows, "Reading this computer's processes." png none; one frame. `settings/activity/page.tsx:297`
- W1-SET-264 Activity away: the away sentences in place of the tables. png none; needs this computer's daemon down. `settings/activity/page.tsx:175`
- W1-SET-265 Activity, nothing matches. png `W1-SET-265-<theme>.png`; drive: settings activity, click "Filter by name, port or thread", type "zzzz". `settings/activity/tables.tsx:519`
- W1-SET-266 Activity Processes, Every process on. png `W1-SET-266-<theme>.png`; drive: settings activity, click Processes, click Every process. `settings/activity/page.tsx:197`
- W1-SET-267 Activity Kinds, then Ports. png `W1-SET-267-<theme>.png`; drive: +working long; settings activity, click Kinds; click Ports. `settings/activity/page.tsx:199`
- W1-SET-268 Activity row hovered, its menu button shown. png `W1-SET-268-<theme>.png`; drive: +working long; settings activity, click long, hover its agent row. `settings/activity/tables.tsx:97`
- W1-SET-269 Computers at 760 px: Cores and Memory gone. png `W1-SET-269-<theme>.png`; drive: width 760; settings computers. `settings/grid.tsx:21`
- W1-SET-270 A limit off its default, reset arrow beside it. png `W1-SET-270-<theme>.png`; drive: +op places.set {"placeId":"here","threads":2}; settings computers, click this computer. `settings/computerSettings.tsx:227`
- W1-SET-271 Update, Sign in, offline, blocked state cells: png none; need another computer behind, unsigned or away. `settings/computers.tsx:292`
- W1-SET-272 Image build box: png none; needs a build here waiting on a sign-in or ended short (init.start, init.build). `settings/ImageCard.tsx:244`
- W1-SET-273 Send the message reads ⌘ Enter once Send with is on it. png `W1-SET-273-<theme>.png`; drive: +pref sendWith=mod-enter; settings keybindings. `settings/keybindings.ts:77`
- W1-SET-274 A model off the picker. png `W1-SET-274-<theme>.png`; drive: none; settings agents, click Claude Code, click a model's Show switch. `settings/agentPage.tsx:207`
- W1-SET-275 An added model id with its x. png `W1-SET-275-<theme>.png`; drive: none; settings agents, click Claude Code, type "my-model" in Add a model by its id, key Enter. `settings/agentPage.tsx:202`
- W1-SET-276 Devices reading: one skeleton row. png none: devices.list answers before a capture. `settings/devices.tsx:80`
- W1-SET-277 Project with its own default agent: "Set here for this project." and its reset arrow. png `W1-SET-277-<theme>.png`; drive: none; settings projects, click bench, click Default agent, click Claude Code. `settings/projects.tsx:201`
- W1-SET-278 A recipe open before recipes.options answers: head, note, Name and Delete only. png none; one frame. `settings/recipes.tsx:158`
- W1-SET-279 A recipe save or rename refused: the host's words under the head. png none; needs the host to refuse recipes.save. `settings/recipes.tsx:157`
- W1-SET-280 Setup act pressed during a setup or sync: "<name> is still being set up; press again once it is done." png none; needs another computer. `settings/add/AddComputerDialog.tsx:360`
- W1-SET-281 A subagent row stays a minute after it ends, then leaves. png none; needs a subagent the stub ends on a timer. `settings/activity/model.ts:59`
- W1-SET-282 A computer page the host does not list (yet) is empty; one being removed keeps its last copy until the dialog closes. png none. `settings/SettingsPage.tsx:79`

### Actions and keys

- W1-SET-283 A list row opens on click, Enter or Space; its own buttons do not. `settings/grid.tsx:83`
- W1-SET-284 Right click a computer or cloud row: the native menu of icons it may show as. `settings/computers.tsx:225`
- W1-SET-285 Picking a thread (palette, chord, notification) or New thread, key `mod+t` (`keybindingDefaults.ts:59`), closes Settings. `apps/web/src/protocol/store/useStore.ts:491`
- W1-SET-286 Under Settings the terminal, right panel, preview, file finder and Open in editor chords are ignored; Search and Toggle the sidebar still act. `shell/shellCommands.ts:345`
- W1-SET-287 Segmented controls, Mode tiles and theme cards are radio groups: Tab lands on the pick, arrow keys move and pick at once. `components/ui/segmented-control.tsx:33`
- W1-SET-288 Restore defaults stands on Appearance, Keybindings and Privacy alone, never on General or on a sub page. `settings/SettingsHeaderActions.tsx:21`
- W1-SET-289 What's new opens the newest release's page, else the releases list, in the browser; its hover says when that page was last read. `settings/about.tsx:116`
- W1-SET-290 A row that opens a page and holds a step (agent row, kind row) is a link: key `Enter` or `Space` on it opens; a press on its control acts alone. `settings/rows.tsx:214`
- W1-SET-291 A cloud row whose setup stands opens AddCloudDialog at that cloud, not the cloud's page. `settings/computers.tsx:264`
- W1-SET-292 An open recipe has no back and no crumb of its own; clicking Recipes again keeps it open; the list returns after Delete or another page. `settings/recipes.tsx:227`
- W1-SET-293 Sign in in a list row's state cell opens that computer's page (held on the page's own state line); Update there runs, dimmed while it runs. `settings/computers.tsx:302`
- W1-SET-294 ArrowUp, ArrowDown, Home, End walk the sidebar rows; ArrowDown from the field enters the list. `settings/SettingsSidebar.tsx:133`
- W1-SET-295 Field Escape clears its text; empty, on the sheet, it shuts the sheet. `settings/SettingsSidebar.tsx:116`
- W1-SET-296 Trail row scrolls its card to the top, smooth unless reduced motion. `settings/SettingsSidebar.tsx:225`
- W1-SET-297 Back row closes Settings. `settings/SettingsSidebar.tsx:301`
- W1-SET-298 Crumb back chevron goes to the group's page, but on a found skill's page to the add page with its query ("Back to <add page>"); the group crumb always to the group. `settings/SettingsCrumbs.tsx:44`
- W1-SET-299 Sheet result tap opens the group, shuts the sheet, scrolls to the row and flashes it. `settings/SettingsSidebar.tsx:275`
- W1-SET-300 Corner gear opens Settings, its chord on the tooltip. `sidebar/SidebarCorner.tsx:19`
- W1-SET-301 Palette row Settings, the drawn groups as its description. `components/palette/paletteItems.tsx:317`
- W1-SET-302 wsp://settings link opens that group; a name it lacks is an error notice. `shell/links.ts:35`
- W1-SET-303 The workspace sidebar and centre stay mounted, hidden, under Settings, so closing is instant. `shell/AppShell.tsx:144`
- W1-SET-304 Row reset arrow writes that one field back to its default. `settings/general.tsx:34`
- W1-SET-305 Restore defaults writes the group's one patch. `settings/SettingsHeaderActions.tsx:29`
- W1-SET-306 Start at login writes through the shell; while off, its reset arrow turns it on. `settings/general.tsx:69`
- W1-SET-307 Update arrow copies the vendor's update line and toasts it; wsp never updates the binary. `settings/agents.tsx:210`
- W1-SET-308 Add search asks 250 ms after the last key, key `Enter` at once. `settings/agentKinds.tsx:644`
- W1-SET-309 Kind search filters by name, path, agent and file; "Nothing matches" line. `settings/agentKinds.tsx:397`
- W1-SET-310 Refresh reads the computer again and asks every server icon again. `settings/agentKinds.tsx:400`
- W1-SET-311 Default agent pick writes defaultAgent; its reset arrow writes null. `settings/agents.tsx:181`
- W1-SET-312 Only an installed agent's row opens its page. `settings/agents.tsx:325`
- W1-SET-313 Kind row opens its item page, named in the crumb; a found row's back returns to the search with its query. `settings/agentKinds.tsx:675`
- W1-SET-314 Sign in turns to Cancel while its run goes; key `Enter` in the key field saves. `components/agents/SignInFlowView.tsx:63`
- W1-SET-315 Switch turns a server, skill or plugin on or off where its kind keeps one per item. `settings/agentKinds.tsx:208`
- W1-SET-316 Fact copy glyph copies; its check stands 1.4 s. `settings/agentKinds.tsx:66`
- W1-SET-317 Add a tool server: key `Enter` in any field submits; switching road clears pairs; x drops a pair. `settings/AddServerRows.tsx:128`
- W1-SET-318 Agent on switch writes agents.setup {on}; Change and Edit open the Program, Config folder, Launch arguments, Environment sheets. `settings/agentPage.tsx:425`
- W1-SET-319 How it runs reset arrows write null at once; a refusal is a toast. `settings/agentPage.tsx:449`
- W1-SET-320 Model, effort and access picks write agentDefaults; each row's reset clears its own. `settings/agentPage.tsx:264`
- W1-SET-321 Shown switch hides a model from the picker; x removes an added id; a bad id is refused under the field. `settings/agentPage.tsx:207`
- W1-SET-322 Add a project opens the Add a project sheet. `settings/projects.tsx:68`
- W1-SET-323 Project picks write projectDefaults; reset arrows clear them. `settings/projects.tsx:203`
- W1-SET-324 Remove project asks, then toasts the runtime's sentence and returns to Projects. `settings/projects.tsx:351`
- W1-SET-325 Revoke asks, then reads the devices again. `settings/devices.tsx:53`
- W1-SET-326 Key `Escape` in the sign-in terminal goes to the login, not to Settings. `components/agents/SignInTerminal.tsx:48`
- W1-SET-327 Shift-click in a recipe's Agents list ticks or clears every row from the last one clicked. `settings/add/PickList.tsx:102`
- W1-SET-328 Enter or leaving the field saves the Spend per day figure; a non-number puts the old one back. `settings/computerSettings.tsx:134`
- W1-SET-329 Enter or leaving the field renames a recipe no computer follows (save under the new name, remove the old). `settings/recipes.tsx:183`
- W1-SET-330 Enter in a sign-in code field sends the code and clears the field. `settings/recipe/SignInCode.tsx:36`
- W1-SET-331 Show setup log toggles the log; a step with output toggles its lines; a failed step starts open. `settings/computers.tsx:617`
- W1-SET-332 Setup step acts: Retry, Skip, Copy keys, Sign in on <computer>; a refusal stands under that row. `settings/add/AddComputerDialog.tsx:421`
- W1-SET-333 Image press: Copy your image here, Copy anyway, Try again, New thread here; a held one says why under the list. `settings/ImageCard.tsx:176`
- W1-SET-334 Build box: Cancel asks (stop or keep building), Close, Change key (opens the key field); no Start over on a computer page. `settings/ImageBuild.tsx:39`
- W1-SET-335 Change on Name or SSH login opens FieldSheet. `settings/computerSettings.tsx:295`
- W1-SET-336 Dial while away; where this wsp cannot dial, no button and "This wsp cannot dial a computer from here."; disabled only while dialling. `settings/computers.tsx:386`
- W1-SET-337 A recipe's icon picked in its head row's select. `settings/recipes.tsx:153`
- W1-SET-338 Activity: click a head to fold; click a column word to sort; pick computer, kind, thread; a filter opens every group. `settings/activity/tables.tsx:127`
- W1-SET-339 Activity container menu: End (stops it, keeps its data), Copy name, Copy container ID. `settings/activity/actions.ts:151`
- W1-SET-340 End on a daemon too old for tree kills: after the wait one notice names them, with Force end. `settings/activity/live.tsx:208`
- W1-SET-341 Open on a port opens a browser tab at it in the thread's workspace. `settings/activity/live.tsx:262`
- W1-SET-342 Check for updates is held while a check runs and absent while checks are off or no release answer stands. `settings/about.tsx:86`
- W1-SET-343 Pressing a command's only default back clears its override; a lone modifier wipes the refusal; leaving the page stops listening. `settings/ChordKeys.tsx:62`
- W1-SET-344 The typed font field commits on key `Enter` or blur, trimmed; empty is Default. `settings/FontPicker.tsx:47`
- W1-SET-345 Doors to a page: the first run's privacy link opens Privacy; a project row's menu and the project switcher's gear open that project's page. `settings/openAt.ts:13`
- W1-SET-346 Kind search text lives per tab and computer; another tab or computer starts it empty. `settings/agents.tsx:338`
- W1-SET-347 Picking a glyph in Icon drops the project's image; Icon's reset arrow stands only while an image is worn. `settings/projects.tsx:317`
- W1-SET-348 A project page whose project is gone falls back to Projects; an agent page never falls back. `settings/settingsStore.ts:47`
- W1-SET-349 Agent on switch held while its write runs, a refusal a notice; a config folder saved says the agent's sign-in line as a done notice. `settings/agentPage.tsx:379`
- W1-SET-350 Sorting is always heaviest first: a column word picks the column, drawn in the foreground with a 12 px down arrow; no reverse order. `settings/activity/tables.tsx:126`
- W1-SET-351 Picking another computer in Activity starts afresh: sort, filter, picks, folds and tile lines reset. `settings/activity/page.tsx:110`
- W1-SET-352 A picked thread that leaves the list puts the Thread select back on All threads. `settings/activity/page.tsx:163`
- W1-SET-353 Shift with Space on a recipe's tick ranges like a shift-click. `settings/add/PickList.tsx:106`
- W1-SET-354 Clearing an MCP server, CLI or skill on a recipe drops its row once the save lands. `settings/add/PickLists.tsx:159`
- W1-SET-355 Name and SSH login sheets: Enter saves, Escape or Cancel closes; a refusal keeps the sheet open, said under the field. `settings/sheetParts.tsx:149`

### Host operations

- W1-SET-356 `ssh.include` with on, from the editor ssh switch. `settings/general.tsx:142`
- W1-SET-357 `release.check` forced, from Check for updates. `settings/about.tsx:86`
- W1-SET-358 `host.restart` from Restart host. `shell/update.ts:138`
- W1-SET-359 window gap `setTitleBar` at each theme change on a Linux window. `settings/theme.ts:52`
- W1-SET-360 window gaps `getBundle` `quitAndOpen` `discardUpdate` `hosts`, the Version step on the host's own computer. `shell/update.ts:91`
- W1-SET-361 `agents.read` the computer's agents, servers, skills, plugins; held 5 min per computer. `components/agents/useAgentsReport.ts:51`
- W1-SET-362 `events.subscribe` agents.changed reads the report again. `components/agents/useAgentsReport.ts:56`
- W1-SET-363 `harnesses.list` each agent's models and efforts on a computer or a project's home. `protocol/store/hooks.ts:97`
- W1-SET-364 `preferences.set` default agent, agent defaults, model picker, project defaults, project look. `settings/agents.tsx:181`
- W1-SET-365 `agents.setup` on, program, config folder, launch arguments, variables. `settings/agentPage.tsx:44`
- W1-SET-366 `usage.accounts` plan and address on an agent page's head. `settings/agentPage.tsx:326`
- W1-SET-367 `broker.status` only while a cloud is set up. `components/agents/useBrokerStatus.ts:21`
- W1-SET-368 `agents.signIn` (or `servers.signIn` for a server) starts a watched sign-in. `components/agents/useAgentActs.ts:181`
- W1-SET-369 `agents.cloudSignIn` the login clouds borrow, while a cloud is set up. `components/agents/useAgentActs.ts:181`
- W1-SET-370 `agents.signInStop` on Cancel. `components/agents/useAgentActs.ts:119`
- W1-SET-371 `agents.signInCode` a code pasted back. `components/agents/useAgentActs.ts:216`
- W1-SET-372 `agents.key` a pasted token or key. `components/agents/useAgentActs.ts:229`
- W1-SET-373 `servers.add` from the form. `components/agents/useServerActs.ts:36`
- W1-SET-374 `servers.remove` once per agent of the entry. `components/agents/useServerActs.ts:37`
- W1-SET-375 `servers.toggle` once per agent of the entry. `components/agents/useServerActs.ts:38`
- W1-SET-376 `servers.tools` state and tools, on show, Check, Reconnect, refresh. `components/agents/useServerTools.ts:36`
- W1-SET-377 `servers.icon` a remote server's icon as a data URL, unless icons are off. `components/agents/useServerIcon.ts:40`
- W1-SET-378 `skills.search` skills.sh search. `components/agents/useSkillActs.ts:70`
- W1-SET-379 `skills.get` a found skill's SKILL.md. `components/agents/useSkillActs.ts:59`
- W1-SET-380 `skills.preview` an installed skill's SKILL.md. `components/agents/useSkillActs.ts:49`
- W1-SET-381 `skills.add` install for the picked agents. `components/agents/useSkillActs.ts:84`
- W1-SET-382 `skills.remove`. `components/agents/useSkillActs.ts:83`
- W1-SET-383 `skills.toggle`. `components/agents/useSkillActs.ts:82`
- W1-SET-384 `plugins.toggle`. `components/agents/usePluginActs.ts:21`
- W1-SET-385 `projects.defaults` what each project's new threads resolve to. `settings/projects.tsx:165`
- W1-SET-386 `projects.remove`. `settings/projects.tsx:118`
- W1-SET-387 `projects.icon` set or clear the project's image. `settings/projects.tsx:307`
- W1-SET-388 `devices.list`. `settings/settingsReads.ts:84`
- W1-SET-389 `devices.revoke`. `settings/devices.tsx:41`
- W1-SET-390 `account.get`. `settings/settingsReads.ts:74`
- W1-SET-391 `daemon.open` the link the in-place sign-in terminal attaches over. `components/agents/SignInTerminal.tsx:29`
- W1-SET-392 `window` native file chooser for the project image. `projects/ImageDialog.tsx:122`
- W1-SET-393 `places.readings` the live band's last 15 minutes. `machine/computerLive.ts:100`
- W1-SET-394 `sys.subscribe` by placeId, the live band's new samples. `machine/computerLive.ts:83`
- W1-SET-395 `places.list` the rows, pending adds and the refusal. `protocol/store/useStore.ts:305`
- W1-SET-396 `places.update` behind "Update wsp on <name>". `protocol/store/useStore.ts:880`
- W1-SET-397 `places.dial` Dial while away. `protocol/store/useStore.ts:872`
- W1-SET-398 `places.setupLog` the open steps' output, every 3 s while one runs. `settings/add/AddComputerDialog.tsx:315`
- W1-SET-399 `places.setup` Retry, Copy keys or a sign-in way on one step. `settings/add/addFlow.ts:364`
- W1-SET-400 `places.skip` Skip and Skip for now. `settings/add/addFlow.ts:341`
- W1-SET-401 `agents.read` the agents on a computer, for Sign-ins and the setup rows. `components/agents/useAgentsReport.ts:51`
- W1-SET-402 `agents.signIn` Sign in on a computer. `components/agents/useAgentActs.ts:181`
- W1-SET-403 `init.get` which clouds are offered, so whether Clouds and the Image row show. `settings/settingsReads.ts:44`
- W1-SET-404 `image.get` the image record and its copies. `settings/settingsReads.ts:102`
- W1-SET-405 `image.build` Copy your image here; force for Copy anyway. `settings/ImageCard.tsx:116`
- W1-SET-406 `init.cancel` stopping a build. `settings/ImageBuild.tsx:37`
- W1-SET-407 `init.retry` Retry on a build sign-in. `settings/ImageBuild.tsx:32`
- W1-SET-408 `init.signInCode` a code typed under a build sign-in. `settings/ImageBuild.tsx:32`
- W1-SET-409 `recipes.list` opening Recipes or the Recipe select, and after a change. `settings/recipesStore.ts:23`
- W1-SET-410 `recipes.options` once a recipe opens. `settings/recipes.tsx:223`
- W1-SET-411 `recipes.save` each tick and a rename. `settings/recipes.tsx:110`
- W1-SET-412 `recipes.remove` Delete, and the old file after a rename. `settings/recipes.tsx:133`
- W1-SET-413 `preferences.set` a computer's icon (computerLook) and a recipe's (recipeLook). `settings/computers.tsx:225`
- W1-SET-414 `daemon.open` the computer's daemon link while Activity shows it. `terminal/daemon-link.ts:219`
- W1-SET-415 `daemon.send` proc.kill, TERM in tree mode with start time and identity (KILL on Force end); container.stop. `machine/procs.ts:116`
- W1-SET-416 `sys.subscribe` by workspaceId, memory in use for the wsp tile. `machine/hostLive.ts:53`
- W1-SET-417 `appProcs.unwatch` when Activity closes. `machine/appProcs.ts:71`
- W1-SET-418 `sessions.interrupt` Stop turn. `settings/activity/live.tsx:283`
- W1-SET-419 `slates.get` a slated thread's title. `slate/store.ts:158`
- W1-SET-420 gap `getWebContentsId` (window, webview) names a browser tab's processes; the host client has no such call. `settings/activity/live.tsx:135`
- W1-SET-421 window opens a sign-in page in the person's browser. `settings/add/AddComputerDialog.tsx:285`
- W1-SET-422 Every read made on open asks nothing inside 60 s of the last (SETTINGS_HOLD_MS); `release.check` is asked at each General open. `settings/settingsReads.ts:25`
- W1-SET-423 window gaps `bundleHover` `updatesInPlace`: Get's hover, and Restart to update in place of Quit and open. `shell/update.ts:90`
- W1-SET-424 pushed `agents.signIn` events: each run's step (running, waiting with page, code, paste; signed-in; failed with its words) to the socket that began it. `components/agents/useAgentActs.ts:49`
- W1-SET-425 `window` clipboard write: update arrow, fact copy glyph, copy rows, device code; a refusal is the notice "Not copied: <said>". `settings/sheetParts.tsx:51`
- W1-SET-426 window a build sign-in's Open link opens its page in the person's browser. `settings/recipe/BuildRows.tsx:150`

### Design

- W1-SET-427 Type sample: no shared piece; it needs the thread view's markdown with its code block head; a gpui-kit TextView::markdown stand-in is a deliberate difference.
- W1-SET-428 RefusalSlot, none yet: a two-line 36 px slot kept empty under Add server, Models, a sign-in key and the projects list, so a refusal moves nothing.
- W1-SET-429 State cell marks: `step_mark` lacks needs you, pending and offline; `StatusMark` is the thread's 12 px mark and does not fit.
- W1-SET-430 Recipe pick rows: `PickRow` lacks the mono tag, agent marks and the right slot; the keys pair is `Choice` with its note.
- W1-SET-431 Recipe delete confirm, Activity ask, Name and SSH sheets: 440 px, a bottom sheet under 640 px; `Dialog` is the 560 px step shell, so none yet.
- W1-SET-432 Do not use gpui-kit's setting::Settings page; compose Card and Row.
- W1-SET-433 Trail and scroll to a card: GPUI ScrollHandle from Zed's crate; the sub list's open: GPUI with_animation.
- W1-SET-434 Acts: Button outline xs, add for Add buttons, primary for Add server; sign-in and paste lines: CopyRow.
- W1-SET-435 Select popups are glass: solid stand-ins where glass is off. Toasts: gpui-kit toasts stand in for notices.
- W1-SET-436 None yet: agent brand marks, the in-place terminal, a plain-word tooltip (shortcut_tooltip carries keycaps).
- W1-SET-437 Setup log: `StepRow` lacks the time, the turning chevron and the warned mark.
- W1-SET-438 Computer and recipe head rows (`HeadRow`: glyph frame, name, mono mark, note, slot): none yet; `glyph_frame` in a line of its own.
- W1-SET-439 The reset arrow beside an off-default title, with its tooltip: `Row` has none; none yet.
- W1-SET-440 Switches: gpui-kit `Switch`; Activity tabs: none yet, gpui-kit `TabBar` restyled to the segmented look.
- W1-SET-441 Skeletons: gpui-kit `Skeleton` restyled to the 2 s sweep; the Activity tree: Zed's `div` list, no gpui-kit `Tree`.
- W1-SET-442 Activity's error notices: gpui-kit toasts stand in for them.
- W1-SET-443 Native row menus: gpui-kit `native_menu`.
- W1-SET-444 controls.rs Button has no ghost xs (Restore defaults, Reset chord).
- W1-SET-445 controls.rs Button lacks the danger hover (Remove, Revoke, Uninstall: neutral at rest, red ink and 50% red edge under the pointer) and a busy spinner.
- W1-SET-446 Facts on one line stand apart by a space and 8 px, never a dot: device rows, project rows, the agent head line. `components/ui/spaced.tsx:6`
- W1-SET-447 Cloud limit band: none yet; plain divs for the label, figure and 4 px bar.

## Usage

### Elements

- W1-USE-040 Identity: 32 px glyph frame, 12 px gap, title 15 px medium, line 13 px muted, both cut; pools, accounts, split rows. `settings/usage.tsx:389`
- W1-USE-041 Split glyphs: agent mark, computer, project; Source terminal or speech bubbles; else dashed circle, monitor, user, box. `settings/usage.tsx:454`
- W1-USE-042 Tabs carry chart-line and gauge glyphs, 16 px, 8 px gap, 14 px words; refresh tooltip "Read usage and limits again" below. `settings/usage.tsx:44`
- W1-USE-043 Dither: 1.4 px cells on 2 px; lead density 0.55, alpha 0.5; rest 0.32, alpha 0.45; phase +5 a rank; thins toward the baseline. `settings/usageChart.tsx:51`
- W1-USE-044 Under 640 px: totals 2 columns, no hairlines; table drops Cache hit, Turns (1fr 72 88); windows stack; controls wrap. `settings/usage.tsx:510`
- W1-USE-045 Limit band: 4 columns (2 under 768 px), 4 px bars, warning from 90%, danger at the limit; "Starts count over..." under. `settings/LimitBand.tsx:61`
- W1-USE-046 Thread row status: "At limit", gauge, warning ink, under Needs you; once armed an hourglass and "resets in 3 h". `components/status/kinds/limited.ts:8`
- W1-USE-047 Totals notes: "in wsp threads only" (or "and outside wsp"), "at list price", "in threads wsp ran", "$0.00 saved"; 2 to 4 cells. `settings/usage.tsx:504`
- W1-USE-048 Chart y-axis: 12 px mono muted, gutter as wide as the widest figure, 12 px off the plot; figures roll in from zeros. `settings/usageChart.tsx:207`
- W1-USE-049 Chart gridlines: 4 dashed (2 on, 4 off) in border ink, a solid baseline 6 px off the bottom. `settings/usageChart.tsx:224`
- W1-USE-050 Chart lines: 1.75 px for the lead (most tokens), 1.4 px for the rest, round joins; lead drawn last. `settings/usageChart.tsx:238`
- W1-USE-051 Chart legend: 13.5 px muted, 14x2 swatch, 12 px agent mark; "All agents together" where the answer has no lines. `settings/usage.tsx:735`
- W1-USE-052 Chart x ticks: weekdays (7 days), every 6th hour (Today), every 5th day "13 Oct" (30 days); first left, last right. `settings/usageChart.tsx:276`
- W1-USE-053 Table head row 13 px muted, "Agent and model" where models nest; columns 1fr 104 96 80 120, gap 24. `settings/usage.tsx:763`
- W1-USE-054 Table share bar: 4 px high, max 256 px, foreground at 8% under, the line's ink over. `settings/usage.tsx:566`
- W1-USE-055 Table model rows: 14 px at fg/80, inset 44 px, 10 px tall padding; "not priced" muted where no price. `settings/usage.tsx:555`
- W1-USE-056 Mix bar: Fresh in primary, Cache write primary/50, Cached fg/20, Out fg/80, 2 px gaps, 2 px floor; 8 px legend squares. `settings/usage.tsx:531`
- W1-USE-057 Pool head row: burn "Using 1.2k tokens a minute across 2 threads", "checked 4 min ago", 12.5 px muted. `settings/usage.tsx:293`
- W1-USE-058 Reset line: "Resets", "2 banked, first expires in 3 d", Use reset outline xs; dialog Cancel and Use reset. `settings/usage.tsx:249`
- W1-USE-059 Verdicts: "Limit reached", "Runs out 14:30", "Ahead of pace", warning ink; "Under pace", "As of 14:02" muted; "On pace" fg/80. `settings/usage.tsx:163`
- W1-USE-060 Segment: 20 px high, radius 5, fg at 9% under; fill fg/55, warning from 90%, fg/20 stale; pace tick 2 px at fg/80. `settings/usage.tsx:207`
- W1-USE-061 Composer drawer row: the agent's resume time on the minute clock. `components/chat/ComposerDrawer.tsx:74`
- W1-USE-062 Turn footer: "limit reached" in muted ink where a usage limit stopped the turn. `components/chat/ChatView.tsx:599`
- W1-USE-063 Agent page: the plan word read off usage.accounts where the agent's own status names none. `settings/agentPage.tsx:326`
- W1-USE-064 Slates: the line chart piece draws with this chart; the usage source binds an account's windows. `slate/pieces/chart.tsx:94`
- W1-USE-065 Activity page shares data-locked and this page's stat cells and inks. `settings/activity/page.tsx:30`
- W1-USE-066 Computers list cloud row note: hours of credit left and the day's spend against its cap. `settings/computers.tsx:488`
- W1-USE-067 Pool title: the one account's label, else "2 Claude Code accounts"; card head is the agent's name; computers as "a, b and c". `settings/usage.tsx:283`
- W1-USE-068 Windows 5-hour, Week, "Week, Opus", "Week, Sonnet", Month in that order; overage never draws; segments by soonest reset. `settings/usage.tsx:137`
- W1-USE-069 Big figure: 100 less the accounts' mean use (each capped at 100), muted when all stale; pace tick runs 4 px past both edges. `settings/usage.tsx:186`
- W1-USE-070 Verdict: paced once 2% of the window is gone; ahead over 1.05x, under below 0.95x; "Runs out" day and time past 24 h. `settings/usage.tsx:170`
- W1-USE-071 Y top: smallest of 1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10 times a power of ten at or over the peak; 5 figures from 0. `settings/usageChart.tsx:68`
- W1-USE-072 Hover columns: one a step, first and last half wide; tooltip date "14:00" on Today, else "Mon 13 Oct". `settings/usageChart.tsx:251`
- W1-USE-073 No plan limit with only no-limit agents: head and line, no box; an own-login row drops its computers line. `settings/usage.tsx:361`
- W1-USE-074 Live band inks: CPU sky 500, memory violet 500, 1.5 px line over a 40% fade; memory in warning ink from 90% full. `settings/LiveBand.tsx:32`
- W1-USE-075 Table without a Turns column where no row has turns (1fr 104 96 120); share bar at fg/45 where the answer has no lines. `settings/usage.tsx:548`

### States

- W1-USE-076 Loading skeletons. png none: the fixture answers in ms; needs a held reply. `settings/usage.tsx:591`
- W1-USE-077 Read refused. png none: the host refuses only a socket let in on a ticket. `settings/usage.tsx:688`
- W1-USE-078 Nothing used in this range. png none: every fixture turn ran today. `settings/usage.tsx:724`
- W1-USE-079 Use reset dialog. png none: needs resets banked on a signed-in Claude. `settings/usage.tsx:261`
- W1-USE-080 Limits loading: a 64 px head bar over one card, identity bars, two window rows. png none: needs a held usage.accounts. `settings/usage.tsx:649`
- W1-USE-081 Limits refused: "The plan limits could not be read: <said>" in a card, only while no answer is held. png none: as W1-USE-077. `settings/usage.tsx:335`
- W1-USE-082 Limit bar armed: "Resumes at the reset", Resume held. png none: needs a turn a usage limit stopped. `components/chat/bars/UsageBar.tsx:22`
- W1-USE-083 Limit bar, no reset named: title and the "did not say when" sentence, no buttons. png none: as W1-USE-082. `components/chat/bars/UsageBar.tsx:47`
- W1-USE-084 30 days, ticks every 5th day. png `W1-USE-084-<theme>.png`; drive: none; usage, click 30 days. `settings/usage.tsx:421`
- W1-USE-085 Width 980: the page fills, nothing folds. png `W1-USE-085-<theme>.png`; drive: width 980; usage. `settings/SettingsPage.tsx:100`
- W1-USE-086 Refreshing: the glyph spins, the button is disabled. png none: the fixture answers in ms. `settings/usage.tsx:65`
- W1-USE-087 Logs line and "and outside wsp". png none: needs the daemon reading the agents' logs; the fixture's daemon is missing. `settings/usage.tsx:786`
- W1-USE-088 No plan limit card with "Not read yet" and keyed rows. png none: needs signed-in accounts. `settings/usage.tsx:320`
- W1-USE-089 Reset dialog under 640 px: a full-width sheet at the foot, Use reset over Cancel. png none: as W1-USE-079. `components/ui/alert-dialog.tsx:68`
- W1-USE-090 Spending: both buttons "Using reset", disabled; 0 banked reads "none banked", no button. png none: as W1-USE-079. `settings/usage.tsx:258`

### Actions and keys

- W1-USE-091 Limit bar: Resume at reset arms the turn and folds; Cancel folds, or disarms when armed; the back button folds. `components/chat/bars/UsageBar.tsx:49`
- W1-USE-092 Range and split start at 7 days and Agent on each visit; a tab switch keeps them and replays the page from zeros. `settings/usage.tsx:85`
- W1-USE-093 Hover a pool segment: account label, "92% used, resets in 3 h", opens at once. `settings/usage.tsx:206`
- W1-USE-094 Open Usage from the sidebar corner. `sidebar/SidebarCorner.tsx:28`
- W1-USE-095 A spent reset puts the account row the host sent back in place at once. `settings/usage.tsx:239`
- W1-USE-096 Settings search finds nothing on Usage and the sidebar lists no sections under it: its one card has no head, no items. `settings/usage.tsx:75`
- W1-USE-097 Tabs, range, split are radio groups: arrows move the pick, Tab lands on it; hover inks a segment foreground. `components/ui/segmented-control.tsx:49`

### Host operations

- W1-USE-098 `usage.accounts` without fresh, for the agent page's plan word. `settings/agentPage.tsx:326`
- W1-USE-099 `usage.accounts` once while a slate binds usage; `usage.account` pushes move its rows. `protocol/store/useStore.ts:444`
- W1-USE-100 event `usage.alert`: a plan alert notice where the plan alerts preference is on. `notices/hostNotices.ts:342`
- W1-USE-101 The Limits tab ignores `usage.account` pushes; only a reread (refresh, 60 s, focus) moves it. `settings/usage.tsx:93`
- W1-USE-102 `usage.used`, `usage.accounts` are shared reads: an ask with the same words while one is out joins it. `protocol/client.ts:243`

### Design

- W1-USE-103 Inks: a project's own hue, else split inks skipping picked ones, no project muted; an agent's 2nd account takes its next shade. `settings/usage.tsx:713`
- W1-USE-104 Identity's 15 px title is not ui `Row`'s 14 px: `glyph_frame` plus own text, or a size on `Row`.
- W1-USE-105 Split inks are Tailwind sky, violet, amber, pink, teal 500 and agent inks come off the catalog; none is in the token table. `settings/usage.tsx:433`
- W1-USE-106 gpui-kit `LineChart` is one series with a gliding cursor tooltip and haloed dots; `AreaChart` fills gradients: neither matches.
- W1-USE-107 gpui-kit `Skeleton` pulses opacity; today's sweeps a highlight band over a border-ink bar.
- W1-USE-108 Skeleton highlight: white at 64% on light, 4% on dark, a 120 degree band from 40% to 60%. `components/ui/skeleton.tsx:12`
- W1-USE-109 Shades by rank: the mark ink, then 50% of it with text, 55% with page, 75% with text; no catalog ink: muted, fg/80, fg/40. `settings/usage.tsx:437`

## Sidebar

### Elements

- W1-SBR-067 Switcher menu: 32 px search, rows (glyph, name, check, 28 px gear), hairline, Add a project or computer. `sidebar/NounSwitcher.tsx:192`
- W1-SBR-068 Tree: 13 px in, 1 px `--sidebar-rail`, elbow at 35 px, 18 px on a slim row; flat past two levels. `sidebar/rowGrammar.ts:56`
- W1-SBR-069 Rename box in the title slot, 18 px, input border, ring on focus, name selected. `sidebar/RowNameInput.tsx:51`
- W1-SBR-070 The list scrolls with no scrollbar drawn, ever (ScrollArea `hideScrollbars`); the head and foot stay put. `components/ui/sidebar.tsx:712`
- W1-SBR-071 Project glyph on every tile, switcher row and card: the project's picked icon in its picked hue (kept on hover and selected), or its own image. `projects/look.tsx:86`
- W1-SBR-072 Row inks: press fill `--sidebar-row-active`; glyphs `--sidebar-icon-color`, foreground on hover, press, selected; rest words `--sidebar-row-rest`. `components/ui/sidebar.tsx:812`
- W1-SBR-073 Search row: search glyph and "Search", no field look; tooltip "Search (<palette chord>)". `sidebar/SearchRow.tsx:22`
- W1-SBR-074 Switcher option rows 36 px, the keyed row in `--accent`; check on the pick; gear tooltip "<name> settings". `sidebar/NounSwitcher.tsx:217`
- W1-SBR-075 Picked project head: a plus at the right on hover ("New thread"), its room kept at rest so nothing moves. `sidebar/ProjectSwitcher.tsx:66`
- W1-SBR-076 Host-asleep line under the switchers, 12 px prose ink. `sidebar/WorkspaceSidebar.tsx:711`
- W1-SBR-077 Crab: 16x14 canvas at the device ratio, tinted `--status-working`. `components/status/Crab.tsx:148`
- W1-SBR-078 Stop armed: the square gives way to "Stop" in `--error-foreground`, no fill; tooltip "Press again to stop". `sidebar/StopAct.tsx:84`
- W1-SBR-079 Snoozed root at the list's foot while its threads run: alarm-clock and "N working" in the row's ink. `sidebar/ThreadTile.tsx:184`
- W1-SBR-080 Agent refused on that computer: the slot says "Refused", the reason on its hover title. `sidebar/ThreadTile.tsx:423`
- W1-SBR-081 Needs you tile under a tree: row one names its opener, a corner-down-right mark over the project glyph. `sidebar/ThreadTile.tsx:124`
- W1-SBR-082 Tile for a copy with no thread: its name as muted title, no agent mark, Failed when its create failed. `sidebar/ThreadTile.tsx:494`
- W1-SBR-083 Send in flight: a tile reading Working with the crab, not a button. `sidebar/ThreadTile.tsx:527`
- W1-SBR-084 Copy being made: Starting with the crab in the slot; refused, Failed with Delete on its menu. `sidebar/ThreadTile.tsx:542`
- W1-SBR-085 Group head of one send to several models: copy glyph, title, copy count, chevron; its tiles show the model. `sidebar/WorkspaceSidebar.tsx:141`
- W1-SBR-086 Finished fold pages 20 rows at a time behind an "n more" row. `sidebar/LeadTree.tsx:124`
- W1-SBR-087 Subagent card: title, status row, what it was asked (3 lines), agent mark and model. `components/threads/SubagentCard.tsx:30`
- W1-SBR-088 Card status row: glyph or crab, word in the kind's ink, time at the right, reason under it, 3 lines. `components/status/StatusLine.tsx:21`
- W1-SBR-089 Drag: 2 px drop line in `--ring` between roots, empty heads stand, dragged tile at 50%, its tree hidden. `sidebar/WorkspaceSidebar.tsx:803`
- W1-SBR-090 Empty bodies: "No threads yet." 13 px centred, "New thread in X"; "Add a project" row; "Projects not read: <why>". `sidebar/WorkspaceSidebar.tsx:777`
- W1-SBR-091 Forwarded ports under the list: head, rows of localhost:port with the name and age, an X to stop. `sidebar/ForwardsList.tsx:32`
- W1-SBR-092 Setting up card: computer glyph, name, done/of, Failed or the needs-you mark, the step line, a 2 px bar. `sidebar/SettingUpSection.tsx:64`
- W1-SBR-093 Update card: mark, "wsp X is out" or "is ready", dismiss X, a line, Get or Restart (outline xs), What's new. `sidebar/UpdateCard.tsx:38`
- W1-SBR-094 Cloud card: cloud glyph, "Run threads on a cloud", dismiss, "Boat from $0.018/h.", Add a cloud. `sidebar/CloudCard.tsx:33`
- W1-SBR-095 Corner: 28 px ghost icon buttons; the Settings tooltip carries its chord in mono. `sidebar/SidebarCorner.tsx:17`
- W1-SBR-096 Slot tooltip: the kind's word plus the running time to the second ("Working 1m 5s"), or the age in muted ink. `components/status/ThreadStatus.tsx:71`
- W1-SBR-097 Time words: ages `now`, `5m`, `3h`, `2d`; slot running time `42s`, then whole minutes `17m`, `1h 5m`, `2d 3h`. `components/status/WorkingSince.tsx:8`
- W1-SBR-098 A resting thread's card status row reads Done with an uninked check and its age, the check a Finished fold row ends in. `components/status/StatusLine.tsx:18`
- W1-SBR-099 Sign-in callback port row: 48 px label "sign-in callback for <name>", mono localhost:port and age, not a link, X to stop. `sidebar/ForwardsList.tsx:40`
- W1-SBR-100 Card's computer line for a thread on a cloud machine: cloud, size, vCPU, memory, rate, cost so far; or napping, deleted in N min, released. `sidebar/tileCard.ts:80`
- W1-SBR-101 Card branch line on a fork: "<branch>, N ahead of <lead>", merged, not pushed or conflicts; no branch line on the lead's own branch. `sidebar/tileCheckout.ts:46`
- W1-SBR-102 Tooltips on tile controls, above them: the Settle act says "Settle", the fold control "Fold" or "Unfold". `sidebar/ThreadTile.tsx:410`
- W1-SBR-103 Setting up list stops at three cards (196 px) and scrolls; a cloud's image build is one more card in it. `sidebar/SettingUpSection.tsx:102`
- W1-SBR-104 The foot shows one offer card: the cloud card waits while the update card or Setting up stands, and until preferences are read. `sidebar/CloudCard.tsx:30`
- W1-SBR-105 Update card lines: "Downloading" held while it fetches; on Restart "Your N running threads carry on. Open terminals close.". `sidebar/UpdateCard.tsx:36`
- W1-SBR-106 Children order under a tile: live first, most pressing (asks, failed, working, waiting) then newest; Finished fold by newest end. `components/threads/leadTree.ts:93`

### States

- W1-SBR-107 No project. png `W1-SBR-107-<theme>.png`; drive: fresh host, `+op projects.remove` bench. `sidebar/WorkspaceSidebar.tsx:795`
- W1-SBR-108 Loading: until the first lists land the body draws nothing, no "No threads yet.", no Add a project row. png none: the local list lands first frame. `sidebar/WorkspaceSidebar.tsx:718`
- W1-SBR-109 Settled by time after settleAfter (15m to 1d, never), on the minute; pinned trees and the open thread stay. png none: needs 15 min quiet. `sidebar/threadTree.ts:230`
- W1-SBR-110 Project switcher open. png `W1-SBR-110-<theme>.png`; drive: click "All projects". `sidebar/NounSwitcher.tsx:192`
- W1-SBR-111 Stop armed. png `W1-SBR-111-<theme>.png`; drive: `+working short-1`; hover short-1, click "Stop thread". `sidebar/StopAct.tsx:78`
- W1-SBR-112 No foot card, 11th tile scrolls. png `W1-SBR-112-<theme>.png`; drive: `+pref cloudCardDismissed=true`, `+thread` x7. `sidebar/CloudCard.tsx:30`
- W1-SBR-113 Forwarded ports. png none: needs a thread that prints a localhost link. `sidebar/ForwardsList.tsx:29`
- W1-SBR-114 Setting up. png none: needs a second computer mid-setup. `sidebar/SettingUpSection.tsx:95`
- W1-SBR-115 Update card. png none: needs a release ahead of 0.3.8. `sidebar/UpdateCard.tsx:33`
- W1-SBR-116 Snoozed, working. png `W1-SBR-116-<theme>.png`; drive: right-click short-1, Snooze thread, In 1 hour; `+working short-1`. `sidebar/ThreadTile.tsx:421`
- W1-SBR-117 Pinned. png `W1-SBR-117-<theme>.png`; drive: right-click short-2, click "Pin thread". `sidebar/WorkspaceSidebar.tsx:737`
- W1-SBR-118 Host asleep. png none: needs a window on another computer whose host went quiet. `sidebar/WorkspaceSidebar.tsx:710`
- W1-SBR-119 Child threads, folded tree. png none: needs a thread opened by another thread's agent. `sidebar/WorkspaceSidebar.tsx:496`
- W1-SBR-120 Group of copies. png none: needs one send to several models. `sidebar/WorkspaceSidebar.tsx:437`
- W1-SBR-121 Open thread inside a folded tree: the folded tile draws selected. png none: needs a thread opened by another thread's agent. `sidebar/ThreadTile.tsx:294`
- W1-SBR-122 Ended subagent's page open: its row stands under the shut Finished head until left. png none: needs a subagent ending mid-turn. `sidebar/LeadTree.tsx:117`
- W1-SBR-123 Waiting, Limited, Resuming, Starting, Released, Expired, Not pushed slots. png none: needs a thread cap, a usage limit or a cloud. `components/status/kinds/index.ts:24`
- W1-SBR-124 Stale pick reads as All. png `W1-SBR-124-<theme>.png`; drive: `+project e`; click "All projects", click "e"; `+op projects.remove e`. `sidebar/picks.ts:42`
- W1-SBR-172 A computer named apart from its hostname, as a Mac is (the fixture's realHost label "zingzy's MacBook Pro"): each tile's first row reads "bench @ zingzy's MacBook Pro", never the hostname. png `W1-SBR-172-<theme>.png`; drive: the fixture's realHost; open the panes thread. `sidebar/ThreadTile.tsx:60`
- W1-SBR-173 Hover card on that computer's thread: its computer line reads the label. png `W1-SBR-173-<theme>.png`; drive: as W1-SBR-172; hover the panes thread's row. `sidebar/ThreadTile.tsx:83`

### Actions and keys

- W1-SBR-125 Alt+Shift+Up or Down moves a root; a live region says where. `sidebar/WorkspaceSidebar.tsx:653`
- W1-SBR-126 Switcher: Down opens, typing filters, Enter picks, Escape; projects drag. `sidebar/NounSwitcher.tsx:136`
- W1-SBR-127 A copy's first tile adds its verbs: Pause/Wake, Rebuild, Reconnect, New thread here, terminal, browser, Bring back, Export, Rename, Copy id, Delete. `actions/workspaceActions.ts:116`
- W1-SBR-128 A copy's Rename task (menu or palette) opens the box on its open or top thread's title, opening Settled and its section; no thread, the copy's name. `sidebar/WorkspaceSidebar.tsx:327`
- W1-SBR-129 New thread's project: the sidebar's pick, the open thread's, the last New thread page's, the first; `newThreadIn=ask` and 2+ projects opens the palette pick. `shell/NewThreadPicks.tsx:40`
- W1-SBR-130 A tile click also closes Settings and writes the window address (`#workspace/thread`), which a reload reopens. `protocol/store/useStore.ts:491`
- W1-SBR-131 Tile menu: Stop, Stop and continue in terminal, Settle, Restore, Rename, Undo rewind, Copy as Markdown, Pin, Snooze, Move up/down/top (alt+shift+arrows), Copy link, terminal, Keep, Delete copies, Forget, Delete. `actions/threadActions.ts:170`
- W1-SBR-132 Right-click the list's blank space: Settle all read; the Settled row adds Delete copies. `sidebar/WorkspaceSidebar.tsx:724`
- W1-SBR-133 Picked project head: plus opens New thread there; right-click: New thread, Resume, Project settings, Remove, folder verbs. `sidebar/WorkspaceSidebar.tsx:704`
- W1-SBR-134 Switcher gear opens that project's or computer's settings; the foot row opens Add a project or Add a computer. `sidebar/NounSwitcher.tsx:241`
- W1-SBR-135 Finished fold row folds locally, shut each draw; its menu: Settle n finished. `sidebar/LeadTree.tsx:106`
- W1-SBR-136 Subagent row opens its page; Stop subagent on hover in two presses, alone on its menu. `sidebar/SubagentRow.tsx:64`
- W1-SBR-137 Stop disarms on Escape, a mouse leaving the row, focus leaving it, or the window blurring. `sidebar/StopAct.tsx:70`
- W1-SBR-138 Rename in flight holds the field; a refusal leaves it open with a toast. `sidebar/WorkspaceSidebar.tsx:396`
- W1-SBR-139 Workspace walk: `workspace.next` on `mod+alt+arrowright` and `ctrl+tab`, `workspace.previous` on `mod+alt+arrowleft` and `ctrl+shift+tab` (`keybindingDefaults.ts:60`), most recent first under the overlay. `shell/shellCommands.ts:219`
- W1-SBR-140 `mod+b` is the terminal's while a focused terminal owns mod (not on macOS); `mod+shift+b` still toggles. `keybindings.ts:207`
- W1-SBR-141 Corner: Settings, key `mod+,` (`keybindingDefaults.ts:58`); Usage; Activity. `sidebar/SidebarCorner.tsx:19`
- W1-SBR-142 Cards: dismiss, Get, Restart or Quit and open, What's new, Add a cloud; a setup card opens Add a computer. `sidebar/UpdateCard.tsx:54`
- W1-SBR-143 Forwarded port row opens localhost in the browser; its X stops forwarding. `sidebar/ForwardsList.tsx:64`
- W1-SBR-144 Group head folds its copies, locally, open at first. `sidebar/WorkspaceSidebar.tsx:141`
- W1-SBR-145 A child tile's menu adds Settle or Restore thread, "Open the thread it replaced" or "Open its restart", and "Settle N finished". `sidebar/WorkspaceSidebar.tsx:489`
- W1-SBR-146 Drop on Settled while a thread of the tree works: error toast "A thread in it is still working", nothing moves. `sidebar/WorkspaceSidebar.tsx:602`
- W1-SBR-147 A dropped placement holds only while the tree's state and its root's turn stay; then the tree files by its own state again. `sidebar/threadTree.ts:344`
- W1-SBR-148 `mod+shift+e` does nothing while the tree works (no toast, unlike the menu's refusal). `shell/shellCommands.ts:272`
- W1-SBR-149 `mod+1` to `mod+9` also put the caret in that workspace's composer. `shell/shellCommands.ts:191`
- W1-SBR-150 A menu verb that fails toasts "<verb title>: <reason>"; Stop's leftover and Stop subagent's refusal toast in the host's words. `actions/contextMenu.ts:81`
- W1-SBR-151 Switcher keys wrap through All, the rows and the Add row; close returns focus to the head; the gear closes the menu first. `sidebar/NounSwitcher.tsx:140`
- W1-SBR-152 Forwarded ports head folds for the moment only: open again on each mount, never stored. `sidebar/ForwardsList.tsx:26`
- W1-SBR-153 More triggers of dialogs listed in `parity/wave-2`: Undo rewind (RewindDialog), Resume conversation picker, Add a computer, Add a cloud, setup page, palette's New thread pick. `actions/verbs.ts:82`

### Host operations

- W1-SBR-154 `sessions.head` in view, `sessions.history` on hover. `components/chat/warmTiles.ts:27`
- W1-SBR-155 `workspaces.checkout` per copy in view. `sidebar/tileCheckout.ts:76`
- W1-SBR-156 `sessions.history` with `whole: true` for Copy as Markdown. `protocol/client.ts:1145`
- W1-SBR-157 `projects.icons` for a project's own image on its glyph. `protocol/client.ts:1233`
- W1-SBR-158 `workspaces.rename` for a tile with no thread. `sidebar/WorkspaceSidebar.tsx:464`
- W1-SBR-159 `sessions.forget`, `projects.remove`, `folder.make`, `workspaces.rebuild` from the menus. `actions/verbs.ts:78`
- W1-SBR-160 `forwards.stop` for a forwarded port. `sidebar/ForwardsList.tsx:64`
- W1-SBR-161 `host.restart` from the update card. `shell/update.ts:138`
- W1-SBR-162 window `getBundle`, `quitAndOpen`, `discardUpdate`, `hosts` for the update card. `shell/update.ts:111`
- W1-SBR-163 The store's reads the sidebar draws: `workspaces.list`, `sessions.list`, `projects.list`, `places.list`, `status.subscribe`. `sidebar/WorkspaceSidebar.tsx:178`
- W1-SBR-164 window clipboard write for Copy thread link, Copy as Markdown, Continue in terminal, Copy computer id. `actions/clipboard.ts:10`
- W1-SBR-165 window `openExternal`, the page's window-open handler, for a forwarded port's link and What's new. `desktop/main.ts:454`

### Design

- W1-SBR-166 `Palette` lacks `--sidebar-border`, `--sidebar-rail`, `--sidebar-row-active`, `--sidebar-whisper` and the mixes `--top-row-meta`, `--sidebar-prose`, `--sidebar-row-rest`, `--sidebar-icon-color`: none yet.
- W1-SBR-167 `StatusMark` lacks Limited, Resuming, Stopped, Released, Expired, Not pushed, Starting, Snoozed, Finished and the slot's Refused word; a tile shows the time alone in the slot.
- W1-SBR-168 The "Add a project" row is a one-line sidebar row with a plus, not the keycap `Button` add: none yet.
- W1-SBR-169 The switcher menu sits on dropdown glass: solid stand-in where glass is off.
- W1-SBR-170 Where the app beats the locked design: Settled rows draw no `#N`; the cloud card says "Run threads on a cloud".
- W1-SBR-171 macOS dark with glass lifts the sidebar's muted inks to `--glass-ink` and lightens the status inks (`index.css:1630`); the solid stand-in keeps the theme's.

## Thread view

### Elements

- W1-THR-063 Plan card: title, actions menu, preview capped 416 px, Expand plan. `components/chat/ProposedPlanCard.tsx:115`
- W1-THR-064 Subagent fold: title, state and elapsed in mono, Open its page; lines on a rail. `components/chat/SubagentFoldRow.tsx:66`
- W1-THR-065 Foot: threads it opened, settled word (interrupted, failed, limit reached), paused line and Wake. `components/chat/ChatView.tsx:386`
- W1-THR-066 Quote button at a reply selection. `components/chat/AssistantSelectionToolbar.tsx:91`
- W1-THR-067 Machine line at the transcript's foot, a 24 px muted rule line: what the machine lacks for wsp, or memory near full. `components/chat/ChatView.tsx:397`
- W1-THR-068 "Updated the slate" rule line under the last reply of a turn that wrote the slate; a press opens the Slate tab. `slate/SlateUpdatedLine.tsx:11`
- W1-THR-069 Setup card standing as a new workspace's first content until its first message: "<name> is set up", every step done, centred. `components/chat/ChatView.tsx:457`
- W1-THR-070 Send in flight: the person's bubble at once, then a mono notice row "Starting <agent>" or "waiting for <computer> to connect; ...". `components/chat/useChatThread.ts:685`
- W1-THR-071 History read refused: a destructive error row "history unavailable: <reason>" where the transcript would be. `components/chat/useChatThread.ts:901`
- W1-THR-072 "Earlier, outside wsp" fold, shut, over a resumed conversation's earlier messages, in the Worked-for fold's look. `adapt/timeline-rows.ts:159`
- W1-THR-073 Waiting-on row: the open prompt of the thread this one waits behind, under "<title> asks; this thread waits on the answer". `adapt/timeline-rows.ts:256`
- W1-THR-074 Retry line in the working line's place: "Overloaded, trying again (2 of 10)" and its seconds, no act. `components/chat/ChatView.tsx:184`
- W1-THR-075 Output band: terminal ground, max 288 px, first 10 lines (a failure its last), "+N lines"/"+N earlier lines", "Fewer lines", "No output", Copy output. `components/chat/timeline/callBlock.tsx:153`
- W1-THR-076 Markdown file chip, 12 px pill: file glyph, name, a parent folder when two share a name, "L12:C3"; full path in a mono tooltip. `components/markdown/links.tsx:360`
- W1-THR-077 Reply images inline in a 480 px box, authored size kept; a local image as an inert file chip; else "Image unavailable: <alt>". `components/ChatMarkdown.tsx:437`
- W1-THR-078 Threads head, 32 px: tree glyph, "Threads", status counts while shut, "Settle N finished" while open, chevron. `tree/TreeRows.tsx:152`
- W1-THR-079 Threads body: live children first (second level on a rail), then Finished and Settled folds with counts, 20 a page, "N more". `tree/TreeRows.tsx:90`
- W1-THR-080 Child row, 36 px (48 with a second line): agent mark, title link, status slot; computer and note in 11 px; hover acts. `components/threads/ThreadRows.tsx:183`
- W1-THR-081 Subagent row: bot glyph, title link to its page, status slot; a card after 450 ms: title, status, what it was asked, model. `components/threads/ThreadRows.tsx:293`
- W1-THR-082 Spawn tiles: the threads and subagents a run of calls started, as child rows where those calls stand in the transcript. `components/threads/SpawnTiles.tsx:64`
- W1-THR-083 Subagent page bar in the composer's shell: mark and model, "Working for 12s", "Worked for 1m", "Failed: <why>", "Stopped after 9s", Back to lead. `components/chat/SubagentBar.tsx:52`
- W1-THR-084 Sub-thread strip over the composer in its shell: glyph frame, title, why, acts (waiting for a machine, killed for memory, not pushed, deleting). `components/chat/SubStrip.tsx:33`
- W1-THR-085 Git split: outline button with the act the checkout calls for (Commit, Commit and push, Push and create PR, View PR), chevron menu. `pull-request/GitSplit.tsx:79`
- W1-THR-086 Open split: default editor's mark and "Open" (spinner while opening), chevron menu of found editors with marks, keycap on the default. `files/OpenSplit.tsx:69`
- W1-THR-087 SSH consent dialog "Open in your editor over SSH": the ssh config line, what the workspace can reach, Cancel, Add to SSH config. `files/EditorConsent.tsx:52`
- W1-THR-088 Context card, 288 px: "Context window", percent and "45k / 200k" in mono, 4 px meter, or why it is missing; Compact context, held line. `components/chat/ContextMeter.tsx:95`
- W1-THR-089 Changed files card under a turn's last reply: sticky head "N changed files" and counts, Expand all folders, Open diff, tree, "Also changed in this folder". `components/chat/ChangedFilesTree.tsx:47`
- W1-THR-090 Person files: 56 px image thumbs opening full size in a dialog, file tiles (name over size), an image not kept as one mono record line. `components/chat/ChatFiles.tsx:79`
- W1-THR-091 Math by KaTeX, loaded on the first formula; a mermaid fence drawn as a diagram once the reply settles, else its source, "Diagram did not parse". `components/ChatMarkdown.tsx:476`
- W1-THR-092 Run box under a shell block: live terminal 232 px, Running, Move to terminal; ended output, end words, Send output to the agent, Open the tab; refusal. `components/chat/InlineRun.tsx:75`
- W1-THR-093 Bars in the composer slot: Tasks (done/total, step rows with times), Usage limit (Cancel, Resume at reset), computer cap (its setting). `components/chat/bars/TasksBar.tsx:28`
- W1-THR-094 Side question tab: "Nothing here is kept" note, question bubble, crab with Asking or Answering and seconds, answer markdown or refusal. `components/chat/AsideSurface.tsx:32`
- W1-THR-095 Fork rule line after a fork's handed turns, with Open while the source stands. `components/chat/timeline/rows.tsx:174`
- W1-THR-096 Sub-thread start line under its message: spinner, elapsed, chevron; open, steps with marks and times, raw log. `components/chat/SpawnLine.tsx:65`
- W1-THR-097 Threads this one opened, as tree rows with where each runs and its state, at the transcript's foot. `components/chat/ChatView.tsx:388`
- W1-THR-098 Markdown details fold, GitHub alerts in five colors, task checkboxes, footnotes, muted blockquote with a 2 px rule. `components/markdown/blocks.tsx:245`
- W1-THR-099 Rename box in the title row, 24 px, in place of the title. `shell/ThreadBreadcrumb.tsx:48`
- W1-THR-100 Subagent page: crumbs end in the lead (a link back) and the subagent's title; its bar stands in the composer slot. `shell/ThreadBreadcrumb.tsx:159`
- W1-THR-101 Opener crumb as a link with the thread's own address; a click selects it. `components/ThreadLink.tsx:29`
- W1-THR-102 No rows and not working: "Send a message to start the conversation." `components/chat/timeline/list.tsx:673`
- W1-THR-103 Machine wait in place of the working line: what it waits for, seconds, its act (Wake). `components/chat/timeline/working.tsx:18`
- W1-THR-104 Crumbs with no thread open: project and "New thread", a creation's name, muted "No thread selected", nothing over the first run. `shell/ThreadBreadcrumb.tsx:131`
- W1-THR-105 Crumbs under 640 px: the project's glyph alone, its name on a tooltip, the slash after it hidden. `shell/ThreadBreadcrumb.tsx:80`
- W1-THR-106 Open command row's head adds Copy command; output past the kept bytes says "First 12 KB of 40 KB" in the band's foot. `components/chat/timeline/callBlock.tsx:214`
- W1-THR-107 Multi-file edit: "Edited 3 files"; open, a head per file (path, counts, Open diff); "The transcript kept part of this change". `components/chat/timeline/callBlock.tsx:252`
- W1-THR-108 Compaction row stands alone, never folded: minimize glyph, "Compacted the context, 120k to 40k tokens". `adapt/timeline-rows.ts:447`
- W1-THR-109 Settled "interrupted" for a stop that never reached the computer adds the reason under it, 12 px muted. `components/chat/ChatView.tsx:607`
- W1-THR-110 A message sent while the agent worked carries the tooltip "Sent while the agent was working". `components/chat/timeline/rows.tsx:113`
- W1-THR-111 Changed files head words: "No files from this thread's edits", "N changed files in this folder"; Open diff's word only from 384 px. `components/chat/ChangedFilesTree.tsx:53`
- W1-THR-112 Subagent page body: what it was asked as a person bubble, its text as replies, its calls as rows, its prompts last. `adapt/session.ts:835`
- W1-THR-113 Subagent page: no context ring, Git or Open split, or panel toggles in the title row; the right panel hidden. `shell/AppShell.tsx:165`
- W1-THR-114 Minimap only with two person messages or more, and only for a fine pointer. `components/chat/timeline/minimap.tsx:145`
- W1-THR-115 Mermaid fence naming an address: "Diagram names an address and is not drawn" under its source. `components/chat/MermaidBlock.tsx:168`
- W1-THR-116 Plan card collapses past 900 chars or 20 lines to a 10-line preview under a 96 px fade; "Collapse plan" when open. `components/chat/ProposedPlanCard.tsx:63`

### States

- W1-THR-117 Machine line. png none: needs a machine lacking what wsp needs, or memory near full; the fixture's local project has neither. `components/chat/ChatView.tsx:397`
- W1-THR-118 History refused. png none: needs a host that refuses sessions.head and sessions.history for the thread. `components/chat/useChatThread.ts:901`
- W1-THR-119 Threads section, open and shut. png none: needs threads whose parentThreadId names short-1; no driver word sets it. `tree/TreeRows.tsx:211`
- W1-THR-120 Waiting-on row. png none: needs a child thread stopped on a question that short-1's running call waits behind. `adapt/timeline-rows.ts:256`
- W1-THR-121 Find, "This pattern took too long." png none: hangs on the regex worker's time cap, which a fast box may never hit. `components/chat/find/ThreadFind.tsx:273`
- W1-THR-122 Find, "Older messages are past what wsp keeps." png none: needs a thread the host's caps trimmed. `components/chat/find/ThreadFind.tsx:273`
- W1-THR-123 Loading transcript. png none: gone in under 100 ms on the fixture; open long. `components/chat/ChatView.tsx:461`
- W1-THR-124 Permission ask docked, record row waiting. png `W1-THR-124-<theme>.png`; drive: stub sends control_request can_use_tool tool_name "Bash", input {"command":"rm -rf build"}, holds; open it. `components/chat/PermissionPromptRow.tsx:110`
- W1-THR-125 Thinking opened. png `W1-THR-125-<theme>.png`; drive: +reply thinking (content [{"type":"thinking","thinking":"<3 lines>","signature":"s"}]); open it, click Worked for, click Thinking. `components/chat/timeline/workEntry.tsx:249`
- W1-THR-126 Plan card. png `W1-THR-126-<theme>.png`; drive: stub sends tool_use name "ExitPlanMode", input {"plan":"# Plan\n<30 lines>"}, then its tool_result; open it. `components/chat/ProposedPlanCard.tsx:115`
- W1-THR-127 Subagent fold open. png `W1-THR-127-<theme>.png`; drive: +reply Task, the stub's child assistant lines carrying parent_tool_use_id = the Task's tool_use id; click the fold. `components/chat/SubagentFoldRow.tsx:66`
- W1-THR-128 Person image, then full size. png `W1-THR-128-<theme>.png`; drive: +op sessions.start attachments [{"mediaType":"image/png","bytes":"<base64>","name":"shot.png"}]; click the thumb. `components/chat/ChatFiles.tsx:42`
- W1-THR-129 Insight note. png `W1-THR-129-<theme>.png`; drive: +reply markdown holding a "`★ Insight ─────`" line, 3 lines, then a "`─────`" line; open it. `components/chat/timeline/rows.tsx:272`
- W1-THR-130 Math and diagram. png `W1-THR-130-<theme>.png`; drive: +reply markdown with "$E=mc^2$", "$$\\int_0^1 x\\,dx$$" and a mermaid fence "graph TD; A-->B"; open it. `components/ChatMarkdown.tsx:542`
- W1-THR-131 Stopped turn: rows kept, "You stopped after Ns", foot word interrupted. png `W1-THR-131-<theme>.png`; drive: +working short-3; open short-3, click Stop. `components/chat/ChatView.tsx:591`
- W1-THR-132 Changed files card. png `W1-THR-132-<theme>.png`; drive: stub writes src/a.ts and src/b/c.ts in its cwd during the turn; open it. `components/chat/timeline/rows.tsx:244`
- W1-THR-133 Rename box open. png `W1-THR-133-<theme>.png`; drive: fixture; open short-1, double-click the title. `shell/ThreadBreadcrumb.tsx:45`
- W1-THR-134 Find, invalid pattern note. png `W1-THR-134-<theme>.png`; drive: fixture; open short-1, key mod+f, key alt+r, type "(". `components/chat/find/ThreadFind.tsx:273`
- W1-THR-135 Panel inline at 1280: minimap inert, no gutter. png `W1-THR-135-<theme>.png`; drive: fixture; open short-1, key mod+alt+b, hover the left edge. `components/chat/timeline/minimap.tsx:167`
- W1-THR-136 Subagent page. png `W1-THR-136-<theme>.png`; drive: as W1-THR-127; click Open its page. `components/chat/ChatView.tsx:107`
- W1-THR-137 Send starting. png none: the stub's session starts within a frame, so "Starting <agent>" never stands. `components/chat/useChatThread.ts:685`
- W1-THR-138 Open menu. png none: needs an editor the host finds installed; with none the split draws nothing. `files/OpenSplit.tsx:79`
- W1-THR-139 Sub-thread strip, subagent fold opened by a question, failed start line with Try again. png none: needs a cloud sub-thread or a subagent prompt. `components/chat/SpawnLine.tsx:95`
- W1-THR-140 Setup card standing. png none: stands only after a create this window started with no message. `components/chat/SetupCard.tsx:35`
- W1-THR-141 Fork on its way: "Loading transcript" while the forking turn runs and the fork's thread is not listed. png none: needs an agent fork. `components/chat/ChatView.tsx:460`
- W1-THR-195 New thread page: folder mark, "What should we build in bench?" with the project's name underlined, the ASCII field behind the column, the composer under the heading, the computer and branch chips under the box. png `W1-THR-195-<theme>.png`; drive: hash `#w/<bench's workspace>/new`, reload. `components/chat/ChatView.tsx:559`
- W1-THR-196 New thread page on a project with no thread: the same page, "What should we build in quiet?". png `W1-THR-196-<theme>.png`; drive: the fixture's realHost; hash `#open/project/<quiet's id>`, reload. `shell/ProjectHome.tsx:67`

### Actions and keys

- W1-THR-142 Run, Run again; Move to terminal, Send output to the agent, Open the tab. `components/markdown/codeBlocks.tsx:163`
- W1-THR-143 Find step opens what hides the match (turn fold, call group, clamped message, call row, plan, subagent fold), scrolls it clear of the bar. `components/chat/timeline/list.tsx:570`
- W1-THR-144 Find keeps query and toggles across thread switches; a new thread searches from its newest match; mod+f with the bar open reselects the field. `components/chat/find/store.ts:74`
- W1-THR-145 Threads head toggles shut, kept per lead in preferences for every window; Settle N finished settles the finished part of the tree. `tree/TreeRows.tsx:201`
- W1-THR-146 Child row: a press opens the thread (a subagent row its page); Send a message opens a one-line field, Enter sends, Esc cancels. `components/threads/ThreadRows.tsx:206`
- W1-THR-147 Child row acts: Stop in two presses, Settle, Restore, Stop subagent; More and a right-click open the row's menu. `components/threads/ThreadRows.tsx:174`
- W1-THR-148 Git quick act: commit every change with the agent's drafted message (no draft opens Changes), push, open a PR; View PR opens the PR pane. `pull-request/GitSplit.tsx:69`
- W1-THR-149 Open: the main part opens the thread's copy in the default editor; a menu pick opens there and becomes the default. `files/OpenSplit.tsx:67`
- W1-THR-150 Back to lead selects the lead thread and puts the caret in its composer. `shell/WorkspaceThread.tsx:70`
- W1-THR-151 Strip acts: Raise opens the computer's settings, Add again opens Add a computer, Start again and Push again send to the lead or thread. `components/chat/SubStrip.tsx:86`
- W1-THR-152 Quote: the selection and the prompt it answered go into the composer; Esc dismisses. `components/chat/AssistantSelectionToolbar.tsx:101`
- W1-THR-153 Scroll to end: animated scroll to the end, following again. `components/chat/ChatView.tsx:493`
- W1-THR-154 A file chip opens the file in Files at its line; Open diff opens Changes on the turn's range at that file. `components/chat/ChatView.tsx:230`
- W1-THR-155 Rewind to here and Fork from here open their dialogs (insides in `parity/wave-2`); Rewind hides while working. `components/chat/timeline/rows.tsx:144`
- W1-THR-156 Show full message / Show less. `components/chat/timeline/rows.tsx:409`
- W1-THR-157 Prompt option buttons answer at once; Open its page selects the subagent's page. `components/chat/SubagentFoldRow.tsx:82`
- W1-THR-158 Wake on the paused line and a machine wait. `components/chat/timeline/working.tsx:28`
- W1-THR-159 Side question: Esc closes its tab. `components/chat/AsideSurface.tsx:26`
- W1-THR-160 Copy: message (check 1.5 s), code (1.2 s), command, output; table Copy as Markdown or CSV. `components/chat/MessageCopyButton.tsx:31`
- W1-THR-161 Context ring popover: Compact. `components/chat/ContextMeter.tsx:115`
- W1-THR-162 A link to a heading scrolls in place; a joined computer's port link opens in a Browser tab. `components/ChatMarkdown.tsx:338`
- W1-THR-163 A one-line selection in the transcript (200 chars max) seeds the find query. `components/chat/find/store.ts:73`
- W1-THR-164 Copy out of a reply puts markdown on the clipboard as text and HTML. `components/ChatMarkdown.tsx:204`
- W1-THR-165 Open the thread's folder in the editor, key `mod+o` (`keybindingDefaults.ts:69`); off under Settings. `shell/shellCommands.ts:414`
- W1-THR-166 Updated the slate opens the Slate tab. `slate/SlateUpdatedLine.tsx:15`
- W1-THR-167 Start line Try again sends the refused message again and clears it from the draft. `components/chat/SpawnLine.tsx:57`
- W1-THR-168 Plan menu: Copy to clipboard (then "Copied"), Download as markdown saving "<title>.md". `components/chat/ProposedPlanCard.tsx:125`
- W1-THR-169 A run whose program takes the full screen moves itself to a terminal tab, still running. `components/chat/InlineRun.tsx:72`
- W1-THR-170 Subagent fold opens itself on each new question from its run; a person's shut stands until the next one. `components/chat/SubagentFoldRow.tsx:57`
- W1-THR-171 A subagent page whose launch no page and no listing holds opens the lead thread instead. `components/chat/ChatView.tsx:127`
- W1-THR-172 A failed rename leaves the box open; a client with no rename shows Rename refused on the title's menu. `shell/ThreadBreadcrumb.tsx:55`

### Host operations

- W1-THR-173 `sessions.run` records a block's run; pty over `daemon.open`. `components/chat/replyRun.ts:82`
- W1-THR-174 `preferences.set` threadsShut for a lead on the Threads head. `tree/TreeRows.tsx:202`
- W1-THR-175 `sys.subscribe`, `sys.unsubscribe` memory readings behind the machine line, for workspaces read on the host. `machine/hostLive.ts:53`
- W1-THR-176 `sessions.history` whole folder history for a view with no thread picked. `components/chat/useChatThread.ts:890`
- W1-THR-177 `sessions.start` Send output to the agent, after a run. `components/chat/InlineRun.tsx:105`
- W1-THR-178 `workspaces.wake` Wake. `components/chat/ChatView.tsx:392`
- W1-THR-179 `sessions.mark` Resume at reset, Cancel. `components/chat/bars/UsageBar.tsx:30`
- W1-THR-180 `daemon.send`, `daemon.close` the run's pty. `protocol/client.ts:1123`
- W1-THR-181 window opens http links in the system browser. `desktop/main.ts:453`
- W1-THR-182 `editor.open` the thread's folder on mod+o and the Open split. `files/openCopy.ts:44`
- W1-THR-183 `project.export` the strip's Copy to <computer>, through the bring-home dialog. `actions/verbs.ts:51`
- W1-THR-184 window save dialog for Download as markdown: Electron's own download prompt, no preload call. `components/chat/ProposedPlanCard.tsx:72`

### Design

- W1-THR-185 One rule-line piece for paused, machine, slate, fork, wait and retry lines: 24 px, 11 px muted, truncated, whole on hover; none yet.
- W1-THR-186 Setup card: ui `Card` with `StepRow`, `step_mark`; strip and subagent bar: the composer's shell (none yet), `glyph_frame`, outline `Button`.
- W1-THR-187 Threads rows: the sidebar's status slot, `Status`, `StatusMark`, `Crab` for counts; row box, folds and hover acts none yet, shared with the sidebar tree.
- W1-THR-188 Git and Open splits: two outline `Button`s joined at a 1 px seam, gpui-kit `PopupMenu`, `KeyCaps` for the default's chord; consent `Dialog`.
- W1-THR-189 Colors: status-input waiting glyph; error-foreground exit words, refusals; destructive runtime error; primary find marks 20%/45%; info-foreground links.
- W1-THR-190 Sizes from preferences: textSize sets reply and bubble text, codeSize every code surface; facts in 11 px mono.
- W1-THR-191 Zed GPUI: `list` with `ListState`, `StyledText` highlights, `canvas` for the minimap ticks and the noise field, `img`.
- W1-THR-192 Diff counts in mono: +N in success ink, -N in error-foreground; the turn card's tree counts neutral.
- W1-THR-193 Links in info-foreground with a dotted underline (0.75 px dots every 4 px) on hover; file chips no underline, 70% accent fill on hover.
- W1-THR-194 Context card: gpui-kit `Popover` in the tooltip skin; none yet in ui/.

## Composer

### Elements

- W1-CMP-063 Reasoning button (brain, effort, bolt): Reasoning, Context window, Fast mode radios. `components/chat/ComposerOptionPickers.tsx:221`
- W1-CMP-064 Banner over the box: fork (Cancel the fork) or resume (Start fresh). `components/chat/ChatComposer.tsx:1181`
- W1-CMP-065 Queue bar in its place: rows (files, why it waits), Edit, Cancel. `components/chat/bars/QueueBar.tsx:58`
- W1-CMP-066 Permission ask docked: the call's words as title, command copy row ("in <dir>" where it differs), file card with -/+ lines or body, key: value lines. `components/chat/PromptDock.tsx:379`
- W1-CMP-067 Permission ask options: Choice rows, a mode option with the catalog's line; Deny opens "What to do instead" (384 px); primary reads Allow or Deny. `components/chat/PromptDock.tsx:157`
- W1-CMP-068 Question forms: earlier answers as "<header>: <picks>" lines, Back and Next, tick rows for multi-select, a lone field with no options. `components/chat/PromptDock.tsx:356`
- W1-CMP-069 Dock refusal under the cards: the host's sentence in error ink, its fix after it in the body ink. `components/chat/PermissionPromptRow.tsx:183`
- W1-CMP-070 Sub-thread strip over the composer: glyph frame, state title, note, acts (Raise, Add again, Start again at <size>, Copy to <here>, Push again). `components/chat/SubStrip.tsx:33`
- W1-CMP-071 Creation room: asking message as a person bubble, card "Setting up <name>" of step rows; failed: reason, Retry, Dismiss; then "<name> is set up". `components/chat/SetupCard.tsx:35`
- W1-CMP-072 Model menu: star tab (favourites only), "No model matches", "default" word, unlisted row "Not in this task's list". `components/chat/ComposerModelPicker.tsx:279`
- W1-CMP-073 Access menu: head, a line on what a pick does mid-turn, rows with the mode icon, "default", note; refused button in error ink. `components/chat/ComposerOptionPickers.tsx:316`
- W1-CMP-074 Chip labels: quote cut at 40 chars, "#n title" (max 320 px), terminal "<label>, N lines" with a 12-line mono tip; "dir/" a folder. `components/ComposerCitationNode.tsx:28`
- W1-CMP-075 Resume picker row: agent mark, title, facts (project, Open in another app or Already a thread, branch, size), time; preview "Runs in <dir>", last 4 messages. `shell/ResumePicker.tsx:263`
- W1-CMP-076 Resume draft: send says "Continue" in words; banner "Continues <title>" or "Continues a copy of <title>", agent name, Start fresh. `shell/ProjectHome.tsx:164`
- W1-CMP-077 Attach: ghost paperclip, caps sentence on hover, a hidden multiple file input; off while the box is shut or waits. `components/chat/ChatComposer.tsx:1125`
- W1-CMP-078 Line under the box (a refusal slot): agent signed out on this computer, a link no project holds, default model not run. `components/chat/ChatComposer.tsx:1326`
- W1-CMP-079 Stash drawer over the box: "Stashed N" head, rows (snippet cut at 90, n files, n dropped, relative time, delete x). `components/chat/ComposerStashMenu.tsx:84`
- W1-CMP-080 Model chips over the editor (New thread, two models or more): agent mark, label, remove x, 24 px. `components/chat/ComposerModelChips.tsx:13`
- W1-CMP-081 Send says "Send to N" for several models, "Start on #n" for a link, a "Review #n" outline button beside it. `components/chat/ChatComposer.tsx:1159`
- W1-CMP-082 Model menu foot, mono 11 px one line: "Start a new thread to use X here", computer away, agent refusal, shift-click line. `components/chat/ComposerModelPicker.tsx:129`
- W1-CMP-083 Chip selection: chips in a selection take Highlight at 30% over them, only while focused. `components/composerInlineChip.ts:15`
- W1-CMP-084 Resume draft page: the conversation's earlier messages, a loading or refusal line, the composer docked at the foot. `shell/ResumeDraft.tsx:67`
- W1-CMP-085 Creation room: the composer docked at the foot; a send waits as a queue row whose hover says "Sends once <name> is up". `shell/WorkspaceCreation.tsx:67`
- W1-CMP-086 Conversation open dialog: title naming the computer, note, Cancel, Try again (where it lets go), Copy. `shell/ConversationConfirm.tsx:48`
- W1-CMP-087 Resume picker: palette dialog, agent, branch and project scopes, rows, preview past 768 px; `enter` continues, `shift+enter` a copy. `shell/ResumePicker.tsx:159`
- W1-CMP-088 Project menu: every project with its computer word, then Add a project. `shell/NewThreadPicks.tsx:76`
- W1-CMP-089 Drawer row parts: glyph ink by kind (question status-input, usage warning, rest muted), muted name, line at 80%, mono count, chevron up. `components/chat/ComposerDrawer.tsx:30`
- W1-CMP-090 Slash, @, # rows: file, PR or issue mark, 12 px medium name (max 45% if described), description up to 48ch, active row accent. `components/chat/ComposerCommandMenu.tsx:127`

### States

- W1-CMP-091 File chips. png none: needs files on the hidden input or a paste. `components/chat/ChatComposer.tsx:1224`
- W1-CMP-092 Drop. png none: needs a dragenter carrying Files. `components/chat/ChatDropZone.tsx:39`
- W1-CMP-093 Line under. png `W1-CMP-093-<theme>.png`; drive: none; new thread, type "https://github.com/x/y/pull/1". `shell/ProjectHome.tsx:200`
- W1-CMP-094 Shut: editor read-only, Attach off, send held with the reason. png none: needs the host link down or the agent lists not in yet. `components/chat/ChatComposer.tsx:426`
- W1-CMP-095 Resume picker, no conversations. png `W1-CMP-095-<theme>.png`; drive: none; new thread, key mod+k, type "Resume a conversation", key enter. `shell/ResumePicker.tsx:222`
- W1-CMP-096 Stash list. png `W1-CMP-096-<theme>.png`; drive: none; open short-1, type "a", key mod+s, type "b", key mod+s, key mod+s. `components/chat/ComposerStashMenu.tsx:84`
- W1-CMP-097 Queue bar. png `W1-CMP-097-<theme>.png`; drive: +working short-1; open short-1, type "a", key enter, click "1 message waiting". `components/chat/bars/QueueBar.tsx:58`
- W1-CMP-098 Stopping. png none: lasts until the stub's turn ends, under a frame. `components/chat/ComposerPrimaryActions.tsx:116`
- W1-CMP-099 One-line box, tall draft. png `W1-CMP-099-<theme>.png`; drive: none; open short-1, type a 200-character line. `components/chat/composerMotion.ts:62`
- W1-CMP-100 Wake and send. png none: the fixture's local project never pauses. `components/chat/ComposerPrimaryActions.tsx:265`
- W1-CMP-101 Machine not answering, box open, send held. png none: needs a computer that stops answering. `components/chat/ChatComposer.tsx:416`
- W1-CMP-102 Fork banner. png none: needs a fork draft, made from another thread's menu. `components/chat/ChatComposer.tsx:1174`
- W1-CMP-103 Resume draft. png none: needs a conversation kept outside wsp on the fixture. `shell/ResumeDraft.tsx:67`
- W1-CMP-104 Width 980. png `W1-CMP-104-<theme>.png`; drive: width 980; open long. `components/chat/ChatComposer.tsx:1176`
- W1-CMP-105 Tasks bar. png `W1-CMP-105-<theme>.png`; drive: as W1-CMP-030; click "Tasks". `components/chat/bars/TasksBar.tsx:28`
- W1-CMP-106 @ menu, no match. png `W1-CMP-106-<theme>.png`; drive: none; open short-1, type "@zzzz". `components/chat/ChatComposer.tsx:565`
- W1-CMP-107 Model menu, favourites empty. png `W1-CMP-107-<theme>.png`; drive: none; new thread, click the model button, click "Favourites". `components/chat/ComposerModelPicker.tsx:341`
- W1-CMP-108 Sub-thread strip. png none: needs a sub-thread on a cloud computer. `components/chat/SubStrip.tsx:69`
- W1-CMP-109 Create failed. png none: needs a create the host refuses. `components/chat/SetupCard.tsx:32`
- W1-CMP-155 Footer chips: the computer by its label ("zingzy's MacBook Pro", with its glyph) and the branch ("main"), never the hostname. png `W1-SBR-172-<theme>.png`; drive: as W1-SBR-172. `components/chat/ComposerCheckoutRow.tsx:96`

### Actions and keys

- W1-CMP-110 A bracket or quote typed over a selection wraps it. `components/composer-editor/surround.ts:11`
- W1-CMP-111 Model menu: arrows, `enter`, `shift+enter` adds, `mod+1`-`9`. `components/chat/ComposerModelPicker.tsx:168`
- W1-CMP-112 Enter on a held send keeps the draft and raises the reason as a note toast, the words the button's hover says. `components/chat/ChatComposer.tsx:812`
- W1-CMP-113 With "New thread in" set to ask and two projects or more, key `mod+t` (`keybindingDefaults.ts:59`) opens the palette's New thread page. `shell/NewThreadPicks.tsx:44`
- W1-CMP-114 Queued rows read back at launch are held: none sends by itself until the next send on that thread; a refused start goes back to the head, held. `components/chat/composerDraftStore.ts:95`
- W1-CMP-115 New thread and a fresh thread put the caret in the box; Write a message instead, a bar's fold and Quote hand the caret back where it stood. `components/chat/ChatComposer.tsx:572`
- W1-CMP-116 Stash list: arrows, `enter` restores, `escape` closes, `mod+backspace` deletes, read in the capture phase. `components/chat/ComposerStashMenu.tsx:50`
- W1-CMP-117 macOS `home`/`end` move to the visual line's edge, with shift extending. `components/composer-editor/plugins.ts:102`
- W1-CMP-118 A pasted quote, terminal or PR/issue block becomes a chip; a pasted @word stays text. `components/composer-editor/plugins.ts:377`
- W1-CMP-119 /btw question goes to the host, starts no turn, opens the side question tab. `components/chat/ChatComposer.tsx:819`
- W1-CMP-120 A command that runs only in the CLI's terminal raises a note naming the wsp control. `components/chat/ChatComposer.tsx:828`
- W1-CMP-121 A queued row's Edit puts its words and files back after the draft. `components/chat/ChatComposer.tsx:880`
- W1-CMP-122 A bar's `escape` folds it back to the composer with the caret in it. `components/chat/composerBar.ts:62`
- W1-CMP-123 A letter typed while the dock or a bar stands lands in the draft and brings the composer back. `components/chat/composerTypeToFocus.ts:42`
- W1-CMP-124 New thread, key `mod+t` (`keybindingDefaults.ts:59`). `shell/NewThreadPicks.tsx:40`
- W1-CMP-125 Dock field: `escape` leaves it (pick back to row 1), a second folds; a digit on a picked field row focuses it; a focused button keeps `enter`. `components/chat/PromptDock.tsx:300`
- W1-CMP-126 A block from outside (Add to chat, Quote, slate fill) lands at the caret on its own lines, caret after; under a dock or bar it joins the draft. `components/chat/composerInsert.ts:11`
- W1-CMP-127 Held drafts: "/" alone, "/name" nothing announced, "/btw" with no question hold the send with a reason; "/name words" goes as text. `adapt/catalog.ts:71`
- W1-CMP-128 @ and # show at most 30 rows: files by name then path, # by number prefix; `escape` keeps the menu shut until the caret leaves the token. `components/chat/composerMenuSearch.ts:11`
- W1-CMP-129 Files are read again for each new @ token, the last answer shown meanwhile; pull requests and issues are kept 5 minutes per repository. `components/chat/ChatComposer.tsx:145`
- W1-CMP-130 Tray branch reread every 5 s and on window focus while a send would open a thread; else after each turn start, end and finished call. `components/chat/ComposerCheckoutRow.tsx:53`
- W1-CMP-131 Warm-up asked on focus into the box, a changed pick, and typing at most once per 30 s; never for a fork, a resume or a several-model send. `components/chat/ChatComposer.tsx:482`
- W1-CMP-132 The signed-out line under the box reads the agents report again each time the window regains focus. `components/chat/ChatComposer.tsx:1077`
- W1-CMP-133 Stash list closes on a pointer down outside it, or on its own head row. `components/chat/ComposerStashMenu.tsx:41`
- W1-CMP-134 A drop the composer cannot take (box shut or waiting) is swallowed with no frame and never opened by the window. `components/chat/ChatDropZone.tsx:47`
- W1-CMP-135 A send to several models checks room first: "<computer> has room for N more <noun> now, and this send needs M". `shell/ProjectHome.tsx:34`
- W1-CMP-136 Resume picker: a row that is already a thread opens that thread; a row open in another app asks first unless picked as a copy. `shell/ResumePicker.tsx:129`

### Host operations

- W1-CMP-137 `agents.read` skills. `components/chat/ChatComposer.tsx:530`
- W1-CMP-138 `projects.branch` New thread. `components/chat/ComposerCheckoutRow.tsx:87`
- W1-CMP-139 `workspaces.landing` computer. `shell/NewThreadPicks.tsx:119`
- W1-CMP-140 `workspaces.start` and `workspaces.review` a GitHub link in New thread. `shell/ProjectHome.tsx:107`
- W1-CMP-141 `places.list` room for a send to several models. `shell/ProjectHome.tsx:44`
- W1-CMP-142 `conversations.read` the resume draft's messages. `shell/conversations.ts:133`
- W1-CMP-143 `sessions.start` a send on a resumed conversation. `shell/conversations.ts:178`
- W1-CMP-144 `sessions.attachment` images of a sent row this window does not hold. `components/chat/composerFiles.ts:283`
- W1-CMP-145 `agents.read` the signed-out line under the box. `components/chat/ChatComposer.tsx:1068`
- W1-CMP-146 `conversations.read` with preview the picker's preview pane, 150 ms after the highlight rests on a row. `shell/conversations.ts:112`
- W1-CMP-147 `project.export` Copy to <here>, `sessions.start` Push again and Start again, in the sub-thread strip. `components/chat/SubStrip.tsx:125`
- W1-CMP-148 window: the platform's open-file dialog, several files, behind Attach. `components/chat/ChatComposer.tsx:1137`

### Design

- W1-CMP-149 Dock frame (question dock, bars): composer shell, 16 px sides; head: mark, 16 px semibold title, aside; cards; foot pb 20, back left, acts right. `components/chat/Dock.tsx:54`
- W1-CMP-150 Under 640 px window width: composer sides 12 px (20 above), tray 36 px with 28 px items, send and stop 36 px, Review on its own row, dock fields full width, headline 24 px.
- W1-CMP-151 Stores beside drafts: favourites `wsp:composer-favourites:v1` (all workspaces), picks `wsp:composer-options:v1` until the opening send, Fast per thread `wsp:composer-modes`.
- W1-CMP-152 Conversation open dialog: `Dialog`, `Button` outline and primary; stash drawer and model chips: none yet.
- W1-CMP-153 The open bar is per thread and per window, never saved: after a restart every thread shows its composer. `components/chat/composerBar.ts:36`
- W1-CMP-154 Sub-thread strip and setup card: `glyph_frame`, `Button` outline xs, `Card` with `StepRow`; the strip's frame none yet.

## More port notes

Each screen's own port notes, numbered on from D1 to D22 in `spec.md`, which govern where one repeats them.

### Frame and layout

- D23 The sheet at 980 px and under, with no media queries in GPUI: read `window.viewport_size().width` in the shell's render, draw the panel inline above 980 px and as our own deferred overlay at 980 and under (scrim of background at 60% with the patch's 4 px backdrop blur, width min(42vw, 448) with a 320 floor, min(88vw, 384) under 760, 32 px slide and fade over 200 ms), and keep one panel entity across the switch; the same overlay on the left draws the sidebar under 768 px. Why: today keeps the panel mounted through the switch and its sheet has no title row or close button, while gpui-kit's `Sheet` always draws both and slides 100 px over 150 ms with no fade.
- D24 The window's minimum size: today's window sets none, and the GPUI app sets none either (no `window_min_size` in `app/src/screens/dialogs/startup.rs`). The centre column's 360 px floor (`CENTER_MIN`, `app/src/screens/app_shell/frame.rs`) governs how the sidebar and panel give way, not how small the window may get.
- D25 Glass on Linux, where our patch blurs inside the window but there is no window vibrancy: draw the solid look today's Linux app draws (it sets the `solid` class and hides Transparency), every glass at its solid ground and the sidebar in --sidebar, `WindowBackgroundAppearance::Opaque`. Why: Electron already gives Linux no glass.
- D26 The reference is shot with Transparency off, every state on every platform (`parity/baseline.mjs`). On Linux the Chromium reference draws the sidebar at 80% with a blur where the desktop app is solid, so otherwise every sidebar pixel differs for a reason the GPUI app cannot fix.
- D27 Grain, a 256 px SVG fractal noise at 3.5% on body, sidebar and inset that GPUI cannot draw, is rendered once in Chromium to a PNG (`app/src/ui/icons.rs`) and tiled as an image on Settings (`app/src/ui/grain.rs`). On Linux, if the tile costs frames, it may be left out as a deliberate difference; the app draws it on Linux. The app must tile it on the main window too, as today does (`index.css:855`); the main window now draws one flat grey at the tile's mean, 2.06%, since the tile as an image held a 12 MB atlas page (`app/src/screens/app_shell/frame.rs`). Why: a bitmap tile is the only way to match the noise.
- D28 Glass on the Mac (Electron sets vibrancy "sidebar", followWindow): the app puts its own NSVisualEffectView with the Sidebar material behind the window, following its active state, and draws the window `Transparent` while the page draws glass, `Opaque` when Transparency is off or the Mac asks for less (`app/src/screens/app_shell/vibrancy.rs`, `app/src/shell/preferences.rs`). GPUI's own Blurred appearance is not used, since it draws a different material with its tint and saturation taken out. Each region paints its material share over it (dark: sidebar 40%, centre 67%, panel 74%; light: its solid ground) (`app/src/screens/app_shell/material.rs`). Today's glass:set turns the material on and off the same way.
- D29 Traffic lights and the title bar on the Mac: `TitlebarOptions { appears_transparent: true, traffic_light_position: Some(point(px(16.), px(19.))) }`, no title drawn, the 52 px header row as the drag area, its left pad 85 px. Verify by screenshot that the lights' centre sits at y 26. Why: Electron and GPUI may measure that position from different corners, and the header's spacing is measured against the lights.
- D30 The title bar on Linux: on the desktops window.ts lists (from XDG_CURRENT_DESKTOP, else XDG_SESSION_DESKTOP), client decorations with min, max and close drawn at the header's right end, 52 px tall, in --titlebar-ground and --titlebar-ink, their width kept clear on the header, the inline tab bar and the banners; server decorations everywhere else. Copy gpui-kit `TitleBar`'s window controls and `WindowBorder` for the resize edges. Why: Electron's titleBarOverlay draws these today and GPUI draws nothing in their place.
- D31 The app menu: GPUI's native menu through `cx.set_menus` on macOS only (wsp, File with Close window on mod+shift+w, Edit, View, Hosts, Window), rebuilt when the host list moves, and no menu on Linux. Why: Electron sets its menu on macOS only, and on Linux its accelerators would take a terminal's Control chords.
- D32 What View holds (Electron's role gives Reload, Force reload, Toggle developer tools, Actual size, Zoom in, Zoom out, Full screen): keep Actual size, Zoom in, Zoom out and Full screen; drop Reload and the developer tools. Why: GPUI has no page to reload or inspect.
- D33 Page zoom, which Electron applies to the whole page on mod+=, mod+- and mod+0 and hands to a focused terminal: a window-wide UI scale on those chords, sent to the terminal's text size while a terminal has focus. Why: a Mac user who zooms today would lose it otherwise.
- D34 Key dispatch: one dispatcher on the root view's key handler that reads DEFAULT_KEYBINDINGS with preferences.keybindings over them and the same when-clause context (terminalFocus, previewFocus, panelTabsFocus, threadOpen, threadFindOpen, terminalOwnsMod), not GPUI keymap contexts. Why: the rules are the person's to rebind, and labels and stand-downs must read the one table today's app reads.
- D35 The launcher's bare letters, which GPUI cannot bind as actions: handle them in the launcher's and the window's key handlers after checking that no field has focus and no layer is open, as keyBelongsElsewhere does. Why: GPUI matches bindings before the input method and binds no bare printable key.
- D36 The context menu: the app's own `ContextMenu` (`app/src/ui/context_menu.rs`), not gpui-kit `NativeMenu`. On macOS it is an OS-drawn `NSMenu` with the refusal as the disabled row's tooltip (`app/src/ui/context_menu/mac.rs`); on Linux and Windows it is an app-drawn menu after the web's `InAppContextMenu`, a refused row dimmed with its refusal on hover, kept inside the window with an 8 px margin. Electron's menu is native on every platform and can reach past the window (`desktop/main.ts:156`); off the Mac the app must show the OS menu, as D10 says.
- D37 The starting window: draw the spinner and "Starting wsp" inside the app window after 800 ms instead of opening a second window. Why: GPUI draws its own page and needs nothing from the host before its window can stand.
- D38 The quit question and the failed start: `window.prompt` with the same message, detail and buttons (Quit, Quit and stop wsp, Cancel; Open log, Quit). Why: it is native on macOS and GPUI draws one on Linux.
- D39 Where the right panel's width and each thread's panel tabs live, today in the browser's localStorage (wsp:right-panel-width, wsp:right-panel-state:v1): a state file in the app's own data folder, not `preferences.set`. Why: both are per window on this computer today and the host record has no field for them.
- D40 Whether the sidebar's open state outlives a launch: no, it opens every launch. Why: today's app writes a sidebar_state cookie and never reads it.
- D41 The window bridge calls (hostToken, setTheme, setGlass, setTitleBar, setTerminalFocus, onShellChord, contextMenu): plain calls inside the app, the theme side through `cx.set_window_appearance` and glass through `set_background_appearance`. Why: GPUI has no shell and page to bridge, and the host client needs no op for them.
- D42 The pair screen: show it only for a host on another computer this app holds no device token for; on this computer the app reads the host's token itself. Why: today it shows in the desktop app only when the shell's token is refused twice.
- D43 The opening ground before preferences.get answers: dark (#09090b), as today. Why: the window opens before the record is read and today's app opens dark too.

### Settings, frame and small pages

- D44 The Version card gets the host's version from `HostClient::host_release()` (`host-client/src/lib.rs`), the `version` the host already puts on every answer that lets a socket in (`auth`, `device.auth`, `pair.redeem`; runtime serve.ts:854 and :889, set from VERSION at host server.ts:642); wsp-mcp's dial parses it (client.rs:138, `Released::of`) and keeps it on its Host after the release check (`daemon/crates/wsp-mcp/src/host.rs:94`), and Settings reads it with the statuses (`app/src/screens/settings/reads.rs`). Why: the web reads the same VERSION constant off the page's boot line (`window.__WSP__.version`, host server.ts:491, read at `shell/shellVersion.ts:19`), and GPUI has no page.
- D45 The card's mark is the host's version, else the app's own release, else "unknown" (ABOUT_WORDS.unknown), in the order `settings/about.tsx:101` uses (`app/src/screens/settings/pages/about.rs`). Since the GPUI binary always knows its release, a host older than the auth answer's version field shows the app's release rather than "unknown".
- D46 The app half (`window.wsp.version`, preload.ts:40) is the GPUI binary's own release version; App and Host lines appear when it differs from the host's, and the meta word and update step test both, as releaseAhead does (`shell/shellVersion.ts:26`) (`app/src/screens/settings/pages/about.rs`); why: the two halves can still be built apart.
- D47 The reference is today's Electron app itself, the desktop shell, launched on the fixture's throwaway state and driven over the Chrome DevTools Protocol (`parity/baseline.mjs`), so its own `window.wsp` draws the desktop rows; why: GPUI is the desktop app, and a browser tab hides rows GPUI must draw.
- D48 Desktop-only rows: (1) General, When you quit (`settings/general.tsx:153`); (2) Start wsp at login, own computer with a registered service (`:154`); (3) Logs card, Open logs (`:171`); (4) Version, App and Host lines when apart (`settings/about.tsx:108`); (5) Version step that downloads, Quit and open or Restart to update, own computer only (`shell/update.ts:91`). The app draws all five (`app/src/screens/settings/pages/general.rs`, `about.rs`, `app/src/update/mod.rs`).
- D49 More desktop-only differences: (6) Glass card hidden on a solid window, Linux and Windows, kept on the Mac (`settings/appearance.tsx:116`); (7) font rows list installed families, typed field otherwise (`settings/FontPicker.tsx:32`); (8) Keybindings draw the chords a tab keeps and add Close the window, mod+shift+W (`keybindings.ts:46`, `:75`).
- D50 Glass: on the Mac the app draws the Glass card and tests transparency in Appearance's off-defaults check; on Linux it leaves both out, as `settings/appearance.tsx:47` does (`app/src/screens/settings/pages/appearance.rs`); why: a Linux window draws no glass (solid stand-ins, D5).
- D51 Privacy learns that the host sends no usage counts (`productUsageOff`) from the page's boot line alone (`settings/privacy.tsx:20`): the app reads that line with one GET of the host's page when Settings reads, held between openings, as the Electron shell reads the boot line (desktop host-lifecycle.ts:63), until the auth answer carries it (`app/src/screens/settings/reads.rs`). The app must read it for a host elsewhere too, as the page's boot line carries it for any host today (`settings/privacy.tsx:20`); it now reads only the host on this computer. Why: the GPUI app then needs no host change.
- D52 The open page is remembered in the GPUI app's own local store under `wsp:settings-at`, as atId words, first page Appearance, never in host preferences (`app/src/screens/settings/kept.rs`); each window keeps its own page once read, and every move writes the store, so a new window opens where any window last was. The app must keep it per host, as today's browser storage is per host page (`settings/settingsStore.ts:24`); the app's key is one for all hosts now.
- D53 Start at login: the app ports the Electron main process's loginStart and setLoginStart (desktop host-lifecycle.ts:271) natively (`app/src/desktop/linux.rs`, `macos.rs`) and shows the row once the service manager reports a service registered for this launch's state file (`app/src/screens/settings/pages/general.rs`). The app must also hide the row while the window is attached to a host elsewhere, as today hides it whenever the shell answers null (`settings/general.tsx:154`); it does not check that now.
- D54 Open logs: the app shows the Logs card always and opens `<wsp home>/logs` with the system opener, the folder today's app keeps its log and crash reports in and the GPUI app writes its own `app.log` into (`app/src/screens/settings/pages/general.rs`, `app/src/desktop/log.rs`); why: the row exists wherever the shell can open its logs.
- D55 Installed fonts: the app lists the text system's families (all_font_names), Default first, the saved family listed when missing; on a host elsewhere too, a deliberate difference (`app/src/screens/settings/pages/appearance.rs`); why: the web falls back to a typed field there only because a remote page may not read this computer, and GPUI draws with local fonts anyway.
- D56 Theme preview on hover: today previews only the window under the pointer (`settings/ThemePicker.tsx:81`), and the app must do the same. The app now restyles every window through `ui::set_theme`, restored on leave and on window blur (`app/src/screens/settings/pages/appearance.rs`, `app/src/screens/settings/mod.rs`).
- D57 Version step: the app has its own updater (`app/src/update/mod.rs`). Get downloads the release; an app that can replace itself checks it and offers Restart to update, and any other offers Quit and open on the disk image or AppImage it saved into Downloads, as getBundle and quitAndOpen do for the Electron bundle. The releases-page link (the web's tab road, `shell/update.ts:139`) stands in where the host runs elsewhere or only the host is behind. Restart host stays a `host.restart` call (`app/src/screens/settings/pages/about.rs`).
- D58 Chord capture: a window-level capture-phase key handler so the pressed chord never reaches the dispatcher, refusing chords a browser tab keeps even in the desktop as today (keybindingOverrides.ts:141); why: the same keybindings record serves every browser tab on the host.
- D59 Escape order: the palette over Settings takes it, then the search field's clear, then close Settings; why: `shell/KeybindingDispatcher.tsx:54` and `settings/SettingsSidebar.tsx:116` split it that way.
- D60 Settings sidebar row height: 36 px (`sidebar/rowGrammar.ts:26` overrides the kit's size sm 28 px) (`app/src/screens/settings/frame.rs`); SettingsSidebar.tsx's header comment still says 28 px and is stale.
- D61 Missing shared pieces: add ui::Switch (gpui-kit switch::Switch restyled: 30 x 18 px track at 640 px and up, 16 px thumb) and a segmented control (gpui-kit TabBar, TabVariant::Segmented, restyled); extend Row with the reset arrow and the mark, Card with a body; why: General, Appearance and Privacy cannot be built from card.rs and controls.rs alone.
- D62 Section trail lighting: watch each card's bounds against the scroll view on scroll and count a card lit while any part shows; why: the web's IntersectionObserver has no GPUI twin.

### Settings, Agents, Projects, Devices, Account

- D63 The status word ends in a colored dot, which the locked design forbids (no status dot): the app keeps the word and the dot as today (`app/src/screens/settings/pages/agents/parts.rs`), since today's look comes first and `settings/agentKinds.tsx:82` names this dot as the color law's one written exception.
- D64 Many reasons live only in an HTML `title` (a held act's reason, a clipped path or command, the stale word's why, a held Add, fact values): show each in a gpui-kit Tooltip with the same words and wsp's tooltip skin, because GPUI has no native title hover.
- D65 A terminal-road sign-in on a joined computer draws a live 344 px terminal in place (`components/agents/SignInTerminal.tsx:48`), and the GPUI app does the same: `SignInTerminal` attaches to the login's pty on that computer over `daemon.open` and draws it in a 344 px box that keeps Escape for the login (`app/src/screens/settings/pages/agents/terminal.rs`). On this computer the row shows the `wsp agents signin <id>` copy line, as today (`app/src/screens/settings/pages/agents.rs`).
- D66 The SKILL.md preview draws the file, frontmatter off, with gpui-kit `TextView` over `TextViewState::markdown`, and opens only http, https and mailto links, as `restrictedHref` does (`app/src/screens/settings/pages/agents/kinds/drawing.rs`). The app must draw it with images and raw HTML off, as today's restricted mode of the chat renderer does (`components/agents/SkillPreview.tsx:24`); nothing in it turns them off now. Skills and threads end on one renderer.
- D67 Agent marks (HarnessMark) and server brand marks (MarkSvg) are the app's own SVGs with their own colors, and no shared piece draws them: ship the same SVG files through `UiAssets`, because lucide has none of them.
- D68 Server icons come back from `servers.icon` as data URLs: decode them into a GPUI image, keep one answer per host for the life of the window, skip the ask while the server icons preference is off, and forget them all on the card's refresh, as `components/agents/useServerIcon.ts:36` does.
- D69 The agents report is held 5 min per computer for the whole window, shared with a task's panel and read again on `agents.changed`: keep that cache in the app's store keyed by target, not in the page, so reopening Settings asks nothing and the panel later shares it.
- D70 The Agents page's cards and tab pages are also what a task's panel draws (`components/agents/AgentsPanel.tsx`): build them as views over a level and an open callback, so the panel reuses them instead of a second copy.
- D71 A devices read that fails for any reason but a ticket socket leaves the skeleton up for good beside a notice, in today's app (`settings/devices.tsx:80`) and in the GPUI app (`app/src/screens/settings/pages/devices.rs`, `app/src/screens/settings/model.rs`, `app/src/screens/settings/reads.rs`). A fix must land in the web app and the GPUI app together, because the states are compared by screenshot.
- D72 Dragging a model's grip shows the browser's ghost of the row: use the row itself at 50% opacity as GPUI's drag view, accept drops only on shown rows of the same list, and keep ArrowUp and ArrowDown on the focused grip, since GPUI has no default drag image.
- D73 Choose an image opens the browser's file input with an image filter: use `cx.prompt_for_paths` and let the Project icon sheet refuse a file that is not an image, because the platform pickers do not all take a type filter; take drops as external paths and pastes as clipboard images on the Icon row.
- D74 Escape inside the in-place sign-in terminal belongs to the login (OpenCode's picker quits on it) while Escape anywhere else leaves Settings: the terminal's key context must consume Escape before the Settings dispatcher sees it.
- D75 The fixture's Agents tab lists whatever agents the driver machine has on PATH (here also Codex, OpenCode, Pi, Hermes and Cursor, and Codex's version line is a long warning it printed): pin the fixture's PATH to the claude stub before baselines are taken, or the web and GPUI shots compare different machines.

### Settings, Computers, Recipes, Activity

- D76 The Image recipe (`ImageRecipe`, `RecipeScreen`, `RoadChoice`, `ReadingRows`, `AgentLine`) is not ported: on main `ImageCard` is drawn only as a computer page's row (`settings/computers.tsx:667`), `useImageCard` has no caller and `openImageRecipe` only lands on the page, so nothing draws them. The GPUI app ports `ImageCard` as that IMAGE row (`app/src/screens/settings/pages/computers/image.rs`) and `ImageBuild`, the view that holds `RecipeStep` and `BuildRows`, as the build box under it (`app/src/screens/settings/pages/computers/image/build.rs`).
- D77 One grid row piece for the Settings lists: `GridRow` (`app/src/ui/grid.rs`: column template with a narrow one, 60 or 48 px floor, hover, Tab reaches a row that opens, right click through `on_menu`). Computers, Pending, Clouds, Recipes, Image and Activity draw it; the Agents list draws `Row` (`app/src/screens/settings/pages/agents.rs`).
- D78 The app must open a port from Activity in its browser pane, as today's Open opens an in-app browser tab (`settings/activity/live.tsx:262`). The app now opens `http://localhost:<port>/` in the system browser (`app/src/screens/settings/pages/activity/tables.rs`), though it has that pane (`app/src/screens/browser`) and the row reads "Open :<port> in a browser tab".
- D79 The GPUI app names its own processes in Activity: on this computer's host it calls `appProcs.offer` and, while a page watches, sends `appProcs.set` with its own pid as `main` every 2 s (`app/src/desktop/app_procs.rs`, started from `app/src/shell.rs`), as Electron's main process does today, through the two calls the host client carries.
- D80 Browser tab processes (Activity's Browser tabs kind, named off webview ids, `settings/activity/live.tsx:135`): the app must name its browser pane's pages as tab rows, as Electron names its webviews. It now sends only its own pid (`app/src/desktop/app_procs.rs`), so the host lists no tab rows, though Activity still draws the kind (`app/src/screens/settings/pages/activity/wire.rs`).
- D81 Row context menus (computer icons, Activity rows) open through the app's `ContextMenu` (`app/src/ui/context_menu.rs`; `app/src/screens/settings/pages/computers/list.rs`, `app/src/screens/settings/pages/activity/tables.rs`): an AppKit `NSMenu` on macOS and the app's own drawn menu elsewhere. Today's rows show the OS menu through `window.wsp.contextMenu` on every platform (`actions/contextMenu.ts:94`); off the Mac the app must too, as D10 says.
- D82 The end-on-an-old-daemon notice carries a Force end act: the app's toast (`app/src/ui/toast`, drawn as a gpui-kit Root plugin) says it as an error naming each process, with one Force end action while any of them is still listed (`app/src/screens/settings/pages/activity.rs`).
- D83 Live band and tile lines: draw with Zed's `canvas` (one filled path, no axes, no tooltip), since gpui-kit's charts bring axes and hover the web's `ChartPlot` does not draw.
- D84 Window minimum width: Electron sets none (`desktop/window.ts:129`) and the GPUI app sets none (`app/src/screens/dialogs/startup.rs`). The Computers and Activity pages are checked at 600 px, where the web drops columns and turns the tabs into a select, as the app does under 640 px (`app/src/screens/settings/pages/activity.rs`, `computers/list.rs`).
- D85 Activity with hundreds of processes: draw every row as the web does, no virtual list, since groups fold and the web never windows them; revisit only if a measured frame drops.
- D86 Stepper (`settings/computerSettings.tsx:67`): build it in `app/src/ui` as a 30 px outlined group over gpui-kit's `NumberInput` behaviour, since no shared piece draws minus, number, plus and the Limits and Threads here cards need three.

### Usage

- D87 Locked sizes: ui `Card` and `section_head` draw 13.5 px medium heads and an 11 px radius with a 12 px head gap, Usage needs 14 px regular, 14 px and 16 px. Add a `locked` switch to both rather than a Usage-only card, since Activity sets data-locked too (`settings/SettingsPage.tsx:97`).
- D88 Unboxed sections: the chart, the By table and the mix are a head over bare content (React `Card` with `body` and no children draws no surface), and ui `Card` has no body slot. Add `Card::body(el)` drawn under the head with no surface, because Appearance, Agents, Devices and Recipes use `body` the same way.
- D89 Chart drawing: gpui-kit `LineChart` is one series and `AreaChart` fills gradients, both draw their own axes and an appear animation. Build the chart as one custom element on gpui `canvas`: polylines and the dashed grid with `PathBuilder::stroke(..).dash_array(&[2px, 4px])` and `paint_path`, the dither as 1.4 px `paint_quad` cells on a 2 px grid (vendored gpui-pre `examples/painting.rs`), because no kit chart draws the Bayer dither or several scaled lines.
- D90 Hover tooltips: today's opens at once above the hovered step's column (side top, 6 px off, not at the cursor), lists the date in 12 px mono and only lines over 0, with one dot on the lead line and a full-height cross line at foreground 30%. gpui-kit `plot::Tooltip` follows the cursor, glides and dots every series with a halo, so it does not match: draw our own popover in the `shortcut_tooltip` skin anchored to the step, keeping only the hit-testing idea of gpui-kit's hitbox.
- D91 Dither cost: today repaints the whole dither on every frame of the 520 ms tween, thousands of cells per line at 1080 px. The GPUI chart rasterises the dither at device scale into one image and paints that (`app/src/ui/chart.rs`), keyed on the frame's points, the size, the scale and the inks, so a still chart reuses it and each tween frame rasterises it again; a quad per cell cost 9 ms a frame.
- D92 Chart inks: lines take the agent's mark ink from the catalog (light and dark), else Tailwind sky, violet, amber, pink and teal 500 by rank (`settings/usage.tsx:433`); the token table names `--primary` for the chart line. The GPUI app reads agent inks off the catalog, mixed by rank, and takes the five split inks from each theme's project hues blue, violet, amber, pink and teal rather than from `split-1` to `split-5` tokens (`app/src/screens/usage/model.rs`), so the view holds no literal colour.
- D93 Segmented control: no ui piece; gpui-kit's `TabBar` Segmented variant is 28 px with its own fill. Add `Segmented` to controls.rs (36 px here, 1 px border, 2 px inner pad, accent fill on the pick, arrows move the pick), because General, Agents, Activity and Usage all use it.
- D94 Rolling figures: no ui piece. Build `DigitRoll` (a clipped 0 to 9 column per changed digit, 500 ms, ended by a 600 ms timer), because every figure on the page rolls on load and a plain swap would show in every comparison.
- D95 Skeleton: gpui-kit `Skeleton` pulses opacity over 2 s; today's sweeps a highlight across a border-ink bar. Draw our own sweep (a gradient quad moved by an animation) in ui/status.rs, since it is small and the loading state is screenshotted.
- D96 Use reset dialog: ui `Dialog` is the 560 px steps dialog; today's is a 440 px alert (title, one question, Cancel and Use reset, Escape blocked while spending). Give `Dialog` a width and no step count rather than a new piece.
- D97 Re-reads: today holds each answer 60 s across visits keyed by range and split, re-reads every 60 s while open and on window focus once 60 s passed, and keeps the last figures while a new range reads. Port that as an app-level cache, because a reread per tab switch would flash skeletons that today never shows.
- D98 Fixture reach: every Limits state needs a turn that prints a `rate_limit_event`, and loading and refusals need a held or refused reply. The parity stub plays a `rate-limit` reply part with `fiveHour`, `sevenDay` and `resetsAt` (`parity/claude-stub.mjs`), which the Limits states use. The driver has no `+hold <op>` or `+refuse <op>`, so the loading and refusal states have no baseline.
- D99 Computers list band: `places.readings` and the place readings watch go through the computer's daemon, which the fixture lacks (`wsp-daemon binary missing` in its log), so the band is absent there. Leave it to the Computers screen, which needs the daemon for its own shots.
- D100 `cost.history`: the host client has it (`host-client/src/ops/usage.rs`) and the web app has a wrapper (`protocol/client.ts:1328`) but no caller. The GPUI app builds nothing on it, because nothing draws it today.
- D101 Reduced motion: today stops the tween, rolls, settle-in, meters, spin and skeleton under prefers-reduced-motion. Read the same system setting gpui-kit's chart appear reads and stop the same set, because today's app honours it.

### Sidebar

- D102 Many threads take a virtual list: GPUI `list` with a `ListState` over a flat row model, `Line` (`app/src/screens/sidebar/mod.rs`, `app/src/screens/sidebar/model/list.rs`), not `uniform_list`, with tiles of 52, slim rows of 36 and heads with their 12 px gap, and Settled's 25-row and Finished's 20-row pages (`app/src/screens/sidebar/model.rs`); today mounts every live tile (memoized, measured at 400) and GPUI rebuilds the whole view on each notify.
- D103 The crab: each crab is its own `CrabView`, drawn cached at its box so a pose redraws the crab alone, and asks for its next pose from its paint only while that paint lands in the window's visible area, at most 30 times a second on a pose grid every crab shares, so all drawn crabs step in one frame (`app/src/ui/status.rs`). Every crab holds still 150 ms after a scroll and draws one frame under reduced motion (`Still`). There is no single shared frame clock, and no crab asks `request_animation_frame` (`components/status/Crab.tsx:58`, `lib/frames.ts:18`).
- D104 The status mark's tick: each row's running time and each resting age is its own small `Age` view, drawn cached in the box its row gives it, with its own timer that wakes when its text next changes: each second under a minute for a running time, the next minute, hour or day for an age (`app/src/screens/sidebar/age.rs`). Ages do not share one minute clock. Today's text writes skip React entirely (`components/status/WorkingSince.tsx:28`); here a tick does not re-render the sidebar, unless a fitted time changes width and draws its row again.
- D105 Text in flex rows rebuilt every frame (the wsp-gpui skill's trap): the crab and the row times stand out of the sidebar view, each its own view drawn cached (`app/src/ui/status.rs`, `app/src/screens/sidebar/age.rs`), so their frames do not rebuild the sidebar. Each tile's title and "project @ computer" are `flex_1` text that truncates (`app/src/screens/sidebar/tile.rs`), not widths computed from the sidebar width with `flex_none`.
- D106 Context menu: the app's own `ContextMenu` (`app/src/ui/context_menu.rs`), not gpui-kit `PopupMenu`. Off the Mac it draws a port of the web's in-app menu, with the registry's groups as rules, refusals as dimmed rows with their refusal on hover, chords at the right, and the web's arrows, Home, End, Enter, Space, Escape and Tab; on macOS the same rows go to an AppKit `NSMenu` (`app/src/ui/context_menu/mac.rs`). Today's desktop app shows the native menu through the Electron preload (`window contextMenu`, `actions/contextMenu.ts:94`); off the Mac the app must too, as D10 says.
- D107 Keyboard focus over the virtual list: one focus handle on the sidebar column, and the focused row kept as an index into the drawn rows that each rebuild finds again by the row's identity string, written in the app's own grammar (`t:<thread>:<inbox>`, `h:<section>`, `g:<attempt>`, `f:`, `s:`, `m:`, `w:`, `creation:`, `forward:`) rather than today's (`thread:`, `inbox:thread:`, `section:`, `settled`, `more:`, `finished:`, `subagent:`); a key walk scrolls the row into view, and unmounted rows hold no handles (`app/src/screens/sidebar/keys.rs`, `app/src/screens/sidebar/mod.rs`).
- D108 A key for the menu on a focused row: the app must open a focused row's menu under the row on Shift+F10 and on the Menu key, as `anchorOf` does when the browser sends a keyboard contextmenu (`actions/contextMenu.ts:58`); the sidebar's keys (`app/src/screens/sidebar/keys.rs`) bind neither yet, and the menu opens only at the pointer on a right click.
- D109 Tile card: the app's own one-at-a-time hover card (`app/src/ui/hover_card.rs`), not gpui-kit `HoverCard` or `Tooltip`, in the tooltip skin (popover ground, border, tooltip shadow, 13 px, 288 px wide at most). It opens after the pointer rests 450 ms on a tile, closes the moment the pointer is off both tile and card, keeps Base UI's 400 ms instant phase so the next card opens at once, and a pass down the list opens none (`app/src/screens/sidebar/card.rs`). The app must fade and slide a card in over 150 ms on a first open, as today's tooltip does (`components/ui/tooltip.tsx:46`); the app's card draws no fade now.
- D110 Drag preview and timing: the source tile stands at 50% with its tree hidden, and the empty heads (Pinned, Needs you, Settled) show at drag start, the rebuild keeping the top row in place as today's scroll compensation does (`app/src/screens/sidebar/drag.rs`, `app/src/screens/sidebar/model/list.rs`). The app must show a copy of the tile at 50% as the drag view, as today (`sidebar/treeDrag.ts:129`); it now shows the tile's title in a small popover pill (`app/src/screens/sidebar/tile.rs`).
- D111 Moves, pins and folds before the host answers: the app must hold each until `sessions.list` rows carry the value or the call fails, as `heldKeys` does (`protocol/store/useStore.ts:749`), so a dropped tile does not jump back. A drop or a move now writes its marks onto local copies of the rows, which the next snapshot replaces whole, and pins and folds show only once the host's rows carry them (`app/src/screens/sidebar/drag.rs`, `app/src/screens/sidebar/mod.rs`).
- D112 Folds and picks live in the app's local store, not `preferences.set`, as today's localStorage keys `wsp:sidebar-folded`, `wsp:sidebar-project` and `wsp:sidebar-computer` do. The app must keep folds per window (`sidebar/WorkspaceSidebar.tsx:238`) and keep the project and computer picks across launches (`sidebar/picks.ts:42`). It now keeps `wsp:sidebar-folded` in one store file the whole app shares (`app/src/local_store.rs`, `app/src/screens/sidebar/model/list.rs`) and holds the picks in memory only (`app/src/screens/sidebar/switcher.rs`).
- D113 `mod+1` to `mod+9` must follow the store's workspace order, not the tiles' drawn order, as today (`shell/shellCommands.ts:297`); it looks like a bug but it is today's behaviour. The app now opens the Nth distinct workspace in the thread list read once from `sessions.list` at connect (`app/src/shell/actions.rs`, `app/src/shell.rs`).
- D114 The tree logic is the app's, not the host's: `threadTree.ts`, `Sidebar.logic.ts`, `treeOrder.ts`, `treeMoves.ts` and `components/threads/leadTree.ts` are ported to Rust with each rule named beside its source (`app/src/screens/sidebar/model.rs`, `model/list.rs`, `model/kinds.rs`, `walk.rs`, `keys.rs`); the host sends rows, and the sections, inbox and settle-by-time are the app's. The tests are the app's own, 19 unit tests in `model/tests.rs` plus GPUI tests in `sidebar/tests/`, not a case-for-case copy of `threadTree.test.ts` (43 cases) and `workspaceRows.test.ts` (7).
- D115 A click on a tile opens the thread in the thread view and sends no `sessions.read` itself; the thread view stamps the read once the first page has been drawn and again whenever a turn ends, only while the window is active, as today's read stamp belongs to the thread view (`components/chat/useReadStamp.ts:17`), so Done stays until a thread is really shown (`app/src/thread_view.rs`).
- D116 Warming: the app sends no `sessions.head`; the thread view reads ahead the newest `sessions.history` page (200 events) of each warm thread, the three threads shown last and those two Ctrl+Tab steps either way from the open one, seven at most, and drops a page once its thread leaves that set (`app/src/screens/warm.rs`, `app/src/thread_view/ahead.rs`, `app/src/thread_view.rs`). Today's app warms tiles' heads with `sessions.head`, two at a time (`components/chat/transcripts.ts`); neither draws anything in the sidebar.
- D117 Every sidebar menu entry runs: Snooze opens its dialog, Delete thread its confirmation, Keep this one and Delete copies the dialog that ends those workspaces, Export project and Add a project their dialogs, and Forget thread forgets at once as today's does (`actions/threadActions.ts:310`); an entry is drawn refused only for a real reason, with its hover line (`app/src/screens/sidebar/menu.rs`, `app/src/screens/sidebar/dialogs.rs`, `app/src/shell.rs`).
- D118 The Search row is live and opens the command palette, as the New thread row beside it is live and starts a thread (`app/src/screens/sidebar/mod.rs`, `app/src/shell.rs`).
- D119 `StatusMark` misses 8 kinds and always puts the crab in the slot: add the variants and a flag that leaves the slot to the time, in `ui/status.rs`, since Settings' computer and usage pages read the same piece.
- D120 The sidebar's 28 px head with its hairline has no piece (`section_head` is Settings' card head): add one in `ui/` for Pinned, Needs you, Settled, Setting up and Forwarded ports.
- D121 The selected tile's 1 px inset ring: draw it as a 1 px border in `--sidebar-row-edge`, which GPUI paints inside the box; GPUI shadows have no inset.
- D122 The tile card wraps greedily, since GPUI lacks today's `text-wrap: balance` (`components/ui/tooltip.tsx`); a small deliberate difference (`app/src/screens/sidebar/card.rs`).
- D123 The polite live region that says "Moved X to n of m": announce through AccessKit where the build has it, else drop it and note it; GPUI's screen reader support is young.

### Thread view

- D124 The minimap holds what the view holds, not the whole thread, as today: ticks come from the rows in memory, as the web derives them (`components/chat/timeline/list.tsx:359`), so `long` opened at its end shows the newest page's prompts and gains ticks as older pages land; ticks are spaced by index, 8 px each, on a rail capped at the window height minus 288 px (18rem; 512 px in an 800 px window), so 900 prompts would sit 0.57 px apart, and filling it means reading all 4,900 events on open, which paging and the memory gate exist to avoid (`app/src/screens/thread_view/minimap.rs`, `app/src/thread_view/render.rs`). The whole thread would be a deliberate difference fed by a new prompts-only host op, since none lists a thread's prompts without its events.
- D125 The transcript's list: GPUI's own `list` over a `ListState` (`FollowMode::Tail` to follow the end, 800 px overdraw, rows taken as 90 px until measured), not gpui-kit `MessageScroller`, with one row per derived timeline row; an older page is prepended by splicing the list's head and scrolling back to the row the reader was on. gpui-kit `VirtualList` is not used, because it needs every row's size up front (`v_virtual_list(view, id, item_sizes, ..)`) and a markdown row's height is known only once laid out (`app/src/thread_view.rs`).
- D126 Holding the end: every thread opens at its end; the list follows the tail and picks it up again when an event lands while the reader is within 40 px of the bottom; a change in the composer's height measures the last row again, so a followed end stays pinned; and a fold toggle pauses tail following so the fold opens downward (`app/src/thread_view.rs`). The app must still hold the end 500 ms when the transcript's foot shrinks, as `components/chat/footerHold.ts:9` does, and must keep the fold pause to today's two frames. These are the rules the web tuned against jumps (`components/chat/MessagesTimeline.logic.ts:146`, `components/chat/timeline/list.tsx:244`).
- D127 The jump button: wsp's own "Scroll to end" pill over the composer, 8 px above its top edge, fading and sliding in over 200 ms and gliding to the end on a press; the plain GPUI list has no jump button of its own to hide, and today's look comes first (`app/src/screens/thread_view/toolbar.rs`).
- D128 An older page loads when the scroll offset is under two viewport heights, again after each page lands while still there, and never before the list first settles at its end (`components/chat/timeline/list.tsx:406`); while find is open every older page loads (`app/src/thread_view/render.rs`), and a prepend keeps the reader's row by its id (`app/src/thread_view.rs`). Row ids must stay stable across a page, as today names a call group by its last call (`adapt/timeline-rows.ts:97`); the app names it by the first call of its run (`app/src/transcript/groups.rs`), so a page that adds calls ahead of a group can rename it.
- D129 Clearing the composer: the last row's foot is padded by the composer's measured height (110 px until it is first drawn) plus the list's end space, and is measured again as that height changes, so a followed end stays pinned, because the composer floats over the list on a glass ground and the last row must end above it (`app/src/screens/thread_view/rows.rs`, `app/src/screens/thread_view/mod.rs`, `app/src/thread_view.rs`).
- D130 Markdown: gpui-kit `TextView` over one `TextViewState` per reply row, made once per row id, so a new block does not re-parse the thread; `code_block_chrome` draws the block head (language or file name, Run, wrap, Copy code) and `table_chrome` the cell toggle and Copy table; `on_link_click` scrolls `#` links within the reply and sends web links to the system browser, while file chips open in Files from their own press; a `TextViewStyle` carries wsp's ladder (14 px text, h1-h4 20/18/16/14, 12 px bordered inline code) (`app/src/screens/thread_view/markdown.rs`). The app must make reply text selectable, as today's replies are (`components/chat/AssistantSelectionToolbar.tsx:90`); the transcript's TextView does not set `selectable(true)` now.
- D131 Streaming needs no token animation: claude's adapter sends whole text blocks, never partial deltas (`packages/adapter-claude/src/adapter.ts:542`), so the app hands each changed row's text to its document with `TextViewState::set_text`, with no stream fade, which matches today (`app/src/thread_view.rs`).
- D132 Code blocks are TextView's own fenced blocks. They are colored by gpui-kit's tree-sitter highlighter, set as the view's shared `code_block_highlighter`, and framed by `CodeBlockChrome` (`app/src/screens/thread_view/markdown.rs`, `markdown/chrome.rs`). No block gets its own `Input` code editor, because an editor entity per block across a 900-turn list costs too much memory, and a selection must run from prose into code as it does on the web.
- D133 Syntax colors: `scripts/pierre-themes.mjs` maps shiki's pierre-dark and pierre-light token colors onto each theme's highlight block, `app/src/ui/syntax.rs` swaps in queries that take them, and `app/tests/pr_highlight.rs` compares every token of ts, tsx, js, rs, json, css, md, sh and py samples with shiki's colors. The app must give each token the color shiki gives it today (`components/markdown/codeBlocks.tsx:207`); the tokens that still differ, since tree-sitter captures are not shiki's TextMate scopes, are listed in `app/tests/pr_highlight/differences.tsv`.
- D134 A code block unwraps. Blocks open wrapped (wordWrap true), and each block keeps its own wrap state. The toggle turns wrapping off, and the block's lines then scroll sideways inside it on a 7 px scrollbar, through TextView's `CodeBlockChrome::wrap` (`app/src/screens/thread_view/markdown/chrome.rs`).
- D135 Math is typeset in the app. Plugins on gpui-kit's markdown hook set `$..$` inline and `$$..$$` as a centred display block, in KaTeX's fonts and metrics by TeX's appendix G rules (`app/src/screens/thread_view/math.rs`), because gpui-kit has no math renderer of its own. A formula outside what this typesetter handles draws as its TeX source, in KaTeX_Main.
- D136 A mermaid fence draws as the code block it is, labelled "mermaid", with no syntax colors (`app/src/screens/thread_view/markdown/chrome.rs`). That is what the web shows while streaming and when a diagram fails to parse. This is a deliberate difference, because nothing on GPUI draws mermaid.
- D137 Find over a virtual list searches the entries' text, never drawn rows, as the web does (`components/chat/find/text.ts`). It reads every row with every fold open, reads a row again only when it changed, and pages in the whole thread while the bar is open (`app/src/screens/thread_view/find.rs`, `app/src/thread_view/render.rs`). The search runs on the UI thread with no time cap, since Rust's regex runs in time linear in the text. A step opens the turn fold, call group, call or row body that hides the match, scrolls the row to 40% of the view, then paints; marks go on mounted rows through `TextViewState::set_range_highlights` at primary 20% and 45%, set again as rows come into view. The app must start from the newest match, as today (`components/chat/find/store.ts:74`); it now counts outward from the row the reader last sees.
- D138 Find marks outside markdown are `StyledText` highlights on plain parts: call labels, commands, output, call bodies, work rows and subagent lines (`app/src/screens/thread_view/find.rs`, `rows/calls.rs`, `rows/block.rs`, `cards/subagent_fold.rs`). The app must also count and paint matches inside an edit's hunks, as the web paints inside the diff's shadow root (`components/chat/find/highlights.ts:14`); it now reads only an edit's label.
- D139 mod+f reaches find from the composer. thread.find is bound in the window root's `ThreadOpen` context, and findOlder and findNewer in the `ThreadFind` context of the thread screen's element, which wraps the composer's input (`app/src/screens/app_shell/keys.rs`, `app/src/shell.rs`, `app/src/screens/thread_view/mod.rs`). Enter, Shift+Enter, Esc and Alt+C/W/R (Cmd+Alt on the Mac) stay the find field's own actions (`app/src/screens/thread_view/find.rs`), because GPUI matches bindings before an input sees the key.
- D140 The web lets a drag cross rows and Select all take the whole transcript (`components/chat/timeline/list.tsx:483`). In the GPUI app each row's TextView holds its own selection and Quote works on one reply's selection (`app/src/screens/thread_view/toolbar.rs`). Cross-row selection is a deliberate difference, because a selection layer over a virtual list is its own build.
- D141 The Quote button stands where the pointer released the selection, held inside the transcript's box and 8 px inside the window, as `resolveSelectionActionPosition` places it when the web has a pointer (`app/src/screens/thread_view/toolbar.rs`). TextView gives `selected_source_range` but no rectangles, so the web's fallback to the end of the selection's last line has no counterpart. A scroll since the button showed puts it away.
- D142 Elapsed counters are each a small `Clock` entity drawn cached over the rows (`app/src/screens/thread_view/rows.rs`). It wakes when its text would next change, every second and past an hour every minute, and redraws only itself, in place, only if the text moved, because a transcript that re-renders every second is the trap the wsp-gpui skill records.
- D143 The live row's shimmer is a band painted over the label: 72 px wide, a 2.2 s linear pass, and the label drawn again in the foreground through the web's mask as 4 px strips, drawn over the cached rows so it rebuilds none (`app/src/screens/thread_view/shimmer.rs`). It is hidden under reduced motion, as today.
- D144 Edit hunks are drawn by the app, not gpui-kit `Diff`: unified, wrapped, syntax colored, with a 4 px bar per changed line (solid for added, dashed for removed), each line's number on its own side, and added and removed fills at 6% of the theme's success and error colors (`app/src/screens/thread_view/rows/block.rs`). One file has no header. Several files each get a head with the path, counts and Open diff. A diff past 20 lines shows its first 16 then "+N lines", which opens to "Fewer lines". This follows the card the web draws with @pierre/diffs.
- D145 Run works as on the web. Run starts the block's command in a pty of its own, and the live run draws its terminal in a 232 px box under the block, taking keys, with Running under it and Move to terminal once it has printed for three seconds. A program that takes the whole screen moves itself to a terminal tab (`app/src/screens/thread_view/run.rs`). Ended runs show their output, how they ended and Send output to the agent, and a moved run offers Open the tab (`app/src/screens/thread_view/markdown/chrome.rs`).
- D146 Reply images: the app must fetch http and https images and show local paths as inert file chips, as today's sanitizer allows (`components/ChatMarkdown.tsx:437`); it now fetches none, drawing every image but a `data:` one as the fallback line "Image unavailable: alt" (`app/src/screens/thread_view/markdown.rs`). A person's images come from `sessions.attachment` as 56 px thumbnails and open full size in the app's dialogs layer (`app/src/screens/thread_view/cards/files.rs`), as `useSentFiles` allows today.
- D147 The title's right click opens `Rename thread` through the app's `ContextMenu` (`app/src/screens/thread_view/mod.rs`, `app/src/ui/context_menu.rs`): an AppKit `NSMenu` on macOS and the app's own drawn menu elsewhere. Electron shows the native menu through `window.wsp.contextMenu` on every platform (`actions/contextMenu.ts:94`); off the Mac the app must too, as D10 says.
- D148 The new-thread noise field is a view of its own on a GPUI `canvas`, repainting every 120 ms while on screen and drawing one still frame under reduced motion (`app/src/screens/composer/hero.rs`). Each frame shapes the 7 visible ramp glyphs once and paints every non-blank cell's glyph straight from them in one layer, because shaping text per cell per frame is the cost that sinks a ticking view. It repaints all cells each frame, not only the cells that changed.
- D149 Glass on the find bar, minimap preview and Scroll to end pill: the web blurs these only off the Mac, and its solid class takes every glass solid (`index.css:1556`), as today's Linux app always is. The app must draw them solid on a solid window. It now blurs them off the Mac whatever the glass setting (8 px on the popover fill at 96% for the bar and preview, 16 px dark or 12 px light at 95% for the pill) and draws the solid popover fill on the Mac (`app/src/screens/thread_view/find.rs`, `minimap.rs`, `toolbar.rs`).
- D150 A thread is stamped read only while the window is active, and a stamp due while it is not waits until it is (`app/src/thread_view.rs`, `app/src/thread_view/render.rs`). This stands in for the web's `document.visibilityState === "visible"` (`components/chat/useReadStamp.ts:17`), because GPUI has no page visibility.
- D151 Text sizes come from `preferences.get`'s textSize and codeSize. They are read into the TextViewStyle, the code block's font size and an edit diff's size (13 px when none is picked), and every row is measured again when they change (`app/src/screens/thread_view/markdown.rs`, `app/src/thread_view.rs`). This follows the web, which moves `--font-size-chat`, `--font-size-code` and `--diffs-font-size` from them.

### Composer

- D152 The composer's local store, and when it writes: today `wsp:composer-drafts:v1` in localStorage keeps each thread's draft (text and caret), its queued messages and the unclaimed drafts of an older build, not the files and not the held flags; zustand's persist rewrites the whole map synchronously on the UI thread on every keystroke and every caret move (`components/chat/composerDraftStore.ts:182`), and the stash writes `wsp:prompt-stash` at once (`components/chat/promptStashStore.ts:71`). The app keeps drafts, queues and stash under those same keys in one JSON file in its data folder, with memory as the truth, each write running on a background task through a temp file and a rename (`app/src/local_store.rs`). A draft is written about 300 ms after the last edit and at once on send, stash, thread switch and quit (`app/src/screens/composer/mod.rs`); only a text change starts the timer, so a caret-only move schedules nothing (`app/src/screens/composer/keys.rs`). Why: the synchronous whole-map rewrite per keystroke is the web app's input cost and needs no copying.
- D153 The store is capped at about 5 MB: today nothing caps drafts or queues and nothing prunes old threads (an emptied draft stays as a key until the next load drops it, `components/chat/composerDraftStore.ts:73`); only the browser's shared ~5 MB origin quota bounds them, and the stash's 20 entries of at most 2.7M base64 characters each were sized against that quota (`components/chat/promptStashStore.ts:14`). The app caps its file at 5 MB (`app/src/local_store.rs`). A write past the cap first drops the least recently edited drafts. It never drops the draft on screen, a queued message or a stash entry, and the write is refused only once no other draft is left. An emptied draft is removed when it is written (`app/src/screens/composer/store.rs`). Why: it keeps the stash's sizing true and keeps a file read at launch small.
- D154 The Electron app's drafts and queues are not imported on first launch; the app reads only its own store file (`app/src/local_store.rs`). Why: nobody runs the GPUI app's store yet, the app ships behind the Experimental switch, and a draft is minutes of typing.
- D155 Text editor: gpui-kit `Textarea` (`TextareaState`), not `Editor`: the code-editor mode refuses inline tokens (`check_token_mode` returns UnsupportedMode), while `Textarea` has `auto_grow(min, max)`, `.token(..)`, `on_token_hover` and `on_paste` (`app/src/screens/composer/field.rs`). The app must let the full box grow from 70 to 200 px at 14 px with line height 1.625, and the one-line box 1 to 8 lines, as today (`components/ComposerPromptEditor.tsx:154`, `components/chat/ChatComposer.tsx:1268`); it now uses `auto_grow(1, 8)` on a 22.75 px line for every layout, so the full box stops at about 182 px (`app/src/screens/composer/mod.rs`). Why: it is the one gpui-kit field with atomic tokens and growth.
- D156 Chips: each chip is an `InlineToken` (one caret step, removed whole by Backspace, which is today's behaviour) drawn by `.token(..)`. A token's text may hold no newline (`InlineToken::validate`), so a pull request or issue chip carries its first line as token text and keeps its body in a map keyed by that line, put back when the draft is read to send or saved (`app/src/screens/composer/keys.rs`, `send.rs`). The app must match today's chip: 1.41em high, 0.5em radius, border at 70%, accent fill at 40%, the path or body as a tooltip, and the file tree's file-type icon on an @file chip (`components/composerInlineChip.ts:5`, `components/chat/FileTagChip.tsx:12`); it now draws 17 px high in 12 px type with a 6 px radius, lucide's File or Folder glyph and no tooltip (`app/src/screens/composer/parts.rs`). The app must also make a terminal excerpt and a quote one-line chips with their block kept beside them, as today (`components/composerInlineChip.ts:5`); it now inserts them as plain lines (`app/src/screens/composer/mod.rs`). Why: the draft store, the queue, history recall and the transcript all speak the sent text.
- D157 Text becomes a chip on a menu pick (`app/src/screens/composer/keys.rs`). A typed `@path ` stays text, as today, where the editor rewrites only on a change from outside (`components/ComposerPromptEditor.tsx:52`). The app must also turn text into chips on a paste of a block and when a draft is loaded, as today's `splitPromptIntoComposerSegments` does (`composer-editor-mentions.ts:124`); it now puts a draft back as plain text with `set_value` and pastes a block as text (`app/src/screens/composer/mod.rs`). Why: a pasted `@decorator` in code must stay text.
- D158 Keys in the field and the IME (the wsp-gpui skill's trap): GPUI matches bindings before the IME. The composer's root carries a `Composer` key context that captures gpui-kit's input actions before the field (MoveUp and MoveDown for the menu's rows and history, Enter, Tab and Escape) and binds the stash's mod+s as `StashDraft` (`app/src/screens/composer/render.rs`, `app/src/screens/composer/mod.rs`). The app must send on Enter or mod+Enter by sendWith, as today (`components/chat/ChatComposer.tsx:1050`); it now builds the field with `submit_on_enter(true)` and plain Enter always sends. The app must hold send while the field has marked text, as today's isComposing guard does (`components/composer-editor/plugins.ts:51`); it now relies on the platform giving Enter to the IME (`app/src/screens/composer/keys.rs`). The app must bind macOS Home and End in the field, as today (`components/composer-editor/plugins.ts:102`); it binds neither now. Why: a chord that is not a bound action in the field's context never reaches it.
- D159 Type to focus, a printable key typed outside any field: the thread's screen and the New thread page handle it in `on_key_down` at their root, only while no text field has focus. The handler focuses the composer and appends the key's text after the draft itself, as `useTypeToWrite` already does for the dock (`components/chat/composerTypeToFocus.ts:42`; `app/src/screens/thread_view/mod.rs`, `app/src/screens/composer/mod.rs`). Why: a bare printable key cannot be bound, and GPUI does not hand the key to the newly focused field as the browser does.
- D160 Paste of images: `Textarea::on_paste` hands the clipboard's entries to the composer (`app/src/screens/composer/field.rs`). A `ClipboardEntry::Image` (PNG, JPEG, WebP, GIF; GPUI's macOS, X11 and Wayland clipboards read them) becomes pasted-image-N, and `ExternalPaths` are read off the window's thread; both go through the one `attach` road with today's caps (5 files, 10 MB each, refused chips with the reason, `components/chat/ChatComposer.tsx:586`; `app/src/screens/composer/attach.rs`, `app/src/screens/composer/model.rs`). An SVG, BMP or TIFF goes as a file, since the protocol's image types are the four; text of 200 lines or 32 KiB becomes pasted-text-N.txt. Files are base64 encoded on the window's thread when the send is built (`app/src/screens/composer/link.rs`). Why: paste, drop and the picker share one road and one set of refusal words today.
- D161 Files dropped on the window: the thread's screen and the New thread page take `ExternalPaths` with `on_drag_move` and `on_drop` over the whole chat, and show the frame only while a drag holds paths over it and the composer can take files (`app/src/screens/composer/drop.rs`). Drops on Wayland have no test of their own. Why: the drop covers the whole chat today (`components/chat/ChatDropZone.tsx:26`).
- D162 The tray, drawer and banner tuck 16 px under the box behind a CSS mask (`components/chat/ComposerSurface.tsx:83`, `components/chat/ComposerBanner.tsx:57`); this GPUI has no masks. Start each at the box's edge instead: the tray and drawer are inset 22 px, where the box's 22 px corner has already reached its flat edge, so no gap shows. Why: two glasses must not stack, and the result matches without a mask.
- D163 The slash menu must sit where today's does: placed from the box's bounds, inset 22 px each side, overlapping the box's top by 17 px, with a max height reaching up to 24 px under the window's top (`components/chat/ComposerCommandMenuLayer.tsx:51`). The app now draws it in `deferred(..)` as an absolute child of the composer's column, inset 22 px each side, its foot on the column's top with no overlap of the box, its list capped at 288 px (`app/src/screens/composer/menus.rs`).
- D164 File tiles and chips are drawn in the transcript too (the person's message, `components/chat/ChatFiles.tsx:79`, 12 px chips); build `FileTile` and `Chip` in `app/src/ui/`, and keep the round send and stop, the drawer row and the model menu in the composer's own module. Why: only the first two have a second user.
- D165 The dead branches of `ComposerPrimaryActions` (pending questions, Refine and Implement, send while running) are not ported; `ChatComposer` passes them constants (`components/chat/ChatComposer.tsx:1152`), and the app draws only stop while a turn runs and send otherwise (`app/src/screens/composer/parts.rs`). Why: nothing reaches them.
- D166 Typing a bracket, quote, backtick, `*` or `_` over a selection wraps it (`components/composer-editor/surround.ts:11`); gpui-kit has no such behaviour. Intercept the insert in the composer: with a selection that touches no chip and no marked text, replace it with open, selection, close and keep the selection inside. Why: it is a key behaviour a person relies on, and IME dead keys arrive as marked text.
- D167 Undo after a thread switch is lost, as today: the editor remounts per thread with its own history (`shell/WorkspaceThread.tsx:129`). The app keeps one `TextareaState` per mounted composer and reuses it across threads. A switch fills it with `set_value`, which clears gpui-kit's undo history (`app/src/screens/composer/mod.rs`). Why: parity, and a history per thread would outlive its draft.
- D168 The Edit menu's Undo, Redo, Cut, Copy, Paste and Select All (`desktop/menu.ts:26`) must dispatch gpui-kit's input actions to the focused composer. Why: Electron's roles reach the field for free; GPUI's menu does not.
- D169 Right-click in the composer shows nothing today (Electron draws no menu for a field). The app's field sets no menu of its own, so gpui-kit's `Textarea` shows its native Cut, Copy, Paste and Select All (`app/src/screens/composer/field.rs`). A small deliberate difference. Why: removing it takes code, and a desktop field has one.
- D170 Spelling: Electron's Chromium underlines misspelled words in the composer (spellcheck is on, only chips turn it off, `components/composer-editor/nodes.tsx:19`). The app does no spell checking. A deliberate difference. Why: there is no spell checker in gpui-kit or GPUI to use.

## Motion by screen

Every transition, keyframe animation and timer, one line each with its source. Tailwind's default curve, cubic-bezier(0.4, 0, 0.2, 1) over 150 ms, holds wherever a line says default.

### Frame and layout

- Sidebar gap column (collapse and open): width, 200 ms, linear, no delay. `components/ui/sidebar.tsx:291`
- Sidebar container: left, right, width, 200 ms, linear, no delay; collapsing slides it left by its own width. `components/ui/sidebar.tsx:302`
- Sidebar during a rail drag: both of the above set to 0 ms, put back on release, so the width follows the pointer once per frame. `components/ui/sidebar.tsx:444`
- Sidebar rail: all properties, 150 ms, linear, no delay (its own fill when the sidebar is collapsed); its 2 px hover line is an ::after with no transition, so it shows at once. `components/ui/sidebar.tsx:616`
- Header row: padding-left, 200 ms, linear, no delay, none under reduced motion (the toggle's room as the sidebar goes and comes). `shell/HeaderRow.tsx:15`
- Right panel sheet scrim: all properties (opacity from and to 0), 200 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay; scrim is background at 60% with a 4 px backdrop blur. `components/ui/sheet.tsx:17`
- Right panel sheet: opacity and translate, 200 ms, ease-in-out, no delay; enters from 32 px to the right and fades in, leaves the same way. `components/ui/sheet.tsx:80`
- Sidebar sheet under 768 px: the same popup, opacity and translate, 200 ms, ease-in-out, from 32 px to the left. `components/ui/sheet.tsx:78`
- Right panel drag handle line: color, background, border, 150 ms, default curve; transparent, border on hover, primary at 60% while pressed. `components/preview/RightPanelResizeHandle.tsx:31`
- Right panel width while dragged: no transition, set once per animation frame. `hooks/useResizableWidth.ts:124`
- Launcher tiles: color, background, border, 150 ms, default curve; hover and the arrow highlight both fill card-hover. `components/RightPanelTabs.tsx:275`
- Panel tabs: no transition; the hover fill and the glyph-to-X swap are instant. `components/RightPanelTabs.tsx:375`
- Add a panel menu: no open or close animation. `components/ui/menu.tsx:62`
- Ghost buttons (sidebar toggle, Add a panel, banner Reload, Open, Dismiss): color, background, border, 150 ms, default curve. `components/ui/button.tsx:22`
- Header drawer and panel toggles (ghost Toggle): box-shadow only, 150 ms, default curve; the hover fill (accent) and the pressed fill change at once. `components/ui/toggle.tsx:10`
- Tooltips (toggles, rail, tab titles): popup width, height, translate, opacity, 150 ms, default curve; opens after Base UI's 600 ms delay, nudged 2 px from its side and faded; none when a tooltip moves to the next trigger (data-instant). `components/ui/tooltip.tsx:46`
- Tooltip positioner: top, left, right, bottom, transform, 150 ms, default curve, so a tooltip slides between neighbouring triggers. `components/ui/tooltip.tsx:39`
- First run Privacy settings link: color, 150 ms, default curve. `shell/FirstRun.tsx:34`
- Starting window spinner: rotate 360 degrees, 1 s, linear, infinite. `desktop/starting.html:25`
- Theme switch: every transition and animation held at 0 s for one frame while the theme class and data-theme change. `index.css:689`, `settings/theme.ts:34`
- Banners (disconnected, sign-in), the panel opening inline and Settings opening: no motion; they appear and go in one frame.
- Switcher cards, background-color highlight, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay; the overlay itself appears in one frame 150 ms into the hold, no fade. `components/switcher/WorkspaceSwitcher.tsx:96`
- First launch glow, opacity, 400 ms, cubic-bezier(0.16, 1, 0.3, 1), no delay; fades out as the welcome leaves. `desktop/onboarding.html:31`
- First launch halo, opacity from 0 and scale from 0.7, 1.2 s, cubic-bezier(0.16, 1, 0.3, 1), 200 ms delay. `desktop/onboarding.html:38`
- First launch mark, stroke-dashoffset 22 to 0, 900 ms, cubic-bezier(0.16, 1, 0.3, 1), no delay. `desktop/onboarding.html:81`
- First launch mark's top light, flood-opacity from 0, 400 ms, cubic-bezier(0.16, 1, 0.3, 1), 700 ms delay. `desktop/onboarding.html:82`
- Get started key, opacity from 0 and translate from 12 px below, 500 ms, cubic-bezier(0.16, 1, 0.3, 1), 1 s delay. `desktop/onboarding.html:83`
- Agents screen, the same rise: head 340 ms no delay, card and hint 340 ms at 60 ms, foot 340 ms at 100 ms, same curve; none under reduced motion. `desktop/onboarding.html:84`
- First launch agent rows, background-color, 150 ms, ease, no delay; hover fill accent at 40%. `desktop/onboarding.html:95`
- First launch keys, box-shadow and background-color, 150 ms, ease, no delay. `desktop/onboarding.html:125`
- First launch theme flip with the computer, every transition held off for one frame. `desktop/onboarding.html:213`
- Terminal drawer, opens and shuts in one frame; its height follows a drag with no transition. `components/ThreadTerminalDrawer.tsx:986`

### Settings, frame and small pages

- Settings opening and closing: no motion; the settings sidebar and page swap in at once, the workspace views hidden under them. `shell/AppShell.tsx:141`
- Sidebar sub list and section trail, opening: height 0 to auto and opacity 0 to 1, 140 ms, cubic-bezier(0.23, 1, 0.32, 1), no delay, fill backwards; none under reduced motion. `settings/SettingsSidebar.tsx:33` (token `index.css:186`, keyframe `index.css:268`)
- Sheet search result, after a tap: background color-mix(foreground 9%) to transparent, 900 ms, ease-out, no delay (Web Animations); skipped under reduced motion. `settings/SettingsSidebar.tsx:88`
- Trail row, press: scrollIntoView smooth, the browser's own duration and curve; instant under reduced motion. `settings/SettingsSidebar.tsx:77`
- Trail row ink as its card enters or leaves view: color, background, border, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `settings/SettingsSidebar.tsx:226`
- Group rows, sub rows, sheet result rows (hover, lift): background-color and color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `sidebar/rowGrammar.ts:11`
- Back row hover: color, background, border, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `sidebar/rowGrammar.ts:74`
- 2 text links (group crumb, search page group head) hover: color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `settings/SettingsCrumbs.tsx:62`
- Buttons (Restore defaults, row reset arrow, crumb back chevron, Check for updates, What's new, update step, Reset chord, Open logs): color, background-color, border-color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/ui/button.tsx:22`
- Tooltips (Restore defaults, row reset arrow): opacity 0 to 1 with a 2 px slide from the trigger side, width and height, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); 0 ms when instant. `components/ui/tooltip.tsx:46`
- Switch track: background-color and box-shadow, 200 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/ui/switch.tsx:12`
- Switch thumb: translate 150 ms, border-radius 150 ms, transform-origin 150 ms, scale 100 ms after a 100 ms delay, curve ease (the shorthand names none). `components/ui/switch.tsx:20`
- Segmented control segment (Send with, While a thread works): color, background, border, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/ui/segmented-control.tsx:49`
- Select trigger: color, box-shadow, background-color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); its popup opens and closes with no motion. `components/ui/select.tsx:17`
- Mode tiles and theme cards: box-shadow (the picked ring) and border-color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `settings/ThemePicker.tsx:49`
- Mode tile label hover and pick: color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `settings/ThemePicker.tsx:70`
- Theme switch and theme preview on hover: every transition and animation held at 0 s for one frame, so the window changes in one paint. `settings/theme.ts:34` (preview `settings/ThemePicker.tsx:141`, rule `index.css:689`)
- Keycaps button (hover, listening fill): color, background, border, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `settings/ChordKeys.tsx:85`
- Row that opens a page, and grid row, hover: background, 150 ms, cubic-bezier(0.4, 0, 0.2, 1) (row grammar; none on these four pages). `settings/rows.tsx:125`, `settings/grid.tsx:68`
- Settings sidebar as a sheet under 768 px: backdrop opacity 200 ms cubic-bezier(0.4, 0, 0.2, 1); panel opacity and a 32 px slide from the left, 200 ms, ease-in-out (cubic-bezier(0.4, 0, 0.2, 1)). `components/ui/sheet.tsx:17`, `components/ui/sheet.tsx:72`
- Unused: keyframes settings-search-target-pulse (650 ms ease-in-out, twice) match nothing, since no element carries data-settings-page-scroll; do not port. `index.css:640`
- Settings page scrollbar, every page: opacity 0 to 1, 100 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay while hovered or scrolling; back to 0 over 150 ms after a 300 ms delay. `settings/SettingsPage.tsx:99` (`components/ui/scroll-area.tsx:81`)
- Its thumb, hover: background-color 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/scroll-area.tsx:89`
- Type sample's Copy code: its check stands 1,200 ms (a timer, no transition); the head's buttons ride the button colour transition. `components/markdown/codeBlocks.tsx:186`

### Settings, Agents, Projects, Devices, Account

- Rows that open a page (agent rows, kind rows, found rows, project rows), hover ground to accent at 60%: background 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `settings/rows.tsx:125`
- Fact line copy glyph, shown while the line is hovered or the glyph has focus: opacity 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay; its check then stands 1,400 ms (a timer, no transition). `settings/agentKinds.tsx:65`
- Fact link (homepage, repo, marketplace), underline on hover: color, text-decoration-color 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay; the underline itself appears at once. `settings/agentKinds.tsx:476`
- Card head refresh glyph while a read runs: rotate 360 degrees over 1 s, linear, infinite; none under reduced motion. `settings/agentKinds.tsx:184`
- Spinner (busy act buttons, Add server while adding, sign-in waits, a busy step mark): rotate 360 degrees over 1 s, linear, infinite. `components/ui/spinner.tsx:9`
- Skeleton bars (the reading row, How it runs while reading, the found list, SKILL.md while read): a highlight band travels translateX -100% to 100% in the first 1.2 s of each 2 s, then holds off the bar, linear, infinite; the band is dropped under reduced motion. `components/ui/skeleton.tsx:12`, keyframes `index.css:274`
- Switch track (agent on, model shown, server, skill and plugin switches, choice rows): background-color, box-shadow 200 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/switch.tsx:12`
- Switch thumb: translate 150 ms, border-radius 150 ms, transform-origin 150 ms, scale 100 ms after a 100 ms delay (widens to 110% while pressed), all on CSS `ease`. `components/ui/switch.tsx:20`
- Segmented control segments (Agents tabs, Access, Reached by): color, background 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/segmented-control.tsx:49`
- Buttons (act buttons, Add, Revoke, Remove, Open, Change, Edit, icon buttons): color, background, border 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay; the red of a danger button on hover rides the same transition. `components/ui/button.tsx:22`
- Select triggers (computer, default agent, model, effort, project picks, icon, colour, add-server agent and where): color, box-shadow, background 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. The popup opens and closes with no animation, and item highlight has none. `components/ui/select.tsx:17`
- Field frames (model id, add-server fields, the sign-in key field): border-color 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/input.tsx:63`
- Search fields (kind search, add search): border-color, box-shadow 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/input-group.tsx:13`
- The input element itself: background-color over 5,000,000 s, ease-in-out, so autofill never paints its ground. `components/ui/input.tsx:23`
- Tooltips (status reasons, refresh, update arrow, step marks, reset arrow): open after Base UI's default 600 ms delay, close at once; opacity 0 to 1 and a 2 px slide from the trigger side over 150 ms, cubic-bezier(0.4, 0, 0.2, 1); width and height the same while the text changes. `components/ui/tooltip.tsx:46`
- Tooltip positioner: top, left, right, bottom, transform 150 ms, cubic-bezier(0.4, 0, 0.2, 1), none when instant. `components/ui/tooltip.tsx:39`
- Model row while dragged: opacity 50%, no transition. `settings/agentPage.tsx:184`
- Icon row while an image file is held over it: ground to accent, no transition. `settings/rows.tsx:272`
- No view transitions and no motion/react animations on these pages.

### Settings, Computers, Recipes, Activity

- List rows on the Computers, Pending, Clouds, Recipes, Image and Activity lists, and the Activity group heads: background to accent at 60% on hover, 150 ms, default curve, no delay. `settings/grid.tsx:68`
- Rows that open a page (What runs here, Agents on this computer): background to accent at 60% on hover, 150 ms, default curve, no delay. `settings/rows.tsx:125`
- Activity process, subagent, container and Task manager rows: background to accent at 60% on hover, 150 ms, default curve, no delay. `settings/activity/tables.tsx:93`
- Activity row menu button (and a port row's Open): opacity 0 to 1 while the row is hovered or holds focus, 150 ms, default curve, no delay; always shown under 768 px. `settings/activity/tables.tsx:97` (Open: `settings/activity/tables.tsx:95`)
- Activity menu button itself: color and background on hover, 150 ms, default curve, no delay. `settings/activity/tables.tsx:98`
- Activity sortable column words: color to foreground on hover, 150 ms, default curve, no delay. `settings/activity/tables.tsx:127`
- Activity group head chevron: transform rotate 0 to 90 deg when folded open, 150 ms, default curve, no delay. `settings/activity/tables.tsx:364`
- Activity page body: `settle-in` keyframes (from opacity 0 and translateY 3 px), 180 ms, cubic-bezier(0.23, 1, 0.32, 1), fill both, no delay; off under reduced motion; plays again when the computer select changes, since the body is keyed by computer. `settings/activity/page.tsx:171` (keyframes `index.css:262`)
- Activity thread head's crab: drawn still (first frame) on this page. `settings/activity/tables.tsx:440`
- Setup log step chevron: transform rotate 0 to 90 deg when open, 150 ms, default curve, no delay; the step button's hover background 150 ms. `settings/add/StepRow.tsx:87` (hover `settings/add/StepRow.tsx:97`)
- Setup log step time and build stage elapsed: text rewritten once a second, no transition. `settings/add/StepRow.tsx:35`, `settings/recipe/BuildRows.tsx:177`
- Build stage chevron: transform rotate -90 to 0 deg when open, 150 ms, default curve, no delay; the stage row's hover background has no transition. `settings/recipe/BuildRows.tsx:212`
- Build progress line under the stages card's top edge: width, 300 ms, cubic-bezier(0.16, 1, 0.3, 1), no delay; none under reduced motion. `settings/recipe/BuildRows.tsx:83`
- Build step's destructive links (Cancel): color to the destructive ink on hover and focus, 150 ms, default curve, no delay. `settings/recipe/RecipeStep.tsx:93`
- Skeletons (Activity tiles and rows while the first frame is out): an ::after highlight sweeping translateX -100% to 100% over the first 60% of 2 s, linear, infinite; removed under reduced motion. `components/ui/skeleton.tsx:12` (keyframes `index.css:274`)
- Spinners (Image row while busy, a state mark that is working, a build or setup step running, an Activity row being ended): rotate one turn, 1 s, linear, infinite. `components/ui/spinner.tsx:9`
- Switches (Agents may start agents, Every process): track background and box-shadow 200 ms, default curve; thumb translate 150 ms, border-radius 150 ms, press scale 100 ms after a 100 ms delay. `components/ui/switch.tsx:12` (thumb `components/ui/switch.tsx:20`)
- Activity view tabs (segmented): color and background, 150 ms, default curve, no delay. `components/ui/segmented-control.tsx:49`
- Buttons (Add, Update, Remove, Sign in, Copy, Retry, Delete): color, background and border, 150 ms, default curve, no delay. `components/ui/button.tsx:22`
- Select triggers (recipe, spans, size, Activity's three): color, box-shadow and background, 150 ms, default curve, no delay; the popup has no open animation. `components/ui/select.tsx:17`
- Reset arrow's tooltip: opacity 0 to 1 and a 2 px slide from its side on open and close, 150 ms, default curve; instant when it opens straight after another. `components/ui/tooltip.tsx:46`
- Live band and Activity tile lines: no animation; each new sample redraws the plot. `settings/LiveBand.tsx:34`, `settings/activity/page.tsx:275`
- Cloud limit band bars: width, 200 ms, cubic-bezier(0.23, 1, 0.32, 1), no delay; none under reduced motion. `settings/LimitBand.tsx:70`
- Recipe delete confirm, Activity ask dialog, Name and SSH login sheets: backdrop opacity 0 to 1 and back, 200 ms, default curve, no delay. `components/ui/dialog-styles.ts:3`
- Same dialogs' popup: scale 0.98 to 1 and opacity 0 to 1 on open, reversed on close, 200 ms, ease-in-out, no delay; under 640 px it also rises 16 px. `components/ui/dialog-styles.ts:6` (sheet `components/ui/dialog-styles.ts:9`)
- State mark tooltips (list state cells, pending rows, setup steps): as the reset arrow's tooltip, opacity and a 2 px slide, 150 ms, default curve. `components/status/StateMark.tsx:54`
- Recipe pick checkboxes: box-shadow, 150 ms, default curve, no delay. `components/ui/checkbox.tsx:16`
- Fields (Activity filter, recipe Name, sign-in code, sheet fields): border color, 150 ms, default curve; Spend per day's group: border and box-shadow, 150 ms. `components/ui/input.tsx:63` (group `components/ui/input-group.tsx:13`)

### Usage

- Page body on load and on each tab switch (keyed by tab): settle-in, opacity 0 to 1 and translateY 3 px to 0, 180 ms, cubic-bezier(0.23, 1, 0.32, 1), fill both, no delay. `settings/usage.tsx:125`, keyframes `index.css:262`
- Table groups (each split row with its models): settle-in as above, delay min(row, 6) x 30 ms, so 0 to 180 ms. `settings/usage.tsx:773`
- Chart lines and y scale: JS tween on requestAnimationFrame, 520 ms, ease-out cubic (1 - (1 - k)^3), from the last drawn frame; on mount every line rises from 0; a new line rises from the baseline; a range of another length is read off the old one at the same share. `settings/usageChart.tsx:75`, step `settings/usageChart.tsx:98`
- Chart dither: repainted on every tween frame, on resize and on a theme flip; no animation of its own. `settings/usageChart.tsx:196`
- Chart lead dot and hover cross line: no transition; the dot follows the tween frame and jumps to the hovered step. `settings/usageChart.tsx:244`
- Rolling figures (3 places: totals figures `settings/usage.tsx:487`, pool "% left" `settings/usage.tsx:196`, chart y figures `settings/usageChart.tsx:215`): each changed digit is a 0 to 9 column moved by translateY, 500 ms, cubic-bezier(0.23, 1, 0.32, 1), no delay; on mount they roll in from zeros; a timer ends the roll at 600 ms; still when the window is hidden. `components/ui/digit-roll.tsx:56`
- Pool meter fill: width, 200 ms, cubic-bezier(0.23, 1, 0.32, 1). `settings/usage.tsx:207`
- Table share bar: width, 500 ms, cubic-bezier(0.23, 1, 0.32, 1). `settings/usage.tsx:566`
- Mix bar segments: no transition. `settings/usage.tsx:532`
- Refresh glyph while reading: spin, 1 s, linear, infinite (Tailwind animate-spin). `settings/usage.tsx:65`
- Skeleton bars (both tabs' loading): a highlight band on ::after, translateX -100% to 100% over the first 60% of 2 s, then parked, linear, infinite. `components/ui/skeleton.tsx:12`, keyframes `index.css:274`
- 3 segmented controls (tabs, range, split): color and background, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/ui/segmented-control.tsx:49`
- Buttons (refresh, Use reset, dialog buttons, the sidebar corner gauge): color, background, border, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/ui/button.tsx:22`
- Tooltips (refresh, chart steps, pool segments): popup opacity 0 to 1 and a 2 px slide from its side, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); positioner top, left, right, bottom, transform 150 ms; 0 ms where base-ui marks it instant. Chart and segment tooltips open with 0 ms delay. `components/ui/tooltip.tsx:46`, positioner `components/ui/tooltip.tsx:39`, delay `settings/usageChart.tsx:250`
- Use reset dialog: popup scale 0.98 to 1 and opacity, 200 ms, ease-in-out (cubic-bezier(0.4, 0, 0.2, 1)); backdrop opacity, 200 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/ui/dialog-styles.ts:6`, backdrop `components/ui/dialog-styles.ts:3`
- Outside Usage, Computers list limit band bars: width, 200 ms, cubic-bezier(0.23, 1, 0.32, 1). `settings/LimitBand.tsx:70`
- Outside Usage, Computers list live band: never animates. `settings/LiveBand.tsx:34`
- Theme switch: every transition is cut to 0 s while the theme changes. `index.css:689`
- Chart line gone from the new answer: vanishes at once, no fall; its legend entry and table row swap at once. `settings/usageChart.tsx:100`
- Table figures swap at once on a new answer; a split change re-keys the groups so their settle-in plays again, a range change does not. `settings/usage.tsx:773`
- Rolling figures: digits match from the right; a digit with no old digit at its place, and every non-digit (k, %, $, .), swaps at once. `components/ui/digit-roll.tsx:50`
- Reset dialog under 640 px: popup also rises 16 px (translateY 16 px to 0), 200 ms, ease-in-out, beside the 0.98 scale and fade. `components/ui/dialog-styles.ts:9`

### Sidebar

- Every sidebar row (tile, one-line row, Search, New thread, switcher heads, settled and subagent rows, Finished and "n more" rows, group head): background-color and color, 150 ms, `cubic-bezier(0.4, 0, 0.2, 1)`, no delay; replaces the kit's width/height/padding transition. `sidebar/rowGrammar.ts:11`
- Setting up card and Forwarded port rows keep the kit's own transition (width, height, padding, 150 ms, default curve), so their hover fill steps at once. `components/ui/sidebar.tsx:813`
- Section head (Pinned, Needs you, Settled, Setting up, Forwarded ports): color, background-color, border-color, 150 ms, default curve. `sidebar/SectionRow.tsx:39`
- Chevrons rotate by transform, 150 ms, default curve: section head -90 deg folded `sidebar/SectionRow.tsx:45`, switcher head 180 deg open `sidebar/NounSwitcher.tsx:188`, group head -90 deg `sidebar/WorkspaceSidebar.tsx:149`, Finished fold -90 deg `sidebar/LeadTree.tsx:115`.
- Drop target under a dragged tree (section or Settled row): background-color to `--sidebar-row-hover`, 150 ms, default curve. `sidebar/WorkspaceSidebar.tsx:163`
- Tile fold control (open state): opacity 0 to 1 on row hover or focus, 150 ms, default curve. `sidebar/ThreadTile.tsx:163`
- Slot acts, Settle and Stop on tiles and subagent rows: opacity 0 to 1 on row hover or focus, 150 ms, default curve. `sidebar/rowGrammar.ts:48`
- Picked project's plus: opacity snaps (it keeps the kit's transform transition, not opacity). `sidebar/ProjectSwitcher.tsx:69`
- Status slot giving way to an act: visibility, instant. `sidebar/rowGrammar.ts:43`
- Stop armed: the glyph swaps for the word at once; it lapses after 2000 ms (`ARM_MS`), no animation. `sidebar/StopAct.tsx:51`
- Rename box: border-color to `--ring` on focus, 150 ms, default curve. `sidebar/RowNameInput.tsx:58`
- Switcher menu open: opacity 0 to 1 and translate from -2 px y, 150 ms, default curve; the glass variant's scale 0.98 start is cancelled to 1; close has no ending style, so it goes at once. `sidebar/NounSwitcher.tsx:196`
- Switcher menu rows: keyed highlight steps at once; gear button color and background 150 ms, default curve. `sidebar/NounSwitcher.tsx:246`
- Every tooltip and the tile and subagent cards: opacity 0 to 1 and a 2 px slide from the side it opens on (cards slide from the left), 150 ms, default curve, in and out; the positioner moves top/left/transform over 150 ms. `components/ui/tooltip.tsx:46`
- Card open delay: 450 ms for tile and subagent cards `sidebar/ThreadTile.tsx:63`, `sidebar/SubagentRow.tsx:27`; 600 ms for every other tooltip (Base UI default). Within 400 ms of one closing, the next opens with no delay and no transition (provider's instant phase). `AppRoot.tsx:12`
- Corner, card and dismiss buttons: color, background-color, border-color, 150 ms, default curve. `components/ui/button.tsx:22`
- Crab: canvas frame per animation frame on one shared loop, only while on screen; legs at 0.003 rad/ms, claws bob `sin(0.0018 t)`, blink 160 ms every sixth 2800 ms cycle; one still frame under reduced motion. `components/status/Crab.tsx:29`
- Crab loop holds every canvas still for 150 ms after any scroll on the page. `lib/frames.ts:12`
- Working time in the slot and on the card: text rewritten each 1000 ms off one shared timer, outside React. `components/status/WorkingSince.tsx:16`
- Ages, settle-by-time and minute words: redrawn on the minute boundary, then every 60000 ms. `hooks/useNowMinute.ts:28`
- Setting up bar: width, 200 ms, `cubic-bezier(0.23, 1, 0.32, 1)`, none under reduced motion. `sidebar/SettingUpSection.tsx:82`
- Tree drag: no animation; the empty heads appear one frame after dragstart and the list scrolls by the shift so the tile stays under the pointer. `sidebar/treeDrag.ts:129`
- Top row's left padding (it follows the frame's inset): padding-left, 200 ms, linear, none under reduced motion. `shell/HeaderRow.tsx:15`
- Collapse and the narrow sheet belong to the frame: the slot's width and left move 200 ms linear. `components/ui/sidebar.tsx:302`
- The list scrolls under a hard edge: no scroll fade. `components/ui/sidebar.tsx:711`
- Tile and subagent slot acts (Settle, Stop): hover fill and ink snap; their class keeps only the opacity transition, which replaces the kit's transform one. `sidebar/rowGrammar.ts:48`
- Tile fold control: hover ink snaps; only its opacity moves, 150 ms. `sidebar/ThreadTile.tsx:163`

### Thread view

- Composer dock gliding from the centred empty stack to the bottom on the first message: bottom, 300 ms, ease-out, no delay; none under reduced motion. `components/chat/ChatView.tsx:491`
- Scroll to end pill showing and hiding: opacity 0 to 1, translate 4 px down to 0, and color on hover, 200 ms, ease-out, no delay; none under reduced motion. `components/chat/ChatView.tsx:546`
- Scroll to end click: the list scrolls to its end animated (LegendList `scrollToEnd({ animated: true })`, the browser's smooth scroll). `components/chat/ChatView.tsx:493`
- Person message meta row (time, Copy, Fork) on hover or focus within: opacity 0 to 1, 200 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/chat/timeline/rows.tsx:123`
- Reply meta row (Copy, Rewind, Fork, time) on hover of the reply: opacity 0 to 1, 200 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/chat/timeline/rows.tsx:251`
- Worked-for fold button: color 150 ms, default curve, on hover. `components/chat/timeline/rows.tsx:202`
- Working line text ("Working for", "Waiting for you", "Setting up worktree"): fades in from opacity 0 when it mounts or its key changes, 150 ms, default curve (CSS `@starting-style`); none under reduced motion. `components/chat/timeline/working.tsx:40`
- Elapsed counters (Working for, a running command, a machine wait, the side question): text replaced once a second, no transition. `components/chat/timeline/working.tsx:98`
- Live row shimmer (the Thinking row, the live call row, Setting up worktree): a 72 px masked band of the foreground ink sweeps left to right, transform translateX 0 to 100% over 2.2 s, linear, infinite, no delay; its inner copy counter-moves -100% so the text stays put; under reduced motion no animation and the band hidden. `index.css:531`, `index.css:543`, used at `components/chat/timeline/workEntry.tsx:19`
- Call group row hover fill: background, color, border 150 ms, default curve. `components/chat/timeline/workGroup.tsx:274`
- Plain call row: hover fill 150 ms, default curve (`components/chat/timeline/workEntry.tsx:208`); its head line declares opacity and translate at 200 ms, default curve (`components/chat/timeline/workEntry.tsx:217`); its chevron turns 180 degrees on open, transform 200 ms, default curve. `components/chat/timeline/workEntry.tsx:242`
- Command and edit rows: frame color, background and border 150 ms as the row opens onto the code surface (`components/chat/timeline/callBlock.tsx:74`); chevron turns 180 degrees, transform 150 ms, default curve. `components/chat/timeline/callBlock.tsx:91`
- Spinner (a running command's glyph, the find count while paging, a sub-thread start): lucide loader-circle, `animate-spin`, one turn per 1 s, linear, infinite. `components/ui/spinner.tsx:9`
- Prompt row file fold, subagent fold, markdown details: chevron turns 90 degrees, transform 150 ms, default curve (`components/chat/PermissionPromptRow.tsx:145`, `components/chat/SubagentFoldRow.tsx:70`, `components/markdown/blocks.tsx:254`); the panel's height 0 to its own, 200 ms, default curve, and back. `components/ui/collapsible.tsx:26`
- Changed files card: rows' hover fill 150 ms, default curve (`components/chat/ChangedFilesTree.tsx:196`, `:238`, `:279`); folder chevron turns 90 degrees, transform 150 ms. `components/chat/ChangedFilesTree.tsx:203`
- Sub-thread start line and its raw log toggle: color 150 ms; chevrons turn 90 degrees, transform 150 ms. `components/chat/SpawnLine.tsx:66`, `:70`, `:103`, `:104`
- Copy confirmations: the copy glyph stands as a check for 1.2 s (code block `components/markdown/codeBlocks.tsx:189`, table `components/markdown/blocks.tsx:147`) or 1.5 s (message copy `components/chat/MessageCopyButton.tsx:9`, command and output copy `components/chat/timeline/callBlock.tsx:131`, `:146`), swapped with no transition.
- A thread switch: no animation; the list it leaves stays mounted hidden until the next one paints. `components/chat/ChatView.tsx:475`
- Rail with no 48 px gutter: opacity 0 to 1 on hover or focus within, 150 ms, default curve; with the gutter it stands at 1 with no transition. `components/chat/timeline/minimap.tsx:155`
- Ticks as the pointer nears: background color and width (8, 10, 16, 24 px by distance), 150 ms, default curve; in-view ticks switch to the dark ink on scroll. `components/chat/timeline/minimap.tsx:225`
- Preview card: appears and moves with the pointer, no transition. `components/chat/timeline/minimap.tsx:248`
- Tick click: the list scrolls to the message animated (LegendList `scrollToIndex({ animated: true, viewOffset: 24 })`). `components/chat/timeline/list.tsx:724`
- Context ring button: color and background 150 ms on hover and while its card is open. `components/chat/ContextMeter.tsx:76`
- Context ring arc: stroke-dashoffset 200 ms, cubic-bezier(0.23, 1, 0.32, 1); none under reduced motion. `components/chat/ContextMeter.tsx:91`
- Context card meter fill: width 200 ms, cubic-bezier(0.23, 1, 0.32, 1); none under reduced motion. `components/chat/ContextMeter.tsx:110`
- Context card (a popover in the tooltip skin): opacity from 0 and translate 2 px from its side, 150 ms, default curve. `components/ui/popover.tsx:55`
- Every Button (copy, rewind, fork, code block acts, prompt options, Wake, Open, Quote, find buttons): color, background, border 150 ms, default curve. `components/ui/button.tsx:22`
- Tooltips (message copy, rewind, fork, code acts, URL, file path, find chords): popup opacity from 0 and translate 2 px from the side it opens on, with width and height, 150 ms, default curve; the positioner's top and left 150 ms; an instant tooltip 0 ms. `components/ui/tooltip.tsx:46`, `:39`
- Find bar toggles: color 150 ms and shadow 150 ms, default curve. `components/ui/toggle.tsx:34`, `:10`
- Find field: border color and shadow 150 ms, default curve. `components/ui/input-group.tsx:13`
- Table scroll area scrollbar: opacity to 1 in 100 ms while hovered or scrolling, back to 0 after a 300 ms delay; thumb color 150 ms. `components/ui/scroll-area.tsx:81`, `:89`
- Image dialog: backdrop opacity 200 ms, default curve; popup scale 0.98 to 1 and opacity 0 to 1, 200 ms, ease-in-out, both ways. `components/ui/dialog-styles.ts:3`, `:6`
- New thread noise field: a canvas repainted every 120 ms, the field drifting 0.018 and 0.006 cells a second; one still frame under reduced motion. `components/chat/EmptyHero.tsx:134`
- Side question crab: the crab's frame loop on the shared frame clock; one still frame under reduced motion. `components/status/Crab.tsx:90`
- Threads child and subagent rows: background on hover and while their menu is open, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/threads/ThreadRows.tsx:148`
- Threads fold rows and the head's toggle: color and background 150 ms, default curve, no delay. `components/threads/FoldRow.tsx:36`, `tree/TreeRows.tsx:162`
- Threads fold and head chevrons: rotate -90 degrees to 0 on open, transform 150 ms, default curve. `components/threads/FoldRow.tsx:45`, `tree/TreeRows.tsx:175`
- Child row acts: opacity 0 to 1 on row hover or focus within, 150 ms, default curve, md and up only; the status slot hides at once. `components/threads/ThreadRows.tsx:100`
- Shut Threads peek: the most pressing row under a bottom fade mask, static, no transition. `tree/TreeRows.tsx:217`
- Subagent card: opens after the pointer rests 450 ms, then the tooltip's own fade and slide. `components/threads/ThreadRows.tsx:312`
- Markdown file chip: background on hover, 150 ms, default curve. `components/markdown/links.tsx:32`
- Updated the slate line: color on hover, 150 ms, default curve. `slate/SlateUpdatedLine.tsx:14`
- Context card: opens on hover with 0 ms delay; closes after 150 ms while Compact stands, else at once. `components/chat/ContextMeter.tsx:74`
- Menus (Git, Open, plan actions, table copy): appear and leave with no transition. `components/ui/menu.tsx:60`
- Open split: spinner in place of the editor mark while an open waits, one turn per 1 s, linear, infinite. `files/OpenSplit.tsx:71`
- Open call group: 24 px fades at its top and bottom edges while scrolled, switched with no transition; at its end it follows new calls, no animation. `components/chat/timeline/workGroup.tsx:216`
- Subagent bar's Working for: text replaced once a second, no transition. `components/chat/SubagentBar.tsx:42`

### Composer

- Composer box (the clip around the surface): height, 180 ms, ease-out cubic-bezier(0, 0, 0.2, 1), no delay; a ResizeObserver sets the height, the first reading lands unanimated (armed a frame later); none under reduced motion. `components/chat/ChatComposer.tsx:1208`, `components/chat/composerMotion.ts:14`
- Picker group and action group when the box switches full, one line, tall: transform translate(dx, dy) to none, 180 ms, cubic-bezier(0.215, 0.61, 0.355, 1), no delay, Web Animations FLIP; skipped under 1 px of travel and under reduced motion. `components/chat/composerMotion.ts:55`
- Composer dock in a fresh thread: bottom from the centred lift to the foot when the first message goes, 300 ms, ease-out cubic-bezier(0, 0, 0.2, 1), no delay; none under reduced motion. `components/chat/ChatView.tsx:491`
- Shell glass (its ::before): background-color, 200 ms, ease-out cubic-bezier(0, 0, 0.2, 1), no delay; none under reduced motion. `components/chat/ComposerSurface.tsx:25`
- Box surface: background-color, 200 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/chat/ChatComposer.tsx:1215`
- Main surface (the glass that takes over while a banner or drawer is attached): color, background, border, 200 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/chat/ComposerSurface.tsx:60`
- Send button: all properties, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); hover scale 1.05 (none while held); fill and ink step between held and ready. `components/chat/ComposerPrimaryActions.tsx:237`
- Stop button: all properties, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); hover scale 1.05 and fill destructive 90% to 100%. `components/chat/ComposerPrimaryActions.tsx:104`
- Spinner in send (sending) and stop (stopping): rotate 360 degrees, 1 s, linear, infinite (animate-spin). `components/ui/spinner.tsx:9`
- 7 ink and fill steps at 150 ms, cubic-bezier(0.4, 0, 0.2, 1): the Stashed N word (color), drawer rows (background), model chip remove (color, background), agent rail tabs (color), model search line (border-color), ghost buttons for Attach and the three pickers (color, background, border), New thread project name (border-color). Example: `components/chat/ComposerDrawer.tsx:53`; also `components/chat/ChatComposer.tsx:1118`, `components/chat/ComposerModelChips.tsx:26`, `components/chat/ComposerModelPicker.tsx:112`, `components/chat/ComposerModelPicker.tsx:323`, `components/ui/button.tsx:22`, `shell/NewThreadPicks.tsx:48`
- Legacy models chevron: transform rotate 0 to 90 degrees, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/chat/ComposerModelPicker.tsx:358`
- File and image tile remove button: opacity 0 to 1 on the tile's hover or focus, 150 ms, cubic-bezier(0.4, 0, 0.2, 1). `components/chat/ChatFiles.tsx:24`
- Model menu popover: opacity from 0 on open, with width, height, scale and translate, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); its positioner moves on transform. `components/ui/popover.tsx:55`
- Tooltips (held send's reason, branch note, chip path, agent names): opacity 0 and 2 px toward their side, in and out, 150 ms, cubic-bezier(0.4, 0, 0.2, 1); instant between tips. `components/ui/tooltip.tsx:46`
- Stash list scrollbar: opacity in over 100 ms on hover or scroll, out over 150 ms after 300 ms. `components/ui/scroll-area.tsx:81`
- New thread hero field: a canvas noise field repainted every 120 ms while on screen, one still frame under reduced motion. `components/chat/EmptyHero.tsx:123`
- No motion: the Reasoning, Access and project menus (appear at once, `components/ui/menu.tsx:60`), the slash, @, # menu layer, the stash drawer, the drawer, the banner, the drop frame, the question dock and the bars; the caret is the platform's.
- Dock and bar buttons (Answer, Allow, Deny, Back, Edit, Cancel, Write a message, Retry, Dismiss): color, background, border, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/button.tsx:22`
- Dock fields (Other, What to do instead): border-color, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/input.tsx:63`
- Dock radios and tick boxes: box-shadow, 150 ms, cubic-bezier(0.4, 0, 0.2, 1), no delay. `components/ui/radio-group.tsx:23`, `components/ui/checkbox.tsx:16`
- Tasks bar step at work: the crab walk drawn each animation frame, one still frame under reduced motion; every running step's elapsed time rewritten once a second. `settings/add/StepRow.tsx:35`
- Setup card step at work: spinner, rotate 360 degrees, 1 s, linear, infinite. `components/status/StateMark.tsx:42`
- Resume picker, conversation open dialog, full-size image: popup scale 0.98 to 1 and opacity 0 to 1, backdrop opacity, 200 ms, cubic-bezier(0.4, 0, 0.2, 1), in and out. `components/ui/dialog-styles.ts:6`
