# Parity spec: browser pane, desktop shell and Experimental switch

What today's app draws and does on the parts the GPUI app must match in `parity/wave-3`, read off `main` at
1516a0cd5 (v0.3.8, 2026-10-17): the browser pane, what the desktop shell does around the window that the GPUI app
must do itself before it can be switched on, and the Experimental switch in today's Settings. Each item has an id that
tests and code cite.

**Reading an item.** One line: id, what, source `path:line`. Paths are under `apps/web/src/`; `desktop:` is
`apps/desktop/src/`, `protocol:`, `runtime:` and `host:` are `packages/<name>/src/`; a test path without a folder is
under `apps/desktop/test/`. GPUI paths in `exists:` are under `apps/desktop-gpui/` (`startup.rs`, `menu.rs` are
`app/src/screens/dialogs/startup.rs`, `app/src/screens/app_shell/menu.rs`); "exists" means gpui/main af1e963
already does it, so the item is checked against that code, and the parenthesis says how far it falls short. A state
marked Shot has baselines `<id>-paper.png` and `<id>-graphite.png`, made by `parity/baseline.mjs --wave 3` from `states.mjs` here;
No shot says why; Check names the test, command or OS read that proves a duty with no picture. Host operations use
the names in `apps/desktop-gpui/host-client/src/ops/`; Gap marks one the client lacks; Electron and OS calls go by
their API names. Design names the `app/src/ui/` piece that draws an element, or none yet, and each transition with
its property, duration and curve ("default curve" is `cubic-bezier(0.4, 0, 0.2, 1)`, 150 ms unless given).
`more.md` holds the items this file had no room for, in the same id sequences.

**Design rules for every part.** Each screen looks exactly like today's Electron app, compared per state in Paper
and Graphite. The GPUI app is a separate app on the same host protocol. It is turned on by a switch in today's
Settings, Experimental, and replaces today's app only at full parity. The base: gpui-kit 14f5933 on `gpui-pre` 0.3.9,
with our blur and rounded clipping patch; the browser pane's page is wry on that base (D1). gpui-kit (Apache-2.0) and wry (MIT or
Apache-2.0) may be used and adapted; Zed's crates (GPL) are read, never pasted; zui and Zeron are never copied. The
shell's native pieces have no `ui::` piece and follow the system (D10). Masks for a pixel diff, beyond those of
`parity/wave-2`: the computer's name in bars and rows, relative ages ("just now", "2m ago"), and the loading bar and
spinner, which move.

## Browser pane

Sources: `browser/`, `components/preview/`, `panes.ts`, `rightPanelStore.ts`, `components/RightPanelTabs.tsx`, `shell/shellCommands.ts`, `desktop:guests.ts`, `desktop:main.ts`, `desktop:app-procs.ts`. The panel's frame, tab strip, launcher, sheet and drag handle are W1-FRM's; this part is what the browser adds. The page is a native web view per D1. Driver kit (`states.mjs`): T, a thread of the state's own; its pane opened by `mod+shift+j`. Q, a page server on 127.0.0.1:47801 run with its folder in bench's, so the thread's port watch lists it (6 s: the daemon reads ports every 5 s); started again after W3-BRW-035 stops it. P, the same pages on 47803 from the driver's own process, a port no thread holds. C, 47802, nothing listening. "Type X": click the field, type X, Enter. A capture holds the `<webview>` page (it composites into the window), and states clip the panel.

### Elements

- W3-BRW-001 Chrome row: Back, Forward, Refresh, field, Open on laptop, Copy URL, More; 40 px, hairline under; inline 28 px. `components/preview/PreviewChromeRow.tsx:121`
- W3-BRW-002 Back, Forward: 24 px ghost icons with tooltips, enabled by the page's own history. `components/preview/BrowserSurface.tsx:176`
- W3-BRW-003 Refresh: RotateCw; while loading it spins, reads "Stop", tooltip "Loading…"; held on the servers list. `components/preview/PreviewChromeRow.tsx:167`
- W3-BRW-004 Field: ghost, 28 px, "Search or enter URL"; a port shows as "localhost:3000/a?x=1", decoded, no token. `components/preview/PreviewChromeRow.tsx:207`
- W3-BRW-005 Open in system browser: ExternalLink at the field's end on hover with a page and the field unfocused. `components/preview/PreviewChromeRow.tsx:216`
- W3-BRW-006 Loading bar: 2 px primary line on the row's bottom edge, grows from the left, 6 px glow. `components/preview/PreviewChromeRow.tsx:318`
- W3-BRW-007 Open on laptop: Laptop, "Open localhost:3000 on this computer", while the host forwards the port. `components/preview/BrowserSurface.tsx:272`
- W3-BRW-008 Copy URL: Copy, then green Check and "Copied" for 1.5 s; copies the real URL, a cloud route's token too. `components/preview/BrowserSurface.tsx:294`
- W3-BRW-009 More ("Preview menu"), 224 px: Hard reload, Copy URL, Open in system browser, New tab, rule, Zoom row. `components/preview/PreviewMoreMenu.tsx:62`
- W3-BRW-010 Zoom row: outline minus, "100%" (48 px, tabular), plus, ghost reset; the menu stays open; held with no page. `components/preview/PreviewMoreMenu.tsx:78`
- W3-BRW-011 Page: white, in the slot under the row and bars; a fixed layer beside the shell, moved off screen when hidden. `components/preview/BrowserGuests.tsx:117`
- W3-BRW-012 Over the page: menus (z 130) and tooltips (z 140) over it (z 10); in a sheet it is z 60 and leaves under a modal. `components/preview/BrowserGuests.tsx:26`
- W3-BRW-013 Zoom pill: top right, 12 px in, mono 12 px "120%", popover ground, border, shadow; 1.5 s after a change. `components/preview/ZoomIndicator.tsx:48`
- W3-BRW-014 Down bar: muted 12 px line, truncated: ":3000 on <computer> is not answering; this tab reconnects when it does". `components/preview/BrowserSurface.tsx:228`
- W3-BRW-015 Its detail: "It stopped listening at <time>, held by <cmd> (pid N), which exited", "now on :M". `adapt/ports.ts:107`
- W3-BRW-016 Refusal over a kept page: medium title and detail; a 401 through the route, an unknown 403. `components/preview/BrowserSurface.tsx:232`
- W3-BRW-017 Refusal card: ":3000 refused the preview host" with the fix for Vite, webpack-dev-server, Rails or Next.js. `browser/refusal.ts:33`
- W3-BRW-018 No route: "<computer> opens no address this pane can reach for port N; it answers as localhost:N on that computer." `protocol:words/computer.ts:64`
- W3-BRW-019 Servers list: column to 576 px, 20 by 32 px padding, scrolls; "Recently used" (8) over "Local servers". `components/preview/PreviewEmptyState.tsx:51`
- W3-BRW-020 Recent row: window glyph, mono host and path, "just now" to "3d ago"; hover X "Remove <label> from history". `components/preview/PreviewRecentUrlCard.tsx:23`
- W3-BRW-021 Server row: window glyph, process name or "Listening", mono "localhost:3000". `components/preview/PreviewLocalServerCard.tsx:14`
- W3-BRW-022 Empty: "No preview yet. Type a port above or run a dev script, and servers listening on the thread's computer show up here." `components/preview/PreviewEmptyState.tsx:14`
- W3-BRW-023 Filter miss: "No address or server matches." `components/preview/PreviewEmptyState.tsx:16`
- W3-BRW-024 Tab: Globe2; a site's title, else its host; a port "<process> :3000/a" or ":3000/a", never the title; else "Browser". `browser/tabs.ts:166`
- W3-BRW-025 Launcher card "Browser", "Open your dev server or a URL.", key B; held "Available while this runs.". `panes.ts:57` exists: `app/src/screens/app_shell/panel.rs`
- W3-BRW-026 Cap notice: "10 browser tabs have a page open, the most wsp keeps. Close one to open another; …". `rightPanelStore.ts:652`
- W3-BRW-027 Leave question, native: "Open <url> in another app?", "A page in the browser tab wants to leave wsp.", Open, Cancel. `desktop:guests.ts:61`

### States

- W3-BRW-028 Empty pane. Shot: T, its pane. `components/preview/PreviewEmptyState.tsx:42`
- W3-BRW-029 Recents and servers. Shot: Q, T; type P's `/settings`, then More, New tab. `components/preview/PreviewEmptyState.tsx:55`
- W3-BRW-030 Filter with no match. Shot: Q, T; type "zzz", no Enter. `components/preview/PreviewEmptyState.tsx:54`
- W3-BRW-031 Page on the thread's port. Shot: Q, T; type `localhost:47801/`. `components/preview/BrowserGuests.tsx:123`
- W3-BRW-032 A port the thread does not hold: page under the down bar. Shot: T; type `localhost:47803/`. `components/preview/BrowserSurface.tsx:84`
- W3-BRW-033 Loading: spinner, bar held near 90%. Shot: Q, T; type `localhost:47801/hold`, 6 s; mask the refresh icon. `index.css:1405`
- W3-BRW-034 Refused: blank page under the down bar. Shot: T; type `localhost:47802/`, 3 s. `components/preview/BrowserSurface.tsx:227`
- W3-BRW-035 Server stopped, with its detail. Shot: from W3-BRW-031, stop Q; mask time, pid, command, computer. `adapt/ports.ts:122`
- W3-BRW-036 A site: URL in the field, page title on the tab. Shot: T; type `parity.localhost:47803/`. `browser/url.ts:94`
- W3-BRW-037 More menu over a page. Shot: from W3-BRW-031, "Preview menu". `components/preview/PreviewMoreMenu.tsx:62`
- W3-BRW-038 Zoomed 120%. Shot: from W3-BRW-037, "Zoom in" twice, close, 2.5 s. `components/preview/BrowserGuests.tsx:114`
- W3-BRW-039 Open in system browser shown. Shot: from W3-BRW-031, hover the field. `components/preview/PreviewChromeRow.tsx:219`
- W3-BRW-040 Field focused over a page: all text selected, ring border, system browser button gone. Shot: W3-BRW-031, click the field. `components/preview/PreviewChromeRow.tsx:194`
- W3-BRW-041 Long URL, long title. Shot: T; type `parity.localhost:47803/long/` and 300 x. `components/RightPanelTabs.tsx:375`
- W3-BRW-042 Many tabs. Shot: Q, T; six tabs by "Add a panel", "Browser", each on its own path. `components/RightPanelTabs.tsx:364`
- W3-BRW-043 A page's new window as a tab. Shot: Q, T; type `localhost:47801/popup`. `components/preview/BrowserGuests.tsx:37`
- W3-BRW-044 Tab cap refused, W3-BRW-026 raised. Shot: Q, T; ten tabs as W3-BRW-042, then "Add a panel", "Browser"; whole window. `rightPanelStore.ts:659`
- W3-BRW-045 Narrow, the page in the sheet at 980. Shot: as W3-BRW-031 at width 980. `components/preview/BrowserGuests.tsx:144`
- W3-BRW-046 Sheet with a menu up: the page leaves while a modal stands over the sheet. Shot: W3-BRW-045, "Preview menu". `components/preview/BrowserGuests.tsx:145`
- W3-BRW-047 Inline panel at its 360 px floor with a page: the field takes what the buttons leave. No shot: the driver's drag of the edge moved nothing in two runs; Check: drag by hand, the panel stops at 360 px. `components/preview/PreviewPanelShell.tsx:69`
- W3-BRW-048 This computer's own panel: no route, "place:here". Shot: New thread, panel's Browser, type `127.0.0.1:47803`. `components/preview/BrowserSurface.tsx:219`
- W3-BRW-049 Refusal bar and cards, route minting, a forwarded port. No shot: cloud or joined computer only; a thread here is never probed. `browser/refusal.ts:77`
- W3-BRW-050 After an app reload the tabs return on the servers list, titled Browser. Check: W3-BRW-031, `location.reload()`, tab reads "Browser". `browser/tabs.ts:82`
- W3-BRW-051 Pages unloaded: hidden 5 min; past the last 3 threads shown. Check: count `[data-browser-guest]` over four threads. `rightPanelStore.ts:696`

### Actions and keys

- W3-BRW-052 Enter opens the trimmed text (empty: nothing) and blurs; Escape restores and blurs; focus selects all. `components/preview/PreviewChromeRow.tsx:191`
- W3-BRW-053 "3000", ":3000", "3000/a", a loopback host are ports; host:port is http (443 https); a dotted host https; else a Google search. `browser/url.ts:44`
- W3-BRW-054 With no page the field filters the list (URL, title, host:port, process); its text stays after blur. `components/preview/PreviewEmptyState.tsx:38`
- W3-BRW-055 Opening a place puts it first in recents, 50 kept; the page's own moves do not. `browser/recents.ts:41`
- W3-BRW-056 A row opens; its X removes the recent. `components/preview/PreviewEmptyState.tsx:64`
- W3-BRW-057 Back, Forward run the page's history; each main-frame page becomes the tab's place; Back never reaches the list. `components/preview/BrowserGuests.tsx:81`
- W3-BRW-058 Refresh, Hard reload and Stop while loading all reload; a failed route is asked again instead. `components/preview/BrowserSurface.tsx:165`
- W3-BRW-059 A typed place loads into the tab's page; a tab with none makes one on that URL. `components/preview/BrowserSurface.tsx:108`
- W3-BRW-060 Zoom: 10% steps, 50% to 200%, reset 100%, per tab, in memory; set at dom-ready and on each change. `browser/tabs.ts:123`
- W3-BRW-061 Open in system browser, Open on laptop: http(s) to the default browser. `desktop:main.ts:453`
- W3-BRW-062 New tab (menu): a tab on the servers list; the cap is asked when it opens a place. `components/preview/BrowserSurface.tsx:196`
- W3-BRW-063 A page's new http(s) window opens as a tab, cap applies; another app's link asks; file, data, about drop. `desktop:guests.ts:51`
- W3-BRW-064 `mod+shift+j`: panel on Browser, or shut; passes a focused terminal; nothing with no thread or in Settings. `shell/shellCommands.ts:360` exists: `app/src/shell.rs:726`
- W3-BRW-065 `mod+w` closes the tab, from the panel or the page (the shell takes it from a page); `mod+shift+w` the window. `desktop:guests.ts:127` exists: `app/src/screens/app_shell/keys.rs:128` (nothing sets the `Preview` context)
- W3-BRW-066 `mod+f` (find in thread) stands down while focus is in the panel or a page. `keybindingDefaults.ts:73`
- W3-BRW-067 `mod+shift+j` and Open browser show the first browser tab, else one "Browser" placeholder, asking no cap; B and Add a panel make a new tab and ask first. `rightPanelStore.ts:152`
- W3-BRW-068 A focused page keeps every other key: no app chord fires until focus is back in the app. `desktop:close.ts:4`
- W3-BRW-069 No key reloads, steps, zooms, finds or focuses the field; `focusUrlNonce` is never passed. `components/preview/PreviewChromeRow.tsx:102`
- W3-BRW-070 Roads in: launcher B, Add a panel, the sidebar's Open browser, Activity's ports, joined loopback links. `actions/workspaceActions.ts:183`
- W3-BRW-071 Closing a tab drops its page; past 10, the one hidden longest goes; a shown tab with none loads again. `browser/guests.ts:96`
- W3-BRW-072 The page follows its slot: slot, panel or window resize, a transition's end, a modal. `components/preview/BrowserGuests.tsx:139`

### Host operations

- W3-BRW-073 A thread here: no op; the page dials localhost:<port> itself. `components/preview/BrowserSurface.tsx:78`
- W3-BRW-074 `workspaces.portReach`: a route per port off this computer; asked again 9 min before expiry or halfway, 30 s floor. `browser/reach.ts:38` exists: `host-client/src/ops/terminal.rs` (typed, no caller)
- W3-BRW-075 `workspaces.portProbe`: per route and reload; a first 401 remints; 502, 504 are down, asked every 2 s. `browser/refusal.ts:90` exists: `host-client/src/ops/terminal.rs` (typed, no caller)
- W3-BRW-076 `forwards.list`, push `forward.open`, `forward.close`: Open on laptop. `protocol/store/hooks.ts:81`
- W3-BRW-077 The port list rides the thread's daemon link, open only while the thread runs and shows; `ports.watch` once live. `terminal/wiring.ts:156` exists: `app/src/screens/terminal/link.rs` (the link only; no `ports.watch`)
- W3-BRW-078 Gap: daemon op `ports.watch` over `daemon.send`; pushes `port.open`, `port.close`, untyped. `terminal/wiring.ts:178`
- W3-BRW-079 `webviewTag`, `will-attach-webview`: own partition only, sandbox, isolation, no preload, web security; the host's page alone. `desktop:guests.ts:27`
- W3-BRW-080 `did-attach-webview`: `setWindowOpenHandler` to IPC `guest:open`; `dialog.showMessageBox`, `shell.openExternal`. `desktop:main.ts:288`
- W3-BRW-081 Page `before-input-event`: IPC `shell:chord`, or `BrowserWindow.close`. `desktop:guests.ts:127`
- W3-BRW-082 `session.fromPartition("persist:wsp-browser")`, permission handlers: fullscreen, clipboard-sanitized-write, pointerLock. `desktop:guests.ts:102`
- W3-BRW-083 `app.getAppMetrics`, `webContents.getAllWebContents`: page pids as kind tab with origins, for Activity. `desktop:app-procs.ts:77`

### Design

- W3-BRW-084 Row icons `ui::Button::icon_xs` with tooltips; ArrowLeft, RotateCw, ExternalLink, Copy, Check, Laptop exist; ArrowRight, MoreVertical none yet.
- W3-BRW-085 Field `ui::Field`; its ghost look (clear border, muted 40% hover, ground and ring on focus) none yet. `components/ui/input-group.tsx:22`
- W3-BRW-086 More: `ui::icon_menu`; the Zoom row inside a menu row none yet.
- W3-BRW-087 Servers list, rows, window glyph, bars, refusal and route cards, zoom pill, loading bar: none yet.
- W3-BRW-088 Held controls: 64% opacity, no pointer, so held Back, Forward, Refresh show no tooltip. `components/ui/button.tsx:22`
- W3-BRW-089 Empty sentence (empty pane, no route, refusal card): centred 13 px muted, 384 px wide, 24 px padding, 48 px from 768 px wide. `components/ui/empty.tsx:8`
- W3-BRW-090 Bars: 12 px muted on muted 40%, a 60% line under, 12 by 6 px padding; the down bar truncates with its text as title. `components/preview/BrowserSurface.tsx:233`
- W3-BRW-091 Page: none yet; the native web view of D1.
- W3-BRW-092 Motion: loading bar scaleX 0.04 to 0.9, 5.3 s, `cubic-bezier(0.1, 0.5, 0.2, 1)`; done 150 ms, fade after 220 ms, CSS `ease-out`. `index.css:1397`
- W3-BRW-093 Motion: refresh spin, rotate, 1 s, linear, endless, also under reduced motion; `ui::status::turning` stops there (`app/src/ui/status.rs:34`). `components/preview/PreviewChromeRow.tsx:172`
- W3-BRW-094 Motion: system browser button opacity and the field's end padding, 150 ms, default curve. `components/preview/PreviewChromeRow.tsx:219`
- W3-BRW-095 Motion: zoom pill opacity and translate y -4 to 0 px, 200 ms, `cubic-bezier(0, 0, 0.2, 1)`. `components/preview/ZoomIndicator.tsx:48`
- W3-BRW-096 Motion: none on the page; it jumps to its slot when the sheet's 200 ms slide ends. `components/preview/BrowserGuests.tsx:154`

## The desktop shell

Sources: `desktop:` whole (`main.ts`, `host-lifecycle.ts`, `starting.ts`, `host-switch.ts`, `shim.ts`, `window.ts`, `menu.ts`, `tray.ts`, `host-feed.ts`, `needs-you.ts`, `deep-link.ts`, `quit.ts`, `close.ts`, `move.ts`, `awake.ts`, `self-update.ts`, `get-bundle.ts`, `app-log.ts`, `app-procs.ts`, `preload.ts`), `host:service.ts`, `host:host-lock.ts`, `host:cli/service.ts`, `apps/desktop/electron-builder.yml`, and the page callers of the bridge. Each list holds the host start first, then the shell around the window. D2 says how GPUI starts the host; D10 and D11 the native pieces; items marked "was W2-UNO" replace those.

### Elements

- W3-SHL-001 Starting window, dark: 16 px spinner, "Starting wsp", muted 14/21; the app window's frame; top band drags. `desktop:starting.html:30` exists: `app/src/screens/dialogs/starting.rs` (no drag band; was W2-DLG-017)
- W3-SHL-002 Start-failed box, no parent: "wsp could not start" (launch) or "wsp could not open" (later), the failure's first line, Open log, Quit. `desktop:starting.ts:38` exists: `app/src/screens/dialogs/startup.rs:365` (launch title only; W2-DLG-039)
- W3-SHL-003 Older host box, no parent: "wsp <theirs> is still serving your threads; restart it as <app>?", Restart as <app>, Quit. `desktop:host-lifecycle.ts:406` (was W2-UNO-015)
- W3-SHL-004 Serves-again box, warning: "wsp <theirs> is serving again after this app restarted it as <app>", detail on the wsp command. `desktop:host-lifecycle.ts:370` (was W2-UNO-015)
- W3-SHL-005 Next window: onboarding (system theme) where the host holds nothing, else the app window, dark until the page says. `desktop:main.ts:623` exists: `startup.rs:338`
- W3-SHL-006 App window 1280 by 800, title "wsp", centred; no minimum size, no bounds kept. `desktop:window.ts:130` exists: `startup.rs:54`
- W3-SHL-007 Mac frame: hiddenInset title bar, lights at x 16 y 19 in the 52 px header, clear ground. `desktop:window.ts:36` exists: `startup.rs:57`
- W3-SHL-008 Mac glass: sidebar material behind the window, following the window's active state. `desktop:window.ts:38` exists: `app/src/screens/app_shell/vibrancy.rs:19`
- W3-SHL-009 Linux on GNOME, KDE, Xfce, Cinnamon, MATE, Unity, Budgie, Pantheon, LXQt, LXDE, Deepin, COSMIC: no title bar, controls over the header's right end. `desktop:window.ts:64`
- W3-SHL-010 Linux elsewhere (tiling, Openbox, none): the desktop's own frame, ground #09090b. `desktop:window.ts:60` exists: `startup.rs:59` (on every desktop; D11)
- W3-SHL-011 The header row drags the window where it is the frame: Mac, and Linux on a listed desktop (`drag-region`). `shell/HeaderRow.tsx:17`
- W3-SHL-012 Window title is the page's: "wsp", or "• wsp" while a thread waits. `notices/hostNotices.ts:374` exists: `app/src/lib.rs:13` ("wsp" only)
- W3-SHL-013 App menu (Mac): About wsp (system panel), Services, Hide wsp, Hide Others, Show All, Quit wsp. `desktop:menu.ts:22` exists: `app/src/screens/app_shell/menu.rs:63` (About opens nothing)
- W3-SHL-014 View, Electron's role: Reload, Force Reload, Toggle Developer Tools, Actual Size, Zoom In, Zoom Out, Toggle Full Screen. `desktop:menu.ts:27` exists: `menu.rs:59` (full screen only; D11)
- W3-SHL-015 Hosts, once there is a host: this computer, each saved alias, a check on the current. `desktop:main.ts:384` exists: `menu.rs:60` ("Show Hosts" opens nothing)
- W3-SHL-016 Linux: no menu bar; Electron's default menu is removed before ready. `desktop:main.ts:373` exists: `menu.rs:90`
- W3-SHL-017 Dock badge: threads waiting on the person, from the page with a window up, from the tray's feed with none (was W2-UNO-013). `desktop:main.ts:740`
- W3-SHL-018 Tray, Mac only (elsewhere the last window's close quits): template mark, "?" while one waits (was W2-UNO-011, 052). `desktop:main.ts:733`
- W3-SHL-019 Tray title: the working count, empty at none; on a host elsewhere "<host> <count>" (was W2-UNO-011, 047). `desktop:tray.ts:154`
- W3-SHL-020 Tray head: "On <host>" elsewhere, then "N working, M waiting on you" or "No threads working" (was W2-UNO-048, 049). `desktop:tray.ts:142`
- W3-SHL-021 Tray row: title; sublabel "<Working|Needs you|Failed> on <computer>" or the refusal; submenu Open, Allow, Deny, Stop (was W2-UNO-011). `desktop:tray.ts:146`
- W3-SHL-022 Tray lost: "wsp is not running", "Start wsp"; elsewhere "<host> is not answering", no Start (was W2-UNO-011, 050). `desktop:tray.ts:132`
- W3-SHL-023 Quit box: "Quit wsp?", detail, Quit (default), Quit and stop wsp, Cancel (Escape) (was W2-UNO-012). `desktop:quit.ts:37`
- W3-SHL-024 Move box (Mac): "Move wsp to your Applications folder?", detail, Move to Applications, Not now (was W2-UNO-014). `desktop:move.ts:33`
- W3-SHL-025 System notification: title, body, app icon; silent unless the line sounds (was W2-UNO-013). `desktop:needs-you.ts:70`
- W3-SHL-026 Scheme `wsp://` in Info.plist and the Linux desktop entry; no file associations. `apps/desktop/electron-builder.yml:18`

### States

- W3-SHL-027 Host serving, own release and login, page digest = sha256(host-token): attach, no manager call. Check: `host-lifecycle.test.ts:843`. `desktop:host-lifecycle.ts:124` exists: `host-client/src/lib.rs:57` (live pid only; D9)
- W3-SHL-028 Nothing serving: write the unit, load it, wait for the lock and a page with the digest, attach. Check: `host-lifecycle.test.ts:245`, `smoke.electron.test.ts:590`. `desktop:host-lifecycle.ts:310`
- W3-SHL-029 Host slow: starting window from 800 ms on launch and Dock opens (not menu bar Start). Shot: W2-DLG-038's baseline, `home: "held"`, before 2 min. `desktop:main.ts:612` exists: `startup.rs:325` (launch only)
- W3-SHL-030 Waits: 20 s while no process runs the unit; 2 min while lock, pid and log size stand still. Check: `host-start-wait.test.ts:143`, `:176`, `:203`. `desktop:host-lifecycle.ts:531`
- W3-SHL-031 Failed, nothing ran the service: "wsp did not start within <t>: nothing ran its service". Check: `host-start-wait.test.ts:262`. `desktop:host-lifecycle.ts:535`
- W3-SHL-032 Failed, host exited: "wsp stopped as it started: <last log line>", or "…and wrote nothing to its log"; 20 lines to the app log. Check: `host-start-wait.test.ts:234`. `desktop:host-lifecycle.ts:542`
- W3-SHL-033 Failed, silent 2 min: "wsp did not start within <t>: it neither answered nor wrote to its log". Check: `host-start-wait.test.ts:226`. `desktop:host-lifecycle.ts:537`
- W3-SHL-034 Failed, manager refused: "<argv> exited <code> and said: <output>". Check: `host-lifecycle.test.ts:710`. `desktop:host-lifecycle.ts:255`
- W3-SHL-035 Lock refused (another login, no wsp page, another digest): "a host (pid N) holds <lock> on port P but <why>: …". Check: `host-lifecycle.test.ts:914`, `:431`. `desktop:host-lifecycle.ts:95`
- W3-SHL-036 Home: WSP_HOME when not empty, else `~/.wsp`; an unpackaged run in a wsp checkout uses its `.wsp/state.json`. Check: `host-lifecycle.test.ts:1474`. `desktop:host-lifecycle.ts:111` exists: `app/src/main.rs:18` (no checkout rule; empty WSP_HOME read as a home)
- W3-SHL-037 Older host a verb or `wsp up` started: SIGTERM its pid, wait 20 s for the lock, start the service. Check: `host-lifecycle.test.ts:927`. `desktop:host-lifecycle.ts:430`
- W3-SHL-038 Older host the service runs with this shim: `host.restart`, wait for close and a new pid; refused, the unit is rewritten. Check: `host-lifecycle.test.ts:1006`. `desktop:host-lifecycle.ts:441`
- W3-SHL-039 Older host with turns running here, or no count: ask first, stop nothing until answered; none: replace unasked. Check: `host-lifecycle.test.ts:944`. `desktop:host-lifecycle.ts:428`
- W3-SHL-040 Older host up while open: replaced on the feed's next dial, asking while turns run; the window reloads on the new port. Check: `host-lifecycle.test.ts:1144`. `desktop:main.ts:689`
- W3-SHL-041 Host newer than the app: attached, nothing stopped; the page says "this app is <app>, the host is <v>". Check: `host-lifecycle.test.ts:1128` (D8). `desktop:host-lifecycle.ts:305`
- W3-SHL-042 One account host, none here: one 8 s dial admits this computer, the window opens there, never onboarding. Check: `host-lifecycle.test.ts:772`. `desktop:host-lifecycle.ts:172` exists: `host-client/src/lib.rs:57` (15 s)
- W3-SHL-043 Account host refusing, silent, or several: a line in the app log, open on the service here. Check: `host-lifecycle.test.ts:795`, `:813`. `desktop:host-lifecycle.ts:184`
- W3-SHL-044 No network: only the account dial leaves this computer, given up after 8 s. Check: `host-lifecycle.test.ts:813` with a silent dial. `desktop:host-lifecycle.ts:163`
- W3-SHL-045 First launch (no sealed golden, no workspace): onboarding, else the app. Shot: W2-DLG-032's baseline, `home: "empty"`. Check: `host-lifecycle.test.ts:1348`. `desktop:host-lifecycle.ts:469` exists: `startup.rs:232`
- W3-SHL-046 Second launch while running: it exits 0; the first raises its window or opens the argv's link. Check: `app/tests/dialogs_onboarding.rs:57`. `desktop:main.ts:213` exists: `startup.rs:399` (no link)
- W3-SHL-047 Second launch while starting: nothing raised, the open goes on. Check: launch twice on a held home. `desktop:main.ts:216` exists: `startup.rs:407` (raises the starting window)
- W3-SHL-048 Mac, no window, Dock click: the whole open again, joining one in flight; nothing during a quit. Check: `smoke.electron.test.ts:1028`. `desktop:main.ts:636` exists: `app/src/main.rs:91` (window only, no host step)
- W3-SHL-049 Theme source dark at open, then the page's pick; onboarding follows the system. Check: port `apps/desktop/test/smoke.electron.test.ts:661`. `desktop:main.ts:430` exists: `app/src/shell.rs:611` (opens on the system's side)
- W3-SHL-050 Glass off for Transparency off or Reduce transparency. Check: the painted alpha read, `smoke.electron.test.ts:1559`. `desktop:main.ts:179` exists: `app/src/shell.rs:635`
- W3-SHL-051 Mac, window closed: the app stays, tray drawn, badge from the feed; a Dock click reopens. Check: close, the process stays, the tray reads. `desktop:main.ts:479`
- W3-SHL-052 Linux, last window closed: the app quits, wsp serves on. Check: `xdotool key ctrl+shift+w`, the process is gone, `wsp status` answers. `desktop:main.ts:581` exists: `app/src/main.rs:100`
- W3-SHL-053 Quit asks on "Ask each time"; "keep" quits, "stop" stops, unasked. Check: port `apps/desktop/test/quit.test.ts:26`. `desktop:quit.ts:49`
- W3-SHL-054 Tray, one working and one asking: "1", "?", both rows, Allow and Deny. Check: `smoke.electron.test.ts:1069`. `desktop:tray.ts:155`
- W3-SHL-055 Tray, 14 threads: every row, no cap. Check: port `apps/desktop/test/tray.test.ts:42` with 14 rows. `desktop:tray.ts:146`
- W3-SHL-056 Tray, a 200-character title: the row holds it whole, the menu widens. Check: `host.once("longTitle")` working, read the row. `desktop:tray.ts:149`
- W3-SHL-057 Tray lost; redial every 3 s; Start brings it back. Check: `wsp stop`, read the rows, Start wsp, read again. `desktop:host-feed.ts:141`
- W3-SHL-058 Notification, window focused: nothing. Check: port `apps/desktop/test/needs-you.test.ts:70`. `desktop:needs-you.ts:64`
- W3-SHL-059 Notification, unfocused: shown, sounded per the pick. Check: `gdbus monitor --session --dest org.freedesktop.Notifications` shows Notify. `desktop:needs-you.ts:70`
- W3-SHL-060 Window closed (Mac): prompts, plan alerts and the end of a thread a person started are said; replays and agents' threads are not (was W2-UNO-070, 071). Check: `smoke.electron.test.ts:1311`. `desktop:main.ts:783`
- W3-SHL-061 Badge: the count, 0 clears, non-integers dropped. Check: `lsappinfo info -only StatusLabel wsp`, one prompt, then none. `desktop:needs-you.ts:103`
- W3-SHL-062 Awake while a thread works on this computer and Keep awake is on; not when lost or elsewhere. Check: `pmset -g assertions`, `smoke.electron.test.ts:1100`. `desktop:main.ts:773`
- W3-SHL-063 Link before the page: held; the first page opens at `#open/<kind>/<id>`. Check: quit, `open 'wsp://settings/general'`. `desktop:deep-link.ts:42`
- W3-SHL-064 Link with the page up: window raised, the page opens it. Check: `open 'wsp://thread/<id>'`. `desktop:deep-link.ts:33`
- W3-SHL-065 Link with the window closed (Mac): held, nothing opens; the next Dock, tray or notification open lands on it. Check: close, `open 'wsp://thread/<id>'`, then a Dock click. `desktop:main.ts:198`
- W3-SHL-066 Link to nothing here: "That link names a thread this wsp does not have.". Shot: load the page at `#open/thread/nope`. `shell/links.ts:16`
- W3-SHL-067 Window moved to another host: the feed closes and dials there by alias; app processes offered only here. Check: Hosts move, the tray title starts with the alias. `desktop:main.ts:665`
- W3-SHL-068 An app that can replace itself downloads a newer release unasked, once per page and release, never one the person dismissed. Check: port `apps/web/test/update-card.test.tsx:123`. `shell/update.ts:160`
- W3-SHL-069 Update downloading: Downloading held, asks share one download. No shot: needs a release above 0.3.8. `shell/update.ts:106`
- W3-SHL-070 In place: zip staged in `.wsp-update` and checked; Restart quits, the swap script takes over. Check: port `self-update.test.ts:391`. `desktop:self-update.ts:271`

### Actions and keys

- W3-SHL-071 Before ready: app log `<home>/logs/app.log`, Chromium's files in `<state folder>/desktop`, `app.requestSingleInstanceLock` else `app.exit(0)`. `desktop:main.ts:86` exists: `app/src/main.rs:75` (own lock)
- W3-SHL-072 At ready, in order: menu, update stage, Move, shim, login PATH, npm relink, open; a throw is the box, then quit. `desktop:main.ts:845`
- W3-SHL-073 Shim `<home>/bin/wsp`, 0755, renamed in: `ELECTRON_RUN_AS_NODE=1 exec <app binary> <cli.mjs> "$@"`, daemon forward in front. Check: `shim.test.ts:21`. `desktop:shim.ts:24`
- W3-SHL-074 One host step at a time: launch, Dock, Start, the feed's check. Check: `host-lifecycle.test.ts:1295`. `desktop:main.ts:414` exists: `startup.rs:35` (a flag, no queue)
- W3-SHL-075 Start-failed: Open log (default) opens the failure's log, else host.log, else app.log; Quit (cancel) quits. Check: `host-start-wait.test.ts:349`. `desktop:starting.ts:53` exists: `startup.rs:380` (else the state folder)
- W3-SHL-076 Onboarding's Open wsp: attach again, open the app window, close onboarding. Check: `smoke.electron.test.ts:742`. `desktop:main.ts:531` exists: `app/src/screens/dialogs/onboarding.rs:662`
- W3-SHL-077 Token: `hosts:token` answers host-token beside the state, read each ask, or the record's device token; never in the page. Check: `host-lifecycle.test.ts:843`. `desktop:host-switch.ts:74` exists: `host-client/src/lib.rs:57`
- W3-SHL-078 Hosts menu move: an account host dialled once (admits, or renews a revoked token by key), window loads its url, feed follows; here moves back. Check: `host-switch.test.ts:147`. `desktop:host-switch.ts:79`
- W3-SHL-079 Cmd+Q and Quit wsp ask per General's pick. `desktop:main.ts:394` exists: `menu.rs:117` (quits, asks nothing)
- W3-SHL-080 Cmd+Shift+W closes the window, menu or not; Cmd+W stays the page's. `desktop:main.ts:470` exists: `menu.rs:98`
- W3-SHL-081 View chords Cmd+R, Shift+Cmd+R, Opt+Cmd+I, Cmd+0, Cmd+Plus, Cmd+-; a focused terminal takes the zoom ones (D11). `desktop:main.ts:475`
- W3-SHL-082 Dock click (`activate`): raise, or open a window; nothing while quitting. `desktop:main.ts:636` exists: `app/src/main.rs:91` (no restore of a minimised window)
- W3-SHL-083 Second launch: its wsp:// argument opens, else the window is raised. `desktop:main.ts:213` exists: `startup.rs:399` (raise only)
- W3-SHL-084 `open-url` (Mac) opens the link; registered before ready. `desktop:main.ts:209`
- W3-SHL-085 Tray Open: the thread's link here, then raise or open; elsewhere raise or open only. `desktop:main.ts:747`
- W3-SHL-086 Tray Allow, Deny, Stop; Start wsp attaches, then redials; Open wsp; Quit wsp asks. `desktop:main.ts:752`
- W3-SHL-087 Notification click: raise, then the page opens that line's thing by id. `desktop:needs-you.ts:73`
- W3-SHL-088 Update Get, Quit and open or Restart, dismiss (drops a checked copy). `shell/update.ts:136` exists: `app/src/screens/sidebar/foot.rs:182` (Get opens the release page)

### Host operations

- W3-SHL-089 `golden.get` {name: "default"}, `workspaces.list`: the first launch read. `desktop:host-lifecycle.ts:472` exists: `startup.rs:237` (W2-DLG-054)
- W3-SHL-090 `sessions.list`, `workspaces.list` past the release check: turns running here, for the older host box. Gap: no such dial in HostClient. `desktop:host-lifecycle.ts:639`
- W3-SHL-091 `host.restart` past the release check, then the socket's close within 20 s. Gap: as W3-SHL-090. `desktop:host-lifecycle.ts:455`
- W3-SHL-092 `auth` reply `version`, `road`: another release refused unless past the check. `host:verbs/client.ts:331` exists: `host-client/src/lib.rs:57` (refuses older and newer; D4)
- W3-SHL-093 GET `http://<lock address>:<port>/`, 2 s: boot line `version`, `tokenHash` against sha256(host-token), `safeEqual`. `desktop:host-lifecycle.ts:51`
- W3-SHL-094 `<state folder>/host.lock`; `kill(pid, 0)` (EPERM: another login); `/proc/<pid>/exe` gone; `SIGTERM`. `host:host-lock.ts:64` exists: `host-client/src/lib.rs:57` (live pid only)
- W3-SHL-095 launchd: `~/Library/LaunchAgents/com.wsp.host.<tag>.plist` 0600, RunAtLoad, KeepAlive; `launchctl print, enable, bootstrap, disable, bootout, print-disabled`. `host:service.ts:186`
- W3-SHL-096 systemd: `~/.config/systemd/user/wsp-host-<tag>.service`, Restart=always, KillMode=process; `systemctl --user show, daemon-reload, enable, restart, disable --now, is-enabled`. `host:service.ts:244`
- W3-SHL-097 Unit: `<shim> up --state <state>`, cwd home, HOME, WSP_STARTED_BY=service, log host.log; tag sha256(state)[0:8]. `desktop:host-lifecycle.ts:196`
- W3-SHL-098 Start at login (`service:login`, `:login-set`): `launchctl print-disabled`, `systemctl --user is-enabled`; enable or disable; null with no unit. Check: `host-lifecycle.test.ts:337`. `desktop:host-lifecycle.ts:271`
- W3-SHL-099 Quit and stop wsp: `sessions.interrupt` per running turn here, then what `wsp down` does: unit removed, or a verb's host SIGTERMed. Gap: no dial past the release check. `desktop:host-lifecycle.ts:663`
- W3-SHL-100 Tray feed: `events.subscribe`, `preferences.get`, `sessions.list`, `workspaces.list`, `places.list`; retried every 3 s. `desktop:host-feed.ts:90`
- W3-SHL-101 Push `preferences.changed`: keepAwake, notifyNeeds, notifyDone, planAlerts, onQuit. `desktop:host-feed.ts:110`
- W3-SHL-102 Push `session.permission`, `session.done`, `usage.alert`: said while the window is closed. `desktop:tray.ts:165`
- W3-SHL-103 `sessions.answer` (tray Allow, Deny), `sessions.interrupt` (tray Stop). `desktop:host-feed.ts:179`
- W3-SHL-104 `appProcs.offer`, `appProcs.set` per sample while watched; push `app.procs.wanted`, Gap: not in the record. `desktop:app-procs.ts:151`
- W3-SHL-105 `Tray`: `setImage` template, `setTitle`, `setToolTip`, `setContextMenu`. `desktop:main.ts:734`
- W3-SHL-106 `Notification`, `Notification.isSupported`, `shell.beep`, `app.dock.bounce`, `app.setBadgeCount`. `desktop:main.ts:225`
- W3-SHL-107 `powerSaveBlocker.start("prevent-app-suspension")`, `powerSaveBlocker.stop`. `desktop:main.ts:775`
- W3-SHL-108 `app.setAsDefaultProtocolClient("wsp")` (packaged), `open-url`, `second-instance`, `app.focus({ steal: true })`. `desktop:main.ts:222`

### Design

- W3-SHL-109 Starting window: `ui::spinner` 16 px in the dark palette, `startup::window_options`. exists: `app/src/screens/dialogs/starting.rs`
- W3-SHL-110 Motion: spinner rotate 360 deg, 1 s, linear, endless; it keeps turning under reduced motion. `desktop:starting.html:25` exists: `app/src/ui/status.rs:98` (stops under reduced motion, `:34`)
- W3-SHL-111 Start-failed box: `Window::prompt` Critical on the starting window; Electron's has no parent. `desktop:main.ts:603` exists: `startup.rs:372`
- W3-SHL-112 Older host and serves-again boxes: none yet; native question and warning, no parent (D10). `desktop:main.ts:419`, `:697`
- W3-SHL-113 Header drag on the app window: none yet; onboarding's `window_control_area(Drag)` and `start_window_move` is the pattern (`app/src/screens/dialogs/onboarding.rs:656`). `shell/HeaderRow.tsx:17`
- W3-SHL-114 App menu: native, `cx.set_menus`, no `ui::` piece. `desktop:main.ts:396` exists: `menu.rs:127`
- W3-SHL-115 Tray: native status item, template image, title, menu with sublabel rows: none yet (D10). `desktop:main.ts:658`
- W3-SHL-116 Notifications and badge: native; none yet (D10). `desktop:main.ts:225`
- W3-SHL-117 Quit, Move, error boxes: app-modal native alerts, no parent window (D10). `desktop:main.ts:563` exists: `startup.rs:372` (the start-failed box only)
- W3-SHL-118 Linux controls over the header: none yet; gpui-kit's TitleBar (D11). `desktop:window.ts:112`

## Experimental switch

Sources: `settings/general.tsx`, `settings/rows.tsx`, `settings/groups.ts`, `settings/settingsReads.ts`, `components/ui/switch.tsx`, `desktop:main.ts`, `desktop:preload.ts`, `desktop:get-bundle.ts`, `desktop:self-update.ts`, `protocol:views/desktop-bridge.ts`. The row is not on `main` at 1516a0cd5; this branch adds it to today's app (`settings/general.tsx:192`, `desktop:experimental.ts`), the only change the GPUI app needs there. Each item names the piece of today's app it is built from; D3 says where it is kept, what it does and where it sits. The row had no pictures when this was read; each state names the shot its look must match.

### Elements

- W3-SWT-001 Row: title, note, switch in the slot, reset arrow off its default; General's `row()`. `settings/general.tsx:54`
- W3-SWT-002 The row it copies: "Start wsp at login", the other shell-kept switch, a bridge read with its own reset rule. `settings/general.tsx:157`
- W3-SWT-003 Place: a desktop-only card at the end of General with one head and one row, as Logs; Version under it (D3). `settings/general.tsx:171`
- W3-SWT-004 Switch: base-ui, `data-k`, `aria-label` the title; 18 by 30 px from 640 px wide, 22 by 38 below. `components/ui/switch.tsx:12`
- W3-SWT-005 Note: 13 px muted, max-w-xl, wraps; names the computer as Start at login's does. `settings/format.ts:321`
- W3-SWT-006 Search reads title and note alone, no keyword list, so "Experimental" is in the title. `settings/rows.tsx:108`
- W3-SWT-007 A headed card adds a trail line under General in the Settings sidebar. `settings/SettingsSidebar.tsx:190`
- W3-SWT-008 Bridge member, optional as `openLogs` and `loginStart`: no member or a null read, no row; a browser tab has no bridge. `settings/settingsReads.ts:62`
- W3-SWT-009 Preload: one `ipcRenderer.invoke` per call on its own channel. `desktop:preload.ts:84`
- W3-SWT-010 Handler through `answer()`, which refuses any page but the app's own host's. `desktop:main.ts:120`

### States

- W3-SWT-011 Off, Paper and Graphite. Shot: today's Startup and quit card, its neighbour (Start at login hides on the fixture, which has no unit); the row's own once it is added. `settings/general.tsx:167`
- W3-SWT-012 On, with its reset arrow, as Start at login off draws one. Shot: the same card with Keep awake off, which draws the arrow. `settings/general.tsx:163`
- W3-SWT-013 GPUI app not found: no row, as Logs draws no card without its bridge member (D3). Check: a page test with the read null. `settings/general.tsx:171`
- W3-SWT-014 Browser tab: no row. Check: a page test with `window.wsp` unset. `settings/general.tsx:152`
- W3-SWT-015 Window on a host elsewhere: no row; the read is skipped while `hosts().current` is set, as Get's download is. Check: as `apps/desktop/test/origin.test.ts:51`. `shell/update.ts:77`
- W3-SWT-016 Narrow: under 640 px the switch drops under the note. Shot: the same card at width 600. `settings/rows.tsx:123`
- W3-SWT-017 Switching: today's window and its Mac menu bar item go, GPUI's starting window at 800 ms, then its window. Check: the `host.lock` pid is the same before and after. `desktop:main.ts:580`
- W3-SWT-018 GPUI did not open: today's window stays, with a notice, as Quit and open's failure. Check: a stub that exits 1. `shell/update.ts:122`
- W3-SWT-019 Launch with the switch on: no window of today's app; GPUI opens. Check: app log and process list. `desktop:main.ts:611`
- W3-SWT-020 The way back: none yet; gpui/main has no Settings screen (D3: the app menu first). Check: `app/src/screens/mod.rs`. `desktop:main.ts:209`
- W3-SWT-021 While GPUI runs instead, today's shell duties stop: no keep-awake hold, menu bar item, dock badge or notifications (D10). Check: `pmset -g assertions` with a thread working. `desktop:main.ts:773`
- W3-SWT-022 Both apps at once: two locks, one host; today's app replaces an older host under GPUI. Check: both locks held, one `host.lock`. `desktop:main.ts:692`

### Actions and keys

- W3-SWT-023 Click or Space flips it; focus ring; held, it takes no press, at 64%. `components/ui/switch.tsx:12`
- W3-SWT-024 A flip writes at once, no Save, as Start at login's `turnLogin`. `settings/general.tsx:161`
- W3-SWT-025 The reset arrow writes the default, off. `settings/general.tsx:163`
- W3-SWT-026 On: the shell opens GPUI and quits only once that went, as Get's Quit and open. `desktop:get-bundle.ts:306`
- W3-SWT-027 That quit is `app.quit()`, never the Quit question, whose Stop ends the host GPUI needs. `desktop:main.ts:559`
- W3-SWT-028 With it on, a Dock click, a second launch and a wsp:// link reach today's app first and must hand over too. `desktop:main.ts:213`

### Host operations

- W3-SWT-029 No bridge member opens an app but a checked wsp download (`quitAndOpen`); `app.relaunch` is used nowhere. Gap: the switch's own member. `desktop:preload.ts:51`
- W3-SWT-030 No op stops or restarts the host on a switch; `stopWsp` runs on the Quit question's Stop alone. `desktop:main.ts:566`
- W3-SWT-031 Gap: nothing reads whether GPUI is installed; a shell read, as `loginStart`. `protocol:views/desktop-bridge.ts:200`
- W3-SWT-032 `app.requestSingleInstanceLock` on `<home>/desktop`; GPUI `File::try_lock` on `desktop-gpui.lock`. `desktop:main.ts:86` exists: `startup.rs:90`
- W3-SWT-033 Mac: `/usr/bin/open` on the bundle, as the swap opens the new app; Linux has none today (the AppImage is only shown); then `app.quit`. `desktop:self-update.ts:327`
- W3-SWT-034 GPUI then asks `golden.get`, `workspaces.list` to pick onboarding or the app. `desktop:host-lifecycle.ts:469` exists: `startup.rs:232`

### Design

- W3-SWT-035 Row `ui::Row` with `.note`, `.control`, `.on_reset`; card `ui::Card` with `.head`. `settings/rows.tsx:153`
- W3-SWT-036 Switch `ui::Switch`, `.held`. `components/ui/switch.tsx:8`
- W3-SWT-037 Switch below 640 px: 20 px thumb, 22 by 38: none yet; `ui::Switch` draws the 16 px thumb alone (`app/src/ui/toggle.rs:19`). `components/ui/switch.tsx:12`
- W3-SWT-038 Slot under the words below 640 px: none yet; `ui::Row` keeps it beside (`app/src/ui/card.rs:705`). `settings/rows.tsx:123`
- W3-SWT-039 Motion: track background-color 200 ms, default curve; thumb translate 150 ms, CSS `ease`; press widens 100 ms after 100 ms. `components/ui/switch.tsx:20`

## Settled choices

How the GPUI app meets today's app where the two differ underneath. Items cite these by label. Read from code on
`main` and gpui/main unless a line says it ran.

**D1. The browser road: the system's web view through wry, a child of GPUI's window on our gpui-pre.** Today
Chromium composites the `<webview>` into the page, so menus, tooltips and dialogs draw live over it; it hides only
under a modal over the sheet (`components/preview/BrowserGuests.tsx:128`), and no snapshot swap exists. The app embeds
wry 0.53.5 (`app/Cargo.toml:49`) on gpui-pre 0.3.9 with our patches, not gpui-kit's `crates/webview` and not
`gpui-fast`. On Linux the page is WebKitGTK in an X11 window wry makes a child of GPUI's, shaped by X11 SHAPE to its
rectangle less whatever GPUI draws over it (`app/src/screens/browser/page/linux.rs:288`); on the Mac it is a WKWebView
subview whose layer is masked to its rectangle less those holes and whose `hitTest:` passes a point in a hole to
GPUI's view (`page/mac.rs:2`). A press on the page is replayed into GPUI, and the app's chords reach GPUI while the
page holds the keys (`page/mod.rs:2`, `page/linux.rs:2`). Every tab shares one store in the app's data folder, as
today's `persist:wsp-browser` (`page/mod.rs:109`, `app/src/main.rs:170`); its service workers are cleared before the
first page loads and an init script stands in for today's block (`page/linux.rs:132`, `:417`, `page/mac.rs:91`). The
engine is WebKit, so Web Inspector stands in for Chromium's tools. The app must hold the page in the pane on a
Wayland session too, as today's does (`components/preview/BrowserGuests.tsx:117`). Now neither WebKitGTK nor wry
embeds a page in a Wayland window, so on one the pane opens each page in the system browser and says so
(`ON_WAYLAND`, `page/mod.rs:84`, its line at `:87`); asking GPUI for X11 so the app runs on XWayland
(`OnWayland::XWayland`, `page/mod.rs:91`) is built but not picked. `desktop-gpui --screen browser` proves the cut, the presses and the keys on a real window
(`app/src/screens/browser/check.rs:2`). What the page must show: `more.md`.

**D2. Host start: `host-lifecycle.ts`'s start, ported into the host client.** The app opens on the host the lock
names, else the one this computer's service manager brings up from its unit, and waits on it with today's waits and
words (`host-client/src/start.rs:1`, `ready` at `:996`). It reads the lock with D9's checks (`start.rs:870`). A unit
that stands is loaded as it stands (`launchctl enable`, `bootstrap gui/<uid> <plist>`; `systemctl --user
daemon-reload`, `enable`, `restart`; names from sha256(state)[0:8]; `start.rs:125`) and never removed by an open. A packaged app
carries its own host as today's does: a Node, the host's built command and the daemon (`scripts/package.sh:4`,
`app/src/service/mod.rs:1`). It writes the wsp command (`<home>/bin/wsp`) to run them; a missing unit is written by
that command's `wsp up --service`; a host or command of an older release is taken over and restarted onto this
bundle, a newer one is left and opened on, and a wsp command today's app wrote, naming an app that stands, is left
alone while Go Back is offered (`service/mod.rs:5`, `start.rs:426`). A build that carries no host writes nothing: with
no unit, the box says to open the wsp app once (`NOT_SET_UP`, `start.rs:66`). Quit and stop wsp stops the host and
removes its unit, as today's does through `wsp down`'s road (`desktop:host-lifecycle.ts:675`): the manager unloads it
(`launchctl bootout`; `systemctl --user disable --now`, `daemon-reload`) and its file goes, so the next login starts
nothing (`host-client/src/start.rs` `stop`, called by `app/src/desktop/quit.rs` `stop_wsp`). Tests run the managers through the `Runner` trait
(`start.rs:344`) with a fake that records each argv (`host-client/src/start/tests.rs:171`).

**D3. One host for both apps; the switch is the shell's, kept as Start at login is.** Both apps serve
`<WSP_HOME or ~/.wsp>/state.json`, `host.lock` and `host-token` beside it (`host:host-lock.ts:132`,
`app/src/main.rs:17`). Quitting today's app stops nothing (`desktop:host-lifecycle.ts:297`,
`apps/desktop/test/smoke.electron.test.ts:590`) but through the Quit question's Stop (`desktop:main.ts:559`). The
locks are different files (`<home>/desktop`, `desktop-gpui.lock`), so both apps can run. Today's app reads no
preference before its window (`desktop:main.ts:429`). The switch lives in today's app (`desktop:experimental.ts:1`,
`desktop:main.ts:300`): (1) a shell setting, a bridge read and write pair as `loginStart` (`settings/settingsReads.ts:62`) over a file in `<home>/desktop/`, read before `whenReady`: no
host field, no record change, no older host refusing it; (2) on: open GPUI, then `app.quit()`, never the Quit
question, once the open went, as Get's Quit and open (`desktop:get-bundle.ts:306`); an open that fails turns the
switch back off and the window stays and says why (`desktop:experimental.ts:132`); no confirm, as no Settings switch confirms; (3) while on, every way in (launch, Dock, second launch, a link,
`desktop:main.ts:209-219`, `:636`) opens GPUI and quits (`desktop:experimental.ts:110`); (4) GPUI's way back clears
the file, opens today's app and quits, from its app menu on the Mac (`app/src/screens/app_shell/menu.rs:92`,
`app/src/desktop/switch.rs:275`); its Settings has no such row, and Linux, with no menu bar, has no way back: the app
must offer one there; (5) each finds the other in the home: the shim names today's
executable (`desktop:shim.ts:24`, AppImage copy `:63`), GPUI writes `<home>/desktop-gpui.path` at launch
(`app/src/desktop/switch.rs:65`); macOS `/usr/bin/open` as the swap does (`desktop:self-update.ts:327`); Linux a
detached spawn whose environment keeps WSP_HOME as `swapEnv` does (`:385`, `app/src/desktop/switch.rs:194`); (6) the
row draws only when that file names an app and the window is on the host here (as Get's download,
`shell/update.ts:77`): no `labs` gate, no `WSP_LABS=1`; (7) a card "Experimental" at the end of General's desktop
cards, as Logs (`settings/general.tsx:192`); (8) with D4 a GPUI older than the host still attaches, so today's app
updating no longer strands it.

**D4. Release rule and closed enums: attach to a newer host, an `Unknown` arm on every string enum.** The client
attaches to a host of its own release or a newer one (`ReleaseRule::OwnOrNewer`, `host-client/src/lib.rs:437`,
`daemon/crates/wsp-mcp/src/release.rs:38`), so a GPUI build older than the host still opens. The protocol's types are
generated from the record and opened to a newer host: no type refuses a field the record does not list, and every word
list and union reads a value it does not know as `Unknown` (`host-client/build.rs:1`). Values come often:
WorkspacePhase gained two on 2026-10-12, event tags went from 17 to 27 in ten days, eight releases in nine days. Each
screen must draw an unknown value neutral and skip an unknown event, as the slate's "This part needs a newer
wsp" (`parity/wave-2/more.md`, W2-SLT-122); today the screens map `Unknown` arms one by one (for example
`app/src/screens/settings/pages/activity/wire.rs:475`). A newer host is said as the page says it ("this app is older than ...: get
the new app", `app/src/screens/notices/words.rs:281`). An older host serving this app's state file is replaced as
today's `replaceHost` replaces it (`desktop:host-lifecycle.ts:301`, `:311`): restarted onto this app's files, or asked
about first while turns run on it (`app/src/screens/dialogs/startup.rs:448`). Rows of other hosts are read whatever their release only to
count the turns running before a replace (`client_any_release`, `lib.rs:444`). A protocol number would need a host
change and a bump nobody remembers.

**D5. Updates: GPUI's own update road, ported from `get-bundle.ts` and `self-update.ts`.** No Electron updater
(`publish: null`, `apps/desktop/electron-builder.yml:16`): the host reads GitHub's latest release 30 s after start and
every 6 h (`host:release.ts:19`). The app downloads the newer release in the background, checks its sha256 and size,
and offers Restart, which swaps it in by a detached script that brings the host and daemon over and rolls back on any
failed step; any other install downloads the disk image or the AppImage into Downloads and offers Quit and open
(`app/src/update/mod.rs:1`, `update/swap.rs:1`). It replaces itself from its own three downloads (`GPUI_NAMES` in
`packages/protocol/src/bundles.mjs`, read at `update/feed.rs:79`), staged in `.wsp-gpui-update` beside the installed
app, not today's `.wsp-update`, since both apps can stand in one Applications (`update/stage.rs:63`). No release
workflow builds or publishes those downloads yet: the release must carry them before the road has anything to fetch.
At cutover the app must also meet today's last Electron release, which swaps into whatever `wsp-<v>-mac.zip` holds
and rolls back unless the new app writes its version to `.wsp-update/up` within 60 s (`desktop:self-update.ts:341`,
`desktop:main.ts:848`). Not used: gpui-updater 0.4 (DMG only, a Team ID), cargo-packager-updater (no release since
2025-07), Sparkle (a feed and a key).

**D6. Signing, notarizing, the DMG and the Linux build.** Today ships ad hoc (`apps/desktop/scripts/after-pack.mjs:25`);
the workflow signs and notarizes once `CSC_LINK`, `CSC_KEY_PASSWORD`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`,
`APPLE_TEAM_ID` are set (`.github/workflows/release.yml:173-197`). The GPUI app is built by one script, not a packager
(`apps/desktop-gpui/scripts/package.sh:1`): on macOS a universal "wsp Experimental.app" whose binary and Node are
joined by `lipo` (`:247`), with `CFBundleIdentifier`, `CFBundleShortVersionString` and the icon, its DMG by `hdiutil`
in today's drag layout and the zip by `ditto -c -k --keepParent` (`:345`); on Linux the binary and an x86_64 AppImage
on today's static runtime (`:61`). With `CSC_LINK` and `CSC_KEY_PASSWORD` set it signs with that Developer ID, inner
Mach-O first, `codesign --options runtime --timestamp`, the bundled Node with today's JIT entitlements and the app's
binary with none (`:298`); else ad hoc. With the three Apple names also set it notarizes with `xcrun notarytool
submit --wait` and staples (`:332`). Its bundle id is `wsp.desktop.experimental`, `WSP_GPUI_BUNDLE_ID` overriding it
(`:53`); no `CFBundleURLTypes` while Experimental, since today's app keeps `wsp://` and hands links over (`:260`).
Still to meet: no release workflow runs the script, so the release must build, sign and publish the GPUI app; the
AppImage carries no libxkbcommon-x11 or Vulkan loader of its own and must run on the system's; at cutover the bundle
id must become `wsp.desktop` and the bundle must claim `wsp://`. Outside the repository it needs an Apple Developer
Program membership and its Team ID; a Developer ID Application certificate, exported as .p12 with a password into
`CSC_LINK`, `CSC_KEY_PASSWORD`; an app-specific password or an App Store Connect key; `APPLE_ID`, `APPLE_TEAM_ID`,
`APPLE_APP_SPECIFIC_PASSWORD` as repository secrets. Linux needs no key. The same secrets sign Electron's next
release.

**D7. Motion for the browser pane: today's, drawn by GPUI around a page that never animates.** Loading bar scaleX
0.04 to 0.9 over 5.3 s, `cubic-bezier(0.1, 0.5, 0.2, 1)`, to 1 in 150 ms, fading after 220 ms, CSS `ease-out`, held
at 0.9 under reduced motion (`index.css:1397`); Refresh spins 1 s linear, reduced motion or not; the system browser
button fades and the field's end padding moves, 150 ms default curve; the zoom pill fades and drops 4 px, 200 ms
`cubic-bezier(0, 0, 0.2, 1)`; tooltips fade and slide 2 px, 150 ms; More opens with no transition; the sheet slides
32 px with opacity, 200 ms default curve; the page jumps to its slot on the slide's `transitionend`
(`BrowserGuests.tsx:154`). The app draws the bar's grow, hold and fade (`app/src/screens/browser/chrome.rs:24`,
`:201`) and the zoom pill's curve (`bars.rs:170`) over gpui-kit's `Transition`, as `parity/wave-2`'s D7. Refresh
must keep turning under reduced motion, as today's does (`components/preview/PreviewChromeRow.tsx:172`); now it turns
with `ui::status::turning`, which stops there (`chrome.rs:125`, `app/src/ui/status.rs:147`). The page is moved to its slot after each frame that
moves the slot (`page/mod.rs:239`), not only once a layout settles; in a sheet it leaves while a menu stands over it
(`screen.rs:718`).

**D8. A newer host at launch.** Today's app means to attach and say the app is older (`desktop:host-lifecycle.ts:305`),
but the first-launch read then dials with the release check, which refuses at `auth` (`:611`,
`host:verbs/client.ts:331`): likely "wsp could not start". Read, not run; no test covers it. GPUI attaches to a newer
host (D4, `host-client/src/lib.rs:437`) and says the app is older (`app/src/screens/notices/words.rs:281`).

**D9. The token leaves only after Electron's three checks:** the lock's pid alive and this login's, a wsp page on its
port, the page's `tokenHash` = sha256(host-token) (`desktop:host-lifecycle.ts:124-142`). wsp-mcp makes all three
before the token is read (`proved_token`, `daemon/crates/wsp-mcp/src/aim.rs:218`), and tests whose listener on the
lock's port serves no wsp page, or another token's digest, never read the token
(`host-client/tests/token_checks.rs:78`, `:91`).

**D10. Native pieces GPUI lacks.** gpui-pre 0.3.9 has notifications (UNUserNotificationCenter; notify-rust over
D-Bus, no GTK, with our Linux patch `patches/gpui-pre-linux-0.3.9.notifications.patch`) with clicks by tag,
`prevent_idle_sleep`, `play_system_bell`, `request_attention`, client decorations, and `on_open_urls` on macOS; no
status item, badge, parentless box or move. The tray is drawn by `tray-icon` 0.26 and `muda` 0.21 (MIT or Apache-2.0)
on macOS alone (`app/Cargo.toml:65`, `:92`); muda has no sublabel, so a row's state rides in its label after a middle
dot, a listed difference (`app/src/desktop/tray.rs:2`). Notifications are GPUI's own with the system's alert sound
beside a line that sounds; when the system refuses one, the app logs it and bounces the Dock (`desktop/notify.rs:2`,
`:275`). The Dock's badge and bounce, the alert sound, app-modal alerts with no parent and the move to Applications go
through `objc2-app-kit` (`desktop/mac.rs:2`); the move copies by `ditto`, then a detached shell relaunches the app
once this process is gone (`mac.rs:56`). Links: `on_open_urls` on macOS (`app/src/main.rs:144`); on Linux the argument
of a second launch, carried over the lock's socket (`desktop/links.rs:2`). Mac only: tray, badge, alerts, `pmset -g
assertions`, `open-url`, notifications (a Developer ID bundle; ad hoc only logs). On Linux: notifications against a
fake server under `dbus-run-session` (`app/tests/desktop_feed.rs:249`), links by argv.

**D11. The Linux frame, Quit and zoom.** Electron draws its controls over the 52 px header on the desktops
`desktop:window.ts:64` lists. The app picks its frame the same way: on those desktops no system frame, with its own
minimize, maximize and close drawn over the header's right end on its ground and ink; on every other Linux desktop the
desktop's own frame (`app/src/desktop/frame.rs:1`, `:96`, `:195`). Linux has no menu or tray, so closing the last
window quits unasked, as today (`app/src/main.rs:160`). The View menu's zoom scales every window by Chromium's steps
and keeps its level across launches, as Chromium keeps a page's level (`desktop/zoom.rs:1`, through our gpui-pre
patch's `set_window_zoom`); a focused terminal keeps the chords for its own text size. View holds Actual Size, Zoom
In, Zoom Out and Toggle Full Screen; Reload, Force Reload and Developer Tools are dropped, as listed differences
(`app/src/screens/app_shell/menu.rs:227`).

**D12. Today's browser faults.** Today Back never returns to the servers list, Stop reloads, Hard reload is a plain
reload, a port tab ignores the page's title, tabs come back empty after a reload, no key focuses the field, and New
thread's panel names "place:here" (W3-BRW items cite each). The app must match each, as the look and behaviour are
today's: Back must never reach the servers list (`components/preview/BrowserGuests.tsx:81`) and Stop must reload
(`components/preview/BrowserSurface.tsx:165`); now the app's Back reaches the list and Stop stops
(`app/src/screens/browser/check.rs:496`, `:489`, `screen.rs:276`). Hard reload is a plain reload (`more.rs:212`) and a
port tab is named by its process and path, never the page's title (`tabs.rs:92`), as today. The frame path for a window
with no guests (a browser tab, a host elsewhere, `BrowserSurface.tsx:246`) goes: GPUI holds a page whatever the host
(`page/mod.rs:100`).

**D13. Items that replace W2-UNO items.** W2-UNO-011 to 015, 047 to 053 and 069 to 071 are replaced by the W3-SHL items marked
"was". The starting window, start-failed box and first-launch read stay W2-DLG-017, 038, 039 and 054; the W3-SHL
items for them name each.
Each `window.wsp` member becomes an in-process call made by the part that lists it (`app/src/desktop/`).

## Baselines

`states.mjs` drives today's app to every Shot state for `parity/baseline.mjs --wave 3`, on the host
`bench/fixture.sh` makes from a `main` checkout with a wsp-daemon staged (`parity/wave-2/spec.md`'s D1): without one the thread's
port list never fills and every port reads as listening, so the down bar states show nothing. Browser states make a
thread each with `wsp run`; Q is started in bench's folder, again after W3-BRW-035 stops it, and stopped by its pid; P runs in the driver; both
keep fixed ports (47801, 47803; 47802 stays free), and the driver refuses to start if another process holds any of the three. Use a fresh fixture: the app keeps a
thread's panel between runs, so a second run on one fixture draws an extra Browser tab in W3-BRW-043. States clip
the panel, a card or the notice; narrow and cap states keep the whole window. `baseline.mjs` needed no change.
Shell duties have no page picture: their Check lines are the proof, most of them a port of the named
`apps/desktop/test` case, and the native ones (tray, menus, alerts, notifications, badge) run on the Mac.
