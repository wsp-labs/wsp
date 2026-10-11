# Live tests and the canary

The unit suite (`pnpm test`) creates no machines and needs no key. Two kinds
of test stay out of it and run only when asked.

## The canary

The provider changes under us. On 2026-09-04 Solari's host pool started
refusing a create field it had accepted and ignored two days earlier, and
every `wsp init` booted nothing until someone read the error off a live run.
The canary sends every body wsp sends to Boat and fails naming Boat's answer
word for word, next to the body it sent.

```sh
pnpm canary                                   # two Boat starts; the daemon may take its 15 minute budget
BOAT_CANARY_IMAGE=<named snapshot> pnpm canary
```

It needs `BOAT_API_KEY` in the environment or in `.env` at the repo root, a
ready wsp image among the account's named snapshots (the newest one named
`wsp-<host>-<image>-v<n>`, or the one `BOAT_CANARY_IMAGE` names), and two
starts left in Boat's minute, hour and day. It makes one box, owned by
`h_canary` so no host's sweep claims it, deletes it by id and reads it gone,
and deletes any other box a create answered with. Run it once a day and
before every release; a red case is the signal to change a body on purpose,
not a flake to rerun.

`packages/engine/test/box.live.test.ts` drives wsp's own `BoxBackend`
through a fetch that records every call, in order:

- `GET /limits` on wsp's base (`ascii.dev/api/box/v1`) and on Boat's
  documented one (`boat.dev/api/v1`), which name the same counts `activeBoxes`
  and `activeSandboxes`; it fails when either drops its own.
- `GET /named-snapshots`, then the create from the image (`POST /boxes` with
  `type`, `ttlSeconds`, `noEnv`, `name`, `from` and an `Idempotency-Key`). The
  same request is sent again the moment the first answers, as a lost answer
  is, and once more after the box is ready, with the same bytes: both must
  give the same box back. A changed `ttlSeconds` under the same key must be
  refused `idempotency_key_reused`.
- The rename `PATCH` (Boat names a box after its snapshot) and the stop timer
  `PATCH`, read back off `archiveAfter`; the listing the sweep reads.
- The daemon's environment landing, commands with their exit code and both
  streams, a 600 KB file through `PUT /files` and back through `GET /files`.
- The daemon's token through `PUT /files`, its hosted route (`POST /host`),
  and a dial with the token and a `ping` until it answers, starting the
  daemon after 60 s of silence as the host does.
- `POST /stop`, `POST /resume` and the daemon again, then `DELETE` with
  `X-Ascii-Confirm-Delete`, and `GET /limits` once more.

Each answer is checked for every field wsp reads off it (`boat-wire.ts`
holds the table), so a renamed field fails naming the call and the answer.
At the end it prints its readings: create to ready, the environment landing,
the daemon's first answer after the create and after the resume, stop,
resume, the replies to the replays, `hydrated`, and the starts the run
counted.

Solari's canary is `pnpm canary:solari` (`WSP_LIVE_LONG=1` adds the 65
minute `createdAt` reading). It needs `SOLARI_API_KEY` in `.env` and covers:

- `packages/engine/test/create-canary.live.test.ts`: the builder body from
  `prepareBuilder`, the smoke fork body from `sealGolden` and the fork body
  from `forkGolden`, each asserted 201 and killed until gone; and the listing
  row of a machine it just created carrying `metadata`, `cpu` and `memMb`,
  which the sweep's owner and cost lines read.
- `packages/runtime/test/create-canary.live.test.ts`: the workspace body the
  runtime itself posts (owner label, lifecycle pause, idle backstop), created
  through `workspaces.create` on a snapshot the case takes and deletes, and
  removed through `workspaces.delete` with the machine read gone.
- `packages/engine/test/solari-quirks.live.test.ts`: the platform bugs we
  coded around, one case each, shouting when the platform changes so a guard
  is relaxed on purpose.
- `packages/engine/test/createdat-drift.live.test.ts` (only with
  `WSP_LIVE_LONG=1`): one base sandbox with the builder's lifecycle, never
  exec'd or paused, read right after create and at 2, 10 and 65 minutes. A
  measurement, not a guard: it prints every `createdAt` and `expiresAt`,
  asserts only that the machine stayed running and is gone after the kill.
  Measured 2026-09-04, the provider's `createdAt` tracks the wall clock about
  five minutes behind, so nothing in wsp reads it for a decision.

## Other live tests

Every file named `*.live.test.ts` creates real machines and runs only under
`WSP_LIVE=1`. They respect the account's two-machine cap, kill everything
they create, never touch a machine labeled `poc=ttl-test`, and check that
nothing is left running at the end. Never add parallelism flags to them.

## The render test

The terminal pane's glyph test
(`apps/web/src/terminal/ghostty/glyphs.browser.test.ts`) starts a Vite dev
server and Playwright's Chromium to draw Nerd Font codepoints through the
real pane and read the pixels back. It runs only under
`WSP_RENDER=1 pnpm test`, needs `pnpm exec playwright install chromium`
once, and skips, saying so, when that browser is missing.
