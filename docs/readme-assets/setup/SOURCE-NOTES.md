# Setup animation source

The existing website source is
`/Users/zingzy/wsp-work/release/0.3.10-installer/apps/www/src/assets/clips/setup.mp4`.
It is 1000×1094 at 30 fps and 63.33 seconds. The website section imports this
clip directly. Git commit `0f037437b7` (2026-10-10) records that the site's app
pictures, including `clips/setup.mp4`, were re-shot from the real app with the
Tungsten patch applied. This is therefore genuine Tungsten-origin footage, not
a Moss or Graphite capture.

The website clip is Tungsten and is not used for the two requested theme
variants. The output GIFs use separate real `setup` scene recordings with
`GLASS=1` and renderer readback confirming each theme:

- Moss: `/Users/zingzy/wsp-work/brand-kit/site-shots/out/try-moss-own-glass/setup.mov`
  (2560×1600, 60 fps, 111.317 seconds; theme=moss, transparency=true).
- Graphite: `/Users/zingzy/wsp-work/brand-kit/site-shots/out/try-graphite-own-glass/setup.mov`
  (2560×1600, 60 fps, 110.317 seconds; theme=graphite, transparency=true).

Both GIFs cut the computer picker (6–14s), agent choices (24–32s), summary
(80–88s), setup progress (86–106s), and final ready view (105.5–108s). The
segments play at 3×, then hold the ready frame for 1.5 seconds. Four keyframe
proof stills per theme are in `proof/`.

`build-setup-gif.sh` accepts a full-window `setup.mov`, output path, wallpaper,
comma-separated `start:duration` cuts, speed, and final-frame hold. It crops the
1120×1280 setup dialog from the centered full-window capture (crop origin is
derived from the source dimensions), preserves its aspect and source theme
pixels, then centers it over the same 1600×1104 owner wallpaper frame for both
themes. It does not add a round mask, shadows, or tint.
