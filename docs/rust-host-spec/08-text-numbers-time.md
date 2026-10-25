# 08 Text, numbers, time and limits

Where JavaScript and Rust compute the same expression and get different answers. Bun's port found most of its 19 regressions after green in code "syntactically identical in both languages but semantically different" (#2440 research); this chapter lists the ones the TS host leans on, each measured on node 22.23.4, and the Rust side with rustc 1.97.1 and serde_json 1.0.151 (the daemon's lock) where a rule says measured. Every `file:line` is at `a3b870a6e`.

## Strings

A JavaScript string is UTF-16: `.length`, `slice`, `substring`, `padStart`, `charAt` and every index count UTF-16 code units. A Rust `String` is UTF-8 and its natural units are bytes and `char`s. The two agree on ASCII and on the Basic Multilingual Plane by char count, and differ by one for each character above U+FFFF (an emoji is one `char` and two units).

A scan of the TS source for named caps and literal slices finds about 300 caps (317 by the first scan's patterns, 288 by a second reader's), at least 69 of them checked against `.length` or a slice, among them every sentence cap a person reads and every size the store keeps. The totals depend on the patterns; the caps that bind are named below.

## Proposal 5: what counts text

> **Proposal, awaiting the owner's ruling** (ticket decision 5, which asked which caps must match exactly). Pick: all of them. Every length, cap, slice and index that reaches the wire, a file, the store or a sentence counts UTF-16 code units, through one module `wsp_words::utf16` (`len`, `cut(s, n)` for the first n units, `tail(s, n)` for the last n, `cmp` for ordering). Counting `char`s is allowed only for a value that never leaves its unit.

Why all and not a list: the tapes compare output exactly, so a cap counted in another unit shows as a difference on the first emoji; deciding cap by cap needs judgement at 69 sites and more, which is where a cheap model goes wrong without a test noticing; and the helper costs a few lines. The ticket's two named caps, `SAID_LINE_CHARS` and `RUN_OUTPUT_MAX_CHARS`, are both on the list below.

8.1. A port MUST NOT use `str::len`, `chars().count()` or a byte slice for any text 8.3 covers.

8.2. Where the TS code counts bytes (`Buffer.byteLength`, a Buffer's length, `TextEncoder`) the port counts UTF-8 bytes. A cap named as bytes is counted the way each site counts it, not the way its name says: `SKILL_HEAD_BYTES` counts units (`collect/src/detect/skills.ts:33,148`); `SLATE_LIMITS.valuesBytes` and `documentBytes` count UTF-8 bytes through `slateBytes` (`protocol/src/slate/limits.ts:48`: `new TextEncoder().encode(text).length`; `slate/batch.ts:133`, `slate/validate.ts:155`, `runtime/src/slates.ts:254`, used at `:923,1053`), except one site that checks `valuesBytes` against `v.length` in units (`slate/expr.ts:924`); `DOOR_BODY_CAP` and `HTTP_URL_MAX` say bytes in their comments and count units (`daemon-channel.ts:108-121`, `wire/limits.ts:115`); `TRANSCRIPT_BYTES` counts `JSON.stringify(event).length` in units when it trims (`runtime/src/transcript-reader.ts:60-65`) and `Buffer.byteLength(line) + 1` in bytes when it reads (`:49`); `ANSWER_CAP` reads at most 1,048,577 bytes (`head -c`) and then checks the length in units (`host/src/server-tools.ts:103,124`), so a body of multibyte characters cut at the byte limit can pass the unit check and be parsed cut, where a body of ASCII refuses (8.24).

8.3. The rule of Proposal 5 MUST hold for every cap, slice and index whose result leaves the unit, including those 8.4 does not list.

8.4. These caps, all counting UTF-16 units, reach a person or the wire and MUST match exactly:

| Cap | Value | Holds | Where |
|---|---|---|---|
| `SAID_LINE_CHARS` | 400 | a kept refusal's line, ` (cut N characters)` | `protocol/src/exit.ts:92` |
| `RUN_OUTPUT_MAX_CHARS` | 65,536 | a run's output tail | `protocol/src/run-block.ts:35` |
| `TOOL_RESULT_KEPT` | 16,384 | a tool result kept in the transcript | `runtime/src/types/internal.ts:219` |
| `TRANSCRIPT_BYTES`, `TRANSCRIPT_CAP` | 4 MiB of event JSON length in units, 5,000 events | a thread's transcript, per thread (rule 2.11) | `internal.ts:212,215` |
| `READ_OUTPUT_CHARS` | 4,000 | a slate read's run output | `runtime/src/slate-read.ts:10` |
| `OPENING_TITLE_MAX`, `OPENING_KEPT`, `GENERATED_TITLE_MAX`, `TITLE_EXCERPT_MAX` | 48, 600, 40, 600 | thread titles and openings | `protocol/src/words/turn.ts:284,306,336,339` |
| `LISTED_LINE_CHARS`, `SUBAGENT_ASKED_CHARS` | 200, 280 | turn listings, sub-thread cards | `words/turn.ts:38,42` |
| `VERSION_MAX_CHARS`, `REFUSAL_REASON_MAX` | 64, 80 | a version word, a computer's refusal reason | `protocol/src/words/places.ts:292,316` |
| `FILE_NAME_MAX` | 120 | an attachment's name | `protocol/src/attachments.ts:209` |
| `DRAFT_TASK_MAX`, `SUBJECT_MAX` | 2,000, 72 | a draft task, a change's subject | `protocol/src/changes.ts:65,85` |
| `NAME_MAX` | 40 | a workspace name | `protocol/src/start.ts:110` |
| `SAID_MAX`, `SSH_LINE_CAP`, `QUOTE_MAX`, `COMMAND_CHARS` | 300, 300, 120, 160 | doctor lines, ssh errors, a sign-in answer in an error, an init log line | `host/src/doctor.ts:990`; `engine/src/ssh-backend.ts:92`; `host/src/relay.ts:126`; `host/src/init-job.ts:118` |
| `DOOR_BODY_CAP` | 512 | a daemon error body in a sentence | `runtime/src/daemon-channel.ts:64` |
| `ANSWER_CAP` | 1,048,576 | an MCP server's tools answer | `host/src/server-tools.ts:44` |
| `HTTP_URL_MAX` | 8,192 | a URL on the wire | `protocol/src/wire/limits.ts:12` |
| `APP_PROC_ORIGIN_CHARS` | 256 | an app process's origin | `protocol/src/views/app-procs.ts:21` |
| slate limits | expression 500, sketch value 80 (units); document 64 KiB, values 256 KiB (UTF-8 bytes, 8.2) | the slate module | `protocol/src/slate/limits.ts:3-40` |
| unnamed slices | 64 (`login`, `address`, `ssh`), 32 (`user`), 40 (`nameOfTask`), 120, 200, 300 | sentences in places, engine and words | `host/src/place-back.ts:76-89`; `host/src/places/add-words.ts:414-555` (32 at `:528,536,556`); `protocol/src/start.ts:185`; `engine/src/copier.ts:86`; `protocol/src/words/units.ts:358` |

8.5. A cut that lands inside a surrogate pair MUST drop the lone half. Two TS cuts already do (`wholeCharacters`, `protocol/src/words/base.ts:40-43`, under every `cutLine`; `keptOpening`, `words/turn.ts:313-317`), with the comment that a JSON reader such as the Rust tool server's refuses the escape. The others keep it (`exit.ts:96`, `run-block.ts:40`, `runtime/src/threads/transcripts.ts:669`, `runtime/src/slate-read.ts:18`, `words/turn.ts:345`, `start.ts:179`, `changes.ts:99`), and `JSON.stringify` then writes it as an escape such as `\ude00` (checked: `"a😀b".slice(-2)` stringifies as `"\ude00b"`); a Rust `String` cannot hold it. At those sites this is the port's deliberate text difference, written in the harness's accepted list as class `lone-surrogate` with this reason.

8.6. `trim()`, `trimStart()`, `trimEnd()` and the regex class `\s` MUST use JavaScript's whitespace set, through `wsp_words::js_trim` and an explicit class. Rust's `str::trim` and the `regex` crate's `\s` use Unicode's White_Space, which differs on two characters: U+FEFF is whitespace to JavaScript and not to Rust, U+0085 the other way round (checked on both).

8.7. Regexes MUST be ported by hand and their unit spec MUST say so. A JavaScript regex without the `u` flag works on UTF-16 units (`.` matches half a surrogate pair; a class range is over units); the `regex` crate works on `char`s, which gives the same matches except on characters above U+FFFF, where a pattern relying on half a pair is noted in its spec. 15 TS regexes use lookbehind, which the `regex` crate does not support: those use `fancy-regex` or code. Only 5 TS regexes on 4 lines use the `u` flag (`host/src/init-job.ts:213`, `host/src/init-layout.ts:20-21`, `protocol/src/slate/validate.ts:655`).

8.8. Ordering MUST be JavaScript's: a bare `.sort()` on strings (53 sites) orders by UTF-16 units, which differs from Rust's byte order only between characters above U+FFFF and those from U+E000 to U+FFFF, through `wsp_words::utf16::cmp`; `localeCompare` (23 lines, 29 calls, `protocol/src/project-seed.ts:123`, `runtime/src/machines/sub-queue.ts:67`, `host/src/agents-reader.ts:152` among them) is ICU collation, through `wsp_words::collate`, built on ICU4X's collator, in the host's default locale: no call passes a locale, so node takes ICU's default from `LC_ALL`, else `LC_MESSAGES`, else `LANG` (`LC_COLLATE` is not read), and `C`, `C.UTF-8`, `POSIX` or none give `en-US` (measured: under `env -i` the collator's locale is `en-US` and `["z","å","ä","a"]` sorts `a å ä z`; under `LANG=sv_SE.UTF-8` or `LC_ALL=sv_SE.UTF-8` it is `sv-SE` and sorts `a z å ä`). The port MUST resolve the locale the same way. The tapes run under `env -i`, so they pin `en-US`; a host started from a person's terminal inherits their `LANG` and sorts in their locale. They differ a lot: node 22 sorts `["b","A","a","B","_x","1","a-b","ab","a_b","Z","é","e","f"]` with `localeCompare` as `_x 1 a A a_b a-b ab b B e é f Z` and with `.sort()` as `1 A B Z _x a a-b a_b ab b e f é`. A listing a person reads is sorted one of these two ways and the port keeps which.

8.9. `toLowerCase` and `toUpperCase` are locale-free full case mappings and match Rust's `to_lowercase` and `to_uppercase`, final sigma included. `toLocaleLowerCase` appears nowhere in the host.

8.10. `Intl.Segmenter` (graphemes, `host/src/init-layout.ts:18`) is the one place the host counts graphemes; it uses the `unicode-segmentation` crate's extended grapheme clusters.

## Numbers

8.11. A number turned into text MUST be written as JavaScript writes it (`String(n)`, a template literal, `JSON.stringify`): through `wsp_wire`'s JavaScript formatter (rule 4.25). Rust's `Display` differs: 10^21 is `1000000000000000000000` against `1e+21`, 10^-7 is `0.0000001` against `1e-7`, and negative zero is `-0` against `0` (checked).

8.12. `Math.round` MUST be `wsp_words::js_round`: JavaScript rounds a half toward positive infinity (`Math.round(-2.5)` is -2), Rust's `f64::round` away from zero (-3).

8.13. `toFixed(n)` (38 sites) MUST be `wsp_words::to_fixed`: JavaScript picks the larger of two equally near results on the exact value of the double, Rust's `{:.n}` rounds ties to even. `(2.5).toFixed(0)` is `3` against Rust's `2`, `(-2.5).toFixed(0)` is `-3` against `-2`, `(0.125).toFixed(2)` is `0.13` against `0.12`, and `(1.005).toFixed(2)` is `1.00` in both (the double is below 1.005).

8.14. `toLocaleString` and `Intl.NumberFormat` appear in a few fixed forms, each ported as a fixed formatter with a test of the TS output: `en-US` with grouping (`fmtCalls`, `protocol/src/words/units.ts:52`; `protocol/src/agents-report.ts:435`; the slate's `kit.ts:182,191`, `expr.ts:540`), dollars with two decimals (`units.ts:421`), rupees in `en-IN` with lakh grouping and the currency sign (`units.ts:427`). A form with no locale would follow 8.8's default (`LANG=de_DE.UTF-8` writes 1234.5 as `1.234,5`); a unit spec that finds one says so. `Number(s)` and `parseInt` (178 lines; `parseFloat` appears nowhere) parse as JavaScript does: `Number("")` is 0, `Number(" 12 ")` is 12, `parseInt("12px")` is 12; each unit spec names the inputs its sites see.

## Time

8.15. Wire times MUST keep their two forms: milliseconds since the epoch as integers (`Date.now()`), and `toISOString()` text (93 lines), `YYYY-MM-DDTHH:mm:ss.sssZ` with three fractional digits and `Z` for years 0000 to 9999, `±YYYYYY-...` outside them (`new Date(8.64e15).toISOString()` is `+275760-09-13T00:00:00.000Z`), and a thrown `RangeError: Invalid time value` for an invalid date (`new Date(NaN)`), which the port raises as the same failure where the TS lets it throw.

8.16. `Intl.DateTimeFormat` and `toLocaleDateString` appear in a few fixed forms (`protocol/src/usage.ts:364-402`; `runtime/src/usage.ts:164`; `runtime/src/slate-sources/time.ts:7-8`; the slate's `expr.ts:646-651`, which is `en-GB`; `host/src/verbs/conversations.ts:22`), each with an explicit locale (`en-CA` dates, `en-GB` 24-hour times and long dates, `en-US` weekdays at `usage.ts:382,402`, `sv-SE` sortable stamps). Two more sites read the host's zone with no `Intl`: `new Date(at).toTimeString().slice(0, 8)` (`protocol/src/words/turn.ts:240`) and the local `getFullYear`, `getMonth`, `getDate` of `protocol/src/cost-history.ts:63-70`. Each is ported as a fixed formatter over `jiff` with its zone, with a test of the TS output. The host's own zone is `Intl.DateTimeFormat().resolvedOptions().timeZone`: `TZ`, else `/etc/localtime`, which is `jiff`'s `TimeZone::system()` order. The service unit passes no `TZ` (06, rule 6.9).

8.17. `Date.parse` (49 lines, 51 calls) is the only string parse: no `new Date(...)` takes a string (77 calls take a number, 28 none). It accepts ISO 8601 and a few other forms; each unit spec lists what its sites are handed, and the port parses those, RFC 3339 first. The sites handed arbitrary text, whose odd inputs a test pins against node: `protocol/src/slate/chart.ts:38,46` and `slate/expr.ts:518` (any slate string), `adapter-claude/src/adapter.ts:1016`, `collect/src/history/reader.ts:78`, `engine/src/golden-import.ts:1382`; the rest read stored ISO stamps and a cloud's `createdAt`.

8.18. A timer delay MUST be clamped as node clamps it: a `setTimeout` or `setInterval` delay above 2,147,483,647 ms fires after 1 ms (node's TimeoutOverflowWarning). The runtime caps its own long timers at that value (`MAX_TIMER_MS`, `runtime/src/types/internal.ts:227`, used at `runtime/src/runtime.ts:885,904`; `TIMER_CAP_MS`, `engine/src/errors.ts:244-251`), but a slate timer's `setInterval(fire, every * 1000)` has no cap, so a huge `every` fires every millisecond (the large-module pilot found it). The Rust clock reproduces the clamp; 14 lists it as an after-port ticket.

## Constants a person sees

The same scan finds about 410 timing constants (417 and 406 by two readers' patterns). They are not copied here: each unit's spec cites the ones it uses, at their `file:line`. The ones that shape what crosses the wire are in 03 (timers), 04 (frame and door limits), 06 (process bounds) and 8.4 above.

8.19. Every constant MUST keep its value and its unit (milliseconds, seconds, units, bytes, lines, items). A constant named for one unit and computed in another (`TRIAL_TTL_S` declared as 7,200 seconds, `engine/src/box-backend.ts:125`; `SWEEP_TIMEOUT_S`, `engine/src/golden-tools.ts:226`) is ported in the unit its use reads.

## How each cap cuts

The pilot's ports both got `asText(null)` and the cut rules wrong because the spec said "capped at N" and nothing more. Each cap below says which end it keeps, what marker it adds and where, whether it trims first, and what an empty input gives. Counts are UTF-16 units unless 8.2 says bytes.

8.20. `markedCut(line, max = SAID_LINE_CHARS)` (`protocol/src/exit.ts:96`) MUST keep the head: over 400 units it is `line.slice(0, 400) + " (cut N characters)"`, N the units dropped; no trim before or after; `""` stays `""`. `keptSaid` (`:99-103`) splits on `\n` only, cuts each of the first 11 lines (`SAID_LINES`, `:94`) and adds a line `(cut N lines)` past them. Both are for what a host keeps: the log line (`runtime/src/serve.ts:373`), a computer's add job (`runtime/src/places/manage.ts:322-326`), a usage reset (`runtime/src/usage-reset.ts:159`). A refusal answered on the socket is not cut: the first tape below echoes a 410-unit name whole.

8.21. `RUN_OUTPUT_MAX_CHARS` (`protocol/src/run-block.ts:38-43`) MUST keep the tail with no marker, and only for a text over 65,536 units: `cut = text.slice(-65536)`, then everything up to and including the first `\n` of `cut` is dropped, so the result can be far shorter than the cap or empty; no trim; `""` stays `""`; the slice can split a pair (8.5). Its callers scrub secrets before the cut (`runtime/src/slate-runs.ts:284,779-780`; `slate-mcp.ts:765,770,778`; `threads/sessions.ts:1258`).

8.22. `TOOL_RESULT_KEPT` (`runtime/src/threads/transcripts.ts:659-671`) MUST clip only a `session.delta` whose text is over 16,384 units and whose kind is `tool_result`, or that has a `parentToolUseId` and kind `text` or `thinking`: `text.slice(0, 16384)`, no marker, and `bytes: event.bytes ?? Buffer.byteLength(event.text)`, the UTF-8 size of the whole text. A patch on the event is cut by `keptPatch` (`protocol/src/tool-result.ts:33-58`), counting each line's units without its newline, keeping the leading lines of the hunk that crosses the cap, recounting that hunk's `oldLines` and `newLines`, and setting `patchCut: true`. The live stream carries the whole text; only the transcript is clipped.

8.23. `TRANSCRIPT_BYTES` (`transcripts.ts:98-125`) MUST trim per thread by dropping the oldest events outside the newest turn first, then the oldest of the newest turn, never the newest event, with no marker, measuring `JSON.stringify(event).length` (8.2), beside the cap of 5,000 events (`TRANSCRIPT_CAP`).

8.24. The other caps a person reads MUST cut as follows:
- `READ_OUTPUT_CHARS` (`runtime/src/slate-read.ts:18`): only an `out` or `err` string, `text.slice(0, 4000) + "… (N characters; values [\"$name.field\"] reads it whole)"`, with U+2026 then a space, N the whole text's length, applied only past 4,000 units; `out` is left out when the run has `json`; `lines` keeps its last 50. The second site (`runtime/src/slates.ts:1246`) writes three ASCII dots, `... (N characters)`, N the length of the stringified JSON.
- `cutLine(text, room)` (`protocol/src/words/base.ts:33-55`), under the title caps: over `room`, it keeps `wordsWithin(text, room - 1)` (up to the last space after index 0 within the room, with a trailing run of `[\s,;:]` dropped), else a hard cut of `room - 1` units with a split pair's high half dropped, then `…`, the ellipsis counted inside the room.
- `openingTitle` (48): the first non-blank line, `\s+` collapsed to one space and trimmed, then its first sentence by `/^.*?[.!?](?=\s|$)/`, then `cutLine`.
- `generatedTitle` (40): trimmed; `null` if empty or holding `\r` or `\n`; one wrapping quote pair stripped, the opening from `"'“‘` and the closing from `"'”’`, which need not match; trailing dots stripped; `null` if then empty; over 40, `wordsWithin(title, 40) ?? null`, no ellipsis.
- `excerpt` (600, `words/turn.ts:343-346`): trimmed first, then `slice(0, 600) + "…"`, 601 units, no word boundary and no pair guard.
- `keptOpening` (600, `turn.ts:313-317`): no trim, no marker, one unit shorter when a high surrogate sits at index 599; `openingCut` is `length >= 599`.
- `NAME_MAX` (40, `protocol/src/start.ts:176-180`): `` `${"Review " or ""}#${n} ${oneLine(title)}`.slice(0, 40).trimEnd() ``: head kept, no marker, no word boundary, trimmed after the cut, a pair can split. `nameOfTask` (`:183-185`) takes the first five words and the same 40.
- `SUBJECT_MAX` (72, `protocol/src/changes.ts:90-100`): strip a code fence, strip quotes, each em dash (U+2014) with the whitespace around it becomes `, `, take the first non-blank line trimmed; over 72, `wordsWithin(first, 72) ?? first.slice(0, 72)`, no marker; an empty answer is `null`; the body is the remaining lines joined and trimmed. `DRAFT_TASK_MAX` (`:77`): trimmed, then `slice(0, 2000)`, no marker.
- `DOOR_BODY_CAP` (`runtime/src/daemon-channel.ts:108-121`): the refused upgrade's body is appended while `body.length < 512`, so it can pass the cap before `body.slice(0, 512)`; each chunk is decoded alone, so a character split between chunks reads as U+FFFD; only its first line, trimmed, is used (`:41`), and an empty one gives `the connection was refused with <status>`, else `the connection was refused with <status>: <line>`.
- `ANSWER_CAP` (`host/src/server-tools.ts:44,103,124,133,186`): refuses, never cuts: `its tools answer is over 1 MB and was not read`, checked in units after a byte-limited read (8.2).
- `HTTP_URL_MAX` (`protocol/src/wire/limits.ts:115`): refuses, never cuts: not a string, over 8,192 units, failing `/^https?:\/\/[^\s\x00-\x1f\x7f]+$/i` (with JavaScript's `\s`, 8.6), or not parsed by `new URL`.

## JSON in and out

8.25. A name quoted in a sentence is `JSON.stringify` of it (`bareNoSuchProjectLine`, `protocol/src/projects.ts:372`, and the other `${JSON.stringify(x)}` sentences), so the sentence holds the escapes as text: a NUL is the six characters `\u0000`, a tab `\t`, a quote `\"`, a lone surrogate `\ud83d`, while non-ASCII, DEL, U+0085, U+FEFF and U+2028 stay as themselves. The port MUST quote with the same escapes (serde_json's string writer matches, measured: `é😀\u{2028}\u{0}\u{1f}\u{7f}/<` writes as `"é😀` U+2028 `\u0000\u001f` DEL `/<"`). A name that is empty or all spaces is quoted as it is, never trimmed or refused: the tapes below show `no project ""` and a name with spaces, U+FEFF and U+0085 kept whole.

8.26. A frame holding an unpaired surrogate escape MUST be read, not refused. The TS host's `JSON.parse` takes `"a\ud83db"` and its answer echoes the half back as an escape (the tape below). serde_json 1.0.151 refuses both halves, into a `String` and into a `Value` alike (measured: `{"ref":"a\ud83db"}` fails with `unexpected end of hex escape`, `"a\ude00b"` with `lone leading surrogate in hex escape`), which would answer `invalid json` and, before auth, close the socket with 4401 (rule 4.29). The Rust host reads the frame as UTF-16 with each unpaired half replaced by U+FFFD (`String::from_utf16_lossy`'s rule), and the echo then differs from the TS echo by that one character: class `lone-surrogate` (8.5) in the harness's accepted list, with this reason. This is a gap the port must fill, not a TS behaviour, and the pick (U+FFFD on the way in, a dropped half on a cut) is the owner's to confirm.

8.27. JSON the host reads MUST parse as `JSON.parse` does where it differs from serde_json's defaults: a duplicate key keeps the last value (`JSON.parse('{"a":1,"a":2}')` is `{a: 2}`); a number past the double's range is `Infinity` (`JSON.parse("1e400")`), which a schema then refuses or `JSON.stringify` writes as `null`; `-0` parses as negative zero and is written back as `0` (rule 4.25).

8.28. Numbers the host writes MUST follow rule 4.25, and non-integral numbers do cross the wire (`rateUsdPerHour`, `protocol/src/capabilities.ts:27,44`, `views/golden-image.ts:272`, `views/sub-row.ts:76`, `sub-thread.ts:28`; `usedPercent`, `costReported`, `costList`, `estimate`, `saved`, `usage.ts:17,218-245`; `angle`, `radius`, `grain`, `opacity`, `workspace-look.ts:46,87,89`). `NaN`, `Infinity` and `-Infinity` are written as `null` (`JSON.stringify([NaN, Infinity, -Infinity])` is `[null,null,null]`), and a reader's `z.number()` then drops the frame whole; an `undefined` in an array is written as `null`, an `undefined` field is left out (rule 4.27). Measured, for 04's editor: serde_json 1.0.151 writes `1e21` as `1e+21`, as JavaScript does, and `-0.0` as `-0.0` where JavaScript writes `0`.

## Tapes

Recorded from a throwaway TS host at `a3b870a6e`. The lines are written with `\u` escapes so no character is invisible on the page; the checker compares after `JSON.stringify` on both sides, and on the wire the host writes non-ASCII raw (rule 4.28). The first answer is a 410-unit name echoed whole, past `SAID_LINE_CHARS` (8.20); then non-ASCII, U+2028 and DEL kept; a lone surrogate, a NUL, a tab and a quote quoted as escapes (8.25, 8.26); an empty name; a name with spaces, U+FEFF and U+0085, none trimmed (8.6).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"projects.resolve","ref":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}}
{"out":{"id":2,"ok":false,"error":"no project \"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"; wsp add <folder> records one","kind":"not-found"}}
{"in":{"id":3,"op":"projects.resolve","ref":"\u00e9\ud83d\ude00\u2028\u007f"}}
{"out":{"id":3,"ok":false,"error":"no project \"\u00e9\ud83d\ude00\u2028\u007f\"; wsp add <folder> records one","kind":"not-found"}}
{"in":{"id":4,"op":"projects.resolve","ref":"a\ud83db\u0000\t\""}}
{"out":{"id":4,"ok":false,"error":"no project \"a\\ud83db\\u0000\\t\\\"\"; wsp add <folder> records one","kind":"not-found"}}
{"in":{"id":5,"op":"projects.resolve","ref":""}}
{"out":{"id":5,"ok":false,"error":"no project \"\"; wsp add <folder> records one","kind":"not-found"}}
{"in":{"id":6,"op":"projects.resolve","ref":" \ufeffx\u0085 "}}
{"out":{"id":6,"ok":false,"error":"no project \" \ufeffx\u0085 \"; wsp add <folder> records one","kind":"not-found"}}
```

A thread's id in a sentence is its first 8 units (`threadWord`, `id.slice(0, 8)`); 07's second tape shows `no thread 0a1b2c3d`.

Why no tape for the rest: the caps of 8.20 to 8.24 cut text a fresh host never holds (a run's output, a transcript, a title, a refusal kept on a job); the numbers of 8.28 need usage, a cost or a cloud price; the locale of 8.8 needs two projects or two agents to sort, whose replies carry random ids (07's tape section). Each is a unit test in its unit's packet, against the TS function's own output.

## After the port

A cut that keeps a lone half (8.5); `ANSWER_CAP`'s byte read checked in units (8.2); `DOOR_BODY_CAP` and `HTTP_URL_MAX` named as bytes and counted in units (8.2); `localeCompare` following the host's locale, so two hosts list the same names in two orders (8.8); a slate `every` past node's timer limit firing every millisecond (8.18).

