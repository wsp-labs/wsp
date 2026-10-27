---
name: gpui-agent
description: Sees and drives wsp's GPUI app (apps/desktop-gpui) from the command line, the way a browser driver drives a page. Use it whenever you need to see what the GPUI app shows or press it - reading a screen as text with the source line of each element, clicking, typing, scrolling, waiting for text, taking screenshots or recording video - for a UI proof, a screen check, or a screenshot taken off the owner's screen, under Xvfb on a Linux box or on a locked Mac.
---

# gpui-agent

This file only says when to reach for the tool. The usage guide comes from the command itself, so it always matches
the installed version:

```sh
gpui-agent skills get core
```

Read it before the first command you run.

## Install

`scripts/box-warm.sh` installs it on a lane machine. Anywhere else:

```sh
cargo install --locked --git https://github.com/wsp-labs/gpui-agent --rev c9b8b6328877755685e1233b3770f78a68f2a5d1 gpui-agent-cli
```

## The app it drives

The app answers only in a build with its `agent` feature, which no release turns on:
`cargo build -p desktop-gpui --features agent`. Run it against a throwaway host on its own state, never the owner's.
Its probes `thread` and `composer` answer with the thread on screen and the composer's text.
