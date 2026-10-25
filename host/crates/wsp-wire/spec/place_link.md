# place_link
Source: packages/protocol/src/wire/place-link.ts (481 lines) at a3b870a6e, less placeStillInstalledLine, placeForgottenLine and ALREADY_JOINED_LINE, which went to wsp-words. Tests: places.test.ts (18 cases), protocol.test.ts (2), host/test/ssh-hosts.test.ts (1); 6 more pinned here against what the TS module printed. Rust: `src/wire/place_link.rs`; tests `tests/place_link.rs` (27).

## What it does
The frames of the place link (a computer you own dialling this host): the join, auth and prove requests and replies, the seal a client opens, the device that comes in through the account, and the report a place sends about itself. Then the door view, the join lines and mint, the ssh host suggestions, the place file a join writes and the held read that finds it over ssh, and the refusal sentences of those roads. No state.

## Behaviour
1. The four base64 values (place-link.ts:18-25) are wsp_frames' `Base64Bytes<N>`: nonce 32 bytes, public key 44 (SPKI DER), signature 64, ephemeral 32. They read by the rule of helpers_limits 8; a wrong length is a `custom` issue at the field ("must be N bytes, base64").
2. `PlaceReport` (:38-102): `name` 1..=200, `platform` darwin|linux, `arch` <=32, `os` <=200, `shape` a WorkspaceSize; `diskFreeBytes`, `diskSizeBytes`, `uptimeMs` non-negative integers; `model` <=200; `login` a map of string to string (a non-string value refuses the report); `engine` none|docker|podman; `copies` a views CopyWord; `daemonVersion` a non-negative integer; `daemonPort` 1..=65535; `wsp` at least one word; `agents` at most 32, each at most 32 units; `agentVersions` each value at most 64 units and at most 32 keys (a `custom` issue "at most 32 agents"); `logins` at most 64 names, each at most 200 units and `is_under_path`; `daemonUnit` at most 256 matching `^[\w@.:-]+\.service$` (`\w` ASCII); `dialed` passes `is_http_url` (helpers_limits 1). Order of the struct's fields is the TS object's.
3. `PlaceJoinRequest`, `PlaceAuthRequest`, `PlaceProveRequest`, `SealOpenRequest`, `DeviceAuthRequest` are members of RuntimeOp (runtime_ops) and hold no `op` of their own; each carries `pub const OP`. `ephemeral` is optional on join and auth (an older wsp sends none, and the host refuses it in its own sentence), required on seal.open. `placeId` at most 64; prove's `code` at most 64 and `client.name` 1..=200; device.auth's `name` 1..=200.
4. `PlaceJoinReply.hostName` 1..=200; `PlaceJoinDevice.deviceToken` at least 1.
5. `place_link_transcript` and `place_refusal_transcript` are wsp_frames' functions, re-exported: `wsp place link v2\n<role>\n<placeId>\n<challenge>\n<answer>\n<challenger eph>\n<answerer eph>\n` and `wsp place refusal v1\n<placeId>\n<nonce>\n<sentence>\n`.
6. `PlaceDoorView` (:315-324): `port` 1..=65535, `addresses` at least one, each a URL by zod's `.url()` (the WHATWG parser takes it), `hostKey` 1..=200, `relay` a URL when present. `JoinRoad`: `url` a URL, `line` non-empty. `JoinMint`: `joins` at least one, `expiresAt` zod's default `.datetime()`: `YYYY-MM-DDTHH:MM:SS[.fraction]Z`, UTC `Z` only, no offset. `SshHostSuggestion`: `alias` 1..=300, `hostName` and `user` <=300, `port` 1..=65535, `from` config|known_hosts.
7. `bare_host(h)` (:354): drop a final `:<digits>` (digits may be none), lower-case, drop one trailing `.`. `host_under(h, d)` is `bare == d` or `bare` ends with `.d`. `is_git_forge` is `host_under` any of `GIT_FORGE_HOSTS`, in its order.
8. `parse_place_file(text)` (:428-447): JSON that is an object with string `placeId`, `name`, `hostName`, `hostPublicKey`, `keyPath` and an array of strings `hostUrls`; anything else, invalid JSON included, is None. `joinedAt` is not checked and any value is kept; every other key is kept in order (`rest`).
9. `place_file_text(f)` is `JSON.stringify(f, null, 2)` plus `\n`: two-space indent, `[]` for an empty array, keys in the file's order (rule 4.26: index-like keys of `rest` first, `js_key_order`).
10. The held read (:451-470): the text after the FIRST line that is exactly `WSP_PLACE_HELD` (searching `"\n" + stdout` for `"\nWSP_PLACE_HELD\n"`); none when the marker is missing or nothing follows it. `held_place_file` parses that text. `read_held_place(stdout, home)`: a parse is `Joined`; else `Broken(place_key)` when the text trimmed is exactly `WSP_PLACE_LEFT key`, else `Broken(place_file)`, the paths from `wsp_frames::place_daemon_paths(home)` (`<home>/.wsp/place-key.pem`, `<home>/.wsp/place.json`).
11. `place_file_names(f, ids, key)`: Some file whose place id is in `ids` and whose `hostPublicKey` equals `key` exactly.
12. `engine_word`: docker over podman over none. Every sentence is a plain format of its arguments, copied byte for byte; `plugins_kept_line` says `is`/`it` for one plugin and `are`/`them` otherwise, zero included.

## Edge cases
- `read_held_place("WSP_PLACE_HELD\n", home)` is None; `"WSP_PLACE_HELD\n  WSP_PLACE_LEFT key  "` is the key path.
- `bare_host("github.com.:")` is `github.com`; `bare_host("a:b:c")` is unchanged.
- `two_places_refusal("box", [])` still writes "holds 0 places".

## Behaviour nobody wrote down
- Rule 4.26: `PlaceReport.login` and `agentVersions` are IndexMaps in the order read; `PlaceFile.rest` keeps unknown keys in order.
- Rule 8.3: every cap above counts UTF-16 units.

## Tapes
`place.join`, `place.auth`, `place.prove`, `seal.open`, `device.auth` (chapter 09's link tapes), `places.door`, `places.mint`, `places.sshHosts`. Held out: none counted here.

## Decisions
1. `PlaceFile` is the wire's own struct, not wsp_frames' (which requires a string `joinedAt` and drops extra keys): the TS host keeps both.
2. `JOIN_ADDRESS_LINE` is `join_address_line()` returning a `TwoPartRefusal`, since the struct holds Strings.
3. The base64 types are wsp_frames' and their Check is `Ok(())`.

## After the port
- The held read takes the first marker line, so a login rc file that prints `WSP_PLACE_HELD` itself would be read as the marker.
