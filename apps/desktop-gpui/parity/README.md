# Parity baselines

`baseline.mjs` drives today's Electron app over the Chrome DevTools Protocol to each state a wave lists and writes one
PNG per state and theme, plus `index.json`.

```sh
node baseline.mjs --wave 1 --fixture <fx.json> --app <Electron app executable> --out <dir> [--only 'W1-SET-*'] [--theme paper|graphite] [--scale <n>]
```

No `--theme` captures both. `--scale 2` launches Electron with `--force-device-scale-factor=2`, a real device scale,
so a Boat run shoots at 2 as a Retina Mac does; without it the capture is at the display's own scale. Nothing is
emulated: Blink rounds font metrics in CSS px under emulation, so text would land a device pixel away from where a
Retina Mac draws it. Narrow states size the real window too (the window has no minimum; the spec uses 980, 760, 600
and 360).

`--only` takes globs (`*`, `?`), comma separated. Each image is `<id>-<theme>.png` with the scale in its pHYs chunk
(scale 2 reads 144 dpi). `index.json` keeps one entry per state and theme: `state, theme, file, bytes, width, height,
scale, clip, reached`, and `reason` when not reached; `clip` is the kept region in CSS px, so the GPUI side crops the
same. A run with `--only` replaces its own entries and keeps the rest.

The fixture is one `apps/desktop-gpui/bench/fixture.sh` made, from a checkout whose `packages/host` is the app's
release; the driver refuses any other. For the run it sets the theme and Transparency off through the fixture host's
`preferences.set`, points the fixture's `bin/claude` at `claude-stub.mjs`, and ends the agents the host keeps up from
the bench's turns. When it ends it stops the turns it left running, puts the link and every preference back.

Every state is shot with Transparency off. A page capture holds no native material, and what shows through glass
depends on what sits behind the window, so solid is the baseline both apps can match; glass is judged by eye on the Mac.

## A state

`wave-<n>/states.mjs` exports the states, and may export a `setup`:

```js
export const setup = async host => {
  await host.once("second");
};
export default [
  { id: "W1-SET-901", clip: "[data-settings-page]", go: app => app.route.settings("general") },
  {
    id: "W1-THR-925",
    clip: () => "[data-chat-view]",
    data: host => host.thread("Trace the scroll writes.", { reply: ["thinking", { kind: "tools", names: ["Task"] }, "markdown"] }),
    go: async (app, id) => {
      await app.route.thread(id);
      await app.click("Worked for");
    },
    cleanup: (host, id) => host.removeThread(id),
  },
];
```

- `setup(host)` runs once before the first state, for data every state may see. Keep it empty unless a state needs it.
- States run in file order, each in every theme before the next. For each shot: reset, `data(host)`, `go(app, data)`,
  capture, then `cleanup(host, data)`, which also runs after a failure. Later states see what earlier ones made unless
  cleanup takes it away; a thread that changes nothing later states show may stay.
- `width`, `height`: the window for this state (1280x800 otherwise).
- `clip`: the region kept. A selector, bare or with `css:` (or a list of them): their box padded 16 px. A
  `{ x, y, width, height }` in CSS px. Or `clip(app, data)` answering either. Clamped to the window. The driver always
  captures the whole window and cuts the clip out in node (`png.mjs`): Page.captureScreenshot's own clip moves the
  page's pointer to the window's centre before it paints, which drops a hover, and runs through a viewport override.
  Whole window when absent: keep that for layout and narrow states, since a full 2x window is about 1.4 MB with the
  grain.
- `skip`: a sentence; the state is recorded as not reached with it, and nothing runs.
- `bridge: false`: the state drives the host's page in a window of its own with no preload and storage of its own, as
  a browser tab opens it with no token: the pair screen. The window closes after the shot.
- `home: "empty"`: the state runs last, in an app of its own on a throwaway host the driver starts on a fresh empty
  home (the fixture host's own `bin.js`, a marked temp folder, the clean environment) and stops by its pid after the
  shot. An empty host opens the onboarding window. `home: "held"` does the same with the host stopped (SIGSTOP), so
  the app shows its starting window. `agents: false` leaves the parity stub claude off that home's PATH, so onboarding
  finds no agent; its PATH is the home's own bin and the system folders (`/usr/bin`, `/bin`), so agents installed
  elsewhere on the computer are not found. `window` names the window to drive first: `onboarding` (default),
  `starting` (default when held), or `app`.
- A state that throws or runs past 120 s is recorded as not reached with its sentence, and the run goes on.

Before each shot on the fixture the driver lets go of held keys and drags, sets the window to 1280x800 (a narrow window
shuts the sidebar), reloads to the home route if the page moved off it or the localStorage keys the app reads back as
layout, picks, drafts or pending creates moved (putting back the first visit's storage), presses Escape twice, closes
standing messages, leaves Settings, opens New thread, clears its draft, and puts back any preference the last state
changed, including one the fixture's record never had.

### `host`

- `thread(prompt, { project, reply, title, access, file })`: a thread whose first message is `prompt`, made once by
  `wsp run`; answers its id. A held reply comes back once the turn is running or asking. A `permission` reply makes the
  thread with access `ask`, since a bypass thread's adapter allows the prompt unseen.
- `send(thread, prompt, { reply })`: one more turn, made once per thread and prompt.
- `working(thread, { reply, prompt })`: a turn left running (the reply's parts, then a held Bash call); nothing new
  where one already runs. `stop(thread)` ends it, as the app's Stop does.
- `removeThread(thread)`, `project(name)`, `removeProject(name)`, `threads()` (the `wsp threads --json` rows).
- `once(name, make)`: made once per fixture (kept in its `parity-made.json`). Shared names with their own maker:
  `manyThreads` (14 threads, ids), `longTitle` ({ project: a 60-character name, thread, title: 200 characters }),
  `second` (a project named second).
- `pref(name, value)`, `op(name, params)` (any host operation on its socket), `wsp(...args)` (the fixture host's own
  command line, clean environment), `script(prompt, reply)` (what the stub plays for a message holding `prompt`, for a
  message typed in the window), `fixture`, `home`.

A thread is named by the fixture's name for it (`short-1`), the prompt a state made it with, or its id.

### `reply`, played by `claude-stub.mjs`

One part or a list of parts, each a kind or `{ kind, ...options }`, played in order as one turn:

| kind | what the stub sends | options |
|---|---|---|
| `markdown` | headings, lists, a table, a link, inline code, a quote | `text` replaces it |
| `text` | `text` as it is | `text` |
| `code` | a fenced block | `lines` (80) |
| `table` | a wide table | `columns` (8) |
| `long` | about 2,000 words and one unbroken 280-character word | |
| `tools` | calls with results: Bash, Read, Edit with a diff, Grep, Write, TodoWrite (three todos), Task (task_started, the subagent's lines with its parent_tool_use_id, task_notification) | `n`, `names` |
| `thinking` | a thinking block | `text` |
| `plan` | ExitPlanMode with a 30-line plan, then its result | `text` |
| `error` | a failed Bash call (exit 1), then a result with is_error | `text` |
| `rate-limit` | a rate_limit_event with five_hour and seven_day use | `fiveHour` (42), `sevenDay` (63), `resetsAt` |
| `retry` | a system api_retry line (attempt 2 of 10, 529) | `attempt`, `max` |
| `files` | writes and reports src/a.ts and src/b/c.ts in the thread's folder | `paths` |
| `question` | AskUserQuestion through can_use_tool, waiting for the answer | |
| `permission` | a Bash call (`rm -rf build`) through can_use_tool, waiting | `command` |
| `hold` | a Bash call with no result, then nothing until Stop | `call: false`, `command` |
| `unstarted` | first of a reply only: no init line, so the turn has not started on the host, until Stop or for `ms` | `ms` |

A message nobody scripted plays what the bench's stub plays. `files` writes into the project's folder, which every
thread on that project shares: give it a project of its own or clean the files up.

### `app`

- Routes: `route.home()`, `route.newThread()`, `route.thread(thread)` (at a narrow width it opens the sidebar's sheet
  first), `route.settings(page)` (a group such as
  `general`, or a page in one: `project:<id>`, `agent:claude`), `route.usage()`.
- Pointer: `click(target, { count, button })`, `hover(target, { wait })` (rests 800 ms), `scroll(target, dy)`,
  `drag(from, to, { hold })` (a draggable tile is played as Chromium's drag events, anything else as plain moves; with
  hold the pointer stays down for the capture), `dropFiles(target, paths)`, `park()` (pointer off the window).
- Menus: the desktop app draws right-click menus natively (`window.wsp.contextMenu`), which no page capture holds.
  `click(target, { button: "right" })` throws a sentence naming the menu's rows, so the state is recorded as not reached.
  `menu(target, ...rows)` right-clicks and picks rows by their words, one per menu opened in turn, to act through one.
- Keys: `key(chord)`, `keyDown(chord)`, `keyUp(chord)`, in the keybinding table's spelling (`mod+shift+k`, `escape`,
  `ctrl`); `mod` is Cmd on a Mac and Ctrl elsewhere. Modifiers go down as keys of their own, so a held `ctrl` reaches
  the page. `type(text)` inserts text.
- `window(kind)`: drive another window of the app from here on: `app`, `onboarding`, `starting` or `pair` (onboarding's
  Open wsp hands over to `app`, where the first run shows).
- `waitFor(pageFn, ms, ...args)`, `page(pageFn, ...args)`, `size(w, h)`, `width(px)`, `height(px)`, `sleep(ms)`,
  `fixture`, `mod`.

A target starting with `[`, `.`, `#` or `css:` is a CSS selector; anything else is the words on the element (its label,
title or text), and the control around them is used. A target is waited for up to 5 s.

## Whole-window pairs: the surface check and the sweep

A fuzz score cannot see a surface one shade off: at 3% fuzz a header band a level away from the area under it scores
as identical. `surface.mjs` samples the flat ground of each region of a whole-window pair (sidebar, its header, thread
pane, its header, right panel, its header, composer) as the rounded mean of a 9 px patch, which the web's grain does not
move, and fails on any difference. Text keeps its fuzz; a surface gets none.

```sh
node surface.mjs <electron.png> <gpui.png> [--regions '{"composer":{"x":498,"y":420},"panel":null}'] [--scale 2]
```

It exits 1 naming each region and both colours. It reads 8-bit RGB or RGBA only, so a GPUI shot is saved as
`PNG24:`. `wave-2/gpui-shots.mjs` runs it on every row without a clip and fails its run on a difference.

`sweep.mjs` reads today's right panel panes (`panes.ts`), address and link forms (`app-address.ts`) and centre screens
(`App.tsx` WorkspaceCenter) and holds a row for each, with the empty and first-run states, the GPUI launch that reaches
it, or why there is none. A kind added to today's source with no row fails its test and its run. Each row is shot whole
window at 1280x800, scale 2, in both themes: today's app by `baseline.mjs --wave sweep` (`wave-sweep/states.mjs`),
desktop-gpui under its own Xvfb. A row fails on a launch the app refuses (exit 64), a centre or panel that draws only a
line or two where today's draws a screen (Nothing open here, Pick a thread), a kind with no launch, or a surface.

```sh
bench/fixture.sh /tmp/fx.json
node parity/sweep.mjs --fixture /tmp/fx.json --app <desktop-gpui> --electron-app <Electron wsp> --out <dir> [--only 'pane:*'] [--theme paper] [--electron-from <earlier dir>/electron] [--starting <out.json>] [--stood <out.json>]
bench/stop.sh /tmp/fx.json
```

Each row writes `<state>-<theme>-electron.png`, `-gpui.png`, `-diff.png` and `-pair.png`; `summary.json` holds the
shares at 0%, 3% and 10% fuzz, the surface rows and each failure. `--electron-from` scores a new GPUI build against
an earlier run's Electron shots. The fixture's real host shape (`fixture.realHost`) is what the sweep drives on: this
computer named "zingzy's MacBook Pro" apart from its hostname, as a Mac is (`bench/real-host.mjs` gives the fixture's
host the label a Mac's `scutil` would), a project `quiet` with no thread, a thread every pane opens on. The `pair:` rows are whole-window pairs beyond today's kinds; `pair:starting` and
`pair:started` run on hosts `starting-host.mjs` seeds (plain, and with `--stood`), whose `.wsp-starting-fixture` mark
`baseline.mjs` also takes; `pair:long-message` runs on a thread `baseline.mjs` makes, whose folded first message
stands whole above the composer.

## What a pixel diff must mask

These move from run to run on the same build: the New thread background field (a canvas redrawn every 120 ms), the
composer caret (it blinks), relative ages on tiles and rows ("4m"), and the computer's name (the machine's hostname,
which differs between Boat and the Mac). The Agents page lists every agent the fixture host finds on its PATH, which
holds `/usr/local/bin` and `/opt/homebrew/bin`, so two machines draw different rows there; compare Electron and GPUI on
one machine.

Start a run within two hours of fixture.sh: the fixture's threads settle after the default two hours, and a settled
thread is not where `route.thread` looks.

## On a Mac

Beside the owner's own wsp, on a fresh home (the fixture's temp folder). Keep the pointer off the test window while it
runs.

```sh
git -C <your wsp clone> worktree add /tmp/wsp-main origin/main
cd /tmp/wsp-main
pnpm install --frozen-lockfile
(cd daemon && cargo build --locked --release -p wsp-daemon-bin --features mcp) && node packages/wspx/scripts/daemon-binary.mjs
pnpm --filter "@wsp/desktop^..." --workspace-concurrency=3 build
pnpm --filter @wsp/desktop run build:app
CSC_IDENTITY_AUTO_DISCOVERY=false pnpm --filter @wsp/desktop exec electron-builder --config electron-builder.yml --mac --dir -c.mac.identity=null --publish never
mkdir -p apps/desktop-gpui
cp -R <gpui checkout>/apps/desktop-gpui/bench <gpui checkout>/apps/desktop-gpui/parity apps/desktop-gpui/
apps/desktop-gpui/bench/fixture.sh /tmp/parity-fx.json
node apps/desktop-gpui/parity/baseline.mjs --wave 1 --fixture /tmp/parity-fx.json --app apps/desktop/dist/mac-arm64/wsp.app/Contents/MacOS/wsp --out ~/parity-w1
apps/desktop-gpui/bench/stop.sh /tmp/parity-fx.json
```

The daemon is built and staged before the packages, or every terminal state fails. On an Intel Mac the app is
`apps/desktop/dist/mac/wsp.app/Contents/MacOS/wsp`.

## On Boat

The checkout that runs fixture.sh needs the daemon too, built for musl:

```sh
apt-get install -y musl-tools gperf
(cd daemon && ./scripts/libseccomp-archive.sh musl && LIBSECCOMP_LIB_PATH=$PWD/target/libseccomp/musl cargo build --locked --release --target x86_64-unknown-linux-musl -p wsp-daemon-bin) && node packages/wspx/scripts/daemon-binary.mjs --triple x86_64-unknown-linux-musl
```

```sh
Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp & x=$!; sleep 1; DISPLAY=:99 openbox & o=$!; sleep 1
DISPLAY=:99 node baseline.mjs --wave 1 --fixture <fx.json> --app <main>/apps/desktop/dist/linux-unpacked/wsp --out <dir> --scale 2
kill $o $x
```

Without a staged daemon the fixture's host cannot run terminals or live readings, `wsp delete` refuses (the driver
deletes through the `sessions.delete` operation either way), and the app raises a message at start, which the reset
closes.
