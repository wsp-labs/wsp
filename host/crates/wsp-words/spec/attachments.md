# attachments
Source: the functions of `packages/protocol/src/attachments.ts` (261 lines) at a3b870a6e; its zod shapes (`Attachment`, `AttachmentRecord`, `KeptAttachment`) and flat constants (`IMAGE_MAX_BYTES`, `FILE_MAX_BYTES`, `FILES_MAX`, `UNTYPED_FILE`, `IMAGE_TYPE_WORDS`, `IMAGE_MAX_WORDS`, `FILE_MAX_WORDS`, `FILES_DIR`) are `wsp_wire::types::attachments`; `IMAGE_TYPES` is a table in this unit. Tests: `packages/protocol/test/attachments.test.ts` (32 cases, ported whole in `tests/attachments.rs`, the three "run for real" cases through bash in a fresh folder under the temp dir; none skipped). No `pinned` module.

## What it does
One file on its way from a person's message to the agent's turn: what it weighs off its base64, what image type its own bytes say, the caps and the refusals of a message that breaks them or an agent that takes no image, the line a transcript prints for it, and where it lands on a machine, with the shell lines that ready and clear that folder.

## Behaviour
1. `IMAGE_TYPES` is four rows in order: png (word png, ext png), jpeg (jpeg, jpg), gif (gif, gif), webp (webp, webp) (attachments.ts:18-23). `is_image(t)` is true for those four (:49).
2. `attachment_bytes(b64)`: `floor(len / 4) * 3`, less 2 for a trailing "==" or 1 for a trailing "=", never below 0; the text is never decoded (:52-56). `attachment_record(file)`: type, that weight, and the name only where the file carries one (:69-71).
3. `image_type_of(bytes)`: the first row of PNG (8 bytes), JPEG (FF D8 FF), GIF ("GIF8"), WebP ("RIFF" at 0 and "WEBP" at 8) whose bytes match; a byte past the end matches nothing; else None (:84-97).
4. `attachment_line(r)`: an image is "[image <fmt_bytes> <word>]"; any other type "[file <fmt_bytes>]" with " <name>" before the bracket where named (:108-112).
5. `files_refusal(files)`, first wrong thing wins: more than `FILES_MAX` (5) is "only 5 files fit one message; this one carries <n>". Then each file in order, called by its name or "image <i>" / "file <i>" (1-based): 0 bytes is "<at> is empty"; an image over `IMAGE_MAX_BYTES` (10 MiB) is "<at> is <fmt_bytes>, over the <IMAGE_MAX_WORDS> an image may be"; any other file over `FILE_MAX_BYTES` (10 MiB) the same with "a file may be". Else None (:118-128).
6. `steer_files_blocked(files, steers_images, agent)`: none for no files; any non-image is "a file that is not an image goes only with a message that starts a turn"; else None where the agent steers images, "<agent> takes no image into a running turn" where it does not (:133-137).
7. `files_blocked(files, road, harness)`: None for no files; else `files_refusal`, then, where the road is None and any file is an image, `no_images_line(harness)`; else None (:149-152). `no_images_line` (:140-142), `not_a_file_line` "there is no file at <path> on this computer" (:101-103), `files_not_landed_line` (:254-256).
8. A plain id is `^[A-Za-z0-9][A-Za-z0-9._-]*$` (:156). `attachment_key(thread, request, i)`: "<thread>.<request>.<i>" where the request id is present and both ids are plain, else None (:161-163).
9. `thread_files_dir(folder, thread)`: the folder less every trailing "/", then "/.wsp-files/", then the thread id where plain, else "thread" (:198-200). `send_files_dir` adds "/<request id where plain, else minted>" (:204-206); `turn_images_dir` adds "/<same>.images" (:175-177). `image_path_in(dir, i, type)`: "<dir>/<i + 1>.<ext>", an error "<type> is not an image type a message carries" for any other type (:182-186).
10. `safe_file_name(name)`, in order: the last segment after any "/" or "\\" ("" for None); every unit outside `A-Za-z0-9._-` a dash; leading dots and dashes off; nothing left but dots and dashes is "file"; 120 units or fewer is kept; else an extension (the last dot past index 0, the dot and the rest 10 units or fewer) is kept on the first `120 - ext` units (:213-221).
11. `file_path_in(dir, name, taken)`: the safe name, else "2-<name>", "3-<name>" and up, the first not in `taken`; the one landed is added to `taken` (:225-231).
12. `land_files_line(folder, dir)` (:238-242): `cd` into the quoted folder; refuse a `.wsp-files` that is a link; `mkdir -p .wsp-files`; in a checkout whose exclude file is no link, add ".wsp-files/" once (`grep -qxF` before the append), any failure there ignored; refuse a thread folder (the dir up to its last "/") that is a link; `mkdir -p` it; then a bare `mkdir` of the dir, which fails where it already stands. Paths are `shell_quote`d.
13. `drop_files_line(folder, ids)`: only plain ids; none is "true"; else `cd` the folder, refuse a `.wsp-files` that is a link, `rm -rf --` each quoted ".wsp-files/<id>" (:247-250).
14. `attached_files_prompt(prompt, paths)`: the prompt alone for no paths; else the prompt, a blank line, `ATTACHED_FILES_HEAD` ("Attached files:") and one "- <path>" per path (:259-261).

## Edge cases
- `attachment_bytes` of `b64(n)` is n for 1, 2, 3, 4, 5, 1023, 1024 and 1 048 577; of "" is 0 (test asserts). "A" and "==" are 0; "AAAAA" is 3.
- An image of 12 MiB is "image 1 is 12 MB, over the 10 MB an image may be"; exactly 10 MiB goes (test asserts). One byte over reads "image 1 is 10 MB, over the 10 MB an image may be".
- `files_refusal` of a record with type "" and 10 bytes is None: any type travels as a file (test asserts).
- `turn_images_dir(.., request, "minted")` reads "minted" for each of "../../../etc", "a/b", "..", "", ".ssh/../..", "-rf"; "a.b-c_1" is kept (test asserts). `thread_files_dir("/root/spoo", "-rf")` is ".../.wsp-files/thread".
- `thread_files_dir("/", "t")` and `thread_files_dir("", "t")` are both "/.wsp-files/t".
- `safe_file_name`: "../../.ssh/authorized_keys" is "authorized_keys", "C:\\Users\\me\\notes.txt" "notes.txt", ".." "file", ".bashrc" "bashrc", "a b\u{0}c$(rm).txt" "a-b-c--rm-.txt", None "file", 300 "x" then ".log" is 116 "x" then ".log" (test asserts). "é.txt" is "txt"; "a😀b" is "a--b"; a 130-unit name with a 12-unit extension is its first 120 units.
- `file_path_in` with "x" and "2-x" taken lands at "3-x".
- `attachment_key("t", Some("r"), 1.5)` is "t.r.1.5"; `image_path_in("/d", 2.5, "image/png")` is "/d/3.5.png".
- `land_files_line` run twice into one send's dir fails the second time; a planted `.wsp-files` link, a thread folder link and an exclude file link each leave the outside untouched (test asserts).

## Behaviour nobody wrote down
- Rules 8.1 and 8.4: `attachment_bytes` reads the base64's length in UTF-16 units; `FILE_NAME_MAX`, the 10-unit extension and the cut count UTF-16 units.
- Rule 8.5: the name cut never leaves half a surrogate pair.
- Rule 8.7: `safe_file_name`'s class works per UTF-16 unit, so a letter above U+FFFF becomes two dashes; the plain id class is ASCII only.
- Rule 8.11: a weight is `fmt_bytes`; an index and a count in a sentence are `String(n)`, so `number_string`.
- `attachment_key` names the image bytes the host keeps until the thread goes, so its form must read the same after a restart.

## Tapes
The person's turn in a transcript (`[image ...]`, `[file ...]`); the send refusals of the composer, `wsp send` and its tool, and the runtime's turn start; the prompt handed to the agent; the landing and drop commands run on a machine.

## Decisions
1. `IMAGE_TYPES` is a slice of `(media type, word, ext)` in the TS order, not a record.
2. `attachment_bytes` takes the base64 text where the TS takes `{ bytes }`, and answers `f64`.
3. `image_path_in` answers `Result<String, HostError>`: the TS throw is the Err.
4. `AttachmentRoad` (adapter-port.ts:248) is declared here as an enum, `inline` or `file`; `files_blocked` takes `Option<&AttachmentRoad>`.
5. `file_path_in`'s `taken` is a `&mut IndexSet<String>` where the TS takes a `Set<string>`; indexes are `f64`.

## After the port
- `IMAGE_TYPES` is a plain object, so `IMAGE_TYPES[t]` finds `Object.prototype` keys: `is_image("constructor")` and `is_image("toString")` are true in the TS, `attachment_line` of type "toString" prints "[image 5 B undefined]", `image_path_in("/d", 0, "toString")` is "/d/1.undefined", and `files_blocked` refuses such a file as an image. The Rust table holds the four types alone.
- `land_files_line` with a dir holding no "/" cuts its last character for the thread folder (`dir.slice(0, -1)`): "abc" checks and makes "ab". Every caller passes a path under the folder.
- A file 1 byte over a cap is refused as "10 MB, over the 10 MB".

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_words_every_road_reads_rather_than_spelling_again::*`, `what_a_transcript_prints_in_place_of_an_attachment::*` |
| 2 | `what_an_attachment_weighs_from_the_base64_the_wire_carries::*`, `what_a_transcript_keeps_of_an_attachment::*` |
| 3 | `the_type_read_off_the_bytes_never_off_the_name::*` |
| 4 | `what_a_transcript_prints_in_place_of_an_attachment::*` |
| 5 | `the_caps_in_the_person_s_words::*`, `the_words_every_road_reads_rather_than_spelling_again::the_four_types_and_the_caps_are_each_one_export_and_the_refusals_are_built_from` |
| 6 | not pinned: no TS test reads `steerFilesBlocked` |
| 7 | `an_agent_that_reads_no_image_is_named_before_the_machine_is_asked::*`, `the_words_every_road_reads_rather_than_spelling_again::a_path_this_computer_has_no_file_at_answers_in_a_sentence_not_in_the_reader_s_ow`; `files_not_landed_line` not pinned |
| 8 | not pinned: no TS test reads `attachmentKey` |
| 9 | `where_a_thread_s_copies_live_on_a_machine::*`, `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::one_folder_per_thread_under_the_folder_s_wsp_files_and_one_per_send_inside_it_ea` |
| 10 | `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::a_name_is_one_plain_path_segment_whatever_the_client_sent` |
| 11 | `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::two_files_of_one_name_in_one_send_each_keep_their_own_path` |
| 12 | `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::run_for_real_in_a_checkout_the_landing_keeps_git_s_status_clean_through_the_copy`, `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::run_for_real_it_lands_in_a_folder_that_is_no_checkout_and_refuses_every_link_tha` |
| 13 | `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::run_for_real_dropping_a_thread_s_files_takes_its_folder_alone_and_a_wsp_files_th` |
| 14 | `where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else::the_agent_s_prompt_names_every_landed_path_under_the_words_the_person_typed` |
