# 13 The porting guide

How one TypeScript unit becomes one Rust unit. The owner's direction on the map (2026-10-25, "tests first, with a spec") set the shape: interface, one-page spec and failing tests before any body, a cheaper model implements until green, a reviewer who did not write it checks it. The port research on the map (#2440, comment 6396136288, 2026-10-25) changed the recipe after reading how other teams ran large ports through models; this chapter writes the changed recipe as the rule.

Two pilots tested the form. The first, on three small host modules: with the interface and tests written first, Haiku 5.5 and Sonnet 5.5 each passed 31 of 31 and 4 of 4 on the first run. The large-module pilot (#2440, comment 6396255198, 2026-10-25) ran it on `packages/runtime/src/slate-runs.ts`, 987 stateful lines with processes, timers and secrets: both models passed 30 of 30 on the first hand-in, Haiku 5.5 for $0.24 and Sonnet 5.5 for $2.05, and neither port was review-clean: an Opus reviewer found 3 to 4 real blockers in each green port, and about 13 of each port's 20 findings came from something the spec left out or worded loosely, with both models making the same wrong choice each time. One round of exact tests written from the findings fixed most of them for $0.09 and $0.56. So the spec carries the quality, green is not done, and a cheap model is as good as a dearer one against a good packet. The pilot's Rust came to 2.1 to 2.4 times the TS source and 1.7 times its tests.

The chapter has three parts: the recipe (for the coordinator and the packet writers), the gates, and the implementer's page, which is the two pages that go into every packet.

> **Proposal 6, awaiting the owner's ruling** (ticket decision 6, the interface form for a module). Pick: the packet of Part 1 (a stub file with `todo!()` bodies and each TS function as a comment above its stub, a one-page spec in the form of 13.7, the ported tests red), the gate of Part 2, and the implementer's page of Part 3, with the ladder, the held-out tapes and the review loop as the research and the large-module pilot set them. The coordinator asked for this recipe to be written as the rule; the owner rules it on the map, and until then the waves follow it as written.

## Part 1: the recipe

### Units and packets

13.1. A **unit** is about 400 TS lines: one source file or a few files that only import each other. A file over 400 lines is split along its interface first; the 73 files over 600 lines hold 44% of what is left to port (64% of the runtime), so splitting them is the packet writer's first job. The limit is for the review, not the implementer: a 987-line unit went green in one attempt on both models, but its review found 20 findings, too many to read well in one pass.

13.2. A **packet** is everything an implementer gets for one unit, and nothing else: 30,000 to 60,000 tokens holding the TS unit and its TS tests, the Rust stub file, the one-page spec, the failing Rust tests, the signatures (not bodies) of the crates it calls, the implementer's page (Part 3), and one or two ported sibling units of the same crate as examples. The implementer never searches outside its crate. One crate's packets share their opening so the prompt cache hits.

13.3. A unit is ported after every unit it imports has its interface checked in. Its tests compile against those stubs, so packet writers run one wave ahead of implementers. A dependency that is not implemented yet is a stub plus a fake in the tests, never a hand-written copy of its logic.

13.4. A crate stays on one box for its wave, with its packet writers, implementers and reviewer, so its build folder stays warm (#2440 research, part 5).

### The packet, written before any body

Packet writers are Opus 5.5 high threads, working under this spec, which was written at xhigh. A packet takes 20 to 25 Opus minutes per 1,000 TS lines, about 7 to 9 per 400-line unit; the pilot's 9-minute packet for 987 lines was too thin, and the time it saved came back in review and rework.

13.5. **The stub file.** One Rust file per unit at the path Proposal 1 assigns (`crates/<crate>/src/<unit>.rs`), holding every public item the unit's callers use: types, constants, functions and traits, each body `todo!()`. A script places each TS function's source as a comment above its stub, so the implementer translates the code beside it rather than recalling it (#2440 research, "script the copying"). The file compiles.

13.6. Names and types follow these rules, one each:

| TS | Rust |
|---|---|
| `runOutputTail`, `RUN_OUTPUT_MAX_CHARS`, `PlaceRecord` | `run_output_tail`, `RUN_OUTPUT_MAX_CHARS`, `PlaceRecord`; wire names kept with `#[serde(rename_all = "camelCase")]` on the struct |
| `interface X { a: string; b?: number }` | `struct X { a: String, #[serde(skip_serializing_if = "Option::is_none")] b: Option<f64> }`; `i64` or `u64` where the TS code or its zod schema says `.int()` (rule 4.25) |
| `"a" \| "b"` | an `enum` whose serde names match the literals exactly |
| a union with a `kind` or `type` field | `#[serde(tag = "kind")]` enum, one variant per member |
| `Record<string, T>` | `IndexMap<String, T>`, ordered as rule 4.26 says |
| `unknown`, `Json` | `serde_json::Value`, with serde_json's `preserve_order` feature (rule 4.26) |
| zod `.default(v)` | `#[serde(default = ...)]` giving `v` |
| `undefined` against `null` | absent is `Option` with `skip_serializing_if`; a field the TS code writes as `null` is `Option` without it (rule 4.27) |
| struct field order | the order the TS code writes the object's keys, since some outputs are compared byte for byte (rule 4.26) |
| an injected deps object (`UsageResetDeps`, `runtime/src/usage-reset.ts:53`) | one trait per unit, same method names, `async fn` where the TS returns a Promise |
| a thrown error | `Result<T, HostError>` (05); the stub's comment names the kinds the TS `throw` sites stamp |
| state shared across the runtime | the context handle's `ctx.with(\|s\| ...)` (rule 3.3), never a lock and never a message per access (Proposal 2); the comment names the state the function reads and writes |
| a cap, slice or count of text | the unit, by rule 8.3 |

13.7. **The one-page spec.** One Markdown file beside the stub, `crates/<crate>/specs/<unit>.md`, under one page:

```
# <unit>
Source: <ts path> (<n> lines) at <commit>. Tests: <ts test paths> (<n> cases).
## What it does
<three to six sentences>
## Behaviour
1. <one pinned behaviour, with ts path:line>
## Edge cases
- <input> gives <output> (<ts path:line> or <test name>)
## Behaviour nobody wrote down
- <the rule of this spec it follows, cited as rule N.M, and the unit's own instance of it>
## Tapes
<the ops it serves and the tape files that send them; held-out tapes are counted here, not named>
## Decisions
1. <a choice the TS code leaves open, and the default taken>
## After the port
- <anything that looks wrong, filed as a ticket, not changed>
```

13.8. Every Behaviour line carries a `file:line` at the commit the wave names. A line the writer cannot point at in the code is a decision, not a behaviour. The spec MUST pin, because the pilot's shared findings all fell here: what each function does with null, absent and empty inputs (`asText(null)` was one); every cap and what happens one past it; the kill and stop paths, and which callbacks may run while state is borrowed (rule 3.3); what survives a host restart and what is rebuilt; map and key order (rules 3.11, 4.26); and how a path is joined where the TS resolves it.

13.9. "After the port" is the only place a wrong-looking behaviour is written. The port keeps it. Changing it in the port is a review blocker: the tapes would stop matching and nobody could tell which side moved.

13.10. **The test list and the hard tests.** The packet writer lists every test the unit must pass, each named against the TS case or the tape it comes from, and writes the hard ones itself: the cases about state, ordering, text units, errors and processes. A test MUST assert the state it protects, never only a count: one of the pilot's review tests checked only the run count, and both models then made the same wrong fix that kept it green and broke the restart case. A loose test does more harm than a missing one. A separate Sonnet 5.5 test-porter thread ports the plain bodies of the rest. Test names are the `it(...)` sentence in snake case, cut at 80 characters; `describe` becomes a module; `it.each` becomes a loop over the same rows; a TS comment saying why a case exists is carried over.

13.11. Fakes replace TS stubs one for one: `vi.useFakeTimers()` and `fakeClock()` become the injected clock (rule 3.15); `writeStub()` scripts (`packages/protocol/test/stub-script.ts`) become fake executables written by the Rust test helper of the same name; `fake-procs.ts` and `stub-backend.ts` become fakes of the traits in 01's seam table.

13.12. **The red proof.** With every body still `todo!()`, the packet writer runs the unit's tests and checks that every listed test fails and that the count matches the list. The output of `cargo xtask unit <name>` showing N failed and 0 passed goes in the unit's state. A test that passes against `todo!()` tests nothing and is rewritten before the packet ships.

13.13. **Held-out tapes.** The shadow-traffic tapes from #2443 and about one in five of each unit's own tapes stay out of the packet. The implementer never sees them; the reviewer runs them. A hidden check is what tells a real port from one shaped to the visible tests (#2440 research: one study passed 100% of the tests it saw and 61.58% to 91.78% of hidden ones).

### Implementing

13.14. There is no Opus lead per batch. An implementer thread on Sonnet 5.5 works one crate's queue, about 10 units, and for each unit starts every attempt in a fresh context on the model the ladder picks, then records the unit's state.

13.15. **The ladder.** Every unit starts on Haiku 5.5, stateful ones too: on the 987-line pilot Haiku matched Sonnet on first-run passes, on review findings and on what one round fixed, at an eighth of the cost. Sonnet 5.5 is the second step, then one attempt on Opus 5.5 high. Haiku and Sonnet get two attempts each; each attempt is a fresh context holding the packet, the latest version of the unit's file, and the gate's trimmed failure output. A unit Opus fails goes back to its packet writer, because the spec or the interface is likely wrong. An attempt gets 20 minutes or 40 tool calls, whichever comes first.

13.16. An implementer changes only bodies and private helpers inside the unit's paths. It never edits the stub's signatures, the spec or a test. A test it believes wrong goes in its report with the reason; the packet writer rules.

13.17. An implementer that meets a question the spec does not answer takes the TS code's behaviour as the answer, writes the question and its answer under Decisions in its report, and keeps going. It never waits for a person.

13.18. **The sweep.** When five or more units fail the same way, the coordinator stops sending that kind of unit, fixes the implementer's page, the interface or the script that made them, and reruns every unit of that kind (#2440 research, "sample, tune, sweep").

### Reviewing and landing

13.19. A unit is done when its review is clean, not when its tests pass: green came on the first attempt every time in the pilot, and clean never did. An Opus 5.5 xhigh reviewer that wrote none of it reads every unit, about ten of one crate a pass, told to assume each is wrong. It reads the TS unit and the Rust unit side by side against the spec's Behaviour list, reads the diff against the path allowlist, and runs the held-out tapes itself. It checks the three things a green suite hides: a shortcut that returns early and passes, a dropped error path, and text counted in the wrong unit (08). Each box keeps its reviewer free of other work so finished units never queue.

13.20. **The review loop.** Each finding becomes an exact test written by the packet writer (about 4 Opus minutes a unit), the unit gets one cheap round against them (the pilot: $0.09 on Haiku, $0.56 on Sonnet), then a short re-review (1 to 2 Opus minutes). A finding both models, or two units, share goes back to the packet writer as a gap in the spec or the interface; another attempt does not fix it.

13.21. When a unit lands, its TS internal tests are deleted in the same change, so the suite never runs a behaviour twice. Its TS source stays until cutover: the TS host is the oracle for every tape.

13.22. A unit's state is one of `packet`, `red`, `attempt <model> <n>`, `green`, `escalated`, `review`, `findings <n>`, `clean`, `landed`, recorded where the coordinator's queue reads it, so a coordinator starts the next unit from the queue when a finish line arrives, without waiting for the owner.

## Part 2: the gates

13.23. One command runs every check for a unit, cheapest first, and stops at the first failure: `cargo xtask unit <name>` runs `cargo fmt --check`, `cargo check`, `cargo clippy -D warnings`, the unit's tests, then the unit's visible tapes. It is the implementer's only test command and the stop hook's condition: the turn is held until it is green or the attempt's budget is spent.

13.24. The same command refuses, before running anything: a changed test file of the unit; fewer tests than the unit's list; a `todo!()` left in the unit; a changed path outside the unit's allowlist (its stub file, private helper files beside it, and nothing else). These are the mechanical cheats other ports found (Bun's builder stubbed out what did not compile; one study's run passed build and tests while covering 3.0% of them).

13.25. A green gate with zero tapes run for the unit's ops is not green (rule 12.6).

13.26. Every cargo build or test on the owner's AWS boxes runs as `/root/cargo-slot.sh cargo <args> -j 3`, which waits for one of the box's two build slots; each thread uses its own `CARGO_TARGET_DIR` under /tmp. No thread runs `git stash`, `git reset` or commits a path that is not its own.

## Part 3: the implementer's page

This part is the porting guide every packet carries. It stays under two pages; a longer guide made results worse in one study (#2440 research, part 1).

**You are porting one unit.** Its TS source is in the packet, and each TS function sits as a comment above the Rust stub you fill. Translate that code line by line. Do not redesign it, simplify it, or fix what looks wrong: write it under After the port in your report and keep the TS behaviour.

**What you may change.** Bodies and private helpers in the unit's own files. Not a signature, not a type the stub declares, not the spec, not a test.

**The one command.** `cargo xtask unit <name>` (as `/root/cargo-slot.sh cargo xtask unit <name> -j 3` on the AWS boxes). Run it after every change. It refuses an edited test, a leftover `todo!()` and a file outside the unit.

**When the spec is silent,** the TS code is the answer. Write the question and what you did under Decisions, and go on.

**Text.** A JS string counts UTF-16 code units. Every `.length`, `slice`, `substring`, `padStart` and index that reaches the wire, a file or a sentence counts UTF-16 units in Rust too, through `wsp_words::utf16` (rule 8.3). `trim()` is `wsp_words::js_trim`, not `str::trim` (rule 8.6). `localeCompare` is `wsp_words::collate`, and a bare `.sort()` on strings is `wsp_words::utf16::cmp` (rule 8.8).

**Numbers.** An integral wire field is `i64` or `u64`; every other number is written by `wsp_wire`'s JavaScript formatter (rule 4.25). `Math.round` is `wsp_words::js_round`, and `toFixed` is `wsp_words::to_fixed` (rules 8.12, 8.13).

**JSON.** serde_json has `preserve_order` on in every crate; declare struct fields in the order the TS code writes the keys; a `Record` is an `IndexMap`; leave a field absent where the TS code leaves it `undefined` (rules 4.25 to 4.27).

**Errors.** Return `HostError` with the kind the TS `throw` stamps and the TS sentence word for word (05). A refusal has two halves joined by `refusal_line` (rule 5.5).

**State and time.** Shared host state is reached through the context handle's `ctx.with(|s| ...)`, never a lock, and no borrow is held across an `.await` or a call out: an `emit`, another area's method, a road's `kill`, a store write (rule 3.3). Time comes from the injected clock.

**Processes and git.** Use the spawn helper the stub names; never call `std::process::Command` directly (06, 07).

**Comments.** Carry the TS doc comment over as one `///` line where it states a constraint the code cannot show. No other comments.

**Your report** ends with: the gate's last lines, Decisions, After the port, and any test you believe wrong with the reason.

## A worked example: `run_block`

The pilot's first unit, as its stub would be checked in. TS source `packages/protocol/src/run-block.ts` (142 lines) at `a3b870a6e`; tests `packages/protocol/test/run-block.test.ts` (103 lines). Under Proposal 1 it lands in `wsp-words`, since it holds sentences and the pure functions that build them. The script's TS comments above each stub are left out here for length.

```rust
//! A reply's shell block that a person can run where it stands: which labels run, the command a block holds, and
//! the text a finished run leaves behind it. Port of packages/protocol/src/run-block.ts at a3b870a6e.

/// The fence labels a reply's block runs under (run-block.ts:7).
pub const SHELL_FENCES: [&str; 5] = ["sh", "bash", "zsh", "shell", "console"];

/// The sentence the launch context and the skill carry (run-block.ts:11; read by engine/src/machine-context.ts).
pub const RUN_BLOCK_WORDS: &str = "A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block";

/// The words a run's block draws (run-block.ts:110; read by protocol/src/views/activity-names.ts).
pub struct RunWords { pub run: &'static str, pub run_again: &'static str, pub running: &'static str, pub move_to: &'static str, pub send: &'static str, pub open_tab: &'static str }
pub const RUN_WORDS: RunWords = RunWords { run: "Run", run_again: "Run again", running: "Running", move_to: "Move to terminal", send: "Send output to the agent", open_tab: "Open the tab" };

/// The most of a run's output the thread keeps, in UTF-16 units: exact, rule 8.4 (run-block.ts:35).
pub const RUN_OUTPUT_MAX_CHARS: usize = 64 * 1024;

/// The most lines of a run's output a sent message carries (run-block.ts:129).
pub const RUN_SENT_LINES: usize = 60;

/// How long a run prints before it is offered a terminal tab (run-block.ts:107).
pub const RUN_MOVE_AFTER_MS: u64 = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState { Exited, Moved, Lost, Running }

pub struct RunEnd { pub state: RunState, pub exit_code: Option<i32>, pub signal: Option<i32> }

/// The command a block holds, or None where the block is not one to run (run-block.ts:19).
pub fn runnable_command(language: &str, code: &str) -> Option<String> { todo!() }

/// The tail of a run's output under the cap, starting at a line's start (run-block.ts:38).
/// Counts UTF-16 units, as `String.length` does (rule 8.4).
pub fn run_output_tail(text: &str, max: usize) -> String { todo!() }

/// A terminal's bytes as the lines a person saw (run-block.ts:48).
pub fn terminal_text(raw: &str) -> String { todo!() }

/// A reply block's name on the thread's record (run-block.ts:103).
pub fn run_block_key(message_id: &str, offset: usize) -> String { todo!() }

/// How a run ended, in the line under its output (run-block.ts:120).
pub fn run_ended_words(run: &RunEnd) -> String { todo!() }

/// What Send output to the agent sends into the thread (run-block.ts:133).
pub fn run_output_message(command: &str, run: &RunEnd, output: &str) -> String { todo!() }
```

Its spec's Decisions would hold three lines, each a question the TS code answers without saying so:

1. `HIDDEN` (run-block.ts:15) is a regex over UTF-16 code units; in Rust it is a `char` match over the same ranges, which is the same set because every range is inside the Basic Multilingual Plane.
2. `trim()` (run-block.ts:26) strips JavaScript's whitespace set; Rust's `str::trim` strips `char::is_whitespace`. They differ on two characters, measured on node 22 and rustc here: U+FEFF is trimmed by JavaScript and not by Rust, U+0085 by Rust and not by JavaScript. The port uses `js_trim` (rule 8.6). The pilot's Sonnet 5.5 run found this unprompted.
3. `runOutputTail` cuts with `slice(-max)` on UTF-16 units and can split a surrogate pair; the next step drops everything up to the first newline, so a split pair survives only when the tail holds no newline. Then the TS host sends the lone half as a `\ude00` escape (`JSON.stringify` writes lone surrogates as escapes), which a Rust `String` cannot hold. The port drops the lone half: the one place it differs, written as an accepted difference with this reason (rule 8.5).

And one After the port line: `runEndedWords` prints `Exited 0` for a run with neither an exit code nor a signal (run-block.ts:125), which reads as a success the run may not have had.
