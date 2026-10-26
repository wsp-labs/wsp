---
title: It's not a memory leak. It's what your agents started.
date: 2026-10-25
summary: That app using 60 GB in the force quit window? Usually it's the MCP servers and dev servers your agents started. Mine froze my Mac six times.
draft: false
author: Aditya Singhi
author_url: https://aditya.singhi.me
author_avatar: /blog/authors/aditya.webp
image: /blog/mcp-freeze/card.png
---

This was back in June. I was working in cmux, running a bunch of Claude Code sessions, and **my laptop just hung.** Then this showed up:

<figure class="inset"><img src="/blog/mcp-freeze/force-quit.webp" alt="The force quit window: &quot;Your system has run out of application memory&quot;, cmux (paused) at 27.52 GB"><figcaption>cmux at 27.52 GB. Arc, Code - Insiders, Slack and WhatsApp got paused too.</figcaption></figure>

**cmux, 27.52 GB.** I thought cmux had a memory leak.

So I restarted the Mac, planning to get my agent to audit it and report it to the cmux team.

The Mac came back, reopened all my apps, cmux auto-launched with them, and within seconds, before I could do anything, **the same screen again.** Force quit did nothing. Pressing it literally did nothing. It was stuck in a loop, and **it happened 5 times** 😭

My EM walked over, laughed, and said **"aur karo use ye naye naye apps"** (go on, keep using these shiny new apps).

We finally brute-forced our way into quitting cmux and disabling it in the middle of a restart. I got scared and switched terminals: opened Ghostty and started my Claude sessions there instead.

To my horror, **the same screen. Now Ghostty was eating all the memory** 😭 My EM, this time not laughing: why do you keep using these new apps. Me, thinking: bro, Ghostty is written in Zig, it's not supposed to leak like this.

So I gave up, went back to VS Code, opened Claude Code in some random folder, and told it: bro, cmux and Ghostty both have memory leaks, help.

Even my agent got scared and audited fast. And the real culprit **wasn't cmux. It wasn't Ghostty either.**

**It was an MCP server** 🫠

## What was actually happening

I had a code-indexing MCP server, [codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) v0.8.1, installed for every Claude Code session, with auto-indexing turned on. **I had 5+ Claude sessions open in the same work repo, and every one of them had it.**

![cmux holds five Claude Code sessions, each session starts its own copy of the indexer, and all five index the same repo with 17 submodules](/blog/mcp-freeze/loop.svg "Five sessions, five indexers, one repo. All of it on cmux's line.")

Every Claude Code session starts **its own copy of every MCP server**. A stdio MCP server is literally a child process of the session that launched it, and nothing is shared between sessions. So five sessions meant five indexers, and all five started indexing the same repo at the same time.

Each one holds the whole index in memory until it's done, and the repo had **17 submodules**, so it never got done. Five copies of that was more than my Mac had.

That also explains the loop. cmux reopens your Claude Code sessions when it relaunches, so every restart brought the sessions back, every session started its indexer again, and with no finished index each one started from zero. **That's how I got six freezes in a row.**

I disabled the MCP, opened cmux again, and everything was fine. Nothing else had changed.

To be fair to the project, its maintainer has fixed most of this since. Indexing now runs in a supervised process, one background service is shared across sessions, and since v0.11.0 the memory budget is a hard limit that spills to disk or stops cleanly.

## Why the terminal took the blame

The force quit window didn't lie, exactly. It just counted differently than I assumed.

On macOS, **everything an app starts gets counted under that app.** cmux started my Claude sessions, and they started the indexers, so **the 27.52 GB next to cmux was really theirs.**

That's also why force quit didn't help. Killing an app doesn't kill what it started. The indexers kept running and kept the memory.

## You've probably seen this too

The day I posted about this on X, my timeline was full of it. Someone got the exact dialog, with **Conductor at 49.94 GB**. The first reply blamed Conductor. It wasn't Conductor:

https://x.com/amaan8429/status/2109973146447352078

A few days later, the same dialog in Portuguese: **"don't use Orca"**, with Orca at 67.26 GB:

https://x.com/BrOrlandi/status/2111260521496236099

Codex gets it too. To be fair, this one looks like Codex's own processes, so not every screenshot is a child process:

https://x.com/maurice_build/status/2111351003974893909

Theo said it best months ago:

https://x.com/theo/status/2084721449114472849

Mitchell Hashimoto, who makes Ghostty, [on HN](https://news.ycombinator.com/item?id=46461860): **"that window also reports all the RAM child processes are using."** And there's a [cmux issue](https://github.com/manaflow-ai/cmux/issues/19026) where the dialog showed **88 GB on cmux's row**, and the logs traced it to test workers that agent sessions had started.

Sometimes the app really does leak, though. Mitchell found a real one in Ghostty a week after that comment, and Cursor wrote about cutting its out-of-memory crashes by 80%. **But a lot of the "X ate 60 GB" screenshots are this.**

## What it taught me

- Your terminal takes the blame for whatever runs inside it, because everything your agents spawn gets billed to the app that started them.
- Every session runs its own copy of every MCP server, with nothing shared. That's a big part of why I lean on CLIs where I can.
- Know what an MCP does when it starts, and don't install random MCPs off the internet without checking.

## That day is why wsp has an Activity page

I build [wsp](https://usewsp.com), an app that runs your coding agents (Claude Code, Codex, OpenCode, Cursor) **on every computer you own**, from one window.

After that day I wanted one thing: **to see what's actually eating my memory.** Not "cmux: 27 GB", but which thread, which agent, which MCP server, and everything each one started.

![The Activity page: wsp itself, then a thread opened down to Claude Code, its tool servers and what they started](/blog/mcp-freeze/activity.mp4 "My own Mac, this morning. Every thread opens down to what it started.")

So that's what the Activity page shows. **Every thread, then its agent, then its MCP servers and tools, then whatever they spawned**, each with its memory, CPU and power. Other tabs list every process flat, group them by kind (agents, tool servers, browsers, shells), and show which ports are open and who opened them. Up top, how much of your Mac wsp and its threads are using right now.

![A thread opened all the way down: Claude Code, its MCP tool servers, then a shell, pnpm and a vite dev server at 2.1 GB and 5.4 W](/blog/mcp-freeze/activity.webp "Five levels down from the thread: the vite dev server an agent started, at 2.1 GB and 5.4 W.")

## How it works

Any activity monitor can read memory. What wsp has to figure out is **whose each process is**.

![Left, the force quit window: one line per app, with cmux at 27.52 GB. Right, wsp: one thread opened down to Claude Code, its MCP servers, and a vite dev server at 2.1 GB](/blog/mcp-freeze/diagram-two-views.webp "macOS adds everything an app started into one line. wsp splits it back out, by thread.")

Here's that same thread from my Mac this morning, as a table:

| Process | What it is | Memory |
| --- | --- | ---: |
| Claude Code | the agent | 266 MB |
| exec + rea | an MCP server and the tool it runs | 292 MB |
| mcp-proxy-for-aws | an MCP server, two processes | 94 MB |
| mcp-search-console | an MCP server, two processes | 79 MB |
| zsh, bash, pnpm | a command the agent ran | 129 MB |
| **vite** | **the dev server that command started** | **2.1 GB** |

In the force quit window, every one of those rows would have been added into the terminal's line.

### Why it's hard

When Claude Code runs a command, it starts a fresh shell for it, and that shell is gone about 24 milliseconds later. If the command started a dev server or an indexer in the background, the server keeps running, but **its parent is gone.** macOS hands it to launchd, and nothing on the process points back to the session that started it.

![One command, pnpm dev, on two timelines. Polling every 2 seconds first sees vite after its shell is gone and cannot tell whose it is. wsp sees both start and keeps vite filed under the thread.](/blog/mcp-freeze/diagram-timeline.webp "By the time a 2-second poll looks, the shell that started vite is long gone.")

A tool that checks every couple of seconds simply misses most of them. When I tested that on macOS 26.4, polling every 2 seconds kept track of **9 out of 16** test processes.

### How wsp works it out

Each computer you add runs a small wsp daemon. On a Mac, instead of looking every few seconds, it asks the kernel to tell it about every new process the moment it starts. So it sees the shell, then vite, and files both under the thread before the shell is even gone. On the same test it kept track of **15 out of 16**.

Some processes detach on purpose, with `setsid` or `nohup`, to get away from the terminal that started them. That doesn't lose them either: every process a thread starts carries a small label saying which thread it belongs to, and detaching doesn't remove it.

On a Linux box it's simpler. Every thread runs in its own cgroup, a group the kernel keeps for it, so everything inside belongs to that thread no matter who its parent is.

And when wsp can't tell whose a process is, it doesn't hide it. It shows up under **"No thread"**, with its memory, so a leftover indexer can't eat your RAM unseen.

The numbers are the ones you'd see in Activity Monitor: each process's memory footprint, and power from the kernel's own energy counter.

It works the same on every computer you add, not just your Mac.

I wish I'd had this in June. It would have taken one glance instead of six restarts and a very confused EM.
