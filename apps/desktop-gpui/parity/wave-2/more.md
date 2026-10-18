# Wave 2 parity spec: more items



The items `spec.md` had no room for, in the same id sequences and the same shape. Builders tick these too.

## Terminal (#2396)

### Elements

- W2-TRM-056 Header toggle "Toggle terminal drawer". `components/chat/PanelLayoutControls.tsx:42`
- W2-TRM-057 This computer's drawer, shells in home: first run, project home, no thread. `components/WorkspaceTerminalDrawer.tsx:55`
- W2-TRM-058 Toast "No terminal on <name>: <said>" on a refused open. `shell/shellCommands.ts:65`
- W2-TRM-059 Panel, no link: "No terminal link for this thread yet." "Terminals connect once <computer> answers." `components/WorkspaceTerminalPanel.tsx:50`
- W2-TRM-060 Toolbar tooltip "<title> (<chord>)", or the refusal. `components/ThreadTerminalDrawer.tsx:82`
- W2-TRM-061 Tab X tooltip "Close <tab>"; a tab with no name reads "Terminal". `components/ThreadTerminalDrawer.tsx:1247`
- W2-TRM-062 Start again reads "Starting…" and is disabled while busy. `components/DaemonDown.tsx:56`
- W2-TRM-063 Start refused: the host's sentence in red under the button. `components/DaemonDown.tsx:59`
- W2-TRM-064 Panel tab title: the active terminal's name, else "Terminal". `panes.ts:84`
- W2-TRM-065 A lost shell prints "[<sentence>]" into its scrollback. `terminal/link.ts:540`
- W2-TRM-066 Sentence "This shell ended when the terminal process on its computer restarted". `adapt/terminal-pane.ts:156`
- W2-TRM-067 Sentence after a stop: "This shell ended when the thread stopped". `adapt/terminal-pane.ts:160`
- W2-TRM-068 Overlay hints in order: memory line, next size up, "Rebuild it from the thread's row". `adapt/terminal-pane.ts:174`
- W2-TRM-069 Font card hint: "A font installed on this computer. Empty draws with <detected>, your terminal's font." (or "the default font."), then "Icons a face lacks come from a bundled symbols font." `components/TerminalFontButton.tsx:41`
- W2-TRM-070 Font input placeholder: the detected family, else the first default face. `components/TerminalFontButton.tsx:40`
- W2-TRM-071 Font button: label and tooltip "Terminal font", expanded while the card is open. `components/TerminalFontButton.tsx:21`
- W2-TRM-072 Menu Copy greyed "Nothing is selected". `actions/terminalActions.ts:34`
- W2-TRM-073 Menu Add to chat shown only with a selection. `actions/terminalActions.ts:41`
- W2-TRM-074 Menu Paste greyed "The clipboard cannot be read here". `actions/terminalActions.ts:51`
- W2-TRM-075 Menu rows grouped: clipboard, screen, panes. `actions/registry.ts:105`
- W2-TRM-076 Menu rows show their chords. `actions/registry.ts:108`
- W2-TRM-077 Menu Close marked destructive. `actions/registry.ts:110`
- W2-TRM-078 Menu action failure toast "<title>: <said>". `actions/contextMenu.ts:81`
- W2-TRM-079 Native menu failure toast "Menu: <said>". `actions/contextMenu.ts:98`
- W2-TRM-080 Pointer cursor over a Ctrl-hovered link. `terminal/ghostty/surface/terminalSurface.ts:970`
- W2-TRM-081 Text cursor over the grid. `terminal/ghostty/surface/terminalSurface.ts:170`
- W2-TRM-082 Default cursor while a program takes the mouse. `terminal/ghostty/surface/terminalSurface.ts:863`
- W2-TRM-083 Canvas paints the theme background before fonts and wasm load, so no empty flash. `terminal/ghostty/surface/terminalSurface.ts:205`

### States

- W2-TRM-084 Grid at the top until scrollback exists, then pinned to the bottom edge. `terminal/ghostty/surface/layout.ts:40`
- W2-TRM-085 Reconnect wipes the screen and replays the daemon's scrollback. `terminal/link.ts:521`
- W2-TRM-086 Ptys from a reload or another window are adopted as tabs. `terminal/link.ts:459`
- W2-TRM-087 A stop that ends processes (pause mode not memory) ends the shells. `terminal/wiring.ts:203`
- W2-TRM-088 Glass off: the pane is opaque whatever the config's opacity. `components/ThreadTerminalDrawer.tsx:267`
- W2-TRM-089 Socket back: config read again, open panes take its colours, font, padding, opacity. `components/ThreadTerminalDrawer.tsx:300`
- W2-TRM-090 Theme flip changes colours only; font, padding, opacity wait for the next open. `components/ThreadTerminalDrawer.tsx:385`

### Actions and keys

- W2-TRM-091 Click a split pane activates it. `components/ThreadTerminalDrawer.tsx:1139`
- W2-TRM-092 Right-click native menu: Copy, Add to chat ("Terminal output from <tab>:" block), Paste, Clear (screen, scrollback), splits, New, Close. `actions/terminalActions.ts:28`
- W2-TRM-093 Drag, double, triple click select. `terminal/ghostty/surface/terminalSurface.ts:817`
- W2-TRM-094 Copy, paste chords. `terminal/ghostty/surface/terminalSurface.ts:561`
- W2-TRM-095 Font card: blur commits, Escape closes. `components/TerminalFontButton.tsx:36`
- W2-TRM-096 Line mode: keys echo locally, Enter sends the line, kept over a reconnect. `terminal/compose.ts:50`
- W2-TRM-097 Drawer, shells per thread, kept on reload; New thread page shells join the thread it opens. `terminal/drawerStore.ts:83`
- W2-TRM-098 Ctrl+C D R K L A E W U Z M I H [ stay the shell's even when a passed command is rebound onto one. `keybindings.ts:321`
- W2-TRM-099 Mac Cmd+Backspace, Cmd+Left, Cmd+Right: kill line, line start, line end. `terminal/ghostty/surface/input.ts:67`
- W2-TRM-100 Mac: other Cmd chords type no text. `terminal/ghostty/surface/input.ts:91`
- W2-TRM-101 Copy: Ctrl+C or Ctrl+Shift+C, Cmd+C on Mac, only with a selection. `terminal/ghostty/surface/input.ts:6`
- W2-TRM-102 Ctrl+C on a selection copies and clears it on Linux, so the next Ctrl+C interrupts. `terminal/ghostty/surface/terminalSurface.ts:579`
- W2-TRM-103 Paste: Ctrl+Shift+V or Shift+Insert, Cmd+V on Mac, bracketed. `terminal/ghostty/surface/input.ts:55`
- W2-TRM-104 AltGr text goes to the shell, never a shortcut. `terminal/ghostty/surface/terminalSurface.ts:557`
- W2-TRM-105 IME composition commits its text; candidate window at the cursor. `terminal/ghostty/surface/terminalSurface.ts:753`
- W2-TRM-106 Key releases sent when a program asks for them (Kitty). `terminal/ghostty/surface/terminalSurface.ts:666`
- W2-TRM-107 A click without a drag clears the selection. `terminal/ghostty/surface/terminalSurface.ts:1014`
- W2-TRM-108 Multi-click counts within the double-click interval and 4 px, cycling 1 to 3. `terminal/ghostty/surface/input.ts:224`
- W2-TRM-109 Link gesture: Cmd on Mac, Ctrl elsewhere. `terminal/ghostty/surface/input.ts:188`
- W2-TRM-110 Ctrl+D, Ctrl+N act on the right panel when its terminal has focus. `shell/shellCommands.ts:368`
- W2-TRM-111 Ctrl+D, Ctrl+N do nothing with no thread selected. `shell/shellCommands.ts:367`
- W2-TRM-112 Ctrl+J does nothing while Settings is open. `shell/shellCommands.ts:364`
- W2-TRM-113 Opening the drawer focuses the active terminal. `components/ThreadTerminalDrawer.tsx:381`
- W2-TRM-114 Overlay takes focus only if the pane held it. `components/ThreadTerminalDrawer.tsx:581`
- W2-TRM-115 Overlay hands focus back to the terminal when it lifts. `components/ThreadTerminalDrawer.tsx:585`
- W2-TRM-116 A key on the overlay shows the typing refusal; Tab and Escape pass. `components/ThreadTerminalDrawer.tsx:599`
- W2-TRM-117 Font card Escape returns focus to the terminal. `components/ThreadTerminalDrawer.tsx:680`
- W2-TRM-118 Group header click activates that group's active or first terminal. `components/ThreadTerminalDrawer.tsx:1205`
- W2-TRM-119 Close, toolbar or menu, closes the active terminal. `components/ThreadTerminalDrawer.tsx:846`
- W2-TRM-120 Height saved on drag end, not each step. `components/ThreadTerminalDrawer.tsx:932`
- W2-TRM-121 Showing the drawer refits its terminals. `components/ThreadTerminalDrawer.tsx:965`
- W2-TRM-122 A split lands right after the active pane. `terminal/groups.ts:272`
- W2-TRM-123 A split sets the whole group's direction. `terminal/groups.ts:277`
- W2-TRM-124 Closing the active terminal activates the next, else the previous. `terminal/groups.ts:352`
- W2-TRM-125 Panel +: joins the surface while it holds 2 or 3, else a new tab. `shell/shellCommands.ts:135`
- W2-TRM-126 Closing a panel terminal tab kills its shells. `shell/RightPanel.tsx:62`
- W2-TRM-127 Panel shells never show in the drawer. `components/WorkspaceTerminalDrawer.tsx:65`
- W2-TRM-128 First open spawns once per drawer; closing the last never respawns. `components/WorkspaceTerminalDrawer.tsx:27`
- W2-TRM-129 A refused first spawn is tried again on the next mount. `components/WorkspaceTerminalDrawer.tsx:30`
- W2-TRM-130 The arrangement follows the daemon's list only once the link is live. `components/WorkspaceTerminalDrawer.tsx:77`
- W2-TRM-131 Pty told a new size 150 ms after the grid settles. `terminal/ghostty/surface/terminalSurface.ts:430`
- W2-TRM-132 New ptys open at the last measured size, kept across reloads. `terminal/link.ts:248`
- W2-TRM-133 Lead's drawer stays mounted but hidden on a subagent page. `App.tsx:138`
- W2-TRM-134 No drawer on a failed fork. `App.tsx:131`
- W2-TRM-135 Links: http(s) URLs. `terminal-links.ts:40`
- W2-TRM-136 Links: paths (~/, ./, /, C:\, a/b) with :line:col. `terminal-links.ts:42`
- W2-TRM-137 Links: trailing punctuation dropped. `terminal-links.ts:43`
- W2-TRM-138 Links: unbalanced closing brackets dropped. `terminal-links.ts:58`
- W2-TRM-139 Links: OSC 8 hyperlinks, across wrapped rows. `terminal/ghostty/surface/terminalSurface.ts:1369`
- W2-TRM-140 A link cut by the viewport edge does not activate. `terminal/ghostty/surface/links.ts:73`
- W2-TRM-141 A path opens in Files, relative to the thread's folder. `files/open.ts:30`
- W2-TRM-142 Font card Enter commits. `hooks/useCommitOnBlur.ts:38`
- W2-TRM-143 An empty font clears the choice. `terminal/fontSetting.ts:37`
- W2-TRM-144 Font choice per viewer, synced across windows. `terminal/fontSetting.ts:77`
- W2-TRM-145 A typed family beats the Ghostty file's; a detected one does not. `terminal/ghosttyConfig.ts:65`
- W2-TRM-146 Base size from the app (14 px) or the Ghostty file, per preference, plus zoom. `terminal/ghosttyConfig.ts:35`
- W2-TRM-147 Proportional faces refused for the default stack. `terminal/ghostty/surface/fonts.ts:69`
- W2-TRM-148 Bundled Symbols Nerd Font Mono loads with the first terminal. `terminal/ghostty/surface/fonts.ts:38`
- W2-TRM-149 Installed faces for the family come from the desktop bridge's `localFonts`. `terminal/ghostty/localFonts.ts:18`
- W2-TRM-150 A screen DPI change refits. `terminal/ghostty/surface/terminalSurface.ts:692`
- W2-TRM-151 A late font load re-measures and refits. `terminal/ghostty/surface/terminalSurface.ts:348`

### Host operations

- W2-TRM-152 `ping` every 10 s. `terminal/daemon-link.ts:195`
- W2-TRM-153 A missed pong drops the channel and redials with backoff. `terminal/daemon-link.ts:197`
- W2-TRM-154 A refused link retries at the backoff ceiling, its sentence kept. `terminal/daemon-link.ts:231`
- W2-TRM-155 A folder on this computer dials only while a drawer, panel or reader shows it. `terminal/wiring.ts:156`
- W2-TRM-156 This computer's daemon dials once its drawer or a reading pane opens. `terminal/wiring.ts:65`

### Design

- W2-TRM-157 Motion: cursor blink, 500 ms per phase, off unfocused or reduced motion; drag-select autoscroll 80 ms. `terminal/ghostty/surface/terminalSurface.ts:24`
- W2-TRM-158 Motion: tooltips, opacity and translate, 150 ms, default curve. `components/ui/popover.tsx:55`
- W2-TRM-159 Theme from `--terminal-background`, `--terminal-foreground`, `--terminal-cursor`, `--terminal-selection-background`, `--terminal-ansi-0` to `15`; the config's colours over them. `components/ThreadTerminalDrawer.tsx:144`
- W2-TRM-160 Dark ANSI palette. `index.css:779`
- W2-TRM-161 Selection: primary at 20% light, 25% dark. `index.css:741`
- W2-TRM-162 Text size token 14 px. `index.css:122`
- W2-TRM-163 Overlay: terminal background at 70%, 1 px blur. `components/ThreadTerminalDrawer.tsx:595`
- W2-TRM-164 Toolbar: rounded border, chrome background, xs shadow. `components/ThreadTerminalDrawer.tsx:1055`
- W2-TRM-165 Split buttons at the limit: 45% opacity, not-allowed cursor. `components/ThreadTerminalDrawer.tsx:1059`
- W2-TRM-166 Inactive split panes: border at 70%. `components/ThreadTerminalDrawer.tsx:1137`
- W2-TRM-167 Tab list: 144 px, left border, muted 10% fill. `components/ThreadTerminalDrawer.tsx:1157`
- W2-TRM-168 Group header: 22 px, 11 px text, active accent at 50%. `components/ThreadTerminalDrawer.tsx:1226`
- W2-TRM-169 Tab row: 24 px, 12 px text, active accent, hover accent at 60%. `components/ThreadTerminalDrawer.tsx:1252`
- W2-TRM-170 Tab X replaces the tab icon on row hover or focus. `components/ui/panel-tab-close-button.tsx:27`
- W2-TRM-171 Font card: 288 px, rounded, popover fill, md shadow, 12 px padding. `components/TerminalFontButton.tsx:34`
- W2-TRM-172 Cursor: bar and underline 2 px, hollow 1 px stroke, block redraws its glyph in the background colour. `terminal/ghostty/renderer.ts:280`
- W2-TRM-173 Underline, strikethrough, overline: 1 px rules. `terminal/ghostty/renderer.ts:262`
- W2-TRM-174 Bold at 700, italic faces. `terminal/ghostty/renderer.ts:76`
- W2-TRM-175 Invisible text not drawn. `terminal/ghostty/renderer.ts:226`
- W2-TRM-176 Cell height: the larger of 1.35 x size and the glyph height. `terminal/ghostty/renderer.ts:91`
- W2-TRM-177 Default selection colour when the theme names none. `terminal/ghostty/renderer.ts:33`
- W2-TRM-178 Padding 4 px each side by default. `terminal/ghostty/surface/layout.ts:6`
- W2-TRM-179 Scrollbar thumb at least 18 px. `terminal/ghostty/surface/layout.ts:8`
- W2-TRM-180 Motion: tab X tooltip, opacity and translate, 150 ms, default curve. `components/ui/tooltip.tsx:46`

## Pull request, Changes and Files (#2397)

### Elements

- W2-PRP-079 "HEAD → base", or "N of M files" with Show all. `diffs/DiffSurface.tsx:478`
- W2-PRP-080 Under the head: 2 MB cut, write note, red error, reveal note; raw patch. `diffs/DiffSurface.tsx:564`
- W2-PRP-081 Description in a 300 px fade clamp, Show more or less. `pull-request/parts.tsx:155`
- W2-PRP-082 Commits: day heads, rail, face, subject, counts, sha, body on press; merges dimmed with branch chip. `pull-request/Commits.tsx:20`
- W2-PRP-083 Skeletons: head, timeline, commits, files, diff. `pull-request/parts.tsx:194`
- W2-PRP-084 Header Open: editor mark, spinner, menu with keycap. `files/OpenSplit.tsx:69`
- W2-PRP-085 Turn card head: "N changed files", "N changed files in this folder" or "No files from this thread's edits", +/- count. `components/chat/ChangedFilesTree.tsx:53`
- W2-PRP-086 Turn card: Expand or Collapse all folders. `components/chat/ChangedFilesTree.tsx:67`
- W2-PRP-087 Turn card: Open diff, its word only from 24rem wide, "Open the full diff" on hover. `components/chat/ChangedFilesTree.tsx:80`
- W2-PRP-088 Turn card: "Also changed in this folder" with its own tree. `components/chat/ChangedFilesTree.tsx:103`
- W2-PRP-089 Face: the initial until the avatar loads. `pull-request/parts.tsx:66`
- W2-PRP-090 Faces: up to 3 stacked, then +N, names on hover. `pull-request/parts.tsx:95`
- W2-PRP-091 Label chip: its description on hover. `pull-request/PullRequestSurface.tsx:235`
- W2-PRP-092 Commit row: relative time on hover. `pull-request/Commits.tsx:56`
- W2-PRP-093 Reaction picker: GitHub's eight emoji, the person's own marked. `pull-request/ThreadActs.tsx:18`
- W2-PRP-094 SSH consent dialog: "Open in your editor over SSH", the one ~/.ssh/config line, what the workspace can reach, "Remove it any time in Settings, General.", Cancel, Add to SSH config. `files/EditorConsent.tsx:51`
- W2-PRP-095 Open menu on a remote machine of 4 GB or less: "An editor here takes about what one more thread does, or more." `files/OpenSplit.tsx:80`
- W2-PRP-096 Open slot held invisible while the host lists editors. `files/OpenSplit.tsx:69`
- W2-PRP-097 Open in editor gives way to a sentence: the opens-here line, or a red refusal. `files/OpenInEditor.tsx:29`
- W2-PRP-098 Markdown preview: images drawn as alt-text chips. `components/files/FileMarkdownPreview.tsx:41`
- W2-PRP-099 Markdown preview: only http links open, others plain primary text. `components/files/FileMarkdownPreview.tsx:33`
- W2-PRP-100 File view: no file header, scrolls sideways where diffs wrap. `files/FilePreviewSurface.tsx:161`
- W2-PRP-101 File tab titled by the file name with a file icon; Files tab a folder icon. `panes.ts:113`
- W2-PRP-102 Crumb: full path on hover. `files/FolderBreadcrumbs.tsx:99`
- W2-PRP-103 Crumb row: sideways scroll, no bar, faded cut edge. `files/CrumbScroller.tsx:21`
- W2-PRP-104 File chevron tinted by kind: added, deleted, changed. `lib/diffRendering.ts:231`
- W2-PRP-105 File head hover: 3 px inset edge. `components/diffs/StyledDiffCodeView.tsx:96`
- W2-PRP-106 Selected line: tint and a 4 px bar. `components/diffs/StyledDiffCodeView.tsx:55`
- W2-PRP-107 Comment field placeholder "Add a comment…". `components/diffs/DiffCommentAnnotation.tsx:54`
- W2-PRP-108 Kept comment's delete: on hover, always under 640 px. `components/diffs/DiffCommentAnnotation.tsx:99`
- W2-PRP-109 Reveal note: "<file> has no <scope noun>". `actions/format.ts:270`
- W2-PRP-110 Hold lines: conflicts, checks, draft, review, GitHub still working it out, ready. `pull-request/words.ts:37`
- W2-PRP-111 Armed merge: "Merges by squash when checks pass". `pull-request/words.ts:48`
- W2-PRP-112 Merge methods: Merge commit, Squash and merge, Rebase and merge; the default first. `pull-request/words.ts:90`
- W2-PRP-113 Merge held at 55% opacity. `pull-request/MergeControls.tsx:24`
- W2-PRP-114 Folder refusal in the row menu: "A folder opens in the tree". `actions/format.ts:257`
- W2-PRP-115 Folder refusal in the row menu: "Only a file has changes". `actions/format.ts:258`
- W2-PRP-116 Launcher row Changes: "Review the changes here.", greyed "Review the changes once it runs." `panes.ts:86`
- W2-PRP-117 Launcher row Pull request: "Checks, review and comments.", listed only with a pull request. `panes.ts:95`
- W2-PRP-118 Launcher row Files: "Browse and read the files here."; on this computer greyed "Pick a thread to read its files." `panes.ts:105`
- W2-PRP-119 Changes head 28 px in inline panel mode. `diffs/DiffSurface.tsx:424`
- W2-PRP-120 Pane focus: inset hairline ring on keyboard focus only. `diffs/DiffSurface.tsx:415`
- W2-PRP-121 Tree: folders start closed. `components/files/FileBrowserPanel.tsx:122`
- W2-PRP-122 Tree order: folders first, natural name order, note rows last. `components/files/fileTreeRows.ts:68`

### States

- W2-PRP-123 Discard dialog "No commit has <file>, so discarding deletes it. This cannot be undone." Not driven yet, reach: B, Uncommitted scope, discard src/added.ts. `diffs/words.ts:30`
- W2-PRP-124 Commit note "No agent here drafts commit messages; write it yourself." No shot: needs a host without the draft op. `protocol:changes.ts:121`
- W2-PRP-125 Commit note "The agent did not draft a message; write it yourself." No shot: depends on what the stub agent answers, not checked. `protocol:changes.ts:122`
- W2-PRP-126 Line endings note "This file's line endings are not edited here yet." Not driven yet, reach: B plus a committed file with CRLF endings edited, Uncommitted scope, press its edit. `diffs/words.ts:40`
- W2-PRP-127 "Every changed file was over the patch budget; nothing to render." Not driven yet, reach: E as the only change. `diffs/DiffSurface.tsx:727`
- W2-PRP-128 "Changes are read on the thread's computer; wake it to read them." No shot: nothing stops bench's daemon. `diffs/DiffSurface.tsx:393`
- W2-PRP-129 "Files are read on the thread's computer; wake it to read them." No shot: nothing stops bench's daemon. `files/FilesSurface.tsx:29`
- W2-PRP-130 "The snapshot for this turn is gone". No shot: stub turns take no snapshots. `diffs/words.ts:55`
- W2-PRP-131 Raw patch "This patch could not be read. Showing it raw." No shot: git's patch must fail to parse. `lib/diffRendering.ts:136`
- W2-PRP-132 Raw patch "Failed to parse patch. Showing raw patch." No shot: the parser must throw. `lib/diffRendering.ts:142`
- W2-PRP-133 File read error, nothing read before. Not driven yet, reach: B, open src/keep.ts as a tab, delete the file, reload the window. `files/FilePreviewSurface.tsx:140`
- W2-PRP-134 Markdown opened at a line: source, no toggle. Not driven yet, reach: write README.md holding the word parity, mod+shift+f, type parity, Enter. `files/FilePreviewSurface.tsx:108`
- W2-PRP-135 Finder "No file matches." Not driven yet, reach: mod+p, type zzzq. `files/FileFinder.tsx:26`
- W2-PRP-136 Finder "No line holds that." Not driven yet, reach: mod+shift+f, type zzzq. `files/FileFinder.tsx:27`
- W2-PRP-137 Finder "Type a word to search the files." Not driven yet, reach: mod+shift+f. `files/FileFinder.tsx:28`
- W2-PRP-138 Finder "Open a running thread to search its files." No shot: nothing stops bench's daemon. `files/FileFinder.tsx:29`
- W2-PRP-139 Toast "Diffs are not highlighted: …". No shot: the diff worker must fail. `components/DiffWorkerPoolProvider.tsx:41`
- W2-PRP-140 PR "No commits yet". No shot: GitHub. `pull-request/words.ts:55`
- W2-PRP-141 PR "No files changed". No shot: GitHub. `pull-request/words.ts:56`
- W2-PRP-142 PR file "GitHub sent no diff for this file". No shot: GitHub. `pull-request/words.ts:57`
- W2-PRP-143 PR file "This host does not read a pull request's diff". No shot: GitHub. `pull-request/words.ts:58`
- W2-PRP-144 PR file "The diff was cut at 2 MB before this file". No shot: GitHub. `pull-request/words.ts:59`
- W2-PRP-145 PR "No description.". No shot: GitHub. `pull-request/words.ts:72`
- W2-PRP-146 Draft review "The reviewer is still working". No shot: GitHub. `pull-request/ReviewDraftSection.tsx:65`
- W2-PRP-147 Draft review "the pull request moved on since the review". No shot: GitHub. `pull-request/ReviewDraftSection.tsx:70`
- W2-PRP-148 Posted review "N went into the summary". No shot: GitHub. `pull-request/ReviewDraftSection.tsx:102`

### Actions and keys

- W2-PRP-149 A crumb pins both panes; chevron picks a root; pin toggles following. `files/FolderBreadcrumbs.tsx:92`
- W2-PRP-150 Backspace or Alt+Up: up a folder. `files/FolderBreadcrumbs.tsx:42`
- W2-PRP-151 Refresh, last diff kept. `diffs/DiffSurface.tsx:514`
- W2-PRP-152 Fold by chevron or head click; fold all; a tree row scrolls to its file. `diffs/DiffSurface.tsx:658`
- W2-PRP-153 Edit: whole file, saved whole; CR endings refused. `diffs/DiffSurface.tsx:328`
- W2-PRP-154 Files: row opens a tab, folder lists lazily, folders first; Search, Escape clears; New thread here. `components/files/FileBrowserPanel.tsx:124`
- W2-PRP-155 Right-click: native Open file, Show in Changes, Copy path; folder: first two greyed. `actions/fileActions.ts:15`
- W2-PRP-156 File tab: crumb opens Files there; markdown toggle (kept); reload; Open in editor, SSH consent. `files/FilePreviewSurface.tsx:102`
- W2-PRP-157 PR items: Send to agent, quote reply, react, resolve, reply, comment (mod+Enter; reply Escape), push line to Commits. `pull-request/Conversation.tsx:84`
- W2-PRP-158 PR Files: row opens its diff, read once; Open in Changes. `pull-request/Files.tsx:112`
- W2-PRP-159 Draft: verdict and ticks saved at once, summary on blur, Post. `pull-request/ReviewDraftSection.tsx:37`
- W2-PRP-160 Launcher: arrows move the highlight, Enter opens it. `components/RightPanelTabs.tsx:186`
- W2-PRP-161 ctrl+tab: next panel tab while the tab strip has focus. `keybindingDefaults.ts:80`
- W2-PRP-162 ctrl+shift+tab: previous panel tab while the tab strip has focus. `keybindingDefaults.ts:81`
- W2-PRP-163 Quote reply scrolls the comment box to centre, smooth, caret after the quote. `pull-request/PullRequestSurface.tsx:150`
- W2-PRP-164 Search files hides rows that do not match. `components/files/FileBrowserPanel.tsx:120`
- W2-PRP-165 Commit done: toast "Committed <subject>". `diffs/words.ts:22`
- W2-PRP-166 Update with conflicts: toast names the files and offers Fix. `pull-request/acts.ts:35`
- W2-PRP-167 Finder asks 120 ms after typing stops. `files/FileFinder.tsx:35`
- W2-PRP-168 Finder shows the best 50, a name match before a folder match. `files/fileFinder.logic.ts:7`

### Host operations

- W2-PRP-169 `fs.list` asked again for failed folders when the link comes back. `files/listing.ts:115`
- W2-PRP-170 `workspaces.pullRequestView` read kept 5 min; refresh asks past the host's hold. `pull-request/PullRequestSurface.tsx:48`

### Design

- W2-PRP-171 Motion: file title, color and underline color, 120 ms, ease. `components/diffs/StyledDiffCodeView.tsx:215`
- W2-PRP-172 Motion: hover acts, opacity, 150 ms, default curve. `pull-request/Conversation.tsx:137`
- W2-PRP-173 Tones: green open, approved; red closed, failed, conflicts; violet merged; amber changes asked; quiet draft, unread. `pull-request/words.ts:120`
- W2-PRP-174 Hollow state dot for draft and unread. `pull-request/words.ts:125`
- W2-PRP-175 Counts: compact "1.2k" in Changes and the turn card. `components/chat/DiffStatLabel.tsx:9`
- W2-PRP-176 Counts: whole "1,234" and a true minus in the PR pane. `components/chat/DiffStatLabel.tsx:36`
- W2-PRP-177 Motion: skeleton token, 2 s, linear, infinite. `index.css:184`

## Slates (#2398)

### Elements

- W2-SLT-119 Only the first accent (chart, donut, timeline, treemap), primary button, large text is loud. `slate/engine.ts:596`
- W2-SLT-120 No thread: "Pick a thread to see its slate." `slate/SlateSurface.tsx:62`
- W2-SLT-121 Chat line "Updated the slate". `slate/SlateUpdatedLine.tsx:17`
- W2-SLT-122 Unknown piece type: its fallback piece, else "This part needs a newer wsp". `slate/SlateView.tsx:165`
- W2-SLT-123 Image loading: spinner in a frame held at the picture's size. `slate/pieces/image.tsx:208`
- W2-SLT-124 Thumbnail domain ask: the domain alone, its question on hover; a failed Allow shows the refusal. `slate/pieces/image.tsx:197`
- W2-SLT-125 Over 60 images: one sentence, no pictures. `slate/pieces/image.tsx:439`
- W2-SLT-126 Compare without two images: "A before and after shows two images; this list has N". `slate/pieces/image.tsx:444`
- W2-SLT-127 Empty images list: "No images yet". `slate/pieces/image.tsx:440`
- W2-SLT-128 Number with no value: "Not read yet" at the figure's height. `slate/pieces/number.tsx:53`
- W2-SLT-129 Meter and ring with no value: "Not read yet" as the note. `slate/pieces/meter.tsx:32`
- W2-SLT-130 Sparkline with no values: "Not read yet". `slate/pieces/sparkline.tsx:38`
- W2-SLT-131 Chart with no rows: "Not read yet". `slate/pieces/chart.tsx:92`
- W2-SLT-132 Donut with a zero whole: "Not read yet". `slate/pieces/donut.tsx:37`
- W2-SLT-133 Timeline with no spans: "Not read yet". `slate/pieces/timeline.tsx:107`
- W2-SLT-134 Treemap with no parts: "Not read yet". `slate/pieces/treemap.tsx:79`
- W2-SLT-135 Bars with no rows: "Nothing here". `slate/pieces/bars.tsx:33`
- W2-SLT-136 Table with no rows: its empty text, else "Nothing here". `slate/pieces/table.tsx:154`
- W2-SLT-137 Checklist with no rows: its empty text, else "Nothing here". `slate/pieces/checklist.tsx:33`
- W2-SLT-138 Chart holding one value: "steady at X" on the legend line, no plot. `slate/pieces/chart.tsx:85`
- W2-SLT-139 Select with no value: "Pick one" unless it has a placeholder. `slate/pieces/select.tsx:48`
- W2-SLT-140 Diagram nodes tinted by shape: decision amber, start and end green, store indigo, in and out sky, prep rose, event grey; steps keep the card. `slate/pieces/shapes.ts:111`
- W2-SLT-141 Diagram decision and event labels wrap near 10 chars a line. `slate/pieces/shapes.ts:80`
- W2-SLT-142 Diagram source in mono while Mermaid loads, and over its line when refused. `slate/pieces/diagram.tsx:100`
- W2-SLT-143 Text beside a heading and a button in a row: 12 px muted status line. `slate/pieces/text.tsx:9`
- W2-SLT-144 Row with no align: buttons, status and facts after text, and a toggle after a select, sit at its end. `slate/pieces/row.tsx:15`
- W2-SLT-145 Grid drops to one column under 360 px of its own width. `slate/pieces/grid.tsx:15`
- W2-SLT-146 Stat strip: at most 3 across under 34rem of card, up to 6 wider, hairlines between cells. `slate/pieces/grid.tsx:68`
- W2-SLT-147 Table columns line up with tables beside it that have as many columns. `slate/pieces/table.tsx:72`
- W2-SLT-148 Table draws 8 columns at most. `slate/pieces/table.tsx:31`
- W2-SLT-149 Table row takes 3 actions at most. `slate/pieces/table.tsx:121`
- W2-SLT-150 Bar switch: a window every name ends in, like "(15m)", said once at the right. `slate/pieces/barswitch.tsx:58`
- W2-SLT-151 Bar switch: a list with no label reads "List N". `slate/pieces/barswitch.tsx:25`
- W2-SLT-152 Checklist checkbox disabled, dashed, unless editable on a bound value. `slate/pieces/checklist.tsx:47`
- W2-SLT-153 Facts with no value are left out. `slate/pieces/facts.tsx:28`
- W2-SLT-154 Input field takes its own line under the label once its placeholder passes 17 mono or 20 sans chars. `slate/pieces/input.tsx:31`
- W2-SLT-155 Secret field: dots by the secret's length, 4 to 24. `slate/pieces/input.tsx:43`
- W2-SLT-156 Output run words and inks: Not run yet, Held, Running, Done, Failed, Cancelled. `slate/pieces/output.tsx:22`
- W2-SLT-157 Output refreshing: spinner in place of the run word, last lines kept. `slate/pieces/output.tsx:85`
- W2-SLT-158 Output log: 3 to 40 lines tall, 8 by default. `slate/pieces/output.tsx:60`
- W2-SLT-159 Treemap tile shows its name from 44x20 px, its size from 56x38 px. `slate/pieces/treemap.tsx:86`
- W2-SLT-160 Section refresh glyph: its runs' cadences on hover. `slate/pieces/section.tsx:53`
- W2-SLT-161 Gallery: 2 across under 400 px, 3 from it. `slate/pieces/image.tsx:345`
- W2-SLT-162 Approvals dialog title "Approvals in this thread". `slate/standing.tsx:13`
- W2-SLT-163 Approvals empty: "Nothing in this thread runs without asking." `slate/standing.tsx:38`
- W2-SLT-164 Approvals send row: "Reactions may message the agent", "send() in a reaction". `slate/standing.tsx:22`
- W2-SLT-165 Forget alert title "Forget this slate's secrets?" `slate/SlateSurface.tsx:49`
- W2-SLT-166 Forget alert body "A secret it kept does not come back. The slate asks you to type it again." `slate/SlateSurface.tsx:50`
- W2-SLT-167 Forget alert: refusal line inside; button off while busy. `slate/SlateSurface.tsx:185`
- W2-SLT-168 Empty lead "Cleared." `slate/SlateSurface.tsx:41`
- W2-SLT-169 Empty lead "Rewound to before this slate existed." `slate/SlateSurface.tsx:42`
- W2-SLT-170 Empty lead "This slate needs a newer wsp (schema N)." `slate/SlateSurface.tsx:43`
- W2-SLT-171 Newer schema: no ask line, no buttons, no held note. `slate/SlateSurface.tsx:312`
- W2-SLT-172 Held note: "Answer the prompt in the thread first." `slate/SlateSurface.tsx:39`
- W2-SLT-173 Empty marks `data-slate-empty`: none, cleared, rewound-before, newer. `slate/SlateSurface.tsx:304`
- W2-SLT-174 Header spinner only when a piece under the root reads a refreshing run the root does not say itself. `slate/SlateSurface.tsx:122`
- W2-SLT-175 Held row names a tool run `server.tool`, a resource `server:uri`. `slate/consent.tsx:110`
- W2-SLT-176 Held row cuts a command to its first line. `slate/consent.tsx:123`
- W2-SLT-177 Command block: 12 lines max in the sheet, 4 in a batch row. `slate/consent.tsx:168`
- W2-SLT-178 Piped command: "Then its result goes on stdin to", 6 lines max. `slate/mcp.tsx:79`
- W2-SLT-179 Files: "It reads NAME, which holds", whole text, 12 lines max. `slate/mcp.tsx:61`
- W2-SLT-180 Env grid: name, value; a secret's dots muted. `slate/consent.tsx:181`
- W2-SLT-181 Args as $1, $2 and on. `slate/consent.tsx:186`
- W2-SLT-182 stdin row. `slate/consent.tsx:192`
- W2-SLT-183 Where: "on X, in Y, N s at most". `slate/consent.tsx:199`
- W2-SLT-184 Why line, capitalised, with a period; none when only waiting for approval. `slate/consent.tsx:201`
- W2-SLT-185 "N more commands wait after this one" (one: "1 more command waits after this one"). `slate/consent.tsx:212`
- W2-SLT-186 Always in this thread absent where the host cannot pin it. `slate/consent.tsx:248`
- W2-SLT-187 Cadence: "Runs every N, only while this slate is on screen", or ", also while this slate is not on screen". `slate/consent.tsx:101`
- W2-SLT-188 Cadence: "Runs each time $x changes or $y finishes", else "Runs when you press it". `slate/consent.tsx:106`
- W2-SLT-189 Cadence time: "N s" under 120 s or not whole minutes, then "N min", then "N h". `slate/consent.tsx:93`
- W2-SLT-190 Tool sheet says "Let this panel read" for a resource. `slate/mcp.tsx:147`
- W2-SLT-191 Tool sheet with confirm: red alert, confirm text, Not now, destructive Run. `slate/mcp.tsx:156`
- W2-SLT-192 Tool sheet: description, then "On X. <cadence>." muted. `slate/mcp.tsx:123`
- W2-SLT-193 Show code: `server.tool` or `server:uri`, args (secret dots muted), pipe, files. `slate/mcp.tsx:132`
- W2-SLT-194 Batch row for a tool: "Run" or "Read" <tool> on <Server>, then the tool body. `slate/mcp.tsx:202`
- W2-SLT-195 Batch title for mixed kinds "Let this slate run these N?" `slate/approvals.tsx:20`
- W2-SLT-196 Batch switch label: the command or `server.tool`. `slate/approvals.tsx:61`
- W2-SLT-197 Link prompt: whole address in mono, breaks anywhere. `slate/consent.tsx:37`
- W2-SLT-198 Link prompt: refusal line; buttons off while busy. `slate/consent.tsx:40`
- W2-SLT-199 Unknown type, fallback "drop": nothing. `slate/SlateView.tsx:162`
- W2-SLT-200 Unknown type, fallback id: that piece. `slate/SlateView.tsx:164`
- W2-SLT-201 Unknown type, object fallback: its text as a quiet line. `slate/SlateView.tsx:165`
- W2-SLT-202 Run words: Not run yet, Held, Running, Done, Failed, Cancelled. `slate/pieces/output.tsx:22`
- W2-SLT-203 Run inks: held status-input, running status-working, failed status-failed, rest muted. `slate/pieces/output.tsx:31`
- W2-SLT-204 Held and failed words in medium weight. `slate/pieces/output.tsx:83`
- W2-SLT-205 Crab before Running. `slate/pieces/output.tsx:84`
- W2-SLT-206 Refreshing run: spinner in place of the word, no crab, no Cancel. `slate/pieces/output.tsx:85`
- W2-SLT-207 Log height: `lines` 3 to 40, 8 by default. `slate/pieces/output.tsx:60`
- W2-SLT-208 Failed tool run shows lines, not its result. `slate/pieces/output.tsx:74`
- W2-SLT-209 Result table capped at 50 rows. `slate/shape.tsx:27`
- W2-SLT-210 Result record: facts grid, nested parts under a group label. `slate/shape.tsx:36`
- W2-SLT-211 Why "4 runs are already running". `runtime:slate-runs.ts:411`
- W2-SLT-212 Why "started 12 times in a minute; press to run it again". `runtime:slate-runs.ts:412`
- W2-SLT-213 Why "pressed 12 times in a minute; it can start again in a minute". `runtime:slate-runs.ts:413`
- W2-SLT-214 Why "the box was asleep, so this tick did not wake it; press to run it now". `runtime:slate-runs.ts:414`
- W2-SLT-215 Why "five starts in a row failed, so its timer starts it once every 5 minutes until one succeeds". `runtime:slate-runs.ts:402`
- W2-SLT-216 Host refusals read "CODE name: message". `runtime:slates.ts:229`
- W2-SLT-217 Updated line only under the last reply of a turn that wrote the slate. `components/chat/ChatView.tsx:322`
- W2-SLT-218 No Updated line while history is paged. `components/chat/ChatView.tsx:433`

### States

- W2-SLT-219 Narrow panel: grid one column, gallery 2 across. Not driven yet, reach: localStorage `wsp:right-panel-width` 360, every.slate. `slate/pieces/grid.tsx:15`

### Actions and keys

- W2-SLT-220 Zoom frame: ctrl/cmd wheel, pinch, drag pans, double click fits; plain wheel scrolls. `slate/pieces/zoom.tsx:106`
- W2-SLT-221 Image dialog Previous, Next, Left/Right arrows; slider drag or arrows 5%. `slate/pieces/image.tsx:240`
- W2-SLT-222 Allow images from a domain, then every image asks again. `slate/pieces/image.tsx:184`
- W2-SLT-223 Section fold, bar switch pick (kept per window). `slate/pieces/section.tsx:73`
- W2-SLT-224 Checklist tick writes `$list[i].field`, raises change. `slate/pieces/checklist.tsx:51`
- W2-SLT-225 Timeline without `now` ticks each second. `slate/pieces/timeline.tsx:75`
- W2-SLT-226 Updated line click opens the tab. `slate/SlateUpdatedLine.tsx:15`
- W2-SLT-227 Ask for one: composer draft "Build a slate for this thread that shows ". `slate/SlateSurface.tsx:296`
- W2-SLT-228 Sheets: Not now refuse, Always thread, Run once once; open alone once per key, not while typing elsewhere. `slate/SlateSurface.tsx:240`
- W2-SLT-229 Link: allowed domain opens at once. `slate/store.ts:42`
- W2-SLT-230 Press: host steps as one event, then fill, open, copy, pane; 500 ms fold; outcome 2 s. `slate/actions.ts:116`
- W2-SLT-231 Typing drawn at once, sent after 300 ms or on blur. `slate/actions.ts:47`
- W2-SLT-232 Focused field holds a foreign write: Keep mine, Take theirs. `slate/engine.ts:278`
- W2-SLT-233 Live pushes fold by revision; 500 lines kept; gap re-gets. `slate/store.ts:189`
- W2-SLT-234 Redraw only pieces whose paths moved, 10/s max. `slate/engine.ts:43`
- W2-SLT-235 Sources from the store, else resolve; clock 1 s or 30 s. `slate/sources/binder.ts:91`
- W2-SLT-236 `when` false: not drawn; failed formula: missing. `slate/SlateView.tsx:136`
- W2-SLT-237 Output log follows its end while running. `slate/pieces/output.tsx:68`
- W2-SLT-238 Images asked for once near view, within a screen of the scroll box. `slate/pieces/image.tsx:153`
- W2-SLT-239 Images asked again at each slate write, run end and allowed domain, naming the held version. `slate/pieces/image.tsx:96`
- W2-SLT-240 Secret sent once on blur or Enter, not while typing; the field then empties. `slate/pieces/input.tsx:45`
- W2-SLT-241 Secret emptied while set clears it. `slate/pieces/input.tsx:46`
- W2-SLT-242 Secret refused: the refusal under the field. `slate/pieces/input.tsx:90`
- W2-SLT-243 Zoom scale 0.2x to 4x. `slate/pieces/zoom.tsx:20`
- W2-SLT-244 Zoom frame shows a grab cursor once the drawing outgrows it. `slate/pieces/zoom.tsx:128`
- W2-SLT-245 Zoom frame edges fade where more of the drawing lies beyond. `slate/pieces/zoom.tsx:118`
- W2-SLT-246 Dialog Zoom in and Zoom out step 1.25x around the centre. `slate/pieces/diagram.tsx:119`
- W2-SLT-247 Number input writes null for text that is not a number. `slate/pieces/input.tsx:124`
- W2-SLT-248 Ask for one puts the caret at the end. `slate/SlateSurface.tsx:299`
- W2-SLT-249 Ask for one folds an open composer bar. `slate/SlateSurface.tsx:301`
- W2-SLT-250 Ask for one off while a prompt is docked in the composer. `slate/SlateSurface.tsx:314`
- W2-SLT-251 Link Always: allows the domain for the thread, then opens. `slate/consent.tsx:52`
- W2-SLT-252 The sheet a press or Review named opens first. `slate/SlateSurface.tsx:246`
- W2-SLT-253 Two or more batchable asks, one unseen: one batch sheet. `slate/SlateSurface.tsx:257`
- W2-SLT-254 Closing the batch marks all its keys seen. `slate/SlateSurface.tsx:260`
- W2-SLT-255 Closing a single sheet marks its key seen. `slate/SlateSurface.tsx:267`
- W2-SLT-256 A run held by a limit has no sheet: asking stops once the record lacks it. `slate/SlateSurface.tsx:253`
- W2-SLT-257 Typing elsewhere: a text input, textarea or editable outside the slate. `slate/SlateSurface.tsx:224`
- W2-SLT-258 Batch takes commands with no confirm and no no-Always flag. `slate/askKinds.tsx:34`
- W2-SLT-259 Batch takes tool runs with no confirm. `slate/askKinds.tsx:39`
- W2-SLT-260 Batch: one row per approval key. `slate/approvals.tsx:30`
- W2-SLT-261 Batch buttons off while no row is on. `slate/approvals.tsx:71`
- W2-SLT-262 One Revoke in flight turns off every Revoke. `slate/standing.tsx:56`
- W2-SLT-263 Pane step refused: "That pane is not available here." `slate/actions.ts:172`
- W2-SLT-264 Copy step without a clipboard: "This window cannot copy." `slate/actions.ts:182`
- W2-SLT-265 Copy step: "Copied", or "Could not copy; click the window and press it again." `slate/actions.ts:183`
- W2-SLT-266 Fill step: text, a blank line, then "> path: value" per carried path. `slate/actions.ts:167`
- W2-SLT-267 An event answer carrying an ask opens its sheet. `slate/actions.ts:132`
- W2-SLT-268 Secret text sent once; the window keeps only its handle. `slate/actions.ts:84`
- W2-SLT-269 A failed value write stays drawn as typed. `slate/actions.ts:96`
- W2-SLT-270 Offline, every act refused: "Not connected to wsp. The slate will catch up when it is." `slate/store.ts:93`
- W2-SLT-271 Each thread's engine lives for the window: folds and unsent text survive switching. `slate/store.ts:109`
- W2-SLT-272 One get in flight per thread, plus one follow-up. `slate/store.ts:145`
- W2-SLT-273 A slate write not by the agent is ignored unless the thread is known or selected. `slate/store.ts:194`
- W2-SLT-274 An agent write opens the tab once. `slate/store.ts:196`
- W2-SLT-275 A newly held run with no ask gets the record again. `slate/store.ts:206`
- W2-SLT-276 An empty `slate.values` push gets the record again. `slate/store.ts:208`
- W2-SLT-277 Nothing loads while Settings is open. `slate/SlateHost.tsx:39`
- W2-SLT-278 Shown hold taken again on reconnect. `slate/SlateSurface.tsx:91`
- W2-SLT-279 One primary button, large figure, accent per slate: the first in order; the rest draw quiet. `slate/engine.ts:577`
- W2-SLT-280 A record older than the drawn document is dropped. `slate/engine.ts:123`
- W2-SLT-281 A refresh keeps the last lines until its first new line. `slate/engine.ts:209`
- W2-SLT-282 Source holds taken again on reconnect. `slate/sources/binder.ts:182`
- W2-SLT-283 Host values whose source moved stay drawn until answered again. `slate/sources/binder.ts:100`
- W2-SLT-284 A path the window lacks is asked of the host once. `slate/sources/binder.ts:47`
- W2-SLT-285 Host: 4 runs at once. `protocol:slate/limits.ts:17`
- W2-SLT-286 Host: 12 starts a minute. `protocol:slate/limits.ts:18`
- W2-SLT-287 Host: timers no faster than 10 s. `runtime:slate-runs.ts:398`
- W2-SLT-288 Host: 60 s default timeout. `runtime:slate-runs.ts:394`

### Host operations

- W2-SLT-289 `slates.event` requestId: thread, version, piece, window id, counter. `slate/actions.ts:126`
- W2-SLT-290 Push `workspace.status`: git and pr sources. `protocol/store/useStore.ts:1129`
- W2-SLT-291 Push `workspace.cost`: cost source. `protocol/store/useStore.ts:1138`
- W2-SLT-292 Push `usage.account`: usage source. `protocol/store/useStore.ts:1178`

### Design

- W2-SLT-293 Motion: section chevron rotate 150 ms, panel height 200 ms, default curves. `slate/pieces/section.tsx:85`
- W2-SLT-294 Motion: refresh glyph, animate-spin, 1 s linear. `slate/pieces/section.tsx:54`
- W2-SLT-295 Motion: chevron rotate 150 ms, fold height 200 ms, default curve. `slate/mcp.tsx:127`
- W2-SLT-296 Motion: spinner 1 s linear, off for reduced motion. `slate/pieces/refreshing.tsx:8`
- W2-SLT-297 Motion: Updated line colors 150 ms default curve. `slate/SlateUpdatedLine.tsx:14`
- W2-SLT-298 Motion: tooltip, opacity and 2 px slide, 150 ms, default curve. `components/ui/tooltip.tsx:46`
- W2-SLT-299 Motion: checkbox shadow, 150 ms, default curve. `components/ui/checkbox.tsx:16`
- W2-SLT-300 Motion: button colors, 150 ms, default curve. `components/ui/button.tsx:22`
- W2-SLT-301 Motion: running output crab, canvas frames, still under reduced motion, `ui::Crab`. `slate/pieces/output.tsx:84`
- W2-SLT-302 Motion: switch thumb translate .15 s, radius .15 s, scale .1 s after .1 s, CSS ease; press stretches it 110%. `components/ui/switch.tsx:20`
- W2-SLT-303 Motion: launcher card colors, 150 ms, default curve. `components/RightPanelTabs.tsx:275`
- W2-SLT-304 Motion: dialog backdrop transition-all, 200 ms, default curve. `components/ui/dialog-styles.ts:3`
- W2-SLT-305 Motion: scrollbar opacity, in 100 ms on hover or scroll, out after 300 ms, default curve. `components/ui/scroll-area.tsx:81`
- W2-SLT-306 Menu popup: popover ground, 10% border, shadow, 12 px radius, 128 px min, no motion. `components/ui/menu.tsx:62`
- W2-SLT-307 `--slate-accent` is primary in paper, graphite, denim, pitch, frost. `themes/graphite.css:15`
- W2-SLT-308 `--slate-accent` is hero-field in tungsten, linen, moss. `themes/tungsten.css:14`
- W2-SLT-309 Quiet line: 12 px, muted, never red. `slate/pieces/quiet.tsx:7`
- W2-SLT-310 Outcome line: 12 px, muted, a refusal in error ink, role status. `slate/pieces/outcome.tsx:8`

## Setup dialogs, onboarding and the pair screen (#2399)

### Elements

- W2-DLG-061 Run heads: Setting up X, X needs you, Setup on X failed, X is ready, Set up in T. `settings/add/AddComputerDialog.tsx:714`
- W2-DLG-062 Step marks: empty circle "Not started", triangle "Passed with a warning", minus "Skipped", crab for an agent at work. `settings/add/StepRow.tsx:66`
- W2-DLG-063 Step log: mono, max-h-48, scrolls to its end; "No output yet." when empty. `settings/add/StepRow.tsx:153`
- W2-DLG-064 Step log keeps each step's last 12 lines, 300 chars a line. `settings/add/setup.ts:277`
- W2-DLG-065 Running note: "Installing A, B and N more.", or "Copying N skills" past three. `settings/add/setup.ts:124`
- W2-DLG-066 Ended note: a count ("3 CLIs", "2 MCP servers, 1 agent file"). `settings/add/setup.ts:136`
- W2-DLG-067 Sign-in items "X sign-in" under Agents. `settings/add/setup.ts:197`
- W2-DLG-068 Waiting sign-in note "A tab opened in your browser.". `settings/add/setup.ts:205`
- W2-DLG-069 Sign-in wait note "You can sign in later in Settings.". `settings/add/AddComputerDialog.tsx:292`
- W2-DLG-070 Expired sign-in: "The sign-in page ran out. Retry opens a fresh one.". `settings/add/AddComputerDialog.tsx:271`
- W2-DLG-071 Order words: As found or Most used, Name, Ticked first. `settings/add/PickList.tsx:63`
- W2-DLG-072 Row hover: "Shift-click to tick or clear every row from the last one you clicked.". `settings/add/PickList.tsx:29`
- W2-DLG-073 Agent sign-in select: Copy the key, "Sign in on X" (plus "with a code" for device sign-in), token, key. `settings/add/PickLists.tsx:63`
- W2-DLG-074 Agent row: version tag, size, note saying how a token or key way works. `settings/add/PickLists.tsx:137`
- W2-DLG-075 MCP keys radio: "Copy the keys to X", note naming the keys. `settings/add/PickLists.tsx:197`
- W2-DLG-076 MCP keys radio: "Leave them on Y", note "A and B are skipped on X until you copy their keys.". `settings/add/PickLists.tsx:198`
- W2-DLG-077 MCP row note "Skipped until you copy its keys." while keys stay. `settings/add/PickLists.tsx:178`
- W2-DLG-078 CLIs: "Select the ones your agents used" while a used CLI is unticked. `settings/add/AddComputerDialog.tsx:681`
- W2-DLG-079 CLI row: call count. `settings/add/PickLists.tsx:234`
- W2-DLG-080 CLI row: size in a column as wide as the widest. `settings/add/PickLists.tsx:240`
- W2-DLG-081 CLI row note: "via. Needs A, B.". `settings/add/PickLists.tsx:227`
- W2-DLG-082 GitHub radio "Use the token from X", note "Signed in as A with scopes.". `settings/add/PickLists.tsx:317`
- W2-DLG-083 GitHub radio "Sign in on X", note "A tab opens in your browser here.". `settings/add/PickLists.tsx:318`
- W2-DLG-084 GitHub radio Skip for now, note "Private repos will not clone until you sign in. You can do it later in Settings.". `settings/add/PickLists.tsx:319`
- W2-DLG-085 Project row: path tag. `settings/add/PickLists.tsx:418`
- W2-DLG-086 Project row note: remote and unpushed commits, or "No origin remote, so the add refuses it; push it somewhere first.". `settings/add/PickLists.tsx:378`
- W2-DLG-087 Project row: size. `settings/add/PickLists.tsx:420`
- W2-DLG-088 Project name field: "X already has a project called Y.", field marked invalid. `settings/add/PickLists.tsx:429`
- W2-DLG-089 Project row note when GitHub is skipped and the repo is private: "Private; needs GitHub to clone. Go back to sign in, or skip it.". `settings/add/PickLists.tsx:407`
- W2-DLG-090 Add a folder button, desktop only. `settings/add/AddComputerDialog.tsx:189`
- W2-DLG-091 Summary: an empty value reads None. `settings/add/AddComputerDialog.tsx:225`
- W2-DLG-092 Summary GitHub line: Token from X, Sign in on X, or Skip for now. `settings/add/AddComputerDialog.tsx:209`
- W2-DLG-093 Disk line: "N needed, N kept free, N free", or "N free" before the weigh answers. `settings/add/AddComputerDialog.tsx:238`
- W2-DLG-094 Disk line tooltip: "N picked rows were not measured". `settings/add/AddComputerDialog.tsx:235`
- W2-DLG-095 Save as a recipe ticked: Name field, Icon select. `settings/add/AddComputerDialog.tsx:250`
- W2-DLG-096 Refusal slot: "Hosts from your ssh config not read: ...". `settings/add/AddComputerDialog.tsx:66`
- W2-DLG-097 Options loading spinner. `settings/add/AddComputerDialog.tsx:657`
- W2-DLG-098 Options refused slot. `settings/add/AddComputerDialog.tsx:655`
- W2-DLG-099 Set up refused slot. `settings/add/AddComputerDialog.tsx:704`
- W2-DLG-100 Row act refusal under its row; a host conflict reads "X is still being set up; press again once it is done.". `settings/add/AddComputerDialog.tsx:360`
- W2-DLG-101 Cloud key held: "A key is saved. Continue uses it.". `settings/add/AddCloudDialog.tsx:146`
- W2-DLG-102 Cloud key placeholder "Paste a new key to replace the saved one" when held. `settings/add/AddCloudDialog.tsx:133`
- W2-DLG-103 Check line: "Checking the key with X". `settings/add/AddCloudDialog.tsx:63`
- W2-DLG-104 Check line works: "Works. Account A, plan, n of m running at once.". `settings/add/cloudFlow.ts:129`
- W2-DLG-105 Check line unreachable: Try again. `settings/add/AddCloudDialog.tsx:70`
- W2-DLG-106 Cloud provider radio: name, fact "from $0.018/h". `settings/add/AddCloudDialog.tsx:86`
- W2-DLG-107 Provider head "Add a cloud provider", "Where your sub-threads run.". `settings/add/AddCloudDialog.tsx:469`
- W2-DLG-108 Key head "Add X", "wsp runs your sub-threads on your X account with this key.". `settings/add/AddCloudDialog.tsx:471`
- W2-DLG-109 Cloud picks loading: spinner, "Reading what X has". `settings/add/AddCloudDialog.tsx:496`
- W2-DLG-110 Cloud agent note "Uses your sign-in on X." or "Needs a sign-in on X first.". `settings/add/AddCloudDialog.tsx:176`
- W2-DLG-111 Cloud row note "Installed signed out." or always on. `settings/add/AddCloudDialog.tsx:186`
- W2-DLG-112 Cloud summary lines: agents by name, others "n of m". `settings/add/AddCloudDialog.tsx:254`
- W2-DLG-113 Sign-ins on X card, lede "Threads on Y use these through wsp. Nothing is signed in on Y.". `settings/add/AddCloudDialog.tsx:263`
- W2-DLG-114 Sign-in row "Reading the sign-ins on X" before the report. `settings/add/AddCloudDialog.tsx:237`
- W2-DLG-115 GitHub sign-in row "Signed in as A" or "Run it in a terminal on X". `settings/add/AddCloudDialog.tsx:246`
- W2-DLG-116 gh auth login copy row under an unsigned GitHub. `settings/add/AddCloudDialog.tsx:269`
- W2-DLG-117 Build failed refusal slot. `settings/add/AddCloudDialog.tsx:310`
- W2-DLG-118 Build again, held while busy. `settings/add/AddCloudDialog.tsx:561`
- W2-DLG-119 Cloud ready line "Image vN sealed in T.", primary Start a thread. `settings/add/AddCloudDialog.tsx:584`
- W2-DLG-120 Remove titles: remove, forget, removed. `settings/RemoveComputerDialog.tsx:141`
- W2-DLG-121 Remove busy words: Removing... and Forgetting... `settings/RemoveComputerDialog.tsx:173`
- W2-DLG-122 Unsaved note scrolls in max-h-9 on a computer; held sub-thread titles on a cloud. `settings/RemoveComputerDialog.tsx:159`
- W2-DLG-123 Slot waiting line while holdings are read. `settings/RemoveComputerDialog.tsx:157`
- W2-DLG-124 Delete thread busy: "Deleting...". `components/DeleteThreadDialog.tsx:97`
- W2-DLG-125 Stop and delete busy: Stopping. `components/DeleteThreadDialog.tsx:105`
- W2-DLG-126 Workspace titles: "Delete X?", "Delete N copies?", "Delete the other N copies?", same for Forget. `components/ForgetWorkspaceDialog.tsx:128`
- W2-DLG-127 Workspace delete busy: "Deleting...". `components/ForgetWorkspaceDialog.tsx:26`
- W2-DLG-128 Workspace forget busy: "Forgetting...". `components/ForgetWorkspaceDialog.tsx:27`
- W2-DLG-129 Onboarding row state: "installs on first run", "installed", "not installed". `desktop:onboarding.html:303`
- W2-DLG-130 Onboarding row of an agent not found: unticked, disabled. `desktop:onboarding.html:294`
- W2-DLG-131 Onboarding rows: found agents first, then the rest. `desktop:onboarding.html:336`
- W2-DLG-132 Onboarding refusal: the raw error on its hover title. `desktop:onboarding.html:239`
- W2-DLG-133 Onboarding refusal names "A, B and N more" past three. `desktop:onboarding.html:226`
- W2-DLG-134 Agents sentence gains "on X" once the computer's name lands. `desktop:onboarding.html:260`
- W2-DLG-135 Pair button reads "Pairing" while busy. `PairScreen.tsx:44`
- W2-DLG-136 Device name: "a Mac in a browser at host". `protocol/pairing.ts:81`
- W2-DLG-137 Failed start lede: "Start it again from the thread that asked for it.". `shell/FailedStart.tsx:17`
- W2-DLG-138 Failed start row: "Asking wsp to start it". `shell/creationLog.ts:24`
- W2-DLG-139 Start failed box: title, first line of the error, Open log, Quit. `desktop:starting.ts:38`

### States

- W2-DLG-140 Filter matches nothing: "No row matches X.". No shot: pick steps need a real SSH host. `settings/add/PickList.tsx:114`

### Actions and keys

- W2-DLG-141 Opens from Settings, palette, switcher, Add project, pending row; Setting up row, notices open running view. `settings/settingsReads.ts:36`
- W2-DLG-142 Esc or outside press closes, picks kept; Esc an inner control takes does not. `settings/add/AddComputerDialog.tsx:815`
- W2-DLG-143 Picks saved 400 ms after a change; saved check redraws at most every 4 s. `settings/add/addFlow.ts:259`
- W2-DLG-144 Shift-click ranges; tools act on filtered rows; locked row tells why. `settings/add/PickList.tsx:55`
- W2-DLG-145 Row click toggles log, read every 3 s while running; first row needing you scrolled to. `settings/add/AddComputerDialog.tsx:470`
- W2-DLG-146 Row acts: Retry, Skip, Skip for now, Copy the keys, Sign in per way, Cancel. `settings/add/AddComputerDialog.tsx:421`
- W2-DLG-147 Close while running posts a notice; Run in background; Start a task here. `settings/add/addFlow.ts:183`
- W2-DLG-148 Key checked on paste or Enter; Build image held until signed in. `settings/add/AddCloudDialog.tsx:399`
- W2-DLG-149 Pair on submit; address code spent once; 2 refusals, code screen; desktop asks shell again on focus. `BootGate.tsx:63`
- W2-DLG-150 Pending add reopens where it was left (per window). `settings/add/addFlow.ts:148`
- W2-DLG-151 Running foot once ready: Close and Next. `settings/add/AddComputerDialog.tsx:794`
- W2-DLG-152 Running foot when failed or needs you: Close only. `settings/add/AddComputerDialog.tsx:801`
- W2-DLG-153 Checks Continue held until the computer joins. `settings/add/AddComputerDialog.tsx:773`
- W2-DLG-154 Projects Continue held while a name is taken. `settings/add/AddComputerDialog.tsx:780`
- W2-DLG-155 An empty first pick step ticks everything here. `settings/add/AddComputerDialog.tsx:606`
- W2-DLG-156 That fill ticks no CLI. `settings/add/choices.ts:54`
- W2-DLG-157 That fill leaves plugins that are off unticked. `settings/add/choices.ts:60`
- W2-DLG-158 Sign-ins the computer cannot serve move to the first one offered. `settings/add/AddComputerDialog.tsx:612`
- W2-DLG-159 A recipe save refused after Set up shows as a notice. `settings/add/addFlow.ts:332`
- W2-DLG-160 A pending add closed on its checks reopens on its checks. `settings/add/addFlow.ts:159`
- W2-DLG-161 Closing the cloud dialog once the build seals posts the sealed notice. `settings/add/AddCloudDialog.tsx:395`
- W2-DLG-162 The sealed notice's Open shows the ready page owed to that build. `settings/add/cloudFlow.ts:121`
- W2-DLG-163 Start a thread opens a new thread here, composer "Start a sub-thread on Boat to ". `settings/add/AddCloudDialog.tsx:317`
- W2-DLG-164 That prefill lands only on an empty draft, caret at its end. `settings/add/AddCloudDialog.tsx:332`
- W2-DLG-165 A build started from another window moves the summary on to the build. `settings/add/AddCloudDialog.tsx:375`
- W2-DLG-166 Remove held while holdings are read, and while a sudo password is asked and empty. `settings/RemoveComputerDialog.tsx:172`
- W2-DLG-167 Delete thread: a stop that left processes says so, then Close. `components/DeleteThreadDialog.tsx:62`
- W2-DLG-168 Workspace Delete held while copies are read. `components/ForgetWorkspaceDialog.tsx:153`
- W2-DLG-169 Ask before deleting off: a clean delete, and any forget, go without the dialog. `components/ForgetWorkspaceDialog.tsx:92`
- W2-DLG-170 The dialog stays unseen while copies are read. `components/ForgetWorkspaceDialog.tsx:94`
- W2-DLG-171 Pair held while the code is empty. `PairScreen.tsx:43`
- W2-DLG-172 A paste that is no code is refused before any host is dialled. `protocol/pairing.ts:59`
- W2-DLG-173 Onboarding follows the system scheme, transitions held for a frame. `desktop:onboarding.html:213`
- W2-DLG-174 Onboarding sends title bar colours on a scheme flip. `desktop:onboarding.html:216`
- W2-DLG-175 Onboarding top band drags the window. `desktop:onboarding.html:25`
- W2-DLG-176 Starting window shows only once an open runs 800 ms. `desktop:starting.ts:9`

### Design

- W2-DLG-177 Motion: saved check stroke 600 ms cubic-bezier(0.16, 1, 0.3, 1), ink 1400 ms ease-out. `index.css:1686`
- W2-DLG-178 Motion: chevron rotate, row hover, 150 ms, default curve; spinner 1 s linear. `settings/add/StepRow.tsx:87`
- W2-DLG-179 Motion: onboarding glow 400 ms, halo 1.2 s, stroke 0.9 s, rises 0.34 to 0.5 s, cubic-bezier(0.16, 1, 0.3, 1). `desktop:onboarding.html:28`
- W2-DLG-180 Motion: nested popup up 1.25rem, scale and opacity less 0.1 per level, nested in and out 2rem, 200 ms ease-in-out. `components/ui/dialog-styles.ts:6`
- W2-DLG-181 Motion: narrow sheet in and out 1rem, opacity per nesting. `components/ui/dialog-styles.ts:9`
- W2-DLG-182 Motion: onboarding row hover background, 150 ms, CSS ease. `desktop:onboarding.html:95`
- W2-DLG-183 Motion: onboarding keycap shadow and background, 150 ms, CSS ease. `desktop:onboarding.html:125`
- W2-DLG-184 Motion: first run privacy link colour, 150 ms, default curve. `shell/FirstRun.tsx:34`
- W2-DLG-185 Motion: reduced motion turns off every onboarding animation. `desktop:onboarding.html:87`
- W2-DLG-186 Motion: reduced motion leaves the saved check still; dialog transitions unchanged. `index.css:1712`

## Palette, switcher, activity and notices (#2400)

### Elements

- W2-PAL-059 Thread row: harness mark, title, task and computer, status, age. `components/palette/paletteItems.tsx:391`
- W2-PAL-060 Task row: glyph, state on computer, "Current task", mod+N. `components/palette/paletteItems.tsx:368`
- W2-PAL-061 Message hit: snippet; "N older threads not searched". `components/palette/paletteItems.tsx:428`
- W2-PAL-062 Footer: arrows Navigate, Enter Run or Select, Esc Close. `components/palette/CommandPaletteContent.tsx:50`
- W2-PAL-063 Tiles: memory, CPU, Power, Processes, lines, tints. `settings/activity/page.tsx:241`
- W2-PAL-064 Controls: computer, kind, thread, Every process, filter. `settings/activity/page.tsx:322`
- W2-PAL-065 Group head, rows: glyph, name, tags, figures, menu. `settings/activity/tables.tsx:342`
- W2-PAL-066 Container, subagent, sampler rows; Ending tag, spinner for menu. `settings/activity/tables.tsx:269`
- W2-PAL-067 Disabled New thread says Not connected to the runtime, or Add a project first. `components/palette/paletteItems.tsx:205`
- W2-PAL-068 Disabled thread steps say Only one thread, No threads to walk, or "Nothing to step to on X; a walk stays inside one task". `components/palette/paletteItems.tsx:255`
- W2-PAL-069 Disabled task steps say Only one task. `components/palette/paletteItems.tsx:264`
- W2-PAL-070 Disabled Toggle right panel says Select a task first. `components/palette/paletteItems.tsx:350`
- W2-PAL-071 Disabled Resume a conversation says to open a project first. `components/palette/paletteItems.tsx:414`
- W2-PAL-072 A refused registry act shows its refusal as the row's description. `components/palette/paletteItems.tsx:120`
- W2-PAL-073 New thread page row: project glyph, computer and folder path. `components/palette/paletteItems.tsx:164`
- W2-PAL-074 New thread page row: mod+N label on the first nine. `components/palette/paletteItems.tsx:165`
- W2-PAL-075 Link rows: wsp on GitHub, wsp docs, Report a bug, each with its address as description. `components/palette/paletteItems.tsx:100`
- W2-PAL-076 Palette under 640 px sits 4vh from the top, else 10vh. `components/ui/command.tsx:46`
- W2-PAL-077 Pane tag for a shell: terminal and its tab title. `components/procs/tree.ts:116`
- W2-PAL-078 Pane tag for an app process: the app part's own line. `components/procs/tree.ts:113`
- W2-PAL-079 Pane count: pending, N of M under a filter, N processes. `components/procs/ProcessesSurface.tsx:101`
- W2-PAL-080 Pane count reads unavailable when the daemon refused the watch. `components/procs/ProcessesSurface.tsx:126`
- W2-PAL-081 Pane agent row with no thread: "a thread's agent: stop its thread to end it". `components/procs/ProcessesSurface.tsx:31`
- W2-PAL-082 Pane kept agent End said: "freed; the thread's next send starts a new one". `components/procs/ProcessesSurface.tsx:33`
- W2-PAL-083 Pane End said where the window cannot ask: "this window cannot end a thread's agent". `components/procs/ProcessesSurface.tsx:34`
- W2-PAL-084 Kill result words: ended, ended by KILL after TERM's grace, still running after a forced end. `components/procs/ProcessesSurface.tsx:294`
- W2-PAL-085 Kill said ending while the daemon works. `components/procs/ProcessesSurface.tsx:338`
- W2-PAL-086 Forced kill said KILL sent. `components/procs/ProcessesSurface.tsx:335`
- W2-PAL-087 Pane Stop turn ask: "Stop <title>?", Cancel, Stop. `components/procs/ProcessesSurface.tsx:433`
- W2-PAL-088 Pane Stop turn ask text: "This process is the thread's agent. Its turn ends as interrupted; what it started keeps running.". `components/procs/ProcessesSurface.tsx:434`
- W2-PAL-089 Pane Stop turn said: stopping, stopped, no turn is running, or the host's words. `components/procs/ProcessesSurface.tsx:415`
- W2-PAL-090 Pane Stop turn said where the window cannot ask: "this window cannot stop a thread". `components/procs/ProcessesSurface.tsx:410`
- W2-PAL-091 Settle ask: refusal line, else the note "Settling leaves these running." and what Stop ends. `components/procs/ProcessesLeft.tsx:102`
- W2-PAL-092 Settle ask: Stop reads Stopping... while busy. `components/procs/ProcessesLeft.tsx:112`
- W2-PAL-093 Kinds view heads: Agents, Tool servers, Dev servers, Test runs, Builds, Slate runs, Terminals, Browser tabs, The app, wsp, Other. `settings/activity/tables.tsx:66`
- W2-PAL-094 Container facts: image, Up <time>, compose folder under No thread. `settings/activity/tables.tsx:271`
- W2-PAL-095 Container tooltip: who placed it, its docker, its compose project, nobody, or a thread wsp no longer knows. `settings/activity/tables.tsx:273`
- W2-PAL-096 Toast title clamps at 2 lines. `notices/Notice.tsx:49`
- W2-PAL-097 Switcher card picture: the page shot 480 px wide. `desktop:previews.ts:6`

### States

- W2-PAL-098 Activity filtered to nothing: "Nothing matches." Not driven yet, reach: filter "zzqx". `settings/activity/tables.tsx:519`
- W2-PAL-099 Activity empty: "No thread is running anything." No shot: wsp's own row always stands on this computer. `settings/activity/tables.tsx:592`
- W2-PAL-100 Three toasts stacked, back two collapsed. Not driven yet, reach: mod+o three times. `notices/store.ts:37`
- W2-PAL-101 Pane away: the computer's said and will lines. No shot: dead daemon. `components/procs/ProcessesSurface.tsx:143`
- W2-PAL-102 Start button reads Starting while busy. No shot: dead daemon. `components/DaemonDown.tsx:56`
- W2-PAL-103 Start refused: a red line under the button. No shot: dead daemon. `components/DaemonDown.tsx:59`
- W2-PAL-104 Thread pane pending: "Reading this computer's processes." No shot: transient. `components/procs/ThreadProcesses.tsx:90`
- W2-PAL-105 Thread pane refused: the daemon's reason. No shot: needs a daemon that refuses the watch. `components/procs/ThreadProcesses.tsx:86`
- W2-PAL-106 Old daemon, pane: TERM went to this process alone, with the update line. No shot: needs an old daemon. `components/procs/ProcessesSurface.tsx:345`
- W2-PAL-107 Old daemon, pane: kill -9 offered once TERM did nothing for 5 s. No shot: needs an old daemon. `components/procs/ProcessesSurface.tsx:350`
- W2-PAL-108 Old daemon, Activity: one error toast naming the processes, Force end button. No shot: needs an old daemon. `settings/activity/live.tsx:208`
- W2-PAL-109 Bring back done toast. No shot: needs a cloud task. `notices/workspaceLines.ts:54`
- W2-PAL-110 Palette under 640 px: footer stacks. Not driven yet, reach: 600 px window, mod+k. `components/palette/CommandPaletteContent.tsx:46`
- W2-PAL-111 A subagent row stays 60 s after it ends. No shot: needs a real subagent. `settings/activity/model.ts:59`

### Actions and keys

- W2-PAL-112 Failed run: error toast. `components/palette/CommandPalette.tsx:196`
- W2-PAL-113 Row menu, right click: port, Stop, End, End all, thread, copies. `settings/activity/actions.ts:40`
- W2-PAL-114 Toast 8 s, 12 s with action, a wait until ended; 4th drops oldest plain; 400 chars. `notices/store.ts:31`
- W2-PAL-115 Title mark while a need or ask stands; dock badge counts waits. `notices/hostNotices.ts:374`
- W2-PAL-116 Link rows open their address in the system browser. `components/palette/paletteItems.tsx:341`
- W2-PAL-117 Page mod digits work only on desktop or in a tab that leaves them free. `components/palette/paletteItems.tsx:134`
- W2-PAL-118 Search: every typed word must appear in the row's terms. `components/palette/CommandPalette.logic.ts:166`
- W2-PAL-119 Rank: an earlier term first, then exact, prefix, contains. `components/palette/CommandPalette.logic.ts:95`
- W2-PAL-120 Pane lit row deselects when its process goes away. `components/procs/ProcessesSurface.tsx:97`
- W2-PAL-121 Pane filter keeps a match's parents above it. `components/procs/tree.ts:47`
- W2-PAL-122 ctrl+shift+tab walks the switcher back. `keybindingDefaults.ts:79`
- W2-PAL-123 Thread chords walk only the open task's threads. `shell/shellCommands.ts:393`
- W2-PAL-124 The switcher opens nothing with no other thread to land on. `shell/shellCommands.ts:245`
- W2-PAL-125 Leaving a thread shoots its page for its card. `components/switcher/WorkspaceSwitcher.tsx:41`
- W2-PAL-126 Switcher paint waits for the shot of the thread on screen. `components/switcher/WorkspaceSwitcher.tsx:65`
- W2-PAL-127 That opening shot gives up after 500 ms. `desktop:previews.ts:12`
- W2-PAL-128 12 pictures kept, oldest dropped, memory only. `desktop:previews.ts:7`
- W2-PAL-129 Pictures filed per theme; another theme's is never drawn. `shell/workspacePreviews.ts:29`
- W2-PAL-130 No shot while a layer, the switcher, a toast, a tooltip or a sheet is over the page. `shell/workspacePreviews.ts:25`
- W2-PAL-131 A browser tab takes no pictures; its wells keep the glyph. `shell/workspacePreviews.ts:36`
- W2-PAL-132 Toast Escape: the focused toast first, else the newest unless a field, dialog or menu holds focus. `notices/Notice.tsx:22`
- W2-PAL-133 Notice Open on a thread event selects that thread. `notices/hostNotices.ts:103`
- W2-PAL-134 Notice Open on a computer event opens Settings on that computer. `notices/hostNotices.ts:107`
- W2-PAL-135 Notice Open on a plan alert opens Settings, Usage. `notices/hostNotices.ts:95`
- W2-PAL-136 Notice Open on a setup opens Add a computer on its steps. `notices/hostNotices.ts:105`
- W2-PAL-137 Thread notices skipped while that thread is on screen. `notices/hostNotices.ts:70`
- W2-PAL-138 workspace.gone skipped while that task is on screen. `notices/hostNotices.ts:72`
- W2-PAL-139 Build notices skipped while its Image card shows. `notices/hostNotices.ts:76`
- W2-PAL-140 Computer notices skipped while its rows or Add a computer show. `notices/hostNotices.ts:86`

### Host operations

- W2-PAL-141 Push `workspace.woken`: a system notification only, no toast. `notices/hostNotices.ts:349`

### Design

- W2-PAL-142 Motion: card, Back, colors 150 ms default. `components/switcher/WorkspaceSwitcher.tsx:96`
- W2-PAL-143 Motion: rows, act, menu, chevron, sort, kill, Open in Activity, 150 ms default. `settings/activity/tables.tsx:93`
- W2-PAL-144 Memory tint: warn past 2 GB, danger past 8 GB. `settings/activity/model.ts:514`
- W2-PAL-145 Share tint: warn past 50% of the computer's memory, danger past 75%. `settings/activity/model.ts:518`
- W2-PAL-146 CPU tint: warn past 100%, danger past 400%. `settings/activity/model.ts:521`
- W2-PAL-147 Power tint: warn past 1 W, danger past 3 W. `settings/activity/model.ts:524`
- W2-PAL-148 Tile line holds 150 points. `settings/activity/model.ts:533`
- W2-PAL-149 Tile line scaled to its highest point times 1.15, never under a floor. `settings/activity/page.tsx:271`
- W2-PAL-150 Toast stack on a narrow window: 8 px top, 12 px sides; else 8 px top, 20 px right. `notices/Notice.tsx:108`
- W2-PAL-151 Motion: switcher has no enter or exit motion; its backdrop has no transition. `components/switcher/WorkspaceSwitcher.tsx:81`
- W2-PAL-152 Motion: front toast exits up 100%, opacity 0, 400 ms ease. `sonner:dist/styles.css:329`
- W2-PAL-153 Motion: collapsed back toast scales 1 minus 0.05 per toast in front, lifted a gap each. `sonner:dist/styles.css:298`
- W2-PAL-154 Motion: hover expands the stack, each toast to its offset, 400 ms ease. `sonner:dist/styles.css:325`
- W2-PAL-155 Motion: toast children opacity 400 ms ease. `sonner:dist/styles.css:304`
- W2-PAL-156 Motion: swipe out keyframes 200 ms ease-out. `sonner:dist/styles.css:361`

## Panes and pieces no ticket owns

### Elements

- W2-UNO-041 Computer: "2 min" and "now" under each plot. `components/machine/MachineSurface.tsx:115`
- W2-UNO-042 Agents: first card names the computer; fork: "<workspace> (<cloud>)". `components/agents/AgentsSurface.tsx:66`
- W2-UNO-043 Side question tab: dashed message glyph, no letter, not in launcher or add menu. `panes.ts:145`
- W2-UNO-044 Browser-tab menu: at pointer, kept in window, destructive red hover, refusal tooltip right. `actions/ContextMenuHost.tsx:33`
- W2-UNO-045 Computer plot aria-label "<label> over the last two minutes". `components/machine/MachineSurface.tsx:111`
- W2-UNO-046 Side question seconds tick every second: "12s", "1m 5s", "1h 2m". `components/chat/timeline/working.tsx:98`
- W2-UNO-047 Tray title adds the remote host's name before the count. `desktop:tray.ts:154`
- W2-UNO-048 Tray "On <host>" row for a remote host. `desktop:tray.ts:143`
- W2-UNO-049 Tray head line "N working, M waiting on you" or "No threads working". `desktop:tray.ts:144`
- W2-UNO-050 Tray, remote host lost: "<host> is not answering", no Start. `desktop:tray.ts:132`
- W2-UNO-051 Tray tooltip "wsp <title>", or "wsp" with no title. `desktop:main.ts:737`
- W2-UNO-052 Tray on mac only; elsewhere closing the last window quits. `desktop:main.ts:733`
- W2-UNO-053 Quit detail: wsp keeps running after the app quits; counts threads working here, says what Quit and stop wsp stops. `desktop:quit.ts:30`

### States

- W2-UNO-054 Computer toast "Live readings for <name> are not coming from the host: <why>", once per socket. No shot: needs the host to refuse `sys.subscribe`. `machine/hostLive.ts:61`
- W2-UNO-055 Computer, local thread with its daemon down: figures stay live, no away word. No shot: no op stops a daemon. `components/machine/MachineSurface.tsx:36`
- W2-UNO-056 Computer reopened: readings kept since it closed draw at once. No shot: looks like the live state. `machine/live.ts:80`
- W2-UNO-057 Agents paused: last report, nap word, acts held. No shot: pause needs a cloud. `components/agents/AgentsPanel.tsx:49`
- W2-UNO-058 Side question long answer scrolls in the pane. No shot: the stub never answers. `components/chat/AsideSurface.tsx:33`
- W2-UNO-059 Side question after reload: tab gone. No shot: draws as before the ask. `panes.ts:154`

### Actions and keys

- W2-UNO-060 Computer: footer button runs its action (Delete, Forget, export open dialogs); tooltip refusal, else hint. `components/machine/MachineSurface.tsx:248`
- W2-UNO-061 Agents: computer name opens its Settings page; fork: Edit image; here a sign-in types its login in a panel terminal. `components/agents/AgentsSurface.tsx:72`
- W2-UNO-062 Context menu: arrows wrap, Home, End; Enter or Space runs; Escape, Tab, outside press, scroll, resize, blur close. `actions/ContextMenuHost.tsx:58`
- W2-UNO-063 Side question: an answer after close or after a newer ask lands nowhere. `components/chat/asideStore.ts:63`
- W2-UNO-064 Agents: an opened page puts focus on its back button. `components/agents/AgentsPanel.tsx:63`
- W2-UNO-065 Agents: acts end in a toast, done line or failure. `components/agents/AgentsPanel.tsx:24`
- W2-UNO-066 Daemon down refused start: host's sentence, then its fix or "Start again.". `components/DaemonDown.tsx:19`
- W2-UNO-067 Browser-tab menu: focus starts on the first enabled row. `actions/ContextMenuHost.tsx:27`
- W2-UNO-068 Browser-tab menu: on close, focus goes back to what held it. `actions/contextMenu.ts:48`
- W2-UNO-069 Notification: a sound-only line beeps and shows nothing. `desktop:needs-you.ts:66`
- W2-UNO-070 With no window open, the tray's feed says lines as system notifications. `desktop:main.ts:783`
- W2-UNO-071 Lines said while closed: prompt, a finished or failed thread unless an agent started it, plan alert; each per the person's choice. `desktop:tray.ts:164`

### Design

- W2-UNO-072 Motion: footer buttons, color, background, border, 150 ms, default curve. `components/ui/button.tsx:22`
- W2-UNO-073 Motion: Computer scrollbar, opacity, 100 ms, 300 ms hide delay, default curve. `components/ui/scroll-area.tsx:81`
- W2-UNO-074 Motion: Agents tabs, colors, 150 ms, default curve. `components/ui/segmented-control.tsx:49`
- W2-UNO-075 Context menus, tray, Quit, Move, older-host boxes, notifications: native in Electron, none yet.
- W2-UNO-076 Motion: Side question crab, one still frame under reduced motion. `components/status/Crab.tsx:54`
- W2-UNO-077 Motion: browser-tab menu refusal tooltip, opacity and 2px slide, 150 ms, default curve. `components/ui/tooltip.tsx:46`
