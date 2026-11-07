# README visuals

[README.md](../README.md) is the Graphite edition. [README.moss.md](../README.moss.md) is the Moss edition. They contain the same product story and use their own theme-matching captures, diagrams and setup GIF. The [local preview](readme-preview.html) renders both Markdown files.

## Original captures

Product shots use the classic Electron client from WSP v0.3.10, commit `b0b3d25ddc033ead7e185ee33171f11989200f2b`, and its matching Rust daemon in the isolated brand lab. The GPUI client is not used for these images. Standard windows are 1440 × 900 points, captured as 2880 × 1800 PNGs. Setup recordings use 1280 × 800 point windows and 2560 × 1600 MOVs.

Both editions use the owner's wallpaper and native transparency. New scenes check the renderer's `data-theme`, glass state and Mac desktop class before capture. The lab restores desktop settings afterwards. Captures with notification banners or interference were rejected. The capture guard checks Notification Center windows above the photographed app, including localized owner names, rather than stale accessibility records or widgets behind it.

Full-resolution PNGs and MOVs remain in the brand lab's `site-shots/out/try-{graphite,moss}-own-glass/` folders. Previously exported treatments are kept locally in `out/readme-export-archive-2026-11-07/`. Reframing never replaces the raw originals.

## Framing

`readme-assets/export-stills.mjs` exports 14 product stills per edition. Feature-table frames are 1600 × 1104. The hero is 1600 × 1048, with exactly 64 pixels of wallpaper on each side of the complete app window. There are no added shadows or rounded outer masks.

The sidebar image isolates a lead and four direct children. Activity and Usage show the complete app page. Tool servers, the PR and editor examples focus their relevant controls. The PR avatar is hidden only in the isolated photo fixture. Files and Auto Settle are no longer feature rows. Resume at reset uses the real native armed usage bar.

The Slate image shows three complete native panels side by side with a vertical stagger: the approved architecture Slate, release readiness and computer activity. No old decorative theme pickers or “illustrative data” labels remain. `readme-assets/features/crops.json` records source hashes, source regions and output placement.

## Slate sources and attribution

The hero preserves the approved live Slate: transparent silver Anatomy figure, Host and Desktop/CLI/MCP facts, then the native context chart. Its three context readings came from the approved thread, rather than a benchmark or a generated curve.

Native sources are included: [hero](readme-assets/slate/hero.slate), [release](readme-assets/slate/release.slate), and [activity](readme-assets/slate/activity.slate). The other panels use demonstration values and native controls.

The Anatomy figure uses Ryan's [wheresryan22/anatomy](https://github.com/wheresryan22/anatomy), under its [MIT license](readme-assets/ANATOMY-LICENSE). Agent marks come from WSP's existing catalog; their source URLs are in `readme-assets/agents/sources.json` and licenses are covered by the catalog's attribution and third-party notices.

## Setup GIFs

Each edition includes a 17-second, 1600 × 1104 GIF from its own verified transparent-app recording. The sequence shows computer selection, agent choices, setup progress, and `office-pc is ready`, with a short hold at the end. These are isolated setup fixtures, not changes to an actual connected computer.

[Source notes](readme-assets/setup/SOURCE-NOTES.md) record clip paths, cuts, speed and dimensions. `readme-assets/setup/build-setup-gif.sh` preserves the recorded UI pixels and places the dialog over the shared wallpaper. It does not recolor a Tungsten recording into another theme.

## Diagrams and providers

Architecture and lifecycle diagrams share icons, typography and square edges in both themes. Editable HTML accompanies the PNGs. `readme-assets/build-diagrams.mjs` regenerates them. Agent icons and concrete examples replace WSP screenshots in the later Desktop/CLI/MCP section.

The provider table is native README HTML. Eight linked PNG logos are displayed at 96 pixels, in equal-width cells. Their original sources are recorded in `readme-assets/providers/sources.json`. The grid uses the existing roadmap providers; no additional providers were invented to fill space.

Wayfinder tickets #2454–#2460 describe the additional adapters as internal-only, with #2461 tracking their benchmark. The README retains the owner's requested “Coming soon” presentation without dates and notes that cloud execution requires a cloud-enabled build. The public-packaging distinction is recorded in [#1807](https://github.com/wsp-labs/wsp-map/issues/1807). Review the proposed public roadmap before merging.

Conversations, usage values and resource histories are demonstration fixtures. The GPUI memory comparison linked in the README is a published app measurement, not a daemon-memory or cloud-startup claim.
