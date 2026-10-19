# Wave 3 parity spec: more items

The items `spec.md` had no room for, in the same id sequences and the same shape. Builders tick these too. The last section holds what the browser spike must show for each road (D1).

## Browser pane (#2401)

### Elements

- W3-BRW-097 Window glyph: small card, three 3 px dots (red, amber, green at 80%), two 2 px lines. `components/preview/BrowserMockup.tsx:7`
- W3-BRW-098 Behind line (joined computer's daemon too old): in the empty sentence's place, or over the list. `components/preview/BrowserSurface.tsx:68`
- W3-BRW-099 Hint bar with no guests: "Only ports on the thread's computer show here, like localhost:3000. Other sites open in a new browser tab." `components/preview/BrowserSurface.tsx:40`
- W3-BRW-100 More button's tooltip "More"; its label stays "Preview menu". `components/preview/PreviewMoreMenu.tsx:60`
- W3-BRW-101 Zoom row: "Zoom" at left; the reset is ghost with a foreground 10% hover. `components/preview/PreviewMoreMenu.tsx:115`
- W3-BRW-102 Refusal titles: ":N answered 401 through the preview route" (detail names the one remint), ":N answered 403 through the preview route". `browser/refusal.ts:25`
- W3-BRW-103 Down bar detail tails: ", which is still running"; ", which still listens", dropped once a fetch finds the port down. `adapt/ports.ts:116`
- W3-BRW-104 Copy failure notice "Not copied: <said>", none while the host link is down. `notices/store.ts:133`
- W3-BRW-105 Cap notice tail: none, "; 1 of them is in another thread", "; N of them are in other threads". `rightPanelStore.ts:653`
- W3-BRW-106 Behind line words: "<computer> is behind: <gap>; wsp add <computer> --update puts this wsp's daemon on it". `protocol:wire/daemon-version.ts:833`
- W3-BRW-107 Recent row X: 28 px ghost, 8 px from the right; shows on hover and on keyboard focus. `components/preview/PreviewRecentUrlCard.tsx:44`

### States

- W3-BRW-108 Server back: bar goes, page reloads once. Check: after W3-BRW-035 start Q; bar gone within 6 s, one reload. `components/preview/BrowserSurface.tsx:128`
- W3-BRW-109 404, 500. No shot: the server's own body; a thread here is never probed, so the app adds nothing. `components/preview/BrowserSurface.tsx:71`
- W3-BRW-110 Search words. No shot: needs the internet; the field reads `https://www.google.com/search?q=hello%20world`. `browser/url.ts:107`
- W3-BRW-111 More menu, no page: page rows and zoom held. Shot: W3-BRW-028, then "Preview menu". `components/preview/PreviewMoreMenu.tsx:43`
- W3-BRW-112 Zoom pill. Shot: W3-BRW-037, "Zoom in", close the menu, about 700 ms. `components/preview/ZoomIndicator.tsx:28`
- W3-BRW-113 Copied. Shot: W3-BRW-031, "Copy URL", within 1 s. `components/preview/BrowserSurface.tsx:316`
- W3-BRW-114 Sheet below 760 px, 384 px wide. Shot: W3-BRW-031, then width 759. `rightPanelLayout.ts:8`
- W3-BRW-115 A port off the list that still answers (Docker), fetched every 2 s while down. No shot: cloud or joined only. `browser/answering.ts:25`
- W3-BRW-116 Behind line. No shot: a joined computer with an old daemon. `components/preview/PreviewEmptyState.tsx:45`
- W3-BRW-117 Leave question. No shot: native dialog. `desktop:main.ts:291`
- W3-BRW-118 Crashed page: logged, nothing drawn. No shot: needs a renderer killed. `desktop:main.ts:100`
- W3-BRW-119 Certificate error, download. No shot: no handler; Electron's defaults (load refused, save dialog). `desktop:guests.ts:120`
- W3-BRW-120 Row hover: accent ground; the recent row's X shows. Shot: W3-BRW-029, hover the recent row. `components/preview/PreviewRecentUrlCard.tsx:27`
- W3-BRW-121 Zoom at a limit: Zoom in at 200% (out at 50%) stays live and does nothing; no pill. No shot: a near duplicate of W3-BRW-038. `browser/tabs.ts:126`
- W3-BRW-122 Ports not said yet: before the daemon's first watch reply no port shows the down bar. Check: no staged daemon, type `localhost:47802`, no `[title*="is not answering"]`. `components/preview/BrowserSurface.tsx:84`
- W3-BRW-123 Route minting: the body stays blank, no bar, until the route is back. No shot: cloud or joined computer only. `components/preview/BrowserSurface.tsx:79`

### Actions and keys

- W3-BRW-124 Copy URL, row or menu; a refused write raises the copy failure notice. `components/preview/BrowserSurface.tsx:155`
- W3-BRW-125 Right-click in a page: no menu. `desktop:guests.ts:120`
- W3-BRW-126 The first place typed in the placeholder asks the cap, then a new tab at the strip's end takes its place. `rightPanelStore.ts:308`
- W3-BRW-127 New tab (menu) on the placeholder opens nothing new: there is one placeholder at most. `components/preview/BrowserSurface.tsx:196`
- W3-BRW-128 Entering the address already shown loads nothing, yet moves it first in recents. `browser/tabs.ts:92`
- W3-BRW-129 Escape with no page empties the field, so the filter clears. `components/preview/PreviewChromeRow.tsx:203`
- W3-BRW-130 A page's in-page moves (hash, pushState) move the field and the tab's place too. `components/preview/BrowserGuests.tsx:100`
- W3-BRW-131 Switching tabs remounts the surface: a typed filter, a half-typed address and the hint go; the page stays. `shell/RightPanel.tsx:53`
- W3-BRW-132 The filter reads only the 8 recents shown (URL, title), then servers by host:port, URL, process. `components/preview/PreviewEmptyState.tsx:35`
- W3-BRW-133 A workspace the host stops listing drops its panels and their browser tabs. `rightPanelStore.ts:678`
- W3-BRW-134 Palette and row menu "Open browser": Globe glyph, mod+shift+j label, found by "open browser", "preview", "ports". `actions/workspaceActions.ts:186`

### Host operations

- W3-BRW-135 Push `port.open`, `port.close`, `workspace.deleted` on the host stream feed the same list. `browser/model.ts:130`
- W3-BRW-136 `webRequest.onBeforeSendHeaders` on that session: a loopback service worker script is cancelled. `desktop:main.ts:437`
- W3-BRW-137 IPC `guests:allowed` (sync): the app's own host's page only; any other frames ports in an iframe. `desktop:main.ts:283`
- W3-BRW-138 `navigator.clipboard.writeText`; localStorage `wsp:browser-recents:v1:<workspace>`. `browser/recents.ts:17`
- W3-BRW-139 `app.on("render-process-gone")`: a guest logs "renderer gone: <reason> (exit N) at <url>; dumps in <dir>"; no reload. `desktop:main.ts:98`
- W3-BRW-140 `dialog.showMessageBox` for the leave question: type question, Cancel the default and Escape's answer. `desktop:main.ts:293`

### Design

- W3-BRW-141 List: headings 12 px muted, 4 px in; groups 24 px apart; heading 8 px over rows; rows 2 px apart; 13 px lines. `components/preview/PreviewEmptyState.tsx:18`
- W3-BRW-142 Rows: 28 px glyph, 12 px gap, 8 px padding, 6 px radius, 2 px inset ring on key focus; address mono 12 px tabular. `components/preview/PreviewLocalServerCard.tsx:21`
- W3-BRW-143 Motion: reduced motion, the bar stands at 0.9 with no transition. `index.css:1410`
- W3-BRW-144 Motion: rows background-color 150 ms, default curve; recent X opacity at once. `components/preview/PreviewRecentUrlCard.tsx:27`
- W3-BRW-145 Motion: buttons color, background, border; field border, shadow; 150 ms, default curve. `components/ui/button.tsx:22`
- W3-BRW-146 Refusal card detail: 12 px muted, 4 px under its title. `components/ui/empty.tsx:41`
- W3-BRW-147 Field type: 14 px, placeholder ink, 26 px input in the 28 px group, 9 px side pad. `components/ui/input.tsx:25`
- W3-BRW-148 Loading bar, inline: the row's 12 px gap sits inside its box, so the bar runs on the page's top edge; round right end. `components/preview/PreviewChromeRow.tsx:318`
- W3-BRW-149 Zoom pill off the Mac: glass backdrop, blur and saturation. `components/preview/ZoomIndicator.tsx:48`
- W3-BRW-150 More menu: dropdown glass, 12 px radius, 4 px pad, 28 px rows, accent highlight, held rows 64%, 6 px under, end aligned. `components/ui/menu.tsx:62`
- W3-BRW-151 Icon buttons: row 24 px with 14 px muted glyph, accent hover with foreground ink; recent X 28 px, 16 px glyph. `components/ui/button.tsx:31`
- W3-BRW-152 Motion: More menu opens and shuts with no transition. `components/ui/menu.tsx:62`
- W3-BRW-153 Motion: row tooltips, opacity and 2 px slide, 150 ms, default curve; 13 px, popover ground, above, 4 px off, z 140. `components/ui/tooltip.tsx:46`
- W3-BRW-154 Motion: panel drag line colour, 150 ms, default curve; W1-FRM-101 has the look only. `components/preview/RightPanelResizeHandle.tsx:31`
- W3-BRW-155 Motion: sheet opacity, translate x 32 px, 200 ms, ease-in-out (default curve); the page stays where measured until transitionend. `components/ui/sheet.tsx:72`

## The desktop shell (#2402)

### Elements

- W3-SHL-119 File (Mac): Close Window alone. `desktop:menu.ts:23` exists: `menu.rs:77`
- W3-SHL-120 Edit, Electron's role: Undo, Redo, Cut, Copy, Paste, Paste and Match Style, Delete, Select All, Speech. `desktop:menu.ts:26` exists: `menu.rs:47` (no Paste and Match Style, Delete, Speech)
- W3-SHL-121 Tray lists running, waiting and failed-unopened threads, waiting first, then newest. `desktop:tray.ts:138`
- W3-SHL-122 Tray tail: rule, "Open wsp", "Quit wsp". `desktop:tray.ts:127`
- W3-SHL-123 Bundle: id `wsp.desktop`, name wsp, developer-tools category, en-US only, Electron 43.5.1, no asar. `apps/desktop/electron-builder.yml:1`
- W3-SHL-124 The Mac bundle carries every target's daemon, the Linux ones for forks, as thin per-arch files. `apps/desktop/electron-builder.yml:35`
- W3-SHL-125 Linux: AppImage, x64 only, static runtime 1.0.3 with its own FUSE. `apps/desktop/electron-builder.yml:64`
- W3-SHL-126 Older host detail: "A thread is working" or "N threads are working on this computer. " where counted, then "Running turns carry on while wsp restarts. This app opens only on wsp <app>, so Quit leaves wsp <theirs> serving and closes the app." `desktop:host-lifecycle.ts:404`
- W3-SHL-127 Serves-again after a failed replace: "wsp <theirs> still serves your threads; this app could not restart it as <app>", detail "The restart failed: <why>. ...". `desktop:host-lifecycle.ts:365`
- W3-SHL-128 Host names: the host here is the computer's name (window, menu, log); an account host is its alias. `desktop:host-lifecycle.ts:78`, `:74`
- W3-SHL-129 Desktop read off XDG_CURRENT_DESKTOP, else XDG_SESSION_DESKTOP mapped (plasma*, ubuntu*, gnome-*, budgie-desktop). `desktop:window.ts:83`
- W3-SHL-130 Ground before the page paints: #09090b; clear on the Mac. `desktop:window.ts:17`
- W3-SHL-131 One app window at a time; onboarding and starting windows hand over to it. `desktop:main.ts:112`
- W3-SHL-132 Window (Mac): Minimize, Zoom, Bring All to Front, the window list. `desktop:menu.ts:29` exists: app/src/screens/app_shell/menu.rs:81 (Minimize, Zoom).
- W3-SHL-133 Windows (never built): File holds Close Window, rule, Quit wsp; Window holds Minimize, Zoom. `desktop:menu.ts:25` exists: app/src/screens/app_shell/menu.rs:83.
- W3-SHL-134 Tray tooltip "wsp <title>", or "wsp" (was W2-UNO-051). `desktop:main.ts:737`
- W3-SHL-135 Tray failed row: Open only; Allow, Deny only while its prompt is open, picked by option effect. `desktop:tray.ts:103`
- W3-SHL-136 Quit detail: wsp keeps running after the app quits; none, one or N threads working here (was W2-UNO-053). `desktop:quit.ts:31`
- W3-SHL-137 "wsp did not stop" error box with the stop's error (was W2-UNO-012). `desktop:main.ts:571`
- W3-SHL-138 Folder picker: system panel, folders only, "Choose a folder", a sheet. `desktop:main.ts:151` exists: app/src/screens/dialogs/add_project.rs:539 (no title).
- W3-SHL-139 Notification copy: "wsp needs you" with the prompt; "<title> finished", where; "<title> stopped", error line; plan alert; setup. `protocol:outside-line.ts:43`
- W3-SHL-140 App icon: the mark #f4f4f5 at 60% on a #09090b square 824/1024, 22.49% corner; icns 16 to 1024. `apps/desktop/scripts/icons.mjs:17`
- W3-SHL-141 Mac: universal dmg, zip, dir; dmg "wsp <version>", 660x400, app at 172,186, Applications 488,186, 128 px. `apps/desktop/electron-builder.yml:25`
- W3-SHL-142 No native Mac window tabs (no tabbingIdentifier), so ctrl+tab and the digit chords reach the page. `desktop:window.ts:32`
- W3-SHL-143 Tray head and lost lines are disabled rows; only Start wsp, Open wsp, Quit wsp and each thread's submenu take a click. `desktop:main.ts:724`
- W3-SHL-144 Get hover, Mac dmg: "Downloads the disk image and checks its sha256. Unsigned builds need Privacy & Security, Open Anyway, once." `desktop:get-bundle.ts:57`
- W3-SHL-145 Get hover, AppImage: "...into Downloads and checks its sha256. The app you run is not replaced."; in place: "...installs it when the app restarts." `desktop:get-bundle.ts:62`
- W3-SHL-146 Get refusals before a fetch: "not a release version", "<v> is not a release above this app", "no wsp bundle is built for <platform>". `desktop:get-bundle.ts:22`
- W3-SHL-147 Get refusals after: no sha256 and size for <asset>, ran past the size, could not be saved, "the checked update is gone; get it again". `desktop:get-bundle.ts:25`
- W3-SHL-148 Staged copy refused and deleted: not a readable zip, an entry reaching outside the app, not exactly one app, stage folder not this account's. `desktop:self-update.ts:26`
- W3-SHL-149 Staged copy refused after unpack: could not be unpacked, signature check, "holds wsp <got>, not <want>", no executable, quarantine flag kept. `desktop:self-update.ts:30`
- W3-SHL-150 Tray refusal copy, first letter raised: prompt closed first, no prompt this host can answer, no turn of that thread, no such option. `protocol:words/tools.ts:693`
- W3-SHL-151 Notification moments past W3-SHL-139: sign-in ("wsp needs you", what), "<workspace> is awake", "<computer>: setup failed" with its line. `protocol:outside-line.ts:47`
- W3-SHL-152 More moments: "wsp needs you" with "<computer> needs you: <what>"; "<computer> is ready" (setup, "; <missed>"); image ready with its name. `protocol:outside-line.ts:59`
- W3-SHL-153 Version notice variants: names "the host <alias>" elsewhere; app newer: ": run the app's own host", no button; no app version: "this app is older...". `protocol:words/places.ts:199`
- W3-SHL-154 Bundle tree: main.mjs, cli.mjs (the wsp command), preload, onboarding and starting pages, tray art, agent glyphs, host assets web, daemon, cli. `apps/desktop/scripts/stage.mjs:28`

### States

- W3-SHL-155 No manager (not darwin or linux): "wsp writes no service on <platform>; …". Check: `host-lifecycle.test.ts:697`. `host:service.ts:371`
- W3-SHL-156 Given up after the long wait: "after <t> of waiting, " and the lock's refusal or "the host at <addr> did not answer". Check: `host-start-wait.test.ts:301`. `desktop:host-lifecycle.ts:590`
- W3-SHL-157 Home not writable: "the wsp command could not be written at <home>/bin/wsp: <why>". Check: none on main; WSP_HOME on a 0500 folder. `desktop:main.ts:802`
- W3-SHL-158 Home missing: made on the way (bin here, logs by the app log, state by the host). Check: `smoke.electron.test.ts:1546`. `desktop:shim.ts:35` exists: `startup.rs:108` (state folder, 0700)
- W3-SHL-159 Older host on another program's unit: rewritten for this shim, its words and login setting kept. Check: `host-lifecycle.test.ts:1040`. `desktop:host-lifecycle.ts:245`
- W3-SHL-160 Older host box, Quit: it keeps serving, the starting window goes, the app quits. Check: `host-lifecycle.test.ts:944`. `desktop:main.ts:616`
- W3-SHL-161 Older host nothing marked: refused, "… serves <state> from a process wsp did not start: …". Check: `host-lifecycle.test.ts:1315`. `desktop:host-lifecycle.ts:425`
- W3-SHL-162 Older release back after its replace, or a failed replace: left serving, serves-again box once a run. Check: `host-lifecycle.test.ts:1200`. `desktop:main.ts:694`
- W3-SHL-163 Host restarting as the app opens: wait for the lock to settle, attach to the next host. Check: `host-lifecycle.test.ts:397`, `:548`. `desktop:host-lifecycle.ts:603`
- W3-SHL-164 Unit naming another app's shim (moved or updated): stopped and rewritten. Check: `host-lifecycle.test.ts:261`. `desktop:host-lifecycle.ts:245`
- W3-SHL-165 Login PATH read only when PATH is launchd's four folders, or on Linux for an AppImage; it lends missing sign-in variables. Check: `packages/host/test/login-path.test.ts:74`. `host:login-path.ts:53`
- W3-SHL-166 Minimised, then raised (Dock, link, launch, notification, tray): restored, active, focused. Check: `xdotool windowminimize`, relaunch, `xdotool getactivewindow`. `desktop:main.ts:237`
- W3-SHL-167 Tray, none working: no count, the mark, "No threads working". Check: `osascript` reads menu 1 of menu bar item 1 of menu bar 2. `desktop:tray.ts:144`
- W3-SHL-168 Sound-only line: the system alert sound, nothing shown (was W2-UNO-069). Check: port `needs-you.test.ts:79`. `desktop:needs-you.ts:65`
- W3-SHL-169 Update failed: "wsp <v> was not downloaded: <said>" (sha256, size, no bytes for 30 s, GitHub's status, offline). Check: port `get-bundle.test.ts:88`. `shell/update.ts:109`
- W3-SHL-170 Launch after a swap: answers it with its version, else sweeps the stage (D5). Check: `.wsp-update/up` holds the version. `desktop:self-update.ts:406`
- W3-SHL-171 Move offered: a packaged Mac bundle outside Applications; declined runs on; a failed move is logged. Check: open from ~/Downloads. `desktop:move.ts:18`
- W3-SHL-172 App log `<home>/logs/app.log`: 2 MB rotation, 4 files, 16 KB entries, credentials blanked. Check: port `app-log.test.ts:41`. `desktop:app-log.ts:14`
- W3-SHL-173 Up and gone, or up as another release: "wsp started and stopped again; ..."; "... came up as wsp <v>; its log is <log>". Check: `host-lifecycle.test.ts:1200`. `desktop:host-lifecycle.ts:318`
- W3-SHL-174 Host before 0.3.0 (page names no release): refused, "a wsp older than this app's <app> ...". Check: `host-lifecycle.test.ts:1112`. `desktop:host-lifecycle.ts:134`
- W3-SHL-175 Unit wsp up --service wrote behind this shim, or let go: loaded as it stands. Check: `host-lifecycle.test.ts:281`, `:317`. `desktop:host-lifecycle.ts:258`
- W3-SHL-176 Unit off at login: loaded, set back off once up. Check: `host-lifecycle.test.ts:327`. `desktop:host-lifecycle.ts:262`
- W3-SHL-177 Host whose program is gone (AppImage mount): manager told to start it again. Check: `host-lifecycle.test.ts:668`. `desktop:host-lifecycle.ts:258`
- W3-SHL-178 Load refused as another launch loaded it a moment before: that load waited on. Check: `host-lifecycle.test.ts:291`. `desktop:host-lifecycle.ts:255`
- W3-SHL-179 Provider key only in the launching shell: a line in the app log. Check: `host-lifecycle.test.ts:305`. `desktop:host-lifecycle.ts:236`
- W3-SHL-180 Host bound beyond loopback: the lock alone is the proof; dialled at its own address. Check: `host-lifecycle.test.ts:907`, `:899`. `desktop:host-lifecycle.ts:136`
- W3-SHL-181 Login shell failed: app log "login shell: no PATH read (<why>); this host keeps the PATH it was started with". Check: `packages/host/test/login-path.test.ts:119`, `:142`, `:150`. `host:login-path.ts:87`
- W3-SHL-182 Wait pacing: lock read every 200 ms; manager asked for its pid every 1 s while no lock names one; a second exit fails. Check: none on main; a fake `print` as `host-start-wait.test.ts:115`. `desktop:host-lifecycle.ts:484`
- W3-SHL-183 Attached host still serving refuses the first-launch read: its refusal is the start-failed line, no redial. Check: `host-lifecycle.test.ts:606`. `desktop:host-lifecycle.ts:622`
- W3-SHL-184 Older host will not go: "wsp <v> (pid N) was stopped and still serves <state> after 20s", or "took the restart and still serves". Check: none on main; a host ignoring SIGTERM. `desktop:host-lifecycle.ts:435`, `:443`
- W3-SHL-185 Stop before a rewrite refused, or its answer unreadable (127, no user bus): that line is the failure, the unit stays. Check: `packages/host/test/service.test.ts:585`. `desktop:host-lifecycle.ts:248`
- W3-SHL-186 Setting the unit back off at login refused: the manager's line goes to the app log, the open goes on. Check: none on main; a runner refusing `launchctl disable`. `desktop:host-lifecycle.ts:264`
- W3-SHL-187 Feed check meets a lock it would refuse: nothing replaced, the dial says why. Check: none on main; a squatter page on the lock's port. `desktop:host-lifecycle.ts:339`
- W3-SHL-188 Lock with no pid, port or startedAt, or not JSON: no lock. Wildcard address: dialled on loopback. Check: `packages/host/test/pair-command.test.ts:347`. `host:host-lock.ts:116`, `:219`
- W3-SHL-189 App window's page fails to load after attach: start-failed box with the load's error; during a quit, no box. No shot: native box, no test on main. `desktop:main.ts:488`, `:603`
- W3-SHL-190 Later open failed, Open log: the log opens and the app stays in the menu bar; Quit quits. Check: `host-start-wait.test.ts:349`. `desktop:starting.ts:53`
- W3-SHL-191 Host here replaced while the window is on an account host: only the way back moves; on here, it reloads at the new address. Check: `host-switch.test.ts:95`. `desktop:main.ts:716`
- W3-SHL-192 Move refused (no record, token with no key, refused, silent): "no host named <a> is connected; this computer holds ..." or the host's line to the app log; window stays. Check: `host-switch.test.ts:160`, `:172`, `:195`. `desktop:host-switch.ts:86`
- W3-SHL-193 AppImage copies: the copy before stays (its host may still run it); older copies and stray `.part` folders go. Check: `shim.test.ts:51`. `desktop:shim.ts:74`
- W3-SHL-194 Bundle with no daemon binary for this computer: the shim runs the bundled command with no forwarder. Check: `shim.test.ts:21`. `desktop:main.ts:820`
- W3-SHL-195 npm `wsp` links: one app log line each (own file, points elsewhere, later release, now runs the shim, could not replace); a throw logs "the wsp on PATH was not checked". Check: `shim.test.ts:128`. `desktop:shim.ts:116`, `desktop:main.ts:867`
- W3-SHL-196 App log at launch: "wsp app <v> started (pid, os arch, electron <v>[, unpackaged])", "wsp command written|kept at", "wsp files at", "attached ...". Check: `smoke.electron.test.ts:1280`. `desktop:main.ts:88`
- W3-SHL-197 App log on failure: "<title>: <whole error>", the host's new lines as "host: <line>" (last 20), "main: <origin>: <stack>", "main: unhandled rejection". Check: `smoke.electron.test.ts:1278`; `host-start-wait.test.ts:234`. `desktop:main.ts:601`
- W3-SHL-198 App log on replace: "... serves <state>; restarting it as <app>", "did not restart itself (<why>)", "keeps serving", warn serves-again. Check: app.log after `smoke.electron.test.ts:1227`. `desktop:host-lifecycle.ts:429`, `desktop:main.ts:696`
- W3-SHL-199 App log on a page crash: "renderer gone: <reason> (exit N) at <url>; dumps in <dir>", then "reloading <url>[ in N s: 3 reloads in the last minute]". Check: `smoke.electron.test.ts:1283`. `desktop:main.ts:98`
- W3-SHL-200 Linux controls take the header's ground and ink per theme. Check: `import -window <id>` of the top right under GNOME, Paper then Graphite. `desktop:main.ts:186`
- W3-SHL-201 Zoomed (Window > Zoom or the frame's maximise). Check: `osascript` reads `size of window 1`. `desktop:menu.ts:29` exists: app/src/screens/app_shell/menu.rs:125.
- W3-SHL-202 Full screen (View, Ctrl+Cmd+F). Check: `osascript` reads AXFullScreen of window 1. `desktop:menu.ts:27` exists: app/src/screens/app_shell/menu.rs:126.
- W3-SHL-203 Reopened after a resize: 1280x800 again. Check: resize, close, Dock click, read the size. `desktop:window.ts:130` exists: app/src/screens/dialogs/startup.rs:55.
- W3-SHL-204 Title "• wsp" while a prompt waits. Check: `xdotool getactivewindow getwindowname`. `notices/hostNotices.ts:374`
- W3-SHL-205 Linux never asks the Quit question: no menu, no tray. Check: close and Ctrl+Q on Linux, no box. `desktop:main.ts:379`
- W3-SHL-206 Quit on a host elsewhere: nothing asked. Check: Hosts move, Cmd+Q, process gone. `desktop:main.ts:557`
- W3-SHL-207 Quit and stop wsp: service stopped, windows destroyed unasked, nothing reopens. Check: `apps/desktop/test/smoke.electron.test.ts:1028`. `desktop:main.ts:567`
- W3-SHL-208 Tray, refused pick: the sublabel is the refusal while the prompt stays. Check: port `apps/desktop/test/tray.test.ts:78`. `desktop:tray.ts:122`
- W3-SHL-209 Refused (no Developer ID): "notification not shown: <error>" logged, Dock bounce, sound if asked. Check: app.log on an ad hoc build. `desktop:main.ts:229`
- W3-SHL-210 Notifications unsupported: nothing built. Check: port `apps/desktop/test/needs-you.test.ts:171`. `desktop:needs-you.ts:69`
- W3-SHL-211 Click after the window closed: a window opens, the gone page is told nothing. Check: `apps/desktop/test/smoke.electron.test.ts:1311`. `desktop:needs-you.ts:53`
- W3-SHL-212 50 notifications held for their clicks, then the oldest let go. Check: port `apps/desktop/test/needs-you.test.ts:108`. `desktop:needs-you.ts:94`
- W3-SHL-213 Long title: "<200 characters> finished" handed over whole. Check: `gdbus monitor` shows the whole summary. `protocol:outside-line.ts:50`
- W3-SHL-214 Link on a host elsewhere: window moved home at the link's hash. Check: port `apps/desktop/test/deep-link.test.ts:59`. `desktop:deep-link.ts:34`
- W3-SHL-215 Not `wsp://<thread|workspace|project|settings>/<id>`: nothing moves. Check: `open 'wsp://thread/a/b'`, no raise. `protocol:app-address.ts:100`
- W3-SHL-216 Mac app that cannot replace itself: Get's line says why (translocated, disk image, not writable). Check: run from the mounted dmg. `desktop:self-update.ts:23`
- W3-SHL-217 Disk image road: Finder opens the dmg, app quits; a changed file is refused. Check: port `apps/desktop/test/get-bundle.test.ts:257`. `desktop:get-bundle.ts:58`
- W3-SHL-218 Linux road: AppImage to Downloads, chmod 755, shown in its folder, app quits. Check: port `apps/desktop/test/get-bundle.test.ts:223`. `desktop:get-bundle.ts:63`
- W3-SHL-219 Swap failed: old app back, service restarted on it; open refused: "wsp did not restart: <said>". Check: port `apps/desktop/test/self-update.test.ts:412`. `shell/update.ts:125`
- W3-SHL-220 Crash dumps in `~/.wsp/logs/crashes`, never uploaded; gone processes logged. Check: `apps/desktop/test/smoke.electron.test.ts:1264`. `desktop:main.ts:93`
- W3-SHL-221 Link arriving while the first page loads: sent once it is up; of several held links the last wins. Check: port `apps/desktop/test/deep-link.test.ts:48`, `:33`. `desktop:deep-link.ts:50`
- W3-SHL-222 Onboarding, starting window or failed start: no menu bar item, no feed, no feed badge; the tray draws once the app window attaches. Check: `wspTray.model()` undefined on `home: "empty"`. `desktop:main.ts:487`
- W3-SHL-223 Tray lost: plain mark, badge 0, title the host's name elsewhere, else empty. Check: `wsp stop`, read `wspTray.model()`. `desktop:tray.ts:133`
- W3-SHL-224 Socket gone: open prompts dropped, so rows lose Allow and Deny until the next read. Check: port `apps/desktop/test/host-feed.test.ts:123`. `desktop:host-feed.ts:158`
- W3-SHL-225 Keep awake runs on Linux too: the feed runs on every platform, only the tray is Mac's. Check: none on main; read the desktop's inhibitor list while a thread works. `desktop:main.ts:676`
- W3-SHL-226 Awake counts only a running thread on this computer's own workspace: not a box's, not one stopped on a prompt. Check: port `apps/desktop/test/awake.test.ts:18`, `:22`. `desktop:awake.ts:10`
- W3-SHL-227 Feed notification click, window closed: a window opens on that thread; on a host elsewhere, the window only. Check: `apps/desktop/test/smoke.electron.test.ts:1311`. `desktop:main.ts:788`
- W3-SHL-228 Click on a line an earlier page said (before a reload): raises the window, opens nothing; the page keeps 50 openers. Check: reload, click a held notification. `shell/needsYou.ts:57`
- W3-SHL-229 Quit asked from the menu bar: app raised first; a working count that fails reads as none working. Check: none on main; tray Quit while `sessions.list` refuses. `desktop:main.ts:560`
- W3-SHL-230 Open logs refused by the system opener: its reason is General's failure notice. Check: an `xdg-open` stub that prints and exits 1. `desktop:main.ts:270`
- W3-SHL-231 Hosts menu move refused: line to the app log, menu redrawn with the check unchanged. Check: an alias whose dial refuses, read app.log. `desktop:main.ts:388`
- W3-SHL-232 App log entry "<ISO time> <level> <text>", later lines indented 2; over 16 KB cut "(N characters cut)"; failed writes dropped. Check: port `apps/desktop/test/app-log.test.ts:23`, `:53`, `:81`. `desktop:app-log.ts:63`
- W3-SHL-233 App log lines: "wsp app <v> started (pid N, <platform> <arch>, electron <v>)", main's uncaught throws and rejections, "update: <step> done" or "failed: ... (cause: ...)". Check: `apps/desktop/test/smoke.electron.test.ts:1264`. `desktop:main.ts:88`
- W3-SHL-234 Dismissed while downloading: the copy checked on landing is deleted. Check: port `apps/web/test/update-card.test.tsx:210`. `shell/update.ts:115`
- W3-SHL-235 Window on a host elsewhere: the update card takes the link form, no Get, no shell download. Check: none on main; Hosts move with a release ahead. `shell/update.ts:83`
- W3-SHL-236 Bundle in Downloads with the published sum, or a version already staged: not fetched again; an older staged copy goes first. Check: port `apps/desktop/test/get-bundle.test.ts:190`, `:344`, `:376`. `desktop:get-bundle.ts:209`
- W3-SHL-237 Get for a prerelease or a release not above the app: refused before any fetch. Check: port `apps/desktop/test/get-bundle.test.ts:249`. `desktop:get-bundle.ts:280`
- W3-SHL-238 Swap waits 60 s for the app to quit, else changes nothing; new app up in 60 s or rolled back; a hung one TERM, KILL after 10 s. Check: port `apps/desktop/test/self-update.test.ts:446`, `:521`. `desktop:self-update.ts:352`
- W3-SHL-239 Swap rides out a logout's TERM, HUP and INT between the renames. Check: port `apps/desktop/test/self-update.test.ts:460`. `desktop:self-update.ts:274`
- W3-SHL-240 Stage folder a link or another account's: nothing staged; ours with group or other bits: tightened to 0700. Check: port `apps/desktop/test/self-update.test.ts:193`, `:201`. `desktop:self-update.ts:227`
- W3-SHL-241 Font family empty or over 200 characters: no faces; a font installed while open is seen after a restart. Check: port `apps/desktop/test/fonts.test.ts:177`. `desktop:fonts.ts:276`

### Actions and keys

- W3-SHL-242 Login PATH: `$SHELL -ilc` once, 15 s, before the unit is written; the service runs on it. `desktop:main.ts:861`
- W3-SHL-243 Older host box: Enter restarts (default), Escape quits (cancel); `app.focus({ steal: true })` first. `desktop:main.ts:419`
- W3-SHL-244 Menu bar Start wsp: the same attach without the starting window, then the feed dials again. `desktop:main.ts:758`
- W3-SHL-245 Agent configs get the shim `<home>/bin/wsp` as the wsp command, never the bundle's path. `desktop:cli.ts:7` exists: `startup.rs:273` (through the host)
- W3-SHL-246 AppImage: mount copied to <home>/app/<version>-<hash>, the shim runs the copy. Check: `shim.test.ts:51`. `desktop:shim.ts:63`
- W3-SHL-247 Older npm `wsp` links on PATH pointed at the shim (packaged, own home only). Check: `shim.test.ts:128`, `:179`. `desktop:shim.ts:107`
- W3-SHL-248 Page process gone (not clean, guest or quit): reload its address, 3 a minute, then paced. Check: `revive.test.ts:6`. `desktop:main.ts:96`
- W3-SHL-249 Open wsp and menu bar Start run the open without its waits: no starting window, no restart settle; failure is onboarding's refusal or app log "menu bar: <why>". `desktop:main.ts:408`, `:745`
- W3-SHL-250 Raise: a minimised window restored, `app.focus({ steal: true })`, then show and focus. `desktop:main.ts:237` exists: app/src/screens/dialogs/startup.rs:406, activate only, no restore
- W3-SHL-251 A host-package question in the shell is refused: "no terminal to ask: <question>". `desktop:main.ts:50`
- W3-SHL-252 Cmd+H, Opt+Cmd+H, Show All, Cmd+M, Ctrl+Cmd+F. `desktop:menu.ts:22` exists: app/src/screens/app_shell/menu.rs:104.
- W3-SHL-253 Edit chords: Cmd+Z, Shift+Cmd+Z, Cmd+X, C, V, Opt+Shift+Cmd+V, Cmd+A. `desktop:menu.ts:26` exists: app/src/screens/app_shell/menu.rs:50.
- W3-SHL-254 Hosts row moves the window; the move is the host lifecycle part's. `desktop:main.ts:387`
- W3-SHL-255 Page links (window.open, _blank, navigating away): http(s) to the default browser, else dropped. `desktop:main.ts:453` exists: app/src/screens/sidebar/foot.rs:193 (`cx.open_url`).
- W3-SHL-256 General's Open logs shows `~/.wsp/logs` in the file manager. Check: `xdg-open` stub, `apps/desktop/test/smoke.electron.test.ts:1290`. `settings/general.tsx:182`
- W3-SHL-257 Move to Applications: Electron moves the bundle, quits, starts the moved one. `desktop:main.ts:851`
- W3-SHL-258 Hosts menu rebuilt after every move, link move and window open; the check follows the current host. `desktop:main.ts:368`
- W3-SHL-259 Tray Allow and Deny on a Codex thread: the prompt is matched by the agent's own session id. `desktop:tray.ts:99`
- W3-SHL-260 Test seam: WSP_DESKTOP_SMOKE=1 puts `wspTray.model`, `pick`, `awake` on globalThis; a menu bar menu is no window a driver reaches. `desktop:main.ts:792`
- W3-SHL-261 Onboarding page sends its header colours and uncaught errors too; the gate takes it beside the app's page. `desktop:main.ts:192`, `desktop:onboarding.html:216`

### Host operations

- W3-SHL-262 Unit environment: WSP_HOME when set, SHELL, cloud, labs, update check, usage, state store, provider variables; never PATH or a key. Check: `packages/host/test/service.test.ts:295`. `host:service.ts:156`
- W3-SHL-263 Manager answers: launchd `print` 0 is held, pid off "\tpid = N", 113 absent; systemd `show` ActiveState, UnitFileState, MainPID. Check: `packages/host/test/service.test.ts:381`. `host:service.ts:232`
- W3-SHL-264 Bridge `localFonts`, `fontFamilies`: `settings/FontPicker.tsx:17`. `desktop:preload.ts:54` exists: `app/src/ui/theme.rs:402`
- W3-SHL-265 Bridge `capturePreview`, `workspacePreview`: 480 px, 12 in memory, 500 ms bound (W2 D12). `desktop:preload.ts:61`
- W3-SHL-266 `release.get`, `release.check`: never sent by the shell; the page's About and update. `protocol/client.ts:1254`
- W3-SHL-267 Manager commands by name off PATH, 15 s each, 127 when missing. `host:service.ts:421`
- W3-SHL-268 Load: launchd `enable` then `bootstrap gui/<uid> <plist>` (a disabled label refuses bootstrap); systemd `daemon-reload`, `enable`, `restart`. `host:service.ts:226`, `:314`
- W3-SHL-269 Unit text: plist Label, ProgramArguments, EnvironmentVariables, WorkingDirectory, outputs to host.log; systemd Type=simple, RestartSec=5, append:, WantedBy=default.target. `host:service.ts:203`, `:285`
- W3-SHL-270 Manager failure line: "<argv> exited <code> and said: <output on one line>", or "and said nothing". `host:service.ts:453`
- W3-SHL-271 `hosts:list`: here, current alias, account records less this computer's relay hostId; a page on an account host sees here and its own. Check: `host-switch.test.ts:183`. `desktop:main.ts:359`
- W3-SHL-272 `hosts:switch`: a page on an account host may move only home; no page on main calls it. `desktop:main.ts:366`
- W3-SHL-273 Each address gets 2 s to connect before the next (node's 250 ms dropped the first), set before any fetch. `desktop:main.ts:37`, `host:entry.ts:28`
- W3-SHL-274 Update download reuses a Downloads file whose sha256 matches; else streams to `<asset>.part` (exclusive create) and renames on a match. `desktop:get-bundle.ts:209`, `:153`
- W3-SHL-275 Update app log: "update: <step>", then "done" or "failed: <error> (cause: <code: message; ...>)". `desktop:get-bundle.ts:187`
- W3-SHL-276 Login shell read: `$SHELL -ilc "env -0"`, own process group, stdin closed; first entry starts after the rc file's last newline. `host:login-path.ts:61`, `:118`
- W3-SHL-277 Feed rereads 150 ms after session.*, thread.marked, workspace.*, place.* pushes. `desktop:host-feed.ts:55`
- W3-SHL-278 `release.get`, `release.check`, push `release.changed`: host checks 30 s after start, every 6 h; the page reads. `host:release.ts:20`
- W3-SHL-279 Electron `Menu.setApplicationMenu`, `Menu.buildFromTemplate`, roles about, services, hide, fileMenu, editMenu, viewMenu, windowMenu. `desktop:main.ts:396`
- W3-SHL-280 Electron `nativeTheme.themeSource`, `setVibrancy`, `setTitleBarOverlay`. `desktop:main.ts:174`
- W3-SHL-281 Electron `dialog.showMessageBox`, `dialog.showErrorBox`, `dialog.showOpenDialog`. `desktop:main.ts:563`
- W3-SHL-282 Electron `shell.openExternal`, `shell.openPath`, `shell.showItemInFolder`, `app.moveToApplicationsFolder`. `desktop:main.ts:322`
- W3-SHL-283 Electron `crashReporter.start({ uploadToServer: false })`, `child-process-gone`, `app.getAppMetrics`, `capturePage`. `desktop:main.ts:94`
- W3-SHL-284 OS: `ditto`, `codesign --verify --deep --strict`, `plutil`, `xattr -dr com.apple.quarantine`, `launchctl kickstart -k`. `desktop:self-update.ts:241`
- W3-SHL-285 Bridge `version`, `bundleHover`, `updatesInPlace`, `updateWhy`: this part; `shell/shellVersion.ts:19`, `sidebar/UpdateCard.tsx:36`. `desktop:preload.ts:40`
- W3-SHL-286 Bridge `getBundle`, `quitAndOpen`, `discardUpdate`: this part; `shell/update.ts:111`, `:127`, `:151`. `desktop:preload.ts:51`
- W3-SHL-287 Bridge `sayOutside`, `onNeedsYouOpen`: this part; `shell/needsYou.ts:60`, `:53`. `desktop:preload.ts:82`
- W3-SHL-288 Bridge `setBadge`: this part; `notices/hostNotices.ts:377`. `desktop:preload.ts:83`
- W3-SHL-289 Bridge `onOpen`: this part; `shell/links.ts:62`. `desktop:preload.ts:87`
- W3-SHL-290 Bridge `setTheme`, `setGlass`, `setTitleBar`: this part; `settings/theme.ts:51`, `:76`, `:52`. `desktop:preload.ts:75`
- W3-SHL-291 Bridge `setTerminalFocus`, `onShellChord`: this part with `zoom.ts`; `shell/KeybindingDispatcher.tsx:87`. `desktop:preload.ts:69`
- W3-SHL-292 Bridge `pickFolder`: this part; `sidebar/AddProjectDialog.tsx:176`, `sidebar/ExportProjectDialog.tsx:43`. `desktop:preload.ts:56`
- W3-SHL-293 Bridge `openLogs`: this part; `settings/general.tsx:70`. `desktop:preload.ts:84`
- W3-SHL-294 Bridge log channel: the preload's error and rejection listeners into app.log. `desktop:preload.ts:104`
- W3-SHL-295 Bridge html class `desktop-mac` or `wco`: this part's frame. `desktop:preload.ts:115`
- W3-SHL-296 Bridge `contextMenu`: #2390 (W2-UNO-010); `actions/contextMenu.ts:94`. `desktop:preload.ts:60` exists: app/src/ui/context_menu.rs:2.
- W3-SHL-297 Bridge `loginStart`, `setLoginStart`: host lifecycle part; `settings/settingsReads.ts:64`, `settings/general.tsx:69`. `desktop:preload.ts:85`
- W3-SHL-298 Bridge `hostToken`, `hosts`, `switchHost`: host lifecycle part; `BootGate.tsx:35`, `shell/update.ts:82`. `desktop:preload.ts:48`
- W3-SHL-299 Bridge `browserGuests`, `onGuestOpen`: browser pane part; `browser/guests.ts:164`. `desktop:preload.ts:63`
- W3-SHL-300 Bridge `here`, `agents`, `install`, `finish`: onboarding (#2399); `desktop:onboarding.html:259`. `desktop:preload.ts:44`
- W3-SHL-301 Bridge `droppedPath`: no page caller on main. `desktop:preload.ts:59`
- W3-SHL-302 `BrowserWindow` webPreferences: sandbox, contextIsolation, no nodeIntegration, webviewTag; one option set for app, onboarding, starting. `desktop:window.ts:134`
- W3-SHL-303 Preload facts ride argv: `--wsp-version=`, `--wsp-html-class=`, `--wsp-bundle-hover=`, `--wsp-update-in-place=1`, `--wsp-update-why=`. `desktop:window.ts:141`
- W3-SHL-304 Header colours: the preload reads `--titlebar-ground`, `--titlebar-ink` off a canvas pixel as #rrggbb; anything else is ignored. `desktop:preload.ts:23`, `desktop:window.ts:116`
- W3-SHL-305 Font index: Mac ~/Library/Fonts, /Library/Fonts, /System/Library/Fonts and Supplemental; Linux XDG data, ~/.fonts, /usr/local, /usr/share; 3 deep, ttf otf ttc. `desktop:fonts.ts:147` exists: app/src/screens/terminal/look.rs:128 (the text system's list)
- W3-SHL-306 `localFonts` pick: stack's first name on family, full or PostScript name (case, spaces, - and _ ignored), plus one Nerd variant, Mono first, never Propo; 4 styles. `desktop:fonts.ts:236`
- W3-SHL-307 `fontFamilies`: each family once, dot-named ones hidden, sorted ignoring case. `desktop:fonts.ts:291` exists: app/src/ui/theme.rs:402 (`all_font_names`)
- W3-SHL-308 App processes labelled main, gpu, network, renderer, tab, frame, helper; tabs and frames carry origins, no path, capped; sent each sampler tick while watched. `desktop:app-procs.ts:77`
- W3-SHL-309 Zip read before unpack: refuse an absolute name, an empty, . or .. step, a path through a link, a link leaving the app (case, NFC folded), not one .app. `desktop:self-update.ts:148`
- W3-SHL-310 Swap: `/bin/sh -c` detached, tools by full path, env PATH /usr/bin:/bin:/usr/sbin:/sbin with HOME, USER, LOGNAME, TMPDIR, LANG, WSP_HOME. `desktop:self-update.ts:385`
- W3-SHL-311 Swap restarts the service only where this app's unit file exists: `launchctl kickstart -k gui/<uid>/<label>`. Check: port `apps/desktop/test/self-update.test.ts:405`. `desktop:main.ts:307`
- W3-SHL-312 Download: GitHub `releases/tags/v<v>`, then the asset, user agent `wsp/<version>`, into Downloads (in place: the stage), `<asset>.part` exclusive, renamed on a match. `desktop:get-bundle.ts:124`
- W3-SHL-313 Mac signing: after-pack signs the Mac daemons, then the app, ad hoc: `codesign --force --options runtime --entitlements`, `--sign -`, `--deep`. `apps/desktop/scripts/after-pack.mjs:25`
- W3-SHL-314 Entitlements under the hardened runtime: allow-jit, allow-unsigned-executable-memory, disable-library-validation; one file for app and inside. `apps/desktop/build/entitlements.mac.plist:8`
- W3-SHL-315 Release check: the AppImage under xvfb loads its page within 180 s and quits within 30 s of its window closing; its service is stopped. `apps/desktop/scripts/appimage-launch.mjs:17`
- W3-SHL-316 Preload error listeners go in the page's world through `contextBridge.executeInMainWorld`, so page throws reach `log:renderer` with the route. `desktop:preload.ts:105`
- W3-SHL-317 Page callers no bridge item names: `pickFolder` in Add a computer, `workspacePreview`, `hosts` for the version notice, `bundleHover` on About and the card, `onGuestOpen`. `settings/add/AddComputerDialog.tsx:175`, `shell/workspacePreviews.ts:44`, `shell/shellVersion.ts:43`, `settings/about.tsx:56`, `sidebar/UpdateCard.tsx:54`, `components/preview/BrowserGuests.tsx:53`

### Design

- W3-SHL-318 Starting spinner keeps turning under reduced motion: no rule in starting.html, none global in the web stylesheet. `desktop:starting.html:25` exists: app/src/ui/status.rs:34, stops it
- W3-SHL-319 Tray art: 18 pt and @2x template PNGs, the mark and a "?" stroked 2.4 on 16, black on clear. `apps/desktop/scripts/tray-icons.mjs:14`
- W3-SHL-320 DMG window: light ground, 20 px dot grid, one glow, hairline arrow, "your setup, on cloud machines". `apps/desktop/scripts/dmg-background.mjs:16`

## Experimental switch (#2402)

### Elements

- W3-SWT-040 Place, if a group instead (D3): an id, a table entry, a page; a group hides only when marked `empty`. `settings/groups.ts:40`
- W3-SWT-041 Held switch, reason as a `title` hover on a span around it, as Privacy's. `settings/privacy.tsx:39`
- W3-SWT-042 Confirm: no Settings switch asks; the nearest is Remove project's alert dialog. `settings/projects.tsx:134`
- W3-SWT-043 Act copy: "Quit and open"; Restart alone adds "Your N running threads carry on. Open terminals close." `shell/update.ts:27`
- W3-SWT-044 Held switch reads off whatever the record holds, and the row carries no reset arrow while held. `settings/privacy.tsx:62`
- W3-SWT-045 A note that names the computer is empty until the name is known; Keep awake's whole row waits for it. `settings/format.ts:76`
- W3-SWT-046 The title is a plain span, not a label: pressing the row's words flips nothing, only the switch takes the press. `settings/rows.tsx:177`
- W3-SWT-047 Tab order in a row: the reset arrow beside the title first, then the control in the slot. `settings/rows.tsx:185`
- W3-SWT-048 General has no `restore` entry, so no Restore defaults: the row's arrow is its only reset; a group of its own could carry one. `settings/groups.ts:52`
- W3-SWT-049 Search walks the cards as drawn in this window: a hidden row is never found, and card heads and group names are not read. `settings/groups.ts:78`
- W3-SWT-050 Precedent for a row on one platform only: Transparency is left out where the window is solid (Linux). `settings/appearance.tsx:116`
- W3-SWT-051 Palette's Settings row: a fixed search-word list and a description of group names; "experimental" finds nothing there. `components/palette/paletteItems.tsx:318`
- W3-SWT-052 Version card under the new card reads "Update checks are off" on the fixture: no Check for updates, What's new with the off hover. `settings/about.tsx:85`

### States

- W3-SWT-053 GPUI app of another release: its client refuses any host not on its own release. No shot: needs a GPUI build of another release. `daemon/crates/wsp-mcp/src/release.rs:22`
- W3-SWT-054 GPUI opened anyway: start-failed box, release sentence, Open log, Quit. No shot: native. exists: `app/src/screens/dialogs/startup.rs:365`. `desktop:starting.ts:45`
- W3-SWT-055 Linux, macOS: one row; the launch differs (bundle, spawned binary). Check: a unit test per platform. `desktop:get-bundle.ts:53`
- W3-SWT-056 Long note wraps at max-w-xl; the slot keeps its 10 rem beside. No shot: the row is new in #2402. `settings/rows.tsx:115`
- W3-SWT-057 Write refused: "That setting was not saved: <said>. It shows the host's value again." No shot: needs a refusing host. `protocol/store/useStore.ts:528`
- W3-SWT-058 Long title wraps; the reset arrow follows it onto the next line. No shot: the row is new in #2402. `settings/rows.tsx:176`
- W3-SWT-059 Phone width search: the row is a result in the sheet; a tap opens General and flashes the row. No shot: the row is new; W1-SET-030 shoots the sheet. `settings/SettingsSidebar.tsx:276`
- W3-SWT-060 Read pending, if built on loginStart's bridge read: no row until the bridge answers, then the card grows. No shot: a moment as Settings opens. `settings/settingsReads.ts:63`
- W3-SWT-061 First launch on an empty home: onboarding opens; a host field reads its default there, a shell-kept switch could still bounce. No shot: native. Check: app log. `desktop:main.ts:624`
- W3-SWT-062 No host here, account host aimed: the window opens on that host and starts no service; the row is hidden, and a launch read reads that host's record. No shot: second host. `desktop:host-lifecycle.ts:308`
- W3-SWT-063 Quit under way: a Dock click opens nothing and a failed open says nothing. No shot. Check: app log after a Dock click mid-quit. `desktop:main.ts:639`

### Actions and keys

- W3-SWT-064 Off in today's app: the write alone. `settings/general.tsx:167`
- W3-SWT-065 Search "experimental": the results page lists the row, live. `settings/SettingsPage.tsx:28`
- W3-SWT-066 GPUI: a later launch raises its window; `--open`, `--screen`, `--bench` skip the lock. exists: `app/src/main.rs:73`. `desktop:main.ts:86`
- W3-SWT-067 Quit and open's "went" is the OS opening the file; it does not wait for the app. The in-place swap waits 60 s for the new app's `up`, else puts the old back. `desktop:self-update.ts:341`
- W3-SWT-068 A launch with the switch on still runs every step before the window: stage settle, Move to Applications, the shim, login PATH, PATH check. `desktop:main.ts:849`

### Host operations

- W3-SWT-069 `preferences.get`; the shell reads it only in its feed, after the window. exists: `host-client/src/ops/settings.rs:19`. `desktop:host-feed.ts:162`
- W3-SWT-070 `preferences.set`, were it a host field (D3 says not): a strict patch, which an older host refuses. exists: `host-client/src/ops/settings.rs:20`. `protocol:views/preferences.ts:203`
- W3-SWT-071 Push `preferences.changed`: the shell's feed and GPUI follow. exists: `app/src/screens/sidebar/host.rs:161`. `desktop:host-feed.ts:110`
- W3-SWT-072 `labs` on `preferences.get`, from WSP_LABS=1 at host start; not settable. `runtime:account/preferences.ts:85`
- W3-SWT-073 Way back: GPUI's `cx.open_url` on a wsp:// link, which today's app takes; GPUI handles none. `desktop:main.ts:222`
- W3-SWT-074 A shell fact can ride the window's argv as `--wsp-<name>=` (version, bundle-hover, update-in-place, update-why): read once, no channel, seen by any page. `desktop:window.ts:141`
- W3-SWT-075 The shell's feed keeps five preference fields, defaults until its first read; a host field the shell acts on needs a slot there. `desktop:host-feed.ts:86`
- W3-SWT-076 A new channel is refused on a host elsewhere unless listed remote; the gate test lists each channel as remote or here-only. `apps/desktop/test/origin.test.ts:49`
- W3-SWT-077 `preferences.set` that went through may still answer a `notice`, raised as an error notice. `protocol/store/useStore.ts:524`
- W3-SWT-078 `release.check` on each opening of General, the switch's page. exists: `host-client/src/ops/settings.rs:25`. `settings/settingsReads.ts:96`

### Design

- W3-SWT-079 Card and head: `ui::Card` `.head`. `settings/rows.tsx:127`
- W3-SWT-080 Held reason as the platform's title hover: none yet; `ui::tip` is wsp's own tooltip. `settings/privacy.tsx:39`
- W3-SWT-081 Confirm, if ruled: `ui::AlertDialog` or #2399's 440 px frame; scale 0.98, opacity, 200 ms ease-in-out. `components/ui/dialog-styles.ts:6`
- W3-SWT-082 Motion: reset arrow color, background, border, 150 ms default curve. `components/ui/button.tsx:22`
- W3-SWT-083 Switch colours: track primary on, input off; thumb foreground off, primary-foreground on, shadow-sm/5; held 64%, not-allowed cursor. exists: `app/src/ui/toggle.rs:65`. `components/ui/switch.tsx:12`
- W3-SWT-084 Pressed thumb also squares its corners to thumb/thumb*1.1, 150 ms CSS ease; `ui::Switch` keeps it round. `components/ui/switch.tsx:20`
- W3-SWT-085 Trail line for a new headed card: opens with height and opacity, 140 ms cubic-bezier(0.23, 1, 0.32, 1); lit color 150 ms default curve. `settings/SettingsSidebar.tsx:226`
- W3-SWT-086 Picked search result flashes: background foreground 9% to clear, 900 ms ease-out; none under reduced motion. `settings/SettingsSidebar.tsx:88`
- W3-SWT-087 Reset arrow: ghost icon-xs 24 px, tooltip "Back to the default" above, 4 px off, opacity and 2 px shift. exists: `app/src/ui/card.rs:661`. `settings/rows.tsx:187`
- W3-SWT-088 Below 640 px the slot stands under the words at the left; it moves right only from 640 (`sm:justify-end`). `settings/rows.tsx:121`

## What the browser spike must show (D1)

The spike thread (`wsp/gpui-browser-spike`) is asked nothing; this is the evidence each road needs before D1 is
ruled. Read on 2026-10-19: gpui-kit 14f5933, gpui-fast 37f3959, wry 0.57.0, zed#62379 through `gh api`.

Road A, gpui-kit's web view on gpui-fast:
1. macOS shot: a GPUI dropdown with its shadow, a tooltip and a dialog over a loaded localhost page.
2. X11 shots with and without a compositing manager: the same menu over the page.
3. A Wayland session: the app starts on XWayland and loads a page; a shot at 1.5x.
4. The page focused on macOS: Cmd+L reaches the address bar, Cmd+R reloads, Cmd+W closes the tab, Cmd+V pastes into
   the page, not the field.
5. The same on Linux with Ctrl: which chords reach GPUI, and GPUI's frame rate while the page has focus.
6. A press on the page closes a GPUI menu; a press outside returns typing to a GPUI field.
7. Page scroll frame times on macOS and on Linux.
8. RSS and PSS of the app and its WebKit processes with one page open, against Electron on the same page.
9. The week-one numbers on gpui-fast, and `effects.rs`, `text.rs`, `pixels.rs` green with our patches cut again.

Road B, wry as a child view on gpui-pre:
1. macOS shot of a GPUI menu opened over the page (expected: under it).
2. The snapshot swap: time from click to menu, and a pixel diff of the snapshot against the live page.
3. Items 4 to 8 of road A.

Road C, zed#62379 on our gpui-pre:
1. The PR applied over `vendor/gpui-pre-0.3.9` with `gpui-patches.sh check` and our pixel tests green.
2. macOS shot of a menu over a WKWebView page.
3. A WebKitGTK page in the PR's native slot on X11 with a menu over it.
4. The lines of patch and glue we would carry.
