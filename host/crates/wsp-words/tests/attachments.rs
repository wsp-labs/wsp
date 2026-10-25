// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/attachments.test.ts whole (:39-317), the bash runs of the three "run for real" cases
//! (:227-298) through std::process::Command in a unique folder under the temp dir.

use std::path::{Path, PathBuf};
use std::process::Command;

use indexmap::IndexSet;
use serde_json::{json, Value};
use wsp_wire::types::attachments::{
    Attachment, AttachmentRecord, FILES_DIR, FILES_MAX, FILE_MAX_BYTES, FILE_MAX_WORDS, IMAGE_MAX_BYTES, IMAGE_MAX_WORDS,
    IMAGE_TYPE_WORDS,
};
use wsp_words::attachments::*;

fn b64(bytes: usize) -> String {
    let mut out = "AAAA".repeat(bytes / 3);
    match bytes % 3 {
        1 => out.push_str("AA=="),
        2 => out.push_str("AAA="),
        _ => {}
    }
    out
}

fn png(bytes: usize) -> Value {
    json!({ "mediaType": "image/png", "bytes": b64(bytes) })
}

fn pdf(bytes: usize, name: &str) -> Value {
    json!({ "mediaType": "application/pdf", "bytes": b64(bytes), "name": name })
}

fn with_name(mut file: Value, name: &str) -> Value {
    file["name"] = json!(name);
    file
}

fn file(v: Value) -> Attachment {
    serde_json::from_value(v).unwrap()
}

fn record(v: Value) -> AttachmentRecord {
    serde_json::from_value(v).unwrap()
}

fn kept(v: Value) -> AttachmentRecord {
    attachment_record(&file(v))
}

const MB: usize = 1024 * 1024;

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("wsp-files-{}-{nanos}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn s(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn bash(line: &str) -> Option<i32> {
    Command::new("bash").arg("-c").arg(line).status().unwrap().code()
}

fn git(copy: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(copy).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

mod what_an_attachment_weighs_from_the_base64_the_wire_carries {
    use super::*;

    #[test]
    fn counts_the_bytes_back_out_of_the_base64_without_decoding_it_padding_and_all() {
        for size in [1usize, 2, 3, 4, 5, 1023, 1024, 1_048_577] {
            assert_eq!(attachment_bytes(&b64(size)), size as f64);
        }
        assert_eq!(attachment_bytes(""), 0.0);
    }
}

mod the_caps_in_the_person_s_words {
    use super::*;

    #[test]
    fn a_12_mb_image_is_refused_with_its_weight_and_the_cap_in_the_sentence() {
        let refusal = files_refusal(&[kept(png(12 * MB))]);
        assert_eq!(refusal.as_deref(), Some("image 1 is 12 MB, over the 10 MB an image may be"));
        // The cap itself is the one in the sentence, so the words cannot drift from the rule.
        assert!(refusal.as_deref().unwrap().contains("10 MB"), "{refusal:?}");
        assert_eq!(IMAGE_MAX_BYTES, (10 * MB) as u64);
    }

    #[test]
    fn a_file_the_person_named_is_refused_by_its_own_name_not_by_its_place_in_the_messa() {
        assert_eq!(
            files_refusal(&[kept(with_name(png(12 * MB), "screenshot.png"))]).as_deref(),
            Some("screenshot.png is 12 MB, over the 10 MB an image may be")
        );
    }

    #[test]
    fn one_image_under_the_cap_goes() {
        assert_eq!(files_refusal(&[kept(png(10 * MB))]), None);
        assert_eq!(files_refusal(&[]), None);
    }

    #[test]
    fn six_files_are_refused_with_both_counts_images_and_the_rest_counted_together() {
        let mut six: Vec<AttachmentRecord> = (0..FILES_MAX).map(|_| kept(png(1024))).collect();
        six.push(kept(pdf(1024, "report.pdf")));
        assert_eq!(files_refusal(&six).as_deref(), Some("only 5 files fit one message; this one carries 6"));
    }

    #[test]
    fn a_file_over_its_cap_is_refused_in_one_sentence_naming_it_its_weight_and_the_cap() {
        let refusal = files_refusal(&[kept(pdf(FILE_MAX_BYTES as usize + 2 * MB, "manual.pdf"))]);
        assert_eq!(refusal.as_deref(), Some(format!("manual.pdf is 12 MB, over the {FILE_MAX_WORDS} a file may be").as_str()));
        assert_eq!(regex::Regex::new(r"[.;]\s").unwrap().split(refusal.as_deref().unwrap()).count(), 1);
        assert_eq!(files_refusal(&[kept(pdf(FILE_MAX_BYTES as usize, "report.pdf"))]), None);
    }

    #[test]
    fn any_type_travels_as_a_file_and_one_with_no_name_is_called_by_its_place() {
        assert_eq!(files_refusal(&[kept(pdf(1024, "report.pdf")), kept(json!({ "mediaType": "", "bytes": b64(10) }))]), None);
        assert_eq!(
            files_refusal(&[kept(json!({ "mediaType": "text/plain", "bytes": "" }))]).as_deref(),
            Some("file 1 is empty")
        );
    }

    #[test]
    fn an_empty_image_is_refused_rather_than_sent_as_nothing() {
        assert_eq!(
            files_refusal(&[kept(json!({ "mediaType": "image/png", "bytes": "" }))]).as_deref(),
            Some("image 1 is empty")
        );
    }

    #[test]
    fn the_first_thing_wrong_is_the_whole_answer_the_count_before_any_one_image() {
        let mut rows = vec![kept(png(12 * MB))];
        rows.extend((0..FILES_MAX).map(|_| kept(png(1024))));
        let refusal = files_refusal(&rows);
        assert!(refusal.as_deref().unwrap_or("").contains("only 5 files fit one message"), "{refusal:?}");
    }
}

mod an_agent_that_reads_no_image_is_named_before_the_machine_is_asked {
    use super::*;

    #[test]
    fn names_the_agent_when_its_adapter_declared_no_road() {
        assert_eq!(files_blocked(&[kept(png(1024))], None, "gemini"), Some(no_images_line("gemini")));
        assert!(no_images_line("gemini").contains("gemini"), "{}", no_images_line("gemini"));
    }

    #[test]
    fn says_nothing_when_the_message_carries_no_image_whatever_the_agent_reads() {
        assert_eq!(files_blocked(&[], None, "gemini"), None);
        // A file that is not an image lands in the thread's folder, which every agent reads.
        assert_eq!(files_blocked(&[kept(pdf(1024, "report.pdf"))], None, "gemini"), None);
    }

    #[test]
    fn the_caps_come_first_so_a_person_fixes_the_image_rather_than_the_agent() {
        let blocked = files_blocked(&[kept(png(12 * MB))], None, "gemini");
        assert!(blocked.as_deref().unwrap_or("").contains("over the 10 MB"), "{blocked:?}");
    }

    #[test]
    fn an_agent_on_either_road_takes_them() {
        assert_eq!(files_blocked(&[kept(png(1024))], Some(&AttachmentRoad::Inline), "claude"), None);
        assert_eq!(files_blocked(&[kept(png(1024))], Some(&AttachmentRoad::File), "codex"), None);
    }
}

mod what_a_transcript_prints_in_place_of_an_attachment {
    use super::*;

    #[test]
    fn a_file_is_its_weight_and_its_name_in_one_bracket() {
        assert_eq!(
            attachment_line(&record(json!({ "mediaType": "application/pdf", "bytes": 4096, "name": "report.pdf" }))),
            "[file 4 KB report.pdf]"
        );
        assert_eq!(attachment_line(&record(json!({ "mediaType": "text/plain", "bytes": 900 }))), "[file 900 B]");
    }

    #[test]
    fn an_image_is_the_weight_and_the_type_in_one_bracket() {
        assert_eq!(attachment_line(&record(json!({ "mediaType": "image/png", "bytes": 1_258_291 }))), "[image 1 MB png]");
        assert_eq!(attachment_line(&record(json!({ "mediaType": "image/jpeg", "bytes": 4096 }))), "[image 4 KB jpeg]");
        assert_eq!(attachment_line(&record(json!({ "mediaType": "image/webp", "bytes": 900 }))), "[image 900 B webp]");
    }
}

mod the_type_read_off_the_bytes_never_off_the_name {
    use super::*;

    fn head(bytes: &[u8]) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out.extend([0u8; 16]);
        out
    }

    #[test]
    fn tells_the_four_apart_by_their_own_first_bytes() {
        assert_eq!(image_type_of(&head(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])).as_deref(), Some("image/png"));
        assert_eq!(image_type_of(&head(&[0xff, 0xd8, 0xff, 0xe0])).as_deref(), Some("image/jpeg"));
        assert_eq!(image_type_of(&head(&[0x47, 0x49, 0x46, 0x38, 0x39, 0x61])).as_deref(), Some("image/gif"));
        assert_eq!(
            image_type_of(&[0x52, 0x49, 0x46, 0x46, 1, 2, 3, 4, 0x57, 0x45, 0x42, 0x50]).as_deref(),
            Some("image/webp")
        );
    }

    #[test]
    fn a_riff_that_is_not_webp_and_anything_else_is_none_of_them() {
        assert_eq!(image_type_of(&[0x52, 0x49, 0x46, 0x46, 1, 2, 3, 4, 0x57, 0x41, 0x56, 0x45]), None);
        assert_eq!(image_type_of(&head(&[0x25, 0x50, 0x44, 0x46])), None);
        assert_eq!(image_type_of(&[]), None);
    }
}

mod where_a_thread_s_copies_live_on_a_machine {
    use super::*;

    const FILES: &str = "/home/dev/app/.wsp-files/thr_1";

    #[test]
    fn one_folder_per_send_beside_its_files_in_the_thread_s_folder_and_one_name_per_ima() {
        let dir = turn_images_dir("/home/dev/app/", "thr_1", Some("req_a"), "minted");
        assert_eq!(dir, format!("{FILES}/req_a.images"));
        assert_eq!(image_path_in(&dir, 0.0, "image/png").unwrap(), format!("{FILES}/req_a.images/1.png"));
        assert_eq!(image_path_in(&dir, 1.0, "image/jpeg").unwrap(), format!("{FILES}/req_a.images/2.jpg"));
        assert_eq!(
            image_path_in(&turn_images_dir("/home/dev/app", "thr_2", Some("req_b"), "minted"), 0.0, "image/webp").unwrap(),
            "/home/dev/app/.wsp-files/thr_2/req_b.images/1.webp"
        );
    }

    #[test]
    fn two_sends_of_one_thread_never_share_a_folder_and_every_send_s_folder_is_under_th() {
        assert_ne!(
            turn_images_dir("/home/dev/app", "thr_1", Some("req_a"), "m"),
            turn_images_dir("/home/dev/app", "thr_1", Some("req_b"), "m")
        );
        for request_id in [Some("req_a"), None] {
            let dir = turn_images_dir("/home/dev/app", "thr_1", request_id, "minted");
            assert!(dir.starts_with(&format!("{}/", thread_files_dir("/home/dev/app", "thr_1"))), "{dir}");
        }
    }

    #[test]
    fn a_send_with_no_request_id_takes_the_minted_name_since_a_shared_folder_would_lose() {
        assert_eq!(turn_images_dir("/home/dev/app", "thr_1", None, "minted"), format!("{FILES}/minted.images"));
    }

    #[test]
    fn a_request_id_shaped_like_a_path_is_not_used_as_one_it_is_a_client_s_string_and_t() {
        for nasty in ["../../../etc", "a/b", "..", "", ".ssh/../..", "-rf"] {
            assert_eq!(turn_images_dir("/home/dev/app", "thr_1", Some(nasty), "minted"), format!("{FILES}/minted.images"));
        }
        // A plain id, whatever its shape otherwise, is the folder's name.
        assert_eq!(turn_images_dir("/home/dev/app", "thr_1", Some("a.b-c_1"), "minted"), format!("{FILES}/a.b-c_1.images"));
    }

    #[test]
    fn a_type_outside_the_table_has_no_name_here_the_caps_refusal_is_what_turns_it_away() {
        let e = image_path_in("/tmp/x", 0.0, "application/pdf").unwrap_err();
        assert_eq!(e.message, "application/pdf is not an image type a message carries");
    }
}

mod the_words_every_road_reads_rather_than_spelling_again {
    use super::*;

    #[test]
    fn the_four_types_and_the_caps_are_each_one_export_and_the_refusals_are_built_from() {
        assert_eq!(IMAGE_TYPE_WORDS, "PNG, JPEG, GIF or WebP");
        assert_eq!(
            IMAGE_TYPES.iter().map(|(media_type, _, _)| *media_type).collect::<Vec<_>>(),
            ["image/png", "image/jpeg", "image/gif", "image/webp"]
        );
        assert_eq!(IMAGE_MAX_WORDS, "10 MB");
        assert_eq!(FILE_MAX_WORDS, "10 MB");
        let refusal = files_refusal(&[kept(json!({ "mediaType": "image/png", "bytes": b64(12 * MB) }))]);
        assert!(refusal.as_deref().unwrap_or("").contains(IMAGE_MAX_WORDS), "{refusal:?}");
    }

    #[test]
    fn a_path_this_computer_has_no_file_at_answers_in_a_sentence_not_in_the_reader_s_ow() {
        assert_eq!(not_a_file_line("/tmp/nope.png"), "there is no file at /tmp/nope.png on this computer");
    }
}

mod where_a_file_the_person_attached_lands_inside_the_thread_s_own_folder_and_nowhere_else {
    use super::*;

    #[test]
    fn one_folder_per_thread_under_the_folder_s_wsp_files_and_one_per_send_inside_it_ea() {
        assert_eq!(FILES_DIR, ".wsp-files");
        assert_eq!(thread_files_dir("/root/spoo", "thr_1"), "/root/spoo/.wsp-files/thr_1");
        assert_eq!(send_files_dir("/root/spoo", "thr_1", Some("req_a"), "minted"), "/root/spoo/.wsp-files/thr_1/req_a");
        for nasty in ["../../../etc", "a/b", "..", "", "-rf"] {
            assert_eq!(send_files_dir("/root/spoo", "thr_1", Some(nasty), "minted"), "/root/spoo/.wsp-files/thr_1/minted");
            assert_eq!(thread_files_dir("/root/spoo", nasty), "/root/spoo/.wsp-files/thread");
        }
    }

    #[test]
    fn a_name_is_one_plain_path_segment_whatever_the_client_sent() {
        assert_eq!(safe_file_name(Some("report.pdf")), "report.pdf");
        assert_eq!(safe_file_name(Some("../../.ssh/authorized_keys")), "authorized_keys");
        assert_eq!(safe_file_name(Some("C:\\Users\\me\\notes.txt")), "notes.txt");
        assert_eq!(safe_file_name(Some("..")), "file");
        assert_eq!(safe_file_name(Some(".bashrc")), "bashrc");
        assert_eq!(safe_file_name(Some("a b\u{0}c$(rm).txt")), "a-b-c--rm-.txt");
        assert_eq!(safe_file_name(None), "file");
        assert_eq!(safe_file_name(Some(&format!("{}.log", "x".repeat(300)))), format!("{}.log", "x".repeat(116)));
    }

    #[test]
    fn two_files_of_one_name_in_one_send_each_keep_their_own_path() {
        let dir = "/root/spoo/.wsp-files/req_a";
        let mut taken: IndexSet<String> = IndexSet::new();
        assert_eq!(file_path_in(dir, Some("notes.txt"), &mut taken), format!("{dir}/notes.txt"));
        assert_eq!(file_path_in(dir, Some("notes.txt"), &mut taken), format!("{dir}/2-notes.txt"));
        for name in ["../x", "/etc/passwd", "..", "a/../../b"] {
            let landed = file_path_in(dir, Some(name), &mut taken);
            assert!(landed.starts_with(&format!("{dir}/")), "{landed}");
        }
    }

    #[test]
    fn run_for_real_in_a_checkout_the_landing_keeps_git_s_status_clean_through_the_copy() {
        let root = Tmp::new();
        let copy = root.join("my copy");
        std::fs::create_dir(&copy).unwrap();
        git(&copy, &["init", "-q"]);
        let copy_s = s(&copy);
        let run = |dir: String| bash(&land_files_line(&copy_s, &dir));
        assert_eq!(run(send_files_dir(&copy_s, "thr_1", Some("req_a"), "m")), Some(0));
        assert_eq!(run(send_files_dir(&copy_s, "thr_1", Some("req_b"), "m")), Some(0));
        std::fs::write(Path::new(&send_files_dir(&copy_s, "thr_1", Some("req_a"), "m")).join("report.pdf"), "%PDF").unwrap();
        assert_eq!(git(&copy, &["status", "--porcelain"]), "");
        let exclude = std::fs::read_to_string(copy.join(".git").join("info").join("exclude")).unwrap();
        assert_eq!(exclude.split('\n').filter(|line| *line == ".wsp-files/").count(), 1);
        assert!(!copy.join(".gitignore").exists());
        assert!(!copy.join(".wsp-files").join(".gitignore").exists());
        // The same send's folder is never reused: a second landing into it is refused.
        assert_ne!(run(send_files_dir(&copy_s, "thr_1", Some("req_a"), "m")), Some(0));
    }

    #[test]
    fn run_for_real_it_lands_in_a_folder_that_is_no_checkout_and_refuses_every_link_tha() {
        use std::os::unix::fs::symlink;
        let root = Tmp::new();
        let land = |folder: &Path, request_id: &str| {
            let folder = s(folder);
            bash(&land_files_line(&folder, &send_files_dir(&folder, "thr_1", Some(request_id), "m")))
        };
        let plain = root.join("plain");
        std::fs::create_dir(&plain).unwrap();
        assert_eq!(land(&plain, "req_a"), Some(0));
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let planted = root.join("planted");
        std::fs::create_dir(&planted).unwrap();
        symlink(&outside, planted.join(".wsp-files")).unwrap();
        assert_ne!(land(&planted, "req_b"), Some(0));
        let thread = root.join("thread");
        std::fs::create_dir_all(thread.join(".wsp-files")).unwrap();
        symlink(&outside, thread.join(".wsp-files").join("thr_1")).unwrap();
        assert_ne!(land(&thread, "req_c"), Some(0));
        // An exclude file planted as a link is not written through.
        let linked = root.join("linked");
        std::fs::create_dir(&linked).unwrap();
        git(&linked, &["init", "-q"]);
        let bait = outside.join("bait");
        std::fs::write(&bait, "").unwrap();
        let _ = std::fs::remove_file(linked.join(".git").join("info").join("exclude"));
        symlink(&bait, linked.join(".git").join("info").join("exclude")).unwrap();
        land(&linked, "req_d");
        assert_eq!(std::fs::read_to_string(&bait).unwrap(), "");
        let mut left: Vec<String> =
            std::fs::read_dir(&outside).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["bait"]);
    }

    #[test]
    fn run_for_real_dropping_a_thread_s_files_takes_its_folder_alone_and_a_wsp_files_th() {
        use std::os::unix::fs::symlink;
        let root = Tmp::new();
        let copy = root.join("copy");
        let copy_s = s(&copy);
        for thread in ["thr_1", "thr_2"] {
            std::fs::create_dir_all(send_files_dir(&copy_s, thread, Some("req_a"), "m")).unwrap();
        }
        assert_eq!(bash(&drop_files_line(&copy_s, &["thr_1", "../../outside"])), Some(0));
        let left: Vec<String> =
            std::fs::read_dir(copy.join(".wsp-files")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["thr_2"]);
        let outside = root.join("outside");
        std::fs::create_dir_all(outside.join("thr_2")).unwrap();
        let planted = root.join("planted");
        std::fs::create_dir(&planted).unwrap();
        symlink(&outside, planted.join(".wsp-files")).unwrap();
        bash(&drop_files_line(&s(&planted), &["thr_2"]));
        assert!(outside.join("thr_2").exists());
    }

    #[test]
    fn the_agent_s_prompt_names_every_landed_path_under_the_words_the_person_typed() {
        assert_eq!(
            attached_files_prompt(
                "what is in these?",
                &["/root/spoo/.wsp-files/req_a/report.pdf", "/root/spoo/.wsp-files/req_a/pasted-text-1.txt"]
            ),
            "what is in these?\n\nAttached files:\n- /root/spoo/.wsp-files/req_a/report.pdf\n- /root/spoo/.wsp-files/req_a/pasted-text-1.txt"
        );
        assert_eq!(attached_files_prompt("hi", &[] as &[&str]), "hi");
    }
}

mod what_a_transcript_keeps_of_an_attachment {
    use super::*;

    #[test]
    fn its_type_its_weight_and_its_name_never_its_pixels() {
        assert_eq!(
            attachment_record(&file(json!({ "mediaType": "image/png", "bytes": b64(2048), "name": "shot.png" }))),
            record(json!({ "mediaType": "image/png", "bytes": 2048, "name": "shot.png" }))
        );
        assert_eq!(
            attachment_record(&file(json!({ "mediaType": "image/png", "bytes": b64(2048) }))),
            record(json!({ "mediaType": "image/png", "bytes": 2048 }))
        );
    }
}
