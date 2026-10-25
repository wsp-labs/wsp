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
2. A struct's pass reads its keys in schema order. Each key is read as the type it holds: a plain type through `read`
   (absent → `required`), `.optional()` through `read_present` (absent is fine, `null` is refused), `.optional().nullable()`
   through `read_double`, a required `.nullable()` as `Option<T>` marked `nullable` (absent is refused, `null` is fine).
   The key's own rules run on the value read, before the next key is read (`rule_<field>`; the doc comment above each
   `check` lists them as zod declares them), and the nested type's issues come back with the key in front (`under`). So the
   first issue is the first key of the schema that fails, whatever kind of issue it is. `Vec`, `Option` and `IndexMap` read
   each value with its index (a number) or key in front. A type with no pass and no rules keeps the trait's defaults.
   A tagged enum's pass reads the tag first (`tag_describe` words a missing or unknown one). A member that is a type of its
   own answers that type's issues; a member written inline reads its fields as a struct does, each key under its own name,
   and the enum's `check()` runs its rules once every key reads clean (`rules_of`), so within an inline member a rule on an
   earlier key answers after a type issue on a later one, the one place the order differs from zod's. No tagged enum reads a
   member through serde's own path tracking (which an internal tag loses), a mirror enum or a payload struct.
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
5. `JsNumber::js_string` is `String(n)`: shortest round-trip digits; fixed notation for 1e-7 < |n| < 1e21
   (`0.000001`, `100000000000000000000`), exponent notation outside with a sign (`1e+21`, `1e-7`, `1.5e+300`), `-0` as
   `0`. Serialize: an integral value with |n| < 2^53 through `serialize_i64` (so `to_value` is an integer); anything
   else through `serialize_f64`. Deserialize: any JSON number; nothing else.
6. `literal!` writes its value and reads only it; `True`/`False` are wsp-frames'. `double_option` reads absent →
   `None`, null → `Some(None)`, a value → `Some(Some(v))` and writes each back. `nullable` makes a required
   `.nullable()` key refuse its absence. `from_json` reads the generated default's JSON (it never fails on generated
   text; a panic is a generator bug).

7. A union follows zod 3's `z.union` (`types.cjs`): the first member that reads with no issues is the value; otherwise
   the first member with no issue whose code aborts (`invalid_type`, `invalid_enum_value`, `invalid_literal`,
   `invalid_union`, `invalid_union_discriminator`) answers with its issues, since a dirty member (a refinement, an unknown
   key on a strict member) is one zod keeps; otherwise the union answers `invalid_union`. A discriminated union (a tagged
   enum) reads the tag first: a missing or unknown tag is `invalid_union_discriminator` at the tag, and a member's own
   issues are answered at the member's own path.

## Edge cases

- `utf16_len("😀") == 2`, `utf16_len("e\u{301}") == 2`; an origin of 129 emoji breaks a 256 cap that 129 chars would
  not.
- `PreferencesTarget` with two unknown keys: one issue, `keys: ["extra","more"]`, path `[]`.

## Decisions

1. serde_json without `arbitrary_precision` cannot write every JavaScript number's text: integral values from 2^53 to
   1e21 and fractions under 1e-6 or past 1e21 come out in ryu's form (`9007199254740994.0`, `1e-6`, `1e21`). Accepted
   difference until the coordinator rules on `arbitrary_precision`; `js_string` is exact everywhere, the serialize
   tests cover only values where both agree.
2. A lone surrogate (rule 8.5) cannot reach a Rust `String`; nothing here handles it.
3. `serde_json` reads with `float_roundtrip`. Its default float reader is up to one unit in the last place off for a long
   mantissa (`123456789012345680000` came back as `123456789012345670000`), and the number text JavaScript writes from it
   would differ from node's.
4. A key the TS writes as `null` when it has nothing (`z.x().nullable()`, required) is an `Option` with no
   `skip_serializing_if`, and carries `deserialize_with = "nullable"` so that an absent key is `Required` and `null` is a
   value; an `Option` that skips its `None` is `.optional()`, and refuses `null`. Inline members of a `skip_serializing_none`
   enum follow the same reading, `serialize_always` marking the nullable ones.
5. The first issue of an inline union member is not always zod's (rule 2): the enum's rules run after the member's keys
   read clean. A member's issues all keep their own paths; only a rule issue on an earlier key and a type issue on a later
   one of the same member swap. Moving each enum's rules into its keys, as the structs have them, closes it.

## Rules pinned → tests (`tests/types_check.rs`)

| Rule | Test |
|---|---|
| 1, 3, 4 serde mapping, regex, refine, issue order and text | `the_first_issue::*` (7 tests, 18 refusals from zod) |
| 2 path prefixing, containers | `a_list_an_option_and_a_map_check_each_value_with_its_index_or_key_in_front`, `the_first_issue::under_puts_a_segment_in_front_of_the_path_and_keeps_the_rest` |
| 3 lengths | `a_length_counts_utf16_units_as_javascript_s_length_does`, the AppProc cases |
| 5 numbers | `javascript_numbers::*` (4 tests) |
| 6 literal, nulls, defaults | `literals_and_nulls::*` (4 tests) |
| every generated shape | `tests/shapes.rs`, 475 tests (spec/shapes.md) |
