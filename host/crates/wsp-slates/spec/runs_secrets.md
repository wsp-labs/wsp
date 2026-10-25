# runs_secrets
Source: slate-runs.ts:499-611 (`secretForms`, `DefaultApprovals`, `createSecrets`), 197-208 (`SlateSecrets`) at
65f6b89c7. Tests: slate-runs.test.ts `secrets` (5 cases); oracle `oracle/helpers.out` (`secretForms.*`, `scrub.*`).

## What it does
Holds the plaintext of every secret a person typed into a slate, per thread and name, and nothing else ever sees
it: a handle answers set, length and time; `scrub` replaces every form a command could print it in. Secrets marked
`keep` are written to `slates.secrets.json` beside the state file and read back by the next host; the rest die with
the process. `DefaultApprovals` is the memory store of "Always" keys when the host gives none.

## Behaviour
1. `create_secrets(file, now)` (:545-556): with a file that exists, reads it as `{thread: {name: {text, at}}}` and
   holds each as kept; a file that does not read or parse holds nothing and is not rewritten.
2. `handle(thread, name)` (:565-568): unset is `{"secret":true,"set":false,"len":0,"at":null}`; set is
   `{"secret":true,"set":true,"len":<UTF-16 units of the text>,"at":<when set>}`, keys in that order.
3. `set(thread, name, text, keep)` (:578-590): an empty text is `forget(thread, name)` and answers the unset handle.
   Otherwise holds `{text, at: now, keep}` (an existing name keeps its place), and saves when this one is kept or the
   one it replaced was. A kept secret set again without `keep` leaves the file.
4. `clear` is `forget(thread, name)` then the handle (:591-594).
5. `forget(thread, name?)` (:569-576): nothing held for the thread does nothing. With a name drops that one, without
   drops the thread; saves when what went held a kept one.
6. `save` (:557-564): with no file does nothing. Otherwise writes, through `write_own` (folder 0700, file 0600),
   compact JSON with no newline: each thread with a kept secret, in the order threads were first held, and under it
   each kept name in held order as `{"text":..,"at":..}`. With none kept it writes `{}`.
7. `scrub(thread, text)` (:597-606): nothing held for the thread, or an empty text, answers the text. Otherwise
   every form of every held secret of this thread (names in held order, forms in `secret_forms` order) is paired
   with `[secret:<name>]`, sorted longest form first (a stable sort: ties keep that order), and each form in turn,
   once, replaces all its occurrences in the text as it stands. Other threads' secrets are not touched.
8. `secret_forms(text)` (:501-525), in this order, duplicates and the empty text dropped: the text; its UTF-8 bytes
   in base64 standard padded, unpadded, URL-safe padded, URL-safe unpadded; for shift 0, 1, 2 the base64 of `shift`
   zero bytes then the text's bytes, sliced from 0 (shift 0) or 4 to `floor((shift + n) / 3) * 4`, kept only when 8
   or more characters, then its URL-safe form; `encodeURIComponent(text)`, the same with each `%XX` lowercased, and
   with each `%20` as `+`.
9. `plaintext(thread, name)` answers the text or none (:607-609).
10. `DefaultApprovals` (:527-543): per thread a set of keys in insertion order; `has`, `allow`, `revoke`, `list`.

## Edge cases
- `set(t, "pin", "4321")` scrubs `pin 4321 elsewhere-secret` to `pin [secret:pin] elsewhere-secret` when
  `elsewhere-secret` is t2's; after `set(t, "pin", "")` the handle is unset and `4321` is no longer scrubbed.
- Forms of `vrcl_Ab+9/zz&q=1 ünï_0123456789` are the seven the oracle lists; a Basic header's base64 keeps only the
  bytes that share a group with `user:` (`dXNlcjp2[secret:token]ODk=`).
- A later, shorter form is matched inside a mark already written: secrets `s = "secret"` and `t = "cret"` scrub
  `a secret` to `a [se[secret:t]:s]` (oracle `scrub.nested`). Ported as is.
- A text with a space also scrubs `a+b` (oracle `scrub.plus`).
- A text holding a lone surrogate makes `encodeURIComponent` throw `URIError: URI malformed`, so every scrub of that
  thread fails (oracle `forms.lone.throws`).
- Kept file: after `set(kept, keep)`, `set(memory)`, the file is 0600 in a 0700 folder and holds no memory secret;
  a second `create_secrets` on it reads the kept one set and the memory one unset; `forget(thread)` rewrites it as
  `{}` (test `only kept ones reach the file`).

## Behaviour nobody wrote down
- Rule 2.22: `slates.secrets.json` is compact, no newline. Rule 2.17: unkept secrets are never persisted.
- Rule 8.2: base64 is over UTF-8 bytes; `len` counts UTF-16 units.

## Decisions
1. A lone surrogate cannot be held in a Rust `String`; the text arrives as JSON from the wire, where serde already
   refuses or replaces it (rule 8.26 reads the escape). The port takes the text as given and never fails a scrub.
2. `encodeURIComponent` is ported as a function keeping JS's unreserved set `A-Za-z0-9-_.!~*'()`, uppercase hex.

## After the port
- Nested marks (`[se[secret:t]:s]`) when one secret's form is inside another's mark.
- A lone surrogate in a secret makes every scrub of its thread throw in the TS.
