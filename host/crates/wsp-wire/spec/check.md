# check

Source: zod 3.25.76's parse as the protocol's schemas use it (`packages/protocol/package.json`), JavaScript's
`Number.prototype.toString`, and rules 4.25 to 4.28 of `docs/rust-host-spec/04-wire.md` (branch
`docs/2442-rust-host-spec`). Rust: `src/types/check.rs`, `js_number.rs`, `literal.rs`. Tests: `tests/types_check.rs`
(18, every expectation captured from zod and node: `fixtures/rules/issues.json`), and every shape test of
`tests/shapes.rs` runs through `parse`. Shared with the wire writer, who builds the op dispatch on it.

## What it does

A type's schema pass, generated from its fields, answers what zod's `safeParse` answers: every issue in zod's order,
and the first of them is the one `parse` returns. Serde reads the value once the pass finds nothing. `Check` carries the
rest: `pass`, the type's rules (`check`) and what zod calls each key (`describe`). `JsNumber` is a plain `z.number()`,
written as JavaScript writes it. `literal!` types are kept `z.literal` keys.

## Behaviour

1. `parse(v)` answers the first issue of `issues_of(v)`: the type's `pass` where it has one, else serde's one error read
   with `serde_path_to_error` and then `check()`. The issues are `SchemaError`s with the path segments zod writes (object
   keys as strings, array indexes as numbers): a missing key → `invalid_type`, message `Required`, expected = the key's zod
   type word, received `undefined`; a wrong JSON type → `invalid_type`, message `Expected <expected>, received <received>`,
   received one of `string number boolean null array object`; a fraction where `.int()` is → `invalid_type` expected
   `integer` received `float`; an unknown enum word → `invalid_enum_value` with `received`, `options` (every word in
   order) and zod's message; an unknown key under `deny_unknown_fields` → one `unrecognized_keys` issue at the object's own
   path naming every unknown key in input order; a wrong `literal!` or `wsp_frames::True`/`False` → `invalid_literal` with
   `received`, `expected`; a tagged union with an unknown or missing tag → `invalid_union_discriminator` at `[tag]` with
   `options` = every tag in declaration order; an untagged union with no member → `invalid_union`.
2. A struct's pass reads its keys in schema order, one call per key (`src/types/keys.rs`): a plain type through
   `req_key` (absent → `required`), `.optional()` through `opt_key` (absent is fine, `null` is refused), `.optional().nullable()`
   through `dbl_key`, a required `.nullable()` as `Option<T>` marked `nullable` (absent is refused, `null` is fine), a
   `z.literal` key through `lit_key`. The key's own rules run on the value read, before the next key is read (`rule_<field>`
   or a closure; the doc comment above each `check` lists them as zod declares them), and the nested type's issues come back
   with the key in front (`under`). So the answer is zod's list of issues in zod's order, whatever kind each issue is, and
   `parse` answers its first. This holds for an enum's inline members as it does for structs: a member written inline
   reads each key through the same calls, with the rules of that key in its `_with` call, so no later key's issue comes
   before an earlier key's. A number whose only fault is the integer rule leaves its bounds to run on the fraction, as
   zod's `ZodNumber` does (`Check::dirty`): a fraction past a bound is two issues. `Vec`, `Option` and `IndexMap` read each
   value with its index (a number) or key in front. A type with no pass and no rules keeps the trait's defaults. A tagged
   enum's pass reads the tag first (`tag_describe` words a missing or unknown one). A member that is a type of its own
   answers that type's issues. No tagged enum reads a member through serde's own path tracking (which an internal tag
   loses), a mirror enum or a payload struct. `describe` words what zod calls a key: a type that gives none is worded from
   serde itself (the default `describe` probes a string: a unit enum answers its words, a struct is an object), and a
   unit enum or struct read from the wrong JSON type answers `invalid_type` before serde reads it, so serde's map form
   (`{"word": null}`) and positional form (`[..]`) are never taken.
3. Numbers: `.int()` is `JsInt`, a `JsNumber` that zod's integer check takes (`Number.isInteger` on the number JSON read):
   `5.0`, `-0`, `1e21` and every integral value past 2^63 are integers, `2.5` is not (`invalid_type`, expected `integer`,
   received `float`). A `JsInt` keeps the number JavaScript reads, so `9007199254740993` reads and writes as
   `9007199254740992`. `>= n` / `> n` / `<= n` / `< n` give `too_small` / `too_big` with
   `minimum`/`maximum`, `type: "number"`, `inclusive`, `exact: false` and zod's message (`Number must be greater than or
   equal to 0`, `Number must be greater than 0`, ...). Strings and arrays: `type: "string"` / `"array"`, messages
   `String must contain at most 256 character(s)`, `Array must contain at most 20 element(s)`, and the `at least` and
   `exactly` forms; a string's length is `utf16_len`. A failed regex is `invalid_string`, message `Invalid`,
   keys `validation code message path` (`validation: "regex"`). A failed refine is `custom` with the refine's own
   words as its message (`an absolute path on the computer`, `one of 13, 14, 15, 16, 17`), keys `code message path`.
4. `issue_text()` is `JSON.stringify([issue], null, 2)`: two-space indent, keys in the order zod builds each code,
   which is: invalid_type (Required) `code expected received path message`; invalid_type (from `.int()`)
   `code expected received message path`; too_small/too_big `code minimum|maximum type inclusive exact message path`;
   invalid_enum_value `received code options path message`; invalid_literal `received code expected path message`;
   unrecognized_keys `code keys path message`; invalid_union_discriminator `code options path message`. `detail` holds
   the keys other than code, path and message in that order.
5. `JsNumber::js_string` is `String(n)`: shortest round-trip digits, the even last digit where two shortest strings sit
   the same distance from the double; fixed notation for 1e-7 < |n| < 1e21 (`0.000001`, `100000000000000000000`), exponent
   notation outside with a sign (`1e+21`, `1e-7`, `1.5e+300`), `-0` as `0`. Serialize: an integral value with |n| < 2^53
   through `serialize_i64` (so `to_value` is an integer); anything else as a raw JSON value holding `js_string`, which
   serde_json's writer copies as it is (`to_string`, `to_writer`) and `to_value` reads back as the number it names. NaN and
   the infinities write `null`, as `JSON.stringify` does. Deserialize: any JSON number; nothing else. `Kept` (a value
   kept as sent: `z.unknown()`, a passthrough object's other keys) and `ReqId` write every number through `JsNumber`, and
   write object keys that read as array indexes first, ascending.
6. `literal!` writes its value and reads only it; `True`/`False` are wsp-frames'. `double_option` reads absent →
   `None`, null → `Some(None)`, a value → `Some(Some(v))` and writes each back. `nullable` makes a required
   `.nullable()` key refuse its absence. `from_json` reads the generated default's JSON (it never fails on generated
   text; a panic is a generator bug).

7. A union follows zod 3's `z.union` (`types.cjs`): the first member that reads with no issues is the value; otherwise
   the first member with no issue whose code aborts (`invalid_type`, `invalid_enum_value`, `invalid_literal`,
   `invalid_union`, `invalid_union_discriminator`) answers with its issues, since a dirty member (a refinement, an unknown
   key on a strict member) is one zod keeps; otherwise the union answers `invalid_union`. A discriminated union (a tagged
   enum) reads the tag first: a missing or unknown tag is `invalid_union_discriminator` at the tag, and a member's own
   issues are answered at the member's own path. The integer check of a number leaves its member dirty, not aborted
   (`ZodNumber` marks the parse dirty and goes on), so a member whose only issues are an integer issue and the rules answers
   with them. An untagged union reads its value as zod does too: `zod_union!` (`src/types/zod_union.rs`) writes
   `Deserialize` that takes the first member whose `issues_of` is empty, where serde's own untagged read takes the first
   member that deserializes, rules or not.

## Edge cases

- `utf16_len("😀") == 2`, `utf16_len("e\u{301}") == 2`; an origin of 129 emoji breaks a 256 cap that 129 chars would
  not.
- `PreferencesTarget` with two unknown keys: one issue, `keys: ["extra","more"]`, path `[]`.

## Decisions

1. serde_json without `arbitrary_precision` cannot hold every JavaScript number's text in a `Value`: a number written
   through a `Value` (`to_value`, then `to_string`) is ryu's (`1e21` for `1e+21`, `1e-7`). Writing a `JsNumber` straight
   (`to_string`, `to_writer`, a derived `Serialize`) is exact everywhere. The wire's frames write straight; only a caller
   that goes through a `Value` meets the difference, and only below 1e-5 in size (`0.000005` is `5e-6`), at 2^64 and
   past, and at -2^63 and below.
2. A lone surrogate (rule 8.5) cannot reach a Rust `String`; nothing here handles it.
3. `serde_json` reads with `float_roundtrip`. Its default float reader is up to one unit in the last place off for a long
   mantissa (`123456789012345680000` came back as `123456789012345670000`), and the number text JavaScript writes from it
   would differ from node's.
4. A key the TS writes as `null` when it has nothing (`z.x().nullable()`, required) is an `Option` with no
   `skip_serializing_if`, and carries `deserialize_with = "nullable"` so that an absent key is `Required` and `null` is a
   value; an `Option` that skips its `None` is `.optional()`, and refuses `null`. Inline members of a `skip_serializing_none`
   enum follow the same reading, `serialize_always` marking the nullable ones.
5. The record key `__proto__` is read and not kept, in a record and in a passthrough object, as zod's object merge drops it.
   Collation, URLs and the other known differences are in their own units' specs.

## Rules pinned → tests (`tests/types_check.rs`)

| Rule | Test |
|---|---|
| 1, 3, 4 serde mapping, regex, refine, issue order and text | `the_first_issue::*` (7 tests, 18 refusals from zod) |
| 2 path prefixing, containers | `a_list_an_option_and_a_map_check_each_value_with_its_index_or_key_in_front`, `the_first_issue::under_puts_a_segment_in_front_of_the_path_and_keeps_the_rest` |
| 3 lengths | `a_length_counts_utf16_units_as_javascript_s_length_does`, the AppProc cases |
| 5 numbers | `javascript_numbers::*` (4 tests) |
| 6 literal, nulls, defaults | `literals_and_nulls::*` (4 tests) |
| every generated shape | `tests/shapes.rs`, 475 tests (spec/shapes.md) |
