# wire_place_link
Source: the sentences of `packages/protocol/src/wire/place-link.ts` (481 lines) that wsp-wire left to this crate, at a3b870a6e: `placeStillInstalledLine` :253, `placeForgottenLine` :257 and `ALREADY_JOINED_LINE` :481. The rest of the file is wsp-wire's `wire::place_link`; so should be `placeLinkTranscript` :169 and `placeRefusalTranscript` :176, byte transcripts a link signs, which neither crate carries yet. Tests: places.test.ts:201 (wsp-wire's port); `pinned::a_joined_computer_that_holds_work_is_forgotten_in_its_own_words` in `tests/view_wire_words.rs`.

## What it does
What a remove says of a computer whose daemon it could not take off, what a forget says of one it could not reach, and the refusal of a join on a computer already joined.

## Behaviour
1. `place_still_installed_line(name)` (:253): "<name> is off this host, but the daemon on it is still installed; run wsp leave on that computer when it is back".
2. `place_forgotten_line(name, forks)` (:257-258): "<name> is forgotten here and nothing was done on it; whatever of wsp's is still there", then ", the copies its forks ran in among it," only when forks is true, then " comes off with wsp leave run on that computer".
3. `ALREADY_JOINED_LINE` (:481): "this computer is already a place in a wsp; wsp leave first".

## Edge cases
- A name holding an apostrophe is written as given (pinned with "it's").

## Behaviour nobody wrote down
- `PLACE_LEAVE_LINE` is "wsp leave" (words/places.ts:333); the constant is written out in full.

## Tapes
`wsp remove`, `wsp forget` and `wsp join` lines.

## Decisions
1. `ALREADY_JOINED_LINE` is a literal, since a Rust constant cannot splice `PLACE_LEAVE_LINE`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `pinned::a_joined_computer_that_holds_work_is_forgotten_in_its_own_words` |
| 3 | no test: a constant copied by hand |
