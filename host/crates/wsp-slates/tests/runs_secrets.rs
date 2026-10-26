// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_secrets: spec/runs_secrets.md, slate-runs.test.ts `describe("secrets")` (the cases that test the store
//! directly) and the oracle's `secretForms.*`, `forms.middle` and `scrub.*`. File modes depend on the umask, so the
//! tests that read them set it first.
mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::rc::Rc;

use support::{temp_root, ManualClock, THREAD};
use wsp_slates::slate_runs::{
    create_secrets, encode_uri_component, secret_forms, DefaultApprovals, RunApprovals, SecretHandle, SlateSecrets,
};

const TOKEN: &str = "vrcl_Ab+9/zz&q=1 \u{fc}n\u{ef}_0123456789";

fn vec_of(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn handle_json(h: &SecretHandle) -> String {
    serde_json::to_string(h).unwrap()
}

fn secrets_file(root: &std::path::Path) -> PathBuf {
    root.join("state").join("slates.secrets.json")
}

fn mode_of(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

mod secrets {
    use super::*;

    // runs_secrets Edge case: slate-runs.test.ts:179
    #[test]
    fn an_empty_text_clears_one_a_short_one_is_still_scrubbed_and_other_threads_secrets() {
        let clock = ManualClock::new(1_000);
        let s = create_secrets(None, clock.clock());
        s.set(THREAD, "pin", "4321", false);
        s.set("t2", "other", "elsewhere-secret", false);
        assert_eq!(s.scrub(THREAD, "pin 4321 elsewhere-secret"), "pin [secret:pin] elsewhere-secret");
        let cleared = s.set(THREAD, "pin", "", false);
        assert_eq!(cleared, SecretHandle { secret: true, set: false, len: 0, at: None });
        assert_eq!(s.scrub(THREAD, "4321"), "4321");
    }

    // runs_secrets Edge case: slate-runs.test.ts:188
    #[test]
    fn only_kept_ones_reach_the_file_at_0600_and_a_new_factory_reads_them_back() {
        unsafe {
            libc::umask(0o022);
        }
        let root = temp_root("secrets-");
        let file = secrets_file(root.path());
        let clock = ManualClock::new(1_000);
        let s = create_secrets(Some(file.clone()), clock.clock());
        s.set(THREAD, "kept", "kept-secret-value", true);
        s.set(THREAD, "memory", "memory-secret-value", false);
        assert_eq!(mode_of(&file), 0o600);
        assert_eq!(mode_of(&root.path().join("state")), 0o700);
        assert!(!std::fs::read_to_string(&file).unwrap().contains("memory-secret-value"));
        let again = create_secrets(Some(file.clone()), clock.clock());
        assert!(again.handle(THREAD, "kept").set);
        assert!(!again.handle(THREAD, "memory").set);
        again.forget(THREAD, None);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
    }
}

// runs_secrets Behaviour 1: a file that exists is read as {thread: {name: {text, at}}}, every secret held as kept
#[test]
fn create_secrets_reads_the_kept_file_and_holds_each_as_kept() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, r#"{"t1":{"a":{"text":"abc","at":5},"b":{"text":"de","at":6}},"t2":{"c":{"text":"x","at":7}}}"#).unwrap();
    let s = create_secrets(Some(file.clone()), ManualClock::new(9_000).clock());
    assert_eq!(handle_json(&s.handle("t1", "a")), r#"{"secret":true,"set":true,"len":3,"at":5}"#);
    assert_eq!(handle_json(&s.handle("t1", "b")), r#"{"secret":true,"set":true,"len":2,"at":6}"#);
    assert_eq!(handle_json(&s.handle("t2", "c")), r#"{"secret":true,"set":true,"len":1,"at":7}"#);
    assert_eq!(s.plaintext("t1", "a"), Some("abc".to_string()));
    s.forget("t1", Some("a"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), r#"{"t1":{"b":{"text":"de","at":6}},"t2":{"c":{"text":"x","at":7}}}"#);
}

// runs_secrets Behaviour 1 / Edge case: oracle json.parse (padding around the JSON is fine)
#[test]
fn create_secrets_reads_a_file_with_whitespace_around_its_json() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, " {\"t1\":{\"a\":{\"text\":\"abc\",\"at\":5}}}\n").unwrap();
    let s = create_secrets(Some(file), ManualClock::new(9_000).clock());
    assert_eq!(handle_json(&s.handle("t1", "a")), r#"{"secret":true,"set":true,"len":3,"at":5}"#);
}

// runs_secrets Behaviour 1 / Edge case: a kept file that does not parse holds nothing and is not rewritten
#[test]
fn a_kept_file_that_does_not_parse_holds_nothing_and_is_left_as_it_is() {
    for body in ["not json at all", "", "{\"t1\":", "[1,2]x"] {
        let root = temp_root("secrets-");
        let file = secrets_file(root.path());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, body).unwrap();
        let s = create_secrets(Some(file.clone()), ManualClock::new(9_000).clock());
        assert_eq!(s.handle("t1", "a"), SecretHandle { secret: true, set: false, len: 0, at: None }, "{body:?}");
        assert_eq!(s.plaintext("t1", "a"), None);
        assert_eq!(s.scrub("t1", "anything"), "anything");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), body);
    }
}

// runs_secrets Behaviour 1: a file that does not exist holds nothing and is not made until something is saved
#[test]
fn create_secrets_with_a_missing_file_holds_nothing_and_writes_nothing() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(9_000).clock());
    assert!(!s.handle(THREAD, "a").set);
    assert!(!file.exists());
}

// runs_secrets Behaviour 2: the handle's two shapes, keys in this order
#[test]
fn handle_is_the_unset_shape_or_the_set_shape_with_the_time_it_was_set() {
    let clock = ManualClock::new(1_000);
    let s = create_secrets(None, clock.clock());
    assert_eq!(handle_json(&s.handle(THREAD, "token")), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
    let set = s.set(THREAD, "token", "abc", false);
    assert_eq!(handle_json(&set), r#"{"secret":true,"set":true,"len":3,"at":1000}"#);
    clock.advance(500);
    let later = s.set(THREAD, "token", "abcd", false);
    assert_eq!(handle_json(&later), r#"{"secret":true,"set":true,"len":4,"at":1500}"#);
    assert_eq!(handle_json(&s.handle(THREAD, "token")), r#"{"secret":true,"set":true,"len":4,"at":1500}"#);
    assert_eq!(handle_json(&s.handle(THREAD, "other")), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
    assert_eq!(handle_json(&s.handle("t2", "token")), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
}

// runs_secrets Behaviour 2 / Rule 8.2: len counts UTF-16 units
#[test]
fn handle_len_counts_utf16_units() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    assert_eq!(s.set(THREAD, "a", "\u{fc}n\u{ef}\u{1f600}", false).len, 5);
    assert_eq!(s.set(THREAD, "b", TOKEN, false).len, 31);
}

// runs_secrets Behaviour 3: an empty text forgets the secret and answers the unset handle
#[test]
fn set_with_an_empty_text_forgets_the_secret_and_answers_the_unset_handle() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "pin", "4321", false);
    assert_eq!(handle_json(&s.set(THREAD, "pin", "", false)), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
    assert_eq!(s.plaintext(THREAD, "pin"), None);
    assert_eq!(handle_json(&s.set(THREAD, "never", "", false)), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
}

// runs_secrets Behaviour 3: an empty text forgets a kept secret out of the file too
#[test]
fn set_with_an_empty_text_takes_a_kept_secret_out_of_the_file() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "pin", "4321", true);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), format!(r#"{{"{THREAD}":{{"pin":{{"text":"4321","at":1000}}}}}}"#));
    s.set(THREAD, "pin", "", false);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
}

// runs_secrets Behaviour 3: a secret set again without keep leaves the file
#[test]
fn a_kept_secret_set_again_without_keep_leaves_the_file() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "first-value", true);
    assert!(std::fs::read_to_string(&file).unwrap().contains("first-value"));
    s.set(THREAD, "a", "second-value", false);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
    assert_eq!(s.plaintext(THREAD, "a"), Some("second-value".to_string()));
}

// runs_secrets Behaviour 3: a secret that is not kept, replacing one that was not kept, saves nothing
#[test]
fn setting_a_secret_that_is_not_kept_writes_no_file() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "one", false);
    s.set(THREAD, "a", "two", false);
    assert!(!file.exists());
}

// runs_secrets Behaviour 3: an existing name keeps its place, and a kept one set again is saved with the new text
#[test]
fn a_secret_set_again_keeps_its_place_in_the_file() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let clock = ManualClock::new(1_000);
    let s = create_secrets(Some(file.clone()), clock.clock());
    s.set(THREAD, "a", "one", true);
    s.set(THREAD, "b", "two", true);
    clock.advance(1_000);
    s.set(THREAD, "a", "three", true);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        format!(r#"{{"{THREAD}":{{"a":{{"text":"three","at":2000}},"b":{{"text":"two","at":1000}}}}}}"#)
    );
}

// runs_secrets Behaviour 4: clear forgets, then answers the handle
#[test]
fn clear_forgets_the_secret_and_answers_the_unset_handle() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "kept-one", true);
    s.set(THREAD, "b", "other-one", true);
    assert_eq!(handle_json(&s.clear(THREAD, "a")), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
    assert_eq!(s.plaintext(THREAD, "a"), None);
    assert_eq!(s.plaintext(THREAD, "b"), Some("other-one".to_string()));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), format!(r#"{{"{THREAD}":{{"b":{{"text":"other-one","at":1000}}}}}}"#));
    assert_eq!(handle_json(&s.clear(THREAD, "never")), r#"{"secret":true,"set":false,"len":0,"at":null}"#);
}

// runs_secrets Behaviour 5: a thread that holds nothing is left alone, and nothing is saved
#[test]
fn forget_of_a_thread_that_holds_nothing_does_nothing() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.forget("nobody", None);
    s.forget("nobody", Some("a"));
    assert!(!file.exists());
}

// runs_secrets Behaviour 5: with a name drops that one; without drops the thread's whole
#[test]
fn forget_drops_one_secret_or_every_secret_of_the_thread() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "aaaa", false);
    s.set(THREAD, "b", "bbbb", false);
    s.set("t2", "a", "cccc", false);
    s.forget(THREAD, Some("a"));
    assert_eq!(s.plaintext(THREAD, "a"), None);
    assert_eq!(s.plaintext(THREAD, "b"), Some("bbbb".to_string()));
    s.forget(THREAD, Some("missing"));
    assert_eq!(s.plaintext(THREAD, "b"), Some("bbbb".to_string()));
    s.forget(THREAD, None);
    assert_eq!(s.plaintext(THREAD, "b"), None);
    assert_eq!(s.plaintext("t2", "a"), Some("cccc".to_string()));
}

// runs_secrets Behaviour 5: it saves only when what went was a kept one
#[test]
fn forget_saves_only_when_a_kept_secret_went() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "kept", "kept-value", true);
    s.set(THREAD, "memory", "memory-value", false);
    std::fs::remove_file(&file).unwrap();
    s.forget(THREAD, Some("memory"));
    assert!(!file.exists());
    s.forget(THREAD, Some("missing"));
    assert!(!file.exists());
    s.forget(THREAD, Some("kept"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
}

// runs_secrets Behaviour 5: a whole thread with a kept secret saves, one with none kept does not
#[test]
fn forget_of_a_whole_thread_saves_only_when_it_held_a_kept_one() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set("mem", "m", "memory-value", false);
    s.set("keep", "k", "kept-value", true);
    std::fs::remove_file(&file).unwrap();
    s.forget("mem", None);
    assert!(!file.exists());
    s.forget("keep", None);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
}

// runs_secrets Behaviour 6: with no file nothing is written and nothing breaks
#[test]
fn save_with_no_file_does_nothing() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "kept-value", true);
    s.save();
    assert_eq!(s.plaintext(THREAD, "a"), Some("kept-value".to_string()));
}

// runs_secrets Behaviour 6: threads in the order they were first held, names in held order, compact, no newline
#[test]
fn save_writes_threads_in_first_held_order_and_names_in_held_order_compact() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let clock = ManualClock::new(1_000);
    let s = create_secrets(Some(file.clone()), clock.clock());
    s.set("t1", "mem", "memory-value", false);
    s.set("t2", "k2", "two", true);
    clock.advance(1_000);
    s.set("t1", "k1", "one", true);
    clock.advance(1_000);
    s.set("t1", "k1b", "uno", true);
    s.set("t3", "only-memory", "memory-three", false);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        r#"{"t1":{"k1":{"text":"one","at":2000},"k1b":{"text":"uno","at":3000}},"t2":{"k2":{"text":"two","at":1000}}}"#
    );
}

// runs_secrets Behaviour 6: with none kept it writes {}
#[test]
fn save_with_none_kept_writes_an_empty_object() {
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "mem", "memory-value", false);
    s.save();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
}

// runs_secrets Behaviour 6: the file is written 0600 in a folder at 0700, whatever the umask
#[test]
fn save_makes_the_folder_0700_and_the_file_0600() {
    unsafe {
        libc::umask(0o022);
    }
    let root = temp_root("secrets-");
    let file = secrets_file(root.path());
    let s = create_secrets(Some(file.clone()), ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "kept-value", true);
    assert_eq!(mode_of(&file), 0o600);
    assert_eq!(mode_of(file.parent().unwrap()), 0o700);
}

// runs_secrets Behaviour 9: plaintext answers the text or none
#[test]
fn plaintext_answers_the_text_or_none() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "the-text", false);
    assert_eq!(s.plaintext(THREAD, "a"), Some("the-text".to_string()));
    assert_eq!(s.plaintext(THREAD, "b"), None);
    assert_eq!(s.plaintext("t2", "a"), None);
}

// runs_secrets Behaviour 7: nothing held for the thread, or an empty text, answers the text
#[test]
fn scrub_of_a_thread_with_nothing_held_or_an_empty_text_answers_the_text() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    assert_eq!(s.scrub(THREAD, "plain text"), "plain text");
    s.set(THREAD, "a", "plain", false);
    assert_eq!(s.scrub(THREAD, ""), "");
    assert_eq!(s.scrub("t2", "plain text"), "plain text");
}

// runs_secrets Edge case: oracle scrub.nested
#[test]
fn scrub_matches_a_later_shorter_form_inside_a_mark_already_written() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "s", "secret", false);
    s.set(THREAD, "t", "cret", false);
    assert_eq!(s.scrub(THREAD, "a secret"), "a [se[secret:t]:s]");
}

// runs_secrets Edge case: oracle scrub.markword
#[test]
fn scrub_replaces_every_occurrence_of_a_form() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "x", "secret", false);
    assert_eq!(s.scrub(THREAD, "secret secret"), "[secret:x] [secret:x]");
}

// runs_secrets Edge case: oracle scrub.plus
#[test]
fn scrub_of_a_text_with_a_space_also_scrubs_the_plus_percent_and_base64_forms() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "sp", "a b", false);
    assert_eq!(s.scrub(THREAD, "a b, a+b, a%20b, YSBi"), "[secret:sp], [secret:sp], [secret:sp], [secret:sp]");
}

// runs_secrets Edge case: oracle scrub.basic
#[test]
fn scrub_of_a_basic_header_keeps_only_the_bytes_that_share_a_group_with_the_user() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "token", TOKEN, false);
    assert_eq!(s.scrub(THREAD, "dXNlcjp2cmNsX0FiKzkvenomcT0xIMO8bsOvXzAxMjM0NTY3ODk="), "dXNlcjp2[secret:token]ODk=");
}

// runs_secrets Edge case: oracle scrub.tie
#[test]
fn scrub_of_two_secrets_of_the_same_length_marks_each_by_its_name() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "a", "abcd", false);
    s.set(THREAD, "b", "wxyz", false);
    assert_eq!(s.scrub(THREAD, "abcd wxyz"), "[secret:a] [secret:b]");
}

// runs_secrets Behaviour 7: the longest form goes first, so a short secret inside a long one does not cut it
#[test]
fn scrub_replaces_the_longest_form_first() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "short", "abc", false);
    s.set(THREAD, "long", "abcdef", false);
    assert_eq!(s.scrub(THREAD, "abcdef abc"), "[secret:long] [secret:short]");
}

// runs_secrets Behaviour 7: every form a command could print the secret in is scrubbed
#[test]
fn scrub_removes_the_secret_in_every_form_it_could_be_printed() {
    let s = create_secrets(None, ManualClock::new(1_000).clock());
    s.set(THREAD, "token", TOKEN, false);
    for form in secret_forms(TOKEN) {
        assert_eq!(s.scrub(THREAD, &format!("<{form}>")), "<[secret:token]>", "{form}");
    }
}

// runs_secrets Behaviour 8 / Edge case: oracle secretForms.token
#[test]
fn secret_forms_of_a_token_are_the_seven_the_oracle_lists() {
    assert_eq!(
        secret_forms(TOKEN),
        vec_of(&[
            "vrcl_Ab+9/zz&q=1 \u{fc}n\u{ef}_0123456789",
            "dnJjbF9BYis5L3p6JnE9MSDDvG7Dr18wMTIzNDU2Nzg5",
            "Y2xfQWIrOS96eiZxPTEgw7xuw69fMDEyMzQ1Njc4",
            "cmNsX0FiKzkvenomcT0xIMO8bsOvXzAxMjM0NTY3",
            "vrcl_Ab%2B9%2Fzz%26q%3D1%20%C3%BCn%C3%AF_0123456789",
            "vrcl_Ab%2b9%2fzz%26q%3d1%20%c3%bcn%c3%af_0123456789",
            "vrcl_Ab%2B9%2Fzz%26q%3D1+%C3%BCn%C3%AF_0123456789",
        ])
    );
}

// runs_secrets Behaviour 8 / Edge case: oracle secretForms.short
#[test]
fn secret_forms_of_a_short_text_has_no_middle_forms() {
    assert_eq!(secret_forms("4321"), vec_of(&["4321", "NDMyMQ==", "NDMyMQ"]));
}

// runs_secrets Behaviour 8 / Edge case: oracle secretForms.space
#[test]
fn secret_forms_of_a_text_with_a_space_has_the_percent_and_plus_forms() {
    assert_eq!(secret_forms("a b"), vec_of(&["a b", "YSBi", "a%20b", "a+b"]));
}

// runs_secrets Behaviour 8: oracle forms.middle
#[test]
fn secret_forms_of_sixteen_bytes_has_the_three_middle_forms() {
    assert_eq!(
        secret_forms("0123456789abcdef"),
        vec_of(&[
            "0123456789abcdef",
            "MDEyMzQ1Njc4OWFiY2RlZg==",
            "MDEyMzQ1Njc4OWFiY2RlZg",
            "MDEyMzQ1Njc4OWFiY2Rl",
            "MjM0NTY3ODlhYmNk",
            "MTIzNDU2Nzg5YWJjZGVm",
        ])
    );
}

// runs_secrets Behaviour 8: the empty text has no forms
#[test]
fn secret_forms_of_the_empty_text_is_empty() {
    assert_eq!(secret_forms(""), Vec::<String>::new());
}

// runs_secrets Decision 2: encodeURIComponent keeps `A-Za-z0-9-_.!~*'()` and writes the rest as uppercase %XX of UTF-8
#[test]
fn encode_uri_component_keeps_the_unreserved_set_and_encodes_the_rest() {
    let kept = "AZaz09-_.!~*'()";
    assert_eq!(encode_uri_component(kept), kept);
    assert_eq!(encode_uri_component("a b"), "a%20b");
    assert_eq!(encode_uri_component(TOKEN), "vrcl_Ab%2B9%2Fzz%26q%3D1%20%C3%BCn%C3%AF_0123456789");
    assert_eq!(encode_uri_component("#?&=+,;:@$/%"), "%23%3F%26%3D%2B%2C%3B%3A%40%24%2F%25");
    assert_eq!(encode_uri_component("\u{20ac}"), "%E2%82%AC");
    assert_eq!(encode_uri_component("\u{1f600}"), "%F0%9F%98%80");
    assert_eq!(encode_uri_component(""), "");
}

// runs_secrets Behaviour 10: has, allow, revoke and list, per thread
#[test]
fn default_approvals_hold_keys_per_thread() {
    let a = DefaultApprovals::default();
    assert!(!a.has("t1", "k1"));
    assert_eq!(a.list("t1"), Vec::<String>::new());
    a.allow("t1", "k1");
    assert!(a.has("t1", "k1"));
    assert!(!a.has("t1", "k2"));
    assert!(!a.has("t2", "k1"));
    assert_eq!(a.list("t2"), Vec::<String>::new());
    a.revoke("t1", "k1");
    assert!(!a.has("t1", "k1"));
    assert_eq!(a.list("t1"), Vec::<String>::new());
}

// runs_secrets Behaviour 10: keys list in insertion order, a repeat is one, and a revoke keeps the rest in order
#[test]
fn default_approvals_list_in_insertion_order_and_a_revoke_keeps_the_order() {
    let a = DefaultApprovals::default();
    for k in ["k1", "k2", "k3", "k2"] {
        a.allow("t1", k);
    }
    assert_eq!(a.list("t1"), vec_of(&["k1", "k2", "k3"]));
    a.revoke("t1", "k1");
    assert_eq!(a.list("t1"), vec_of(&["k2", "k3"]));
    a.allow("t1", "k1");
    assert_eq!(a.list("t1"), vec_of(&["k2", "k3", "k1"]));
}

// runs_secrets Behaviour 10: a revoke of a key or a thread that is not there does nothing
#[test]
fn default_approvals_revoke_of_a_missing_key_or_thread_does_nothing() {
    let a = DefaultApprovals::default();
    a.revoke("nobody", "k1");
    a.allow("t1", "k1");
    a.revoke("t1", "missing");
    a.revoke("t2", "k1");
    assert_eq!(a.list("t1"), vec_of(&["k1"]));
    assert!(a.has("t1", "k1"));
}

// runs_secrets Behaviour 10: the store is shared by the trait object the table holds
#[test]
fn default_approvals_work_through_the_trait_object() {
    let a: Rc<dyn RunApprovals> = Rc::new(DefaultApprovals::default());
    a.allow("t1", "k1");
    assert!(a.has("t1", "k1"));
    assert_eq!(a.list("t1"), vec_of(&["k1"]));
}
