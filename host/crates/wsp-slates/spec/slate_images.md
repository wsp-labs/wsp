# slate_images
Source: packages/runtime/src/slate-images.ts (138 lines) and slate-image-fetch.ts (180 lines) at 65f6b89c7. TS tests: `test/slate-images.test.ts` (395 lines, 33 cases counting the 22 rows of its `it.each`), `test/slate-box.test.ts:660-696` (an image on a box, through the runtime). Rust: `src/slate_images.rs`, `src/slate_image_fetch.rs`; tests `tests/slate_images.rs`.

## What it does
Reads a slate's image piece for the window: a path on this computer's disk or, through its daemon, on the thread's other computer; or an http(s) address the host fetches once the person allows its domain. Only a regular file whose bytes are PNG, JPEG, GIF or WebP and that weighs at most 10 MiB (SLATE_IMAGE_MAX_BYTES = 10,485,760) shows. Each answer carries a version; a window that holds that version hears `unchanged`. A thread reads at most four images at once.

## Behaviour
1. `slate_image(src, roads)` (`slate-images.ts:226-239`): `slate_image_source(src)` (slates-a's `pending`) gives a problem, an address `{url, domain}` or a path.
   - address: not `allowed(domain)` answers `{ask: {domain}}` and fetches nothing; else `kept(have, fetch)` inside the thread's turn.
   - path: whole when, with no `on`, it is absolute on this computer, or with `on`, it starts with `/`. Not whole and no folder: R903 `<path> is not a whole path, and this host knows no folder for the thread to read it under`. A relative path is joined under the folder (`posix_join` there, `resolve` here). Read inside the thread's turn by the daemon (`on_daemon`) or here (`read_here`). The shown name is the path as the slate wrote it.
2. Turns (`:211-224`): per thread at most IMAGES_AT_ONCE = 4 reads run; a fifth waits in a FIFO; a finishing read hands its slot straight to the next waiter; the thread's entry goes when none run. Other threads are not limited by this one. The state is process-wide (one map for the host), as the TS module's.
3. `kept(have, answer)`: a shown answer whose version equals `have` becomes `{unchanged: true, version}`.
4. `version_of(modified_ms, size, inode?, changed_ns?)`: `round(modified_ms):size[:inode][:changed_ns]`, joined by `:`. Here all four are given (a whole-ms mtime, the size, the inode, the ctime in ns), so a `cp -p` over the file moves the version; from a daemon only `modified:size` plus what it sends.
5. `read_here(shown, path, have)` (`:254-293`): open read-only and non-blocking (a fifo cannot hold the read). Open errors: ENOENT or ENOTDIR R900 `no file at <shown>`; else when a stat says not a regular file R914 `<shown> is not a file; an image is a regular file`; else R903 `<shown> could not be opened: <code>`. On the open handle: not a regular file R914; size over the cap R915 `slate_image_too_big(shown, size)`; version; equal to `have` answers unchanged without reading; else read into a buffer of `size + 1` bytes, doubling (capped at cap + 1) while full; more than the cap read R915 `slate_image_too_big(shown)` (no size). The buffer held is what the file weighs plus one, never the cap (TS spies `Buffer.alloc`; Rust pins it with a reader that records its largest buffer, see Decisions). Bytes typed by `image_type_of`; none: R916 `slate_not_an_image(shown, slate_looks_svg(shown, bytes))`. Shown: `{mediaType, bytes: base64, version}`. The handle is always closed.
6. `on_daemon(shown, path, on, have)` (`:296-312`): `on.read(path)`; a refusal with code `not-found` R900, `not-a-file` R914, the message `unknown_op_line("fs.image")` R903 `<name> runs a daemon too old to read images; <place_update_line(name)>` (`wsp add dev4 --update updates it`), anything else R903 `<name> could not read <shown>: <message>`. Reply size over the cap R915 with the size; no media type or no content R916 with `svg == true || slate_looks_svg(shown, empty)`; version `modified:size[:inode][:changed]` only when `modified` is present; then `kept`.
7. `local_address(address)` (`slate-image-fetch.ts:367-374`): not an IP (Node's `isIP`, which takes an IPv6 zone `%eth0`) `is not an address`; IPv4 in a local block `is this computer or its network`; IPv6 carrying an IPv4 (rule 8) is judged as that IPv4; else IPv6 in a local block; else `None`. IPv4 blocks: 0/8, 10/8, 100.64/10, 127/8, 169.254/16, 172.16/12, 192.0.0/24, 192.168/16, 198.18/15, 224/4, 240/4. IPv6 blocks: ::/128, ::1/128, fc00::/7, fe80::/10, fec0::/10, ff00::/8, 100::/64, 64:ff9b:1::/48.
8. `carried(v6)` (`:353-364`), groups read with a dotted IPv4 tail as the last two: first six zero and not (g6 = 0 and g7 <= 1): g6.g7; first five zero and g5 = ffff (mapped); first four zero, g4 = ffff, g5 = 0 (translated); `64:ff9b` and groups 2-5 zero (NAT64); `2002` (6to4): g1.g2. So `::2` carries 0.0.0.2 and is local; `::` and `::1` fall to the IPv6 blocks.
9. `fetch_slate_image(href, domain, road)` (`:451-492`), one deadline of `road.timeout_ms` (default 15,000) over the whole fetch, at most 5 redirects. Each hop, in order: a protocol other than http/https R917 `slate_address_refused(href, "leads to <protocol> by a redirect, which wsp does not follow")`; a lower-cased hostname other than `domain` R917 `redirects to <hostname>, a domain other than the <domain> you allowed`; a username or password R917 `redirects to an address with a login, which wsp never sends`; vet: an IP literal (brackets off) is its own address, else `road.lookup(host)` raced against the deadline (a lookup failure reads as no addresses); none R917 `names <host>, which resolves to nothing`; any refused address R917 on the hop's url `resolves to <address>, which <why>; a slate fetches images from the internet alone`; the first address is dialled, pinned, so a second resolve cannot move the socket (lookup called once per hop). GET with no agent pool, headers exactly `user-agent: wsp`, `accept: image/png,image/jpeg,image/gif,image/webp` (IMAGE_TYPES keys joined by `,`), plus what the client adds (`host`, `connection`); no cookie, no login.
   - 3xx with `location`: past 5 redirects R917 `redirects more than 5 times` (on `href`); else resolve `location` against the url and loop.
   - non-2xx: R903 `<domain> answered <status> for <href>`.
   - a declared content-length over the cap: R915 `slate_image_too_big(href, length)` unread.
   - the body read to the cap: one byte past stops the socket and answers R915 `slate_image_too_big(href)`.
   - typed: none R916 `slate_not_an_image(href, slate_looks_svg(url.pathname, bytes))`; else `{mediaType, bytes, version: etag else last-modified, when present}`.
   - the deadline: R903 `<domain> did not answer within <round(timeout/1000)> s for <href>` (a 500 ms deadline says `1 s`); any other error R903 `<href> could not be read: <message>`. No task, timer or listener outlives the call (TS: no unhandled rejection).
10. `FETCH_ROAD`: the system resolver (all addresses, order as given), `local_address`, 15,000 ms.

## Edge cases (TS words pinned in the tests)
- R916 for a text named .png: `<path> is not a PNG, JPEG, GIF or WebP image`; an SVG by name or bytes: `<path> is an SVG, which a slate does not show since it can load other things; save it as a PNG`.
- R915 one byte over: `<path> is 10.1 MB, over the 10 MB an image may weigh; save it smaller, or show a part of it`; a file of exactly the cap shows.
- `/dev/null`, `/dev/zero`, a fifo, a socket, a folder: R914.
- Empty src: `R900 an image names no file...`; `~/x`: `R900 ~/shots/a.png starts at a home folder, which wsp does not guess; write the whole path`; `file:` and `data:` R917; a login in the address R917 `https://me:secret@acme.test/a.png carries a login, which wsp never sends` (all from `slate_image_source`).
- A version string from this disk matches `^\d+:<size>:\d+:\d+$`.

## Decisions
1. The turn map is a `thread_local!` (one thread, rule 3.1), the TS's module-level map.
2. `FetchRoad` is a trait (`lookup`, `refuses`, `timeout_ms`), so tests reach a local server under made-up names.
3. Rule 5's buffer is pinned in its own test binary (`tests/slate_images_alloc.rs`) with a counting global allocator: reading a 70-byte PNG makes no single allocation of 1 MiB or more, where a reader sized by the cap would make one of 10 MiB.

## After the port
- A daemon reply with no `modified` carries no version, so a window can never be told unchanged for it; kept.
- `slate_looks_svg(shown, empty bytes)` for a daemon's untyped reply judges by name alone unless the daemon set `svg`; kept.

## Tests (`tests/slate_images.rs`, `tests/slate_images_alloc.rs`)
| Rule | Test |
|---|---|
| 1, 5 | `shows_a_real_image_of_each_type_by_whole_path_and_relative_under_the_folder`, `refuses_a_text_file_named_png_by_its_bytes`, `refuses_an_svg_by_name_or_bytes`, `refuses_a_file_over_10_mb_with_its_size`, `refuses_a_folder_a_device_a_pipe_and_a_socket_without_waiting_on_the_pipe`, `says_one_byte_over_as_10_1_mb`, `refuses_a_missing_file_a_path_through_a_file_an_empty_src_and_a_home_path`, `refuses_an_address_that_is_not_http_or_carries_a_login` |
| 5 (buffer) | `holds_what_the_file_weighs_not_the_cap` (alloc binary) |
| 3, 4 | `answers_unchanged_for_the_held_version_and_the_new_picture_after_a_write`, `shows_a_same_size_picture_copied_with_cp_p` |
| 6 | `reads_a_whole_path_there_and_a_relative_one_under_the_folder_there`, `names_an_svg_the_same_on_another_computer`, `answers_unchanged_for_a_daemon_version`, `says_each_daemon_refusal_and_names_the_update` |
| 2 | `reads_at_most_four_of_one_threads_images_at_once` |
| 7, 8 | `refuses_each_local_address` (the TS table's 22 rows, both with the test road and the default road), `local_address_reads_carried_and_zoned_addresses` |
| 9 | `is_asked_for_and_fetches_nothing_until_allowed`, `sends_no_cookie_and_no_login`, `refuses_a_name_resolving_to_any_private_address`, `dials_the_address_it_checked`, `follows_a_redirect_within_the_domain_and_refuses_others`, `checks_the_address_again_at_a_redirect`, `stops_at_10_mb_as_bytes_stream_and_refuses_a_declared_length`, `leaves_nothing_running_after_a_deadline`, `gives_up_on_a_lookup_that_never_answers`, `carries_the_etag_as_version`, `gives_up_on_a_silent_server`, `refuses_what_is_not_an_image_an_svg_and_a_failed_status` |
