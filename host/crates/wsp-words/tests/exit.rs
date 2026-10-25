// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/exit.test.ts (8 cases; the RuntimeResponse case at :48 is the wire's and stays with
//! wsp-wire) and adds the cases spec/exit.md pins: kind round-trips, the 400-unit and 11-line edges, key order of
//! the failure object, and JSON.stringify's layout under json_line.

use serde_json::json;
use wsp_words::exit::*;

fn err_with_kind(message: &str, kind: &str) -> HostError {
    HostError::with_kind(message, Kind::parse(kind))
}

mod the_exit_code_every_wsp_verb_answers_with {
    use super::*;

    #[test]
    fn is_one_table_ok_0_provider_1_auth_2_usage_3_each_class_with_its_words() {
        let codes: Vec<(&str, i32)> = ExitClass::ALL.iter().map(|c| (c.as_str(), c.code())).collect();
        assert_eq!(codes, vec![("ok", 0), ("provider", 1), ("auth", 2), ("usage", 3)]);
        assert_eq!(ExitClass::Ok.words(), "it did what its line says; with --json stdout holds the answer");
        assert_eq!(ExitClass::Provider.words(), "the host, the runtime, a provider or the machine refused or failed");
        assert_eq!(ExitClass::Auth.words(), "no key, no sign-in, or the host refused the token");
        assert_eq!(
            ExitClass::Usage.words(),
            "the line was refused before anything ran: a missing argument, an unknown flag or a value nothing takes"
        );
        for c in ExitClass::ALL {
            assert!(!c.words().contains('\u{2014}'), "{c:?}");
        }
    }

    #[test]
    fn classes_an_error_by_the_kind_stamped_on_it_and_never_by_its_words_usage_and_in() {
        assert_eq!(exit_class_of(&usage_refusal("wsp new takes one name.", "usage: wsp new <name>")), ExitClass::Usage);
        assert_eq!(exit_class_of(&err_with_kind("2x9 is not a size", "invalid")), ExitClass::Usage);
        assert_eq!(exit_class_of(&auth_refusal("unauthorized")), ExitClass::Auth);
        // A name this host holds nothing by is a value nothing takes, whichever door was typed and however far it got.
        assert_eq!(exit_class_of(&not_found_refusal("no workspace nope")), ExitClass::Usage);
        for kind in ["concurrency", "plan", "missing", "conflict", "exists", "transient", "unknown"] {
            assert_eq!(exit_class_of(&err_with_kind("no", kind)), ExitClass::Provider, "{kind}");
        }
        assert_eq!(exit_class_of(&HostError::new("Unknown option '--json'")), ExitClass::Provider);
        assert_eq!(exit_class_of(&HostError::new("no host answered for /x within 20.0s")), ExitClass::Provider);
    }

    #[test]
    fn the_failure_object_is_the_message_the_class_and_the_code_the_class_owns_and_par() {
        let failure = verb_failure(&usage_refusal("wsp forget takes one workspace.", "usage: wsp forget <workspace>"));
        assert_eq!(
            serde_json::to_value(&failure).unwrap(),
            json!({ "error": "wsp forget takes one workspace. usage: wsp forget <workspace>", "class": "usage", "exit": 3 })
        );
        let parsed: VerbFailure = serde_json::from_value(serde_json::to_value(&failure).unwrap()).unwrap();
        assert_eq!(parsed, failure);
        assert_eq!(
            serde_json::to_value(verb_failure(&auth_refusal("the host's token file is missing: /x"))).unwrap(),
            json!({ "error": "the host's token file is missing: /x", "class": "auth", "exit": 2 })
        );
        assert_eq!(
            serde_json::to_value(verb_failure(&HostError::new("no workspace nope"))).unwrap(),
            json!({ "error": "no workspace nope", "class": "provider", "exit": 1 })
        );
        assert!(serde_json::from_value::<VerbFailure>(json!({ "error": "x", "class": "ok", "exit": 0 })).is_err());
    }

    #[test]
    fn a_refusal_keeps_both_halves_apart_for_a_client_and_still_prints_as_the_one_line() {
        let e = refusal("spoo names no user", "Type user@spoo.", Some(Kind::Invalid));
        assert_eq!(e.message, "spoo names no user. Type user@spoo.");
        assert_eq!(e.fix.as_deref(), Some("Type user@spoo."));
        assert_eq!(e.kind, Some(Kind::Invalid));
        assert_eq!(exit_class_of(&e), ExitClass::Usage);
        assert_eq!(refusal("the host is busy", "Try again.", None).kind, None);
        let usage = usage_refusal("wsp new takes one name.", "usage: wsp new <name>");
        assert_eq!(usage.message, "wsp new takes one name. usage: wsp new <name>");
        assert_eq!(usage.fix.as_deref(), Some("usage: wsp new <name>"));
        assert_eq!(usage.kind, Some(Kind::Usage));
    }
}

mod a_refusal_a_host_keeps_for_every_client {
    use super::*;

    #[test]
    fn reads_the_halves_a_refusal_was_stamped_with_and_nothing_that_is_not_a_string() {
        assert_eq!(
            refusal_parts(&refusal("spoo has no curl", "Install it.", Some(Kind::Missing))),
            RefusalParts { said: "spoo has no curl.".into(), fix: Some("Install it.".into()), kind: Some("missing".into()) }
        );
        assert_eq!(refusal_parts(&HostError::new("no")), RefusalParts { said: "no".into(), fix: None, kind: None });
    }

    #[test]
    fn cuts_each_line_at_400_characters_and_the_lines_past_the_eleventh_saying_each_cut() {
        let mut lines = vec!["box did not dial back".to_string(), "y".repeat(401)];
        lines.extend((0..12).map(|n| format!("line {n}")));
        let kept = kept_said(&lines.join("\n"));
        let kept: Vec<&str> = kept.split('\n').collect();
        assert_eq!(kept.len(), 12);
        assert_eq!(kept[0], "box did not dial back");
        assert_eq!(kept[1], format!("{} (cut 1 characters)", "y".repeat(400)));
        assert_eq!(kept[10], "line 8");
        assert_eq!(kept[11], "(cut 3 lines)");
        assert_eq!(kept_said("short\nsaid"), "short\nsaid");
    }
}

mod the_json_a_verb_prints {
    use super::*;

    #[test]
    fn escapes_del_and_every_c1_control_character_as_json_stringify_escapes_c0_and_par() {
        let value = json!({ "path": "/opt/s\u{9b}2J\u{1b}]0;x\u{7}\u{85}\u{7f}", "n": 1 });
        let line = json_line(&value, None);
        assert!(!line.chars().any(|c| (c as u32) < 0x20 || (0x7f..=0x9f).contains(&(c as u32))), "{line}");
        assert!(line.contains("\\u009b"), "{line}");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&line).unwrap(), value);
        let pretty = json_line(&value, Some(2));
        assert!(!pretty.chars().any(|c| (0x7f..=0x9f).contains(&(c as u32))), "{pretty}");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&pretty).unwrap(), value);
        assert_eq!(escape_c1(&serde_json::to_string(&value).unwrap()), line);
    }
}

// Cases spec/exit.md pins that the TS suite does not.
mod pinned {
    use super::*;

    #[test]
    fn kind_reads_back_as_the_string_it_was_stamped_with() {
        let listed = [
            ("usage", Kind::Usage),
            ("invalid", Kind::Invalid),
            ("not-found", Kind::NotFound),
            ("processes-left", Kind::ProcessesLeft),
            ("auth", Kind::Auth),
            ("ticket", Kind::Ticket),
            ("conflict", Kind::Conflict),
            ("startLimit", Kind::StartLimit),
            ("snapshotUnavailable", Kind::SnapshotUnavailable),
            ("host-key", Kind::HostKey),
            ("sshBehind", Kind::SshBehind),
            ("reauth", Kind::Reauth),
        ];
        for (s, k) in listed {
            assert_eq!(Kind::parse(s), k, "{s}");
            assert_eq!(k.as_str(), s);
            assert_eq!(serde_json::to_value(&k).unwrap(), json!(s));
            assert_eq!(serde_json::from_value::<Kind>(json!(s)).unwrap(), k);
        }
        // Kinds the TS stamps that chapter 05's table leaves out keep their string and class as the provider's.
        for s in ["unavailable", "quota", "provider", "Usage", ""] {
            let k = Kind::parse(s);
            assert_eq!(k, Kind::Other(s.to_string()));
            assert_eq!(k.as_str(), s);
            assert_eq!(k.class(), ExitClass::Provider, "{s}");
            assert_eq!(serde_json::to_value(&k).unwrap(), json!(s));
        }
    }

    #[test]
    fn every_kind_classes_by_kind_class_and_none_is_the_providers() {
        assert_eq!(Kind::Usage.class(), ExitClass::Usage);
        assert_eq!(Kind::Invalid.class(), ExitClass::Usage);
        assert_eq!(Kind::NotFound.class(), ExitClass::Usage);
        assert_eq!(Kind::ProcessesLeft.class(), ExitClass::Usage);
        assert_eq!(Kind::Auth.class(), ExitClass::Auth);
        for k in [
            Kind::Ticket,
            Kind::Conflict,
            Kind::Concurrency,
            Kind::StartLimit,
            Kind::Plan,
            Kind::Missing,
            Kind::Unreachable,
            Kind::Refused,
            Kind::Reauth,
        ] {
            assert_eq!(k.class(), ExitClass::Provider, "{k:?}");
        }
        let none = HostError::new("x");
        assert_eq!(none.class(), ExitClass::Provider);
        assert_eq!(none.exit_code(), 1);
        assert_eq!(usage_refusal("a", "b").exit_code(), 3);
        assert_eq!(auth_refusal("a").exit_code(), 2);
    }

    #[test]
    fn refusal_fields_carry_left_only_under_processes_left() {
        let mut e = HostError::with_kind("still running", Kind::ProcessesLeft);
        e.left = Some(vec![json!({ "threadId": "t1" })]);
        e.errors = Some(vec![json!("bad")]);
        assert_eq!(serde_json::to_value(refusal_fields(&e)).unwrap(), json!({ "errors": ["bad"], "left": [{ "threadId": "t1" }] }));
        let mut other = HostError::with_kind("still running", Kind::Conflict);
        other.left = Some(vec![json!({ "threadId": "t1" })]);
        assert_eq!(serde_json::to_value(refusal_fields(&other)).unwrap(), json!({}));
        let mut slate = HostError::new("slate refused");
        slate.warnings = Some(vec![json!("w")]);
        slate.errors = Some(vec![json!("e")]);
        let failure = verb_failure(&slate);
        assert_eq!(
            serde_json::to_string(&failure).unwrap(),
            r#"{"error":"slate refused","class":"provider","exit":1,"errors":["e"],"warnings":["w"]}"#
        );
        assert_eq!(serde_json::to_value(problem_lists_of(&slate)).unwrap(), json!({ "errors": ["e"], "warnings": ["w"] }));
    }

    #[test]
    fn refusal_said_takes_only_the_fix_it_was_joined_with_off_the_end() {
        assert_eq!(refusal_said("spoo has no curl. Install it.", Some("Install it.")), "spoo has no curl.");
        assert_eq!(refusal_said("spoo has no curl.Install it.", Some("Install it.")), "spoo has no curl.Install it.");
        assert_eq!(refusal_said("Install it.", Some("Install it.")), "Install it.");
        assert_eq!(refusal_said("no", None), "no");
        assert_eq!(refusal_said("a b", Some("")), "a b");
        assert_eq!(refusal_said("a ", Some("")), "a");
    }

    #[test]
    fn marked_cut_counts_utf16_units_and_says_how_many_it_took() {
        assert_eq!(marked_cut(&"y".repeat(400), SAID_LINE_CHARS), "y".repeat(400));
        let emoji = "\u{1F600}".repeat(201);
        assert_eq!(marked_cut(&emoji, SAID_LINE_CHARS), format!("{} (cut 2 characters)", "\u{1F600}".repeat(200)));
        assert_eq!(marked_cut("abcdef", 3), "abc (cut 3 characters)");
        assert_eq!(marked_cut("", 3), "");
    }

    #[test]
    fn kept_said_keeps_exactly_eleven_lines_and_says_none_cut_at_eleven() {
        let eleven: Vec<String> = (0..11).map(|n| format!("l{n}")).collect();
        assert_eq!(kept_said(&eleven.join("\n")), eleven.join("\n"));
        let twelve: Vec<String> = (0..12).map(|n| format!("l{n}")).collect();
        assert_eq!(kept_said(&twelve.join("\n")), format!("{}\n(cut 1 lines)", eleven.join("\n")));
        assert_eq!(kept_said(""), "");
        assert_eq!(kept_said("a\n\nb"), "a\n\nb");
    }

    #[test]
    fn cloud_off_refusal_names_the_line_and_the_variable_that_turns_the_cloud_on() {
        let e = cloud_off_refusal("--on boat");
        assert_eq!(e.message, "--on boat needs the cloud, which is off on this computer: start the host with WSP_CLOUD=1 to turn it on.");
        assert_eq!(e.fix.as_deref(), Some("start the host with WSP_CLOUD=1 to turn it on."));
        assert_eq!(e.kind, Some(Kind::Usage));
        let a = auth_refusal("no key");
        assert_eq!((a.message.as_str(), a.fix.as_deref(), a.kind.clone()), ("no key", None, Some(Kind::Auth)));
        let n = not_found_refusal("no thread t");
        assert_eq!((n.message.as_str(), n.fix.as_deref(), n.kind.clone()), ("no thread t", None, Some(Kind::NotFound)));
        assert_eq!(n.to_string(), "no thread t");
    }

    #[test]
    fn json_line_lays_out_as_json_stringify_with_keys_in_the_order_held() {
        let value = json!({ "z": 1, "a": [], "m": {}, "n": [1, { "b": true, "a": null }], "f": 1.5, "i": 3.0 });
        assert_eq!(json_line(&value, None), r#"{"z":1,"a":[],"m":{},"n":[1,{"b":true,"a":null}],"f":1.5,"i":3}"#);
        assert_eq!(
            json_line(&value, Some(2)),
            "{\n  \"z\": 1,\n  \"a\": [],\n  \"m\": {},\n  \"n\": [\n    1,\n    {\n      \"b\": true,\n      \"a\": null\n    }\n  ],\n  \"f\": 1.5,\n  \"i\": 3\n}"
        );
        assert_eq!(json_line(&json!(null), None), "null");
        assert_eq!(json_line(&json!("\u{2028}é/"), None), "\"\u{2028}é/\"");
    }
}

mod the_host_key_an_installer_error_carries {
    use super::*;

    #[test]
    fn rides_an_error_of_kind_host_key_and_is_offered_back_only_there() {
        // install.ts:69: Object.assign(new Error(line), { kind: "host-key", hostKey: key }); manage.ts:321 reads it
        // only when the kind is host-key and the key is a string.
        let e = HostError::with_host_key("the key is not confirmed", "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK");
        assert_eq!(e.kind, Some(Kind::HostKey));
        assert_eq!(e.offered_host_key(), Some("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK"));
        assert_eq!(exit_class_of(&e), ExitClass::Provider);
        assert_eq!(e.message, "the key is not confirmed");

        let other = HostError { host_key: Some("ssh-ed25519 AAAA".to_string()), ..err_with_kind("no", "refused") };
        assert_eq!(other.offered_host_key(), None);
        assert_eq!(HostError::new("plain").offered_host_key(), None);
        assert_eq!(err_with_kind("no key", "host-key").offered_host_key(), None);
    }
}

mod the_code_and_the_status_an_error_carries {
    use super::*;

    #[test]
    fn ride_the_error_beside_its_kind_and_reach_no_reply() {
        // create.ts:156 and golden.ts:635: Object.assign(new Error(line), { kind: "concurrency", status: refusal.status }); machines.ts:88 reads a
        // numeric status. A Node system error carries `code` (ENOENT, EADDRINUSE).
        let e = HostError {
            status: Some(429.0),
            code: Some("EADDRINUSE".to_string()),
            ..err_with_kind("the provider asked for time", "concurrency")
        };
        assert_eq!((e.status, e.code.as_deref()), (Some(429.0), Some("EADDRINUSE")));
        assert_eq!((HostError::new("plain").status, HostError::new("plain").code), (None, None));
        // The failure object (exit.ts:57) names neither, and the exit class is still the kind's.
        assert_eq!(verb_failure(&e).error, "the provider asked for time");
        assert_eq!(
            serde_json::to_value(verb_failure(&e)).unwrap().as_object().unwrap().keys().collect::<Vec<_>>(),
            ["error", "class", "exit"]
        );
        assert_eq!(exit_class_of(&e), ExitClass::Provider);
    }
}
