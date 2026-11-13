# Native window outline

These two PNGs contain white RGB pixels with the unmodified alpha channel of the classic Electron window captured by macOS on 2026-11-09. No product pixels or conversation content are included.

The source captures use `screencapture -x -o -l <window-id>` on the isolated photo app, at 1440 × 900 and 1280 × 800 points on a 2× display. The CGWindow number belongs to the photo app process. Original source captures remain in the brand lab under `out/try-tungsten-own-glass/native-window-alpha-{2880x1800,2560x1600}.png`.

The alpha channels retain macOS's native corner curve and edge antialiasing. All interior pixels are opaque. Each matte has 804 fully transparent pixels around its corners. These are captured outlines, not SVG rounded rectangles or estimated radii.

`../window-mask.mjs` applies the matching matte to original screen-region captures before scaling. This removes the old desktop pixels baked outside the window while preserving the photographed glass inside it. The setup GIF helper uses the same alpha before resizing. Outer feature frames remain square.
