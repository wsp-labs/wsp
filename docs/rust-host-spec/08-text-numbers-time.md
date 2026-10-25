# 08 Text, numbers, time and limits

Where JavaScript and Rust compute the same expression and get different answers. Bun's port found most of its 19 regressions after green in code "syntactically identical in both languages but semantically different" (#2440 research); this chapter lists the ones the TS host leans on, each measured on node 22.23.4 and rustc 1.97 on this box. Every `file:line` is at `a3b870a6e`.

## Strings

A JavaScript string is UTF-16: `.length`, `slice`, `substring`, `padStart`, `charAt` and every index count UTF-16 code units. A Rust `String` is UTF-8 and its natural units are bytes and `char`s. The two agree on ASCII and on the Basic Multilingual Plane by char count, and differ by one for each character above U+FFFF (an emoji is one `char` and two units).

A scan of the TS source for named caps and literal slices finds 317 caps, at least 69 of them checked against `.length` or a slice, among them every sentence cap a person reads and every size the store keeps.

## Proposal 5: what counts text

> **Proposal, awaiting the owner's ruling** (ticket decision 5, which asked which caps must match exactly). Pick: all of them. Every length, cap, slice and index that reaches the wire, a file, the store or a sentence counts UTF-16 code units, through one module `wsp_words::utf16` (`len`, `cut(s, n)` for the first n units, `tail(s, n)` for the last n, `cmp` for ordering). Counting `char`s is allowed only for a value that never leaves its unit.

Why all and not a list: the tapes compare output exactly, so a cap counted in another unit shows as a difference on the first emoji; deciding cap by cap needs judgement at 69 sites and more, which is where a cheap model goes wrong without a test noticing; and the helper costs a few lines. The ticket's two named caps, `SAID_LINE_CHARS` and `RUN_OUTPUT_MAX_CHARS`, are both on the list below.

8.1. A port MUST NOT use `str::len`, `chars().count()` or a byte slice for any text 8.3 covers.

8.2. Where the TS code counts bytes (`Buffer.byteLength`, a Buffer's length) the port counts UTF-8 bytes. Two caps named as bytes count units in the TS code and MUST stay units: `SKILL_HEAD_BYTES` (`collect/src/detect/skills.ts:33,148`) and `SLATE_LIMITS.valuesBytes`.

8.3. The rule of Proposal 5 MUST hold for every cap, slice and index whose result leaves the unit, including those 8.4 does not list.

8.4. These caps, all counting UTF-16 units, reach a person or the wire and MUST match exactly:

| Cap | Value | Holds | Where |
|---|---|---|---|
| `SAID_LINE_CHARS` | 400 | a kept refusal's line, ` (cut N characters)` | `protocol/src/exit.ts:92` |
| `RUN_OUTPUT_MAX_CHARS` | 65,536 | a run's output tail | `protocol/src/run-block.ts:35` |
| `TOOL_RESULT_KEPT` | 16,384 | a tool result kept in the transcript | `runtime/src/types/internal.ts:219` |
| `TRANSCRIPT_BYTES` | 4 MiB of event JSON length | a thread's transcript (rule 2.11) | `internal.ts:215` |
| `READ_OUTPUT_CHARS` | 4,000 | a slate read's run output | `runtime/src/slate-read.ts:10` |
| `OPENING_TITLE_MAX`, `GENERATED_TITLE_MAX`, `TITLE_EXCERPT_MAX`, `OPENING_KEPT` | 48, 40, 600, 600 | thread titles and openings | `protocol/src/words/turn.ts:284,306,336,339` |
| `LISTED_LINE_CHARS`, `SUBAGENT_ASKED_CHARS` | 200, 280 | turn listings, sub-thread cards | `words/turn.ts:38,42` |
| `REFUSAL_REASON_MAX`, `VERSION_MAX_CHARS` | 80, 64 | a computer's refusal reason, a version word | `protocol/src/words/places.ts:292,316` |
| `FILE_NAME_MAX` | 120 | an attachment's name | `protocol/src/attachments.ts:209` |
| `SUBJECT_MAX`, `DRAFT_TASK_MAX` | 72, 2,000 | a change's subject, a draft task | `protocol/src/changes.ts:65,85` |
| `NAME_MAX` | 40 | a workspace name | `protocol/src/start.ts:110` |
| `SAID_MAX`, `SSH_LINE_CAP`, `QUOTE_MAX`, `COMMAND_CHARS` | 300, 300, 120, 160 | doctor lines, ssh errors, a sign-in answer in an error, an init log line | `host/src/doctor.ts:990`; `engine/src/ssh-backend.ts:92`; `host/src/relay.ts:126`; `host/src/init-job.ts:118` |
| `DOOR_BODY_CAP` | 512 | a daemon error body in a sentence | `runtime/src/daemon-channel.ts:64` |
| `ANSWER_CAP` | 1,048,576 | an MCP server's tools answer | `host/src/server-tools.ts:44` |
| `HTTP_URL_MAX` | 8,192 | a URL on the wire | `protocol/src/wire/limits.ts:12` |
| `APP_PROC_ORIGIN_CHARS` | 256 | an app process's origin | `protocol/src/views/app-procs.ts:21` |
| slate limits | expression 500, sketch value 80, document 64 KiB, values 256 KiB | the slate module | `protocol/src/slate/limits.ts:3-40` |
| unnamed slices | 64 (`login`, `address`, `ssh`), 120, 200, 300 | sentences in places, engine and words | `host/src/place-back.ts:76-89`; `host/src/places/add-words.ts:414-555`; `engine/src/copier.ts:86`; `protocol/src/words/units.ts:358` |

8.5. A cut that lands inside a surrogate pair MUST drop the lone half. The TS host keeps it, and `JSON.stringify` then writes it as an escape such as `\ude00` (checked: `"a😀b".slice(-2)` stringifies as `"\ude00b"`); a Rust `String` cannot hold it. This is the port's one deliberate text difference, written in the harness's accepted list as class `lone-surrogate` with this reason.

8.6. `trim()`, `trimStart()`, `trimEnd()` and the regex class `\s` MUST use JavaScript's whitespace set, through `wsp_words::js_trim` and an explicit class. Rust's `str::trim` and the `regex` crate's `\s` use Unicode's White_Space, which differs on two characters: U+FEFF is whitespace to JavaScript and not to Rust, U+0085 the other way round (checked on both).

8.7. Regexes MUST be ported by hand and their unit spec MUST say so. A JavaScript regex without the `u` flag works on UTF-16 units (`.` matches half a surrogate pair; a class range is over units); the `regex` crate works on `char`s, which gives the same matches except on characters above U+FFFF, where a pattern relying on half a pair is noted in its spec. 15 TS regexes use lookbehind, which the `regex` crate does not support: those use `fancy-regex` or code. Only 3 TS regexes use the `u` flag.

8.8. Ordering MUST be JavaScript's: a bare `.sort()` on strings (53 sites) orders by UTF-16 units, which differs from Rust's byte order only between characters above U+FFFF and those from U+E000 to U+FFFF, through `wsp_words::utf16::cmp`; `localeCompare` (23 sites, `protocol/src/project-seed.ts:123`, `runtime/src/machines/sub-queue.ts:67`, `host/src/agents-reader.ts:152` among them) is ICU collation, through `wsp_words::collate`, built on ICU4X's collator with the root locale. They differ a lot: node 22 sorts `["b","A","a","B","_x","1","a-b","ab","a_b","Z","é","e","f"]` with `localeCompare` as `_x 1 a A a_b a-b ab b B e é f Z` and with `.sort()` as `1 A B Z _x a a-b a_b ab b e f é`. A listing a person reads is sorted one of these two ways and the port keeps which.

8.9. `toLowerCase` and `toUpperCase` are locale-free full case mappings and match Rust's `to_lowercase` and `to_uppercase`, final sigma included. `toLocaleLowerCase` appears nowhere in the host.

8.10. `Intl.Segmenter` (graphemes, `host/src/init-layout.ts:18`) is the one place the host counts graphemes; it uses the `unicode-segmentation` crate's extended grapheme clusters.

## Numbers

8.11. A number turned into text MUST be written as JavaScript writes it (`String(n)`, a template literal, `JSON.stringify`): through `wsp_wire`'s JavaScript formatter (rule 4.25). Rust's `Display` differs: 10^21 is `1000000000000000000000` against `1e+21`, 10^-7 is `0.0000001` against `1e-7`, and negative zero is `-0` against `0` (checked).

8.12. `Math.round` MUST be `wsp_words::js_round`: JavaScript rounds a half toward positive infinity (`Math.round(-2.5)` is -2), Rust's `f64::round` away from zero (-3).

8.13. `toFixed(n)` (38 sites) MUST be `wsp_words::to_fixed`: JavaScript picks the larger of two equally near results on the exact value of the double, Rust's `{:.n}` rounds ties to even. `(2.5).toFixed(0)` is `3` against Rust's `2`, `(-2.5).toFixed(0)` is `-3` against `-2`, `(0.125).toFixed(2)` is `0.13` against `0.12`, and `(1.005).toFixed(2)` is `1.00` in both (the double is below 1.005).

8.14. `toLocaleString` and `Intl.NumberFormat` appear in a few fixed forms, each ported as a fixed formatter with a test of the TS output: `en-US` with grouping (`fmtCalls`, `protocol/src/words/units.ts:52`; the slate's `kit.ts:182,191`, `expr.ts:540`), dollars with two decimals (`units.ts:421`), rupees in `en-IN` with lakh grouping and the currency sign (`units.ts:427`). `Number(s)`, `parseInt` and `parseFloat` (177 sites) parse as JavaScript does: `Number("")` is 0, `Number(" 12 ")` is 12, `parseInt("12px")` is 12; each unit spec names the inputs its sites see.

## Time

8.15. Wire times MUST keep their two forms: milliseconds since the epoch as integers (`Date.now()`), and `toISOString()` text (93 sites), always `YYYY-MM-DDTHH:mm:ss.sssZ` with three fractional digits and `Z`.

8.16. `Intl.DateTimeFormat` and `toLocaleDateString` appear in a few fixed forms (`protocol/src/usage.ts:364-402`; `runtime/src/usage.ts:164`; `runtime/src/slate-sources/time.ts:7-8`; the slate's `expr.ts:646-651`; `host/src/verbs/conversations.ts:22`), each with an explicit locale (`en-CA` dates, `en-GB` 24-hour times and long dates, `en-US` weekdays, `sv-SE` sortable stamps). Each is ported as a fixed formatter over `jiff` with its zone, with a test of the TS output. The host's own zone is `Intl.DateTimeFormat().resolvedOptions().timeZone`: `TZ`, else `/etc/localtime`, which is `jiff`'s `TimeZone::system()` order. The service unit passes no `TZ` (06, rule 6.9).

8.17. `Date.parse` and `new Date(string)` (125 sites) accept ISO 8601 and a few other forms; each unit spec lists what its sites are handed, and the port parses those, RFC 3339 first.

8.18. A timer delay MUST be clamped as node clamps it: a `setTimeout` or `setInterval` delay above 2,147,483,647 ms fires after 1 ms (node's TimeoutOverflowWarning). The runtime caps its own long timers at that value (`MAX_TIMER_MS`, `runtime/src/types/internal.ts:227`, used at `runtime/src/runtime.ts:885,904`; `TIMER_CAP_MS`, `engine/src/errors.ts:244-251`), but a slate timer's `setInterval(fire, every * 1000)` has no cap, so a huge `every` fires every millisecond (the large-module pilot found it). The Rust clock reproduces the clamp; 14 lists it as an after-port ticket.

## Constants a person sees

The same scan finds 417 timing constants. They are not copied here: each unit's spec cites the ones it uses, at their `file:line`. The ones that shape what crosses the wire are in 03 (timers), 04 (frame and door limits), 06 (process bounds) and 8.4 above.

8.19. Every constant MUST keep its value and its unit (milliseconds, seconds, units, bytes, lines, items). A constant named for one unit and computed in another (`TRIAL_TTL_S` declared as 7,200 seconds, `engine/src/box-backend.ts:125`; `SWEEP_TIMEOUT_S`, `engine/src/golden-tools.ts:226`) is ported in the unit its use reads.
