---
name: wsp-gpui
description: The guide for porting wsp's desktop app to GPUI (map wsp-map#2364, branch gpui/main). Load it before building, reviewing or testing any GPUI screen of wsp, together with gpui-kit, gpui-kit-design-guides and wsp-design-language. It holds the lane's locked decisions, how wsp's Settings pieces map onto gpui-kit components and theme tokens, the traps other GPUI ports hit, where working code lives, the parity audit and screenshot loops, and how to build and test on a Boat 8x16 machine.
---

# wsp on GPUI

Every claim here cites a link or a file. When a cited file and this page
disagree, the file wins and this page is the bug.

Load these first:

- `gpui-kit` and `gpui-kit-design-guides`, vendored from Longbridge at the
  commit in each folder's `UPSTREAM` file:
  [gpui-kit](../gpui-kit/SKILL.md), [design guides](../gpui-kit-design-guides/SKILL.md).
  Refresh both with [skill-refresh.sh](../../../scripts/skill-refresh.sh)
  (`scripts/skill-refresh.sh .claude/skills/gpui-kit`).
- `wsp-design-language`, the owner's own skill on wsp's UI. It is a user
  skill, not in this repo; it lists the source files it reads, among them
  [layout.ts](../../../apps/web/src/settings/layout.ts) and
  [rows.tsx](../../../apps/web/src/settings/rows.tsx).
- `wsp-design`, the locked look and its token table:
  [wsp-design](../wsp-design/SKILL.md), with the mockup in
  [references/mockup](../wsp-design/references/mockup/index.html).
- `wsp-review` for the repo's laws before any commit:
  [wsp-review](../wsp-review/SKILL.md).

## The locked decisions

From [map #2364](https://github.com/wsp-labs/wsp-map/issues/2364), locked by
the owner on 2026-10-17. In short:

1. A slow lane: one or two builder threads, lowest priority, never blocking a
   release.
2. Every build and test thread runs on Boat at `--size 8x16`.
3. Work branches off `gpui/main` and merges back into it. Nothing reaches
   main before the cutover ticket. main is merged into `gpui/main` weekly.
4. A separate app, `apps/desktop-gpui`, on the same host websocket protocol
   as Electron ([packages/protocol/src/wire](../../../packages/protocol/src/wire/runtime.ts)).
   Electron keeps shipping until parity.
5. Port from the locked designs and the protocol, not from the React code,
   which changes almost daily.
6. Order: Settings and Usage first, then the sidebar, then the thread view and
   composer, the browser pane last.
7. Week one is a gate: the fork, memory, and a long transcript's scroll rate.
   If GPUI does not beat Electron by a wide margin there, the lane stops.
8. Rollout like Cap: a switch in Settings, Experimental, and a cutover only at
   full parity.
9. A coordinator thread runs the map; builders report to it.

Zeron (MIT) and zui (Apache-2.0), Zeron's GPUI fork, are references for how a
thing is done, never code to paste (map #2364, notes), and neither is our base.

## The fork

**gpui-kit on its own GPUI, no zui.** The owner reversed the
[wsp-map#2366](https://github.com/wsp-labs/wsp-map/issues/2366) ruling (gpui-kit
on zui) on 2026-10-17 at 21:18 IST. This is the one section written to change
when a pin moves. Pinned in [app/Cargo.toml](../../../apps/desktop-gpui/app/Cargo.toml):

- gpui-kit `14f59330a4aa0cb5becf140e067d64a616328612` (0.7.3, 2026-10-17), a git
  dependency with `tree-sitter-languages`, and `test-support` under
  `[dev-dependencies]`. The app lists only `gpui-kit`. `use gpui_kit::*` is
  GPUI, `gpui_kit::component` is gpui-component, `gpui_kit::application()`
  opens the platform and `gpui_kit::init(cx)` initializes the layers
  ([kit README](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/kit/README.md)).
- GPUI comes through gpui-kit's own pin, the crates.io `gpui-pre` family at
  `=0.3.9` ([Cargo.toml lines 62-80](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/Cargo.toml#L62-L80)), a snapshot of
  Zed `93d7456` ([gpui-pre 0.3.9](https://crates.io/crates/gpui-pre/0.3.9)).
  No `[patch."https://github.com/zed-industries/zed"]`, no zeronsh source.
- gpui-kit's pin turns on `runtime_shaders` for `gpui-pre-platform`
  ([line 70](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/Cargo.toml#L70)), so our manifests do not name it. It
  reaches `gpui-pre-apple` on macOS (`cargo tree -e features -i gpui-pre-macos
  --target aarch64-apple-darwin`), whose build script then stitches the shaders
  for launch instead of running `xcrun metal` (its `build.rs`, lines 21-24), so
  a Mac build needs no Xcode Metal toolchain.
- [Cargo.lock](../../../apps/desktop-gpui/Cargo.lock) committed; builds run `--locked`.
- Rust 1.99.1 ([rust-toolchain.toml](../../../apps/desktop-gpui/rust-toolchain.toml)).

Why (the reversal):

- No competitor in the chain. Between wsp and Zed's GPUI sits only Longbridge.
- Current versions. gpui-kit ships a weekly release on an exact `gpui-pre`
  snapshot ([lines 62-68](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/Cargo.toml#L62-L68)).
- Headless pixels on Boat today. gpui-kit's pixel tests
  ([rendering.rs](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/kit/tests/rendering.rs)) render through
  `HeadlessAppContext::capture_screenshot` on WGPU with Mesa lavapipe, and
  passed on a Boat machine with no display (the wsp-map#2365 review). The
  app's own pixel test is `apps/desktop-gpui/app/tests/pixels.rs`.

What we give up for now: backdrop blur and rounded clipping of children.
Panels are solid. Our own blur and rounded clip on this base is its own ticket
when a screen needs them. Zed's closed effect PRs are the reference
([#63773](https://github.com/zed-industries/zed/pull/63773),
[#63777](https://github.com/zed-industries/zed/pull/63777),
[#63780](https://github.com/zed-industries/zed/pull/63780)), and the work
could go back upstream.

The vendored gpui-kit skills match this pin. Their `UPSTREAM` commit d4c5681
is 14f5933's parent, and `git log --oneline -- skills/` at 14f5933 shows no
change to gpui-kit's `skills/` since 29eaf42 (checked 2026-10-17). Their
`gpui_kit::*` paths and `#[gpui_kit::test]` apply as written.

## wsp's design on gpui-kit

wsp's sizes and words come from the Settings pieces in
[layout.ts](../../../apps/web/src/settings/layout.ts); its colors from the
token table in [wsp-design](../wsp-design/SKILL.md) and the theme files in
[apps/web/src/themes](../../../apps/web/src/themes/graphite.css). gpui-kit
supplies behavior: focus, keyboard, overlays, inputs. `gpui_kit::component` is
the styled crate and `gpui_kit::base` the unstyled one
([base README](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/base/README.md)); where wsp's look differs from a
component's, the look is wsp's (locked decision 5).

Sizes carry over as the pixels `wsp-design-language` gives them. gpui-kit's
default `font_size` is 16 px
([theme/mod.rs](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/component/src/theme/mod.rs)).

| wsp piece ([layout.ts](../../../apps/web/src/settings/layout.ts), [rows.tsx](../../../apps/web/src/settings/rows.tsx)) | Size | gpui-kit part |
|---|---|---|
| Card: `SECTION_HEAD`, then `CARD_SURFACE` | head 13.5 px medium at 70% ink; card radius 11 px, border at 60%, card fill at 40% | a plain `v_flex`; gpui-kit's `setting::Settings` groups carry their own spacing, so check them against the sizes before use |
| Row: `CARD_INSET`, `ROW_FLOOR`, `SETTING_TITLE` or `LIST_TITLE`, `NOTE` | 20 px sides, 60 px floor, `py-3`; title 14 px (choice) or 15 px (named thing), note 13 px muted | `h_flex`; note in `muted_foreground` |
| Line: `LINE_FLOOR` | 48 px | `h_flex`, keycaps in `kbd::Kbd` |
| Lead: `GLYPH_FRAME`, `GLYPH` | 32 px frame, `rounded-md` (8 px), 16 px glyph at 80% ink | `Icon` with `IconName`; both apps use Lucide |
| Select in a row: `SELECT_WIDTH` | 176 px wide, 30 px high, radius 7 px, 13 px text | `select::Select` |
| Field in a row: `ROW_FIELD` | 30 px high, radius 7 px, 13 px text | `input::Input` |
| Picks | radio or checkbox at the left of a row | `radio::Radio`, `checkbox::Checkbox` |
| Buttons | outline xs for inline acts, one primary per view, a quiet text button | `Button` with `.outline()` and `.xsmall()`, `.primary()`, `.text()` ([button.rs](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/component/src/button/button.rs)) |
| Dialog | 560 px; title, "n of N", cards, a foot | `Dialog` through `window.open_dialog` |
| Copyable command | `CopyRow` ([sheetParts.tsx](../../../apps/web/src/settings/sheetParts.tsx)): a 40 px field, `rounded-md`, the value in mono, a copy button | an `h_flex` field; `clipboard::Clipboard` is only its copy button |
| Shortcut | lives in a tooltip, never a hint strip | `.tooltip()` with `Kbd` |
| Loading | the plain spinner | `spinner::Spinner` |
| An agent working | the crab ([Crab.tsx](../../../apps/web/src/components/status/Crab.tsx)) | none; port it as our own element and keep its credit ([THIRD_PARTY_NOTICES](../../../THIRD_PARTY_NOTICES), item 10) |

`rounded-md` is 8 px and `rounded-xl` 14 px, from `--radius: 0.625rem` in
[index.css](../../../apps/web/src/index.css). The Usage page sets
`data-locked`, which turns the card head to 14 px regular and the card radius
to `rounded-xl`
([SettingsPage.tsx](../../../apps/web/src/settings/SettingsPage.tsx)).

The rules that do not show in a table:

- **Colors.** gpui-kit's theme has no `card` token and names things its own
  way (`muted_foreground`, `danger`, `success`, `warning`, `primary`;
  [theme_color.rs](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/component/src/theme/theme_color.rs)).
  Write wsp's themes as gpui-kit theme JSON, loaded with
  `ThemeRegistry::load_themes_from_str` or `watch_dir`
  ([registry.rs](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/component/src/theme/registry.rs)),
  one file per wsp theme, values from the token table. No literal color in a
  view (wsp-review, Design laws; gpui-kit design guides, Non-negotiables).
- **Color law.** A colored use the wsp-design token table does not name is a
  finding. `success`, `warning` and `danger` carry only the uses that table
  lists ([wsp-review](../wsp-review/SKILL.md), Color law).
- **State.** Never a status dot or a chip for state; an icon in the slot each
  row keeps (`wsp-design-language`, State). gpui-kit's `Badge` and `Marker`
  are not state marks here.
- **Words.** Sentence case, no em dashes, name the computer (`wsp-design-language`,
  Words). gpui-kit's copy rule agrees on naming the object and the verb
  ([design guides](../gpui-kit-design-guides/SKILL.md), Non-negotiables).

## Traps others hit

**Text in flex items, rebuilt every frame.** Cap's 3D panel measured 19.1 fps:
GPUI rebuilds the whole view when it re-renders, the playhead notified 60 times
a second, and each `flex_1` card with a label made the layout shape its text on
every sizing probe, about 40 ms a frame for 16 cards. The fix was a computed
width and `flex_none()` on each card
([Cap README](https://github.com/CapSoftware/Cap/blob/main/apps/desktop-gpui/README.md),
"One real performance trap"). For wsp: a repeated row holding text gets a
fixed or computed width. Keep a view that ticks (elapsed time, the crab, a
streaming reply) in its own entity, and measure that the transcript does not
re-render with it. Use a virtual list for the transcript (`uniform_list` or `list`
upstream, `VirtualList` in gpui-kit; [gpui-kit](../gpui-kit/SKILL.md)).

**The first build is long.** Cap: about 11 minutes and 6.4 GB of `target/` for
the first build, about 22 s for an app-crate rebuild
([Cap README](https://github.com/CapSoftware/Cap/blob/main/apps/desktop-gpui/README.md),
Running it). Cap builds dependencies at `opt-level = 2` and keeps pixel loops in
a crate at `opt-level = 3`, because unoptimised its frame conversion cost
42.7 ms against 1.0 ms (same README). What this page measured on Boat is under
"Build and test on Boat".

**No hot reload.** Cap built a rebuild-and-relaunch loop that swaps the app
only when the build succeeds and reopens the same windows and page
(`dev.sh`, Cap README, The dev loop). gpui-ce has hot patching through Dioxus
Subsecond: debug builds only, and a patch can crash the app
([hot-patching.typ](https://github.com/gpui-ce/gpui-ce/blob/main/docs/pages/hot-patching.typ)).

**A separate cargo workspace.** Cap gave its GPUI app its own `[workspace]` so
the root lock file and CI stay untouched, and pinned `stable` because GPUI's
dependencies need Rust 1.89 or later (Cap README, Why it is a separate
workspace). wsp already has a cargo workspace in
[daemon/Cargo.toml](../../../daemon/Cargo.toml), and Cap's reason holds for it:
GPUI's tree would move its lock file and its CI.

**The webview is experimental.** On Windows gpui-kit's webview draws over GPUI
overlays, so a popover opens behind it; it does not run on Wayland; and it
switches the whole app to GPUI Fast
([webview README](https://github.com/longbridge/gpui-kit/tree/14f59330a4aa0cb5becf140e067d64a616328612/crates/webview)).
Webviews under GPUI overlays upstream are still an open PR
([zed#62379](https://github.com/zed-industries/zed/pull/62379)).

**No stable GPUI release.** crates.io's `gpui` stops at 0.2.2, published
2025-10-22 ([crates.io](https://crates.io/crates/gpui)); gpui-kit, and so wsp,
pins a published snapshot, `gpui-pre` `=0.3.9` (The fork);
Cap pins a commit of wingleeio/zed
([Cap Cargo.toml](https://github.com/CapSoftware/Cap/blob/main/apps/desktop-gpui/Cargo.toml)).
Upgrades change behavior without saying so: one app's input lag went from
6.5 ms to 35 ms when inactive windows were capped at 30 fps (map #2364,
Costs). A rev bump is its own change, with the week-one numbers taken before
and after.

**Upstream has no effects yet.** A Zed maintainer closed the effect layer stack
unmerged on 2026-09-04 because it broke the limit of three open PRs per
contributor, and asked for one or two small PRs instead (comments on
[#63771](https://github.com/zed-industries/zed/pull/63771); the stack is
[#63773](https://github.com/zed-industries/zed/pull/63773),
[#63777](https://github.com/zed-industries/zed/pull/63777) and
[#63780](https://github.com/zed-industries/zed/pull/63780)). Its author closed
[#63794](https://github.com/zed-industries/zed/pull/63794) the next day. The
one-off shaders issue moved off Zed's tracker to a maintainer's own, where it
is open ([#43273](https://github.com/zed-industries/zed/issues/43273),
[gpui-projects#10](https://github.com/mikayla-maki/gpui-projects/issues/10)).
Cap shipped without its header's backdrop blur because its pinned rev has no
hook for it (Cap README, "No header backdrop filter"). wsp has none for now
(The fork).

**Keys in a text field.** GPUI matches key bindings before the platform's input
method sees a key, so every chord in a field must be a bound action in the
field's key context, and a bare printable key can never be bound; a screen's
single-key shortcut has to check that no field has focus (Cap README, Text
input). gpui-kit's `Input` already handles the field
([gpui-kit](../gpui-kit/SKILL.md)).

**Accessibility is young.** AccessKit landed in GPUI on 2026-05-27
([zed#56065](https://github.com/zed-industries/zed/pull/56065)); screen readers
on Windows are still an open issue
([zed#41138](https://github.com/zed-industries/zed/issues/41138)).

## Where to read working code

1. gpui-kit's examples at our pin, `hello_world` among them:
   [examples](https://github.com/longbridge/gpui-kit/tree/14f59330a4aa0cb5becf140e067d64a616328612/examples). gpui-ce's teaching examples:
   [gpui-ce examples](https://github.com/gpui-ce/gpui-ce/tree/main/crates/gpui/examples).
2. Skills in real repos: Zed's
   [gpui-bench](https://github.com/zed-industries/zed/tree/main/.agents/skills/gpui-bench)
   (benchmarks that follow the production path) and
   [gpui-test](https://github.com/zed-industries/zed/tree/main/.agents/skills/gpui-test);
   herdr-gpui's
   [gpui-mockup](https://github.com/penso/herdr-gpui/blob/main/.claude/skills/gpui-mockup/SKILL.md);
   jayjay's
   [gpui-parity-audit](https://github.com/hewigovens/jayjay/blob/main/.agents/skills/gpui-parity-audit/SKILL.md);
   OpenLogi's
   [testing-openlogi-ui](https://github.com/AprilNEA/OpenLogi/blob/master/.agents/skills/testing-openlogi-ui/SKILL.md);
   zqlz's GPUI skills on entities, async, focus and layout
   ([zqlz/.claude/skills](https://github.com/samurmaykrr/zqlz/tree/main/.claude/skills)).
3. Zed's own docs and examples:
   [crates/gpui/docs](https://github.com/zed-industries/zed/tree/main/crates/gpui/docs)
   (contexts, key dispatch) and
   [crates/gpui/examples](https://github.com/zed-industries/zed/tree/main/crates/gpui/examples)
   (46 entries at 56934ec: 42 files and 4 folders).
4. Apps on GPUI, agent tools among them (Arbor, Hadron, Reviu, Runner):
   [awesome-gpui](https://github.com/zed-industries/awesome-gpui).
5. Whole ports: Cap's
   [desktop-gpui README](https://github.com/CapSoftware/Cap/blob/main/apps/desktop-gpui/README.md)
   and [PR #2442](https://github.com/CapSoftware/Cap/pull/2442);
   syncplay-gpui's [PR #65](https://github.com/everpcpc/syncplay-gpui/pull/65);
   Frame's [write-up](https://www.madebyhex.com/blog/frame-rewriting-a-tauri-app-in-gpui-ce).
6. Zed's blog on GPUI ownership:
   [gpui-ownership](https://zed.dev/blog/gpui-ownership).
7. Tools: [gpui-devtools](https://github.com/joris-gallot/gpui-devtools),
   [gpui-storybook](https://github.com/stayhydated/gpui-storybook),
   [gpui-kit-canvas](https://github.com/JetSquirrel/gpui-kit-canvas).

Stale, read only to know they are stale:
[gpui-book](https://github.com/MatinAniss/gpui-book),
[create-gpui-app](https://github.com/zed-industries/create-gpui-app)
(map #2364, Resources).

## The parity audit

Run it on a ported screen before its ticket reports done, and whenever the
coordinator asks where a screen stands. Adapted from jayjay's gpui-parity-audit
(linked above), which audits a GPUI shell against a SwiftUI one.

1. **Gather the three references.** The locked design for the screen
   ([wsp-design](../wsp-design/SKILL.md) and its mockup), the protocol messages
   the screen reads and sends
   ([packages/protocol/src/wire](../../../packages/protocol/src/wire/runtime.ts)),
   and screenshots of the Electron app on the same screen and state, from the
   owner's Mac. Not the React code (locked decision 5).
2. **List what a person can see or do,** one row each: a value shown, a
   control, a state (empty, loading, failed, long text), a keyboard path. A
   screen is not a row.
3. **Fill each row with evidence:** Electron's behavior, GPUI's behavior, and
   the proof. Proof is a test name or a screenshot pair. Reading the code is
   not proof, and calling a row absent needs a search of the GPUI code first.
4. **Give each row one verdict:** same; gap; or a difference on purpose, with
   the ruling linked. A row that works but breaks the design language (a status
   dot, a filled pill for a pick, a second primary button) is a gap.
5. **Check both themes and the widths `wsp-design-language` names** (Before
   you show anything), or the narrowest window the app allows where that is
   wider. Write which you checked; claim nothing beyond them.
6. **Report the table.** An audit is findings only. Close gaps only when the
   ticket asks for it, with a test per state the fix touches.

## The mockup and screenshot loop

Adapted from herdr-gpui's gpui-mockup (linked above), which writes design
variants as real GPUI code, shows them side by side and captures the window to
check its own work. Its capture works on macOS only (its SKILL.md, Limits), so
on Boat use Xvfb or the headless road below.

**Shoot on Boat.** gpui-kit's `hello_world` at d4c5681, on the same
`gpui-pre` 0.3.9 as our pin, drew its text and button here on 2026-10-17 with
these lines, as did wsp's window against `apps/desktop-gpui/bench/fixture.sh`.
Run them as one script in the foreground so both processes end inside it:

```sh
apt-get install -y xvfb libxkbcommon-x11-dev
Xvfb :99 -screen 0 1440x900x24 & xpid=$!
DISPLAY=:99 timeout 25 target/debug/<app> & pid=$!
sleep 15; DISPLAY=:99 import -window root shot.png
wait $pid; kill $xpid
```

Stop only the pids you started (wsp-review, Process laws). A window takes
focus only with a window manager on the display: start `openbox` on `:99`
first, or the bench exits 3 (`FOCUS_WAIT` in [shell.rs](../../../apps/desktop-gpui/app/src/shell.rs)).
Boat has no GPU (no `/dev/dri`; Mesa printed a DRI3 warning) and Linux fonts,
so a Boat shot checks layout, clipping, overflow, alignment and states; type,
vibrancy and color are judged on the owner's Mac every few days (map #2364).

**Open straight into the state.** Give each screen a way to start on itself in
a fixture state, so a shot needs no clicks. Cap opens a settings page at launch
with `CAP_GPUI_AUTO_SETTINGS=<page>`
(Cap README, Verifying changes).

**Measure, do not eyeball.** Cap's 2x scale bug was invisible until two headers
were compared pixel by pixel (Cap README, Verifying changes). In tests, tag
elements with `debug_selector`
([div.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/elements/div.rs),
recorded only in debug builds with the `test-support` feature) and read their
boxes with `debug_bounds`
([test_context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/test_context.rs));
compare row floors, insets and radii to [layout.ts](../../../apps/web/src/settings/layout.ts).

**Shoot headless.** `HeadlessAppContext::capture_screenshot` renders a window
to an image with no display
([headless_app_context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/headless_app_context.rs)).
On our pin it runs on WGPU on Linux
([test.md](../gpui-kit/references/gpui/test.md), Assertion boundaries), and
gpui-kit's pixel tests passed on Boat that way (The fork). Run them with
`cargo test -p gpui-kit --features test-support --test rendering --locked`
([TESTING.md lines 191-198](https://github.com/longbridge/gpui-kit/blob/14f59330a4aa0cb5becf140e067d64a616328612/crates/kit/TESTING.md#L191-L198)).

**Variants only where the design is silent.** Settings and Usage have locked
designs; port them, do not redesign them. Where a design is silent, write two
to four variants in one file in a `mktemp -d` folder outside the repo, shoot
them beside a Settings page in the same theme and width
(`wsp-design-language`, Before you show anything), and send the shots to the
coordinator for the owner's pick. The picked variant moves into the screen's
module with tests on its geometry; the scratch file never enters the repo.

## Build and test on Boat

Read on the 8x16 machine that wrote this page, 2026-10-17:

| What | Value | Command |
|---|---|---|
| CPUs, memory | 8, 15 GiB | `nproc`, `free -g` |
| Root disk | 147 GB, 115 GB free | `df -h /` |
| OS | Ubuntu 24.04.4 LTS | `cat /etc/os-release` |
| Rust | rustup with no default toolchain; `rustup default stable` gave rustc 1.99.1 and cargo 1.99.1 in 10 s | `rustc --version` |
| Cargo home | `/opt/wsp/cargo` | `echo $CARGO_HOME` |
| GPU | none, no `/dev/dri` | `ls /dev/dri` |

Present: clang 18, `libxkbcommon-dev`, `libwayland-dev`, `libfontconfig-dev`,
`libvulkan1`, `mesa-vulkan-drivers`, ImageMagick, `xdotool`. Missing:
`libxkbcommon-x11-dev`, without which the link fails with
`unable to find library -lxkbcommon-x11`, and `xvfb`. Install both as above.
The Xorg on `:0` refused our connection; use your own Xvfb display.

Measured: `cargo build -p hello_world` in a fresh gpui-kit checkout (d4c5681) took
4 min 5 s from cold, downloading 552 crates, and left 3.9 GB in `target/`.
That is gpui-kit's smallest example, not wsp's app; for a full app use Cap's
11 minutes (above) until we measure our own.

Measured on our pin, 8x16: cold `cargo build -p desktop-gpui`, downloads included, 124 s, 5.9 GB.

**The rebuild loop.** [dev.sh](../../../apps/desktop-gpui/dev.sh) rebuilds when
`app/`, `host-client/src` or `Cargo.toml` changes and swaps the running app
only when the build passes, like Cap's `dev.sh` (Cap README, The dev loop).
Measured on our pin: a rebuild after editing three files of `app/src` took 4.4 s.

Keep cargo's output on screen (`| tee`, never a bare redirect) so a long build
keeps printing. Run tests with `cargo test`, `#[gpui_kit::test]` at our pin (The fork).
Benchmarks follow Zed's gpui-bench rules: no `test-support` feature, a hard
timeout, never two measured runs at once (linked above).
