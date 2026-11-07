# Setup animation sources

The GIFs preserve the complete app window and setup dialog from the original
recordings. The app remains visible behind the dialog. The full window is fit
inside a 1600×1104 frame over each theme's wallpaper. Before resizing, the
recording is masked with the saved native RGBA window alpha extracted from a
macOS window-ID screenshot: `../window-masks/native-2560x1600.png`. The mask
preserves native antialiasing and removes baked desktop pixels outside the
rounded app window. No dialog-only crop, recoloring, or shadow is applied.

- Moss uses `/Users/zingzy/wsp-work/brand-kit/site-shots/out/try-moss-own-glass/setup.mov`
  (2560×1600, 60 fps, 111.317 seconds; GLASS=1, renderer theme=moss,
  transparency=true), backed by the owner wallpaper
  `/Users/zingzy/Library/Application Support/so.cap.desktop/bg-1791190531541-wallhaven-zpoy6g.png`.
- Tungsten uses the refreshed full-app `setup.mov` capture on the canonical
  `/Users/zingzy/wsp-work/brand-kit/site-shots/wallpaper/maranjab-55-3024x1964.png`
  wallpaper: `/Users/zingzy/wsp-work/brand-kit/site-shots/out/try-tungsten-own-glass/setup.mov`
  (2560×1600, 60 fps, 109.267 seconds; GLASS=1, renderer theme=tungsten,
  transparency=true, amber accent).

Both clips cover computer selection, agent choices, summary, setup progress,
and the final ready state. Selected segments play at 3× and the final ready
frame holds for 1.5 seconds. Moss cuts `6:8,24:8,80:8,86:20,105.5:2.5`;
Tungsten cuts `6:8,24:8,80:8,86:18,104:4`. `proof/` contains inspected
frames from each final GIF.

`build-setup-gif.sh` takes a source MOV, output GIF, wallpaper, comma-separated
`start:duration` cuts, playback speed, and final-frame hold. It gets the
dimension-matched native alpha mask from `../window-mask.mjs` before scaling.
