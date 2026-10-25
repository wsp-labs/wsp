// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/protocol.test.ts, copy.test.ts, init-job.test.ts, places.test.ts and
//! machine-link.test.ts: capabilities_devices.
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_frames::Base64Bytes;
use wsp_wire::wire::capabilities::*;
use wsp_wire::wire::devices::*;
use wsp_wire::wire::runtime::{parse_runtime_request, RuntimeOp};

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

fn with(base: Value, key: &str, value: Value) -> Value {
    let mut v = base;
    v.as_object_mut().unwrap().insert(key.to_owned(), value);
    v
}

fn without(base: &Value, key: &str) -> Value {
    let mut v = base.clone();
    v.as_object_mut().unwrap().remove(key);
    v
}

fn capabilities(sizes: Value, disk_snapshots: bool) -> Capabilities {
    serde_json::from_value(json!({
        "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false,
        "callbackRelay": false, "diskSnapshots": disk_snapshots, "images": false, "snapshotsAnyLife": false,
        "snapshotListing": false, "templates": false, "sizes": sizes, "kept": true, "copies": true, "ownNetwork": false,
    }))
    .unwrap()
}

fn room(mem_room_mb: f64, disk_free_bytes: f64) -> PlaceCapacity {
    serde_json::from_value(json!({
        "cores": 4, "memMb": 16384, "memRoomMb": mem_room_mb, "machineMemMb": 2048, "diskFreeBytes": disk_free_bytes,
        "images": [], "machines": { "running": 0, "paused": 0 },
    }))
    .unwrap()
}

mod a_copy_of_the_image_beside_the_record {
    use super::*;

    // protocol.test.ts:182
    #[test]
    fn a_place_builds_a_copy_only_where_it_both_forks_a_machine_and_copies_its_disk() {
        let size = json!([{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.1 }]);
        assert!(builds_images(&capabilities(size.clone(), true)));
        // A computer somebody joined: it runs their agents and forks nothing.
        assert!(!builds_images(&capabilities(json!([]), false)));
        // A provider that forks but keeps no disk copy has nothing to seal a version out of.
        assert!(!builds_images(&capabilities(size, false)));
    }
}

mod backend_capabilities {
    use super::*;

    fn full() -> Value {
        json!({
            "liveCloneForks": true, "pauseMode": "memory", "replacesMachine": true, "previewUrls": true, "signedUrls": true,
            "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": false, "snapshotListing": true,
            "templates": true, "kept": false, "copies": true, "ownNetwork": true,
            "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.11 }],
        })
    }

    // protocol.test.ts:758
    #[test]
    fn requires_every_flag_callbackrelay_templates_disksnapshots_images_snapshotsanylif() {
        let c: Capabilities = reads(full());
        assert!(c.callback_relay && c.templates && c.disk_snapshots && c.images && c.replaces_machine && c.copies && c.own_network);
        assert!(!c.kept && !c.snapshots_any_life);
        assert_eq!(c.sizes.len(), 1);
        // Each flag a backend might leave unsaid, each refused on its own name: callbackRelay and templates; a
        // backend that never says whether its machine's disk can be copied would have the snapshot verb guessing;
        // one that never says whether it keeps an image at all would have the fork road reading three flags to learn it;
        // which life the copy may be taken from would have the golden road guessing whether a builder that woke still
        // has a seal in it; whether a fresh machine may stand in for another leaves the rebuild guessing; whether its
        // machine is the person's own would have every turn's access decided for it; whether it makes a workspace by
        // copying itself, or gives a copy a network of its own, leaves the port base and the row's ports cell guessing.
        for flag in [
            "templates",
            "callbackRelay",
            "diskSnapshots",
            "images",
            "snapshotsAnyLife",
            "replacesMachine",
            "kept",
            "copies",
            "ownNetwork",
            "sizes",
        ] {
            let e = refused::<Capabilities>(without(&full(), flag));
            assert_eq!(e.path, path![flag], "{flag}");
            assert_eq!(e.code, "invalid_type", "{flag}");
        }
        let off: Capabilities = reads(with(full(), "images", json!(false)));
        assert!(!off.images);
        let e = refused::<Capabilities>(with(full(), "sizes", json!([{ "cpu": 2, "memMb": 4096 }])));
        assert_eq!(e.path, path!["sizes", 0, "rateUsdPerHour"]);
        assert_eq!(e.code, "invalid_type");
    }

    // protocol.test.ts:795
    #[test]
    fn whether_a_request_names_a_size_has_one_home_so_the_create_s_gate_reads_no_copy_o() {
        // Either half names one; a create naming neither takes the size its golden was sealed at.
        assert_eq!([names_size(Some(4.0), None), names_size(None, Some(8192.0)), names_size(Some(4.0), Some(8192.0))], [true, true, true]);
        // `{}`, undefined, and an object holding only keys that are no size: none of them names one.
        assert_eq!([names_size(None, None), names_size(None, None), names_size(None, None)], [false, false, false]);
    }

    // protocol.test.ts:801
    #[test]
    fn pausemode_is_optional_and_one_of_memory_or_disk_a_boolean_is_refused() {
        let base = json!({
            "liveCloneForks": true, "replacesMachine": true, "previewUrls": true, "signedUrls": true, "callbackRelay": true,
            "diskSnapshots": true, "images": true, "snapshotsAnyLife": false, "snapshotListing": true, "templates": true,
            "kept": false, "copies": true, "ownNetwork": true, "sizes": [],
        });
        // Absent is a machine that cannot be paused: this computer, a machine reached over ssh.
        let none: Capabilities = reads(base.clone());
        assert_eq!(none.pause_mode, None);
        let memory: Capabilities = reads(with(base.clone(), "pauseMode", json!("memory")));
        assert_eq!(memory.pause_mode, Some(PauseMode::Memory));
        let disk: Capabilities = reads(with(base.clone(), "pauseMode", json!("disk")));
        assert_eq!(disk.pause_mode, Some(PauseMode::Disk));
        assert_eq!(
            [serde_json::to_value(PauseMode::Memory).unwrap(), serde_json::to_value(PauseMode::Disk).unwrap()],
            [json!("memory"), json!("disk")]
        );
        // The old boolean said two things at once; a backend still declaring it is refused rather than read as a mode.
        let boolean = refused::<Capabilities>(with(base.clone(), "pauseMode", json!(true)));
        assert_eq!(boolean.path, path!["pauseMode"]);
        assert_eq!(boolean.code, "invalid_type");
        let frozen = refused::<Capabilities>(with(base, "pauseMode", json!("frozen")));
        assert_eq!(frozen.path, path!["pauseMode"]);
        assert_eq!(frozen.code, "invalid_enum_value");
    }
}

mod the_two_flags_a_computer_declares_about_copies {
    use super::*;

    // copy.test.ts:52
    #[test]
    fn are_both_required_so_a_backend_that_declares_neither_is_refused_rather_than_read() {
        let flags = json!({
            "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
            "diskSnapshots": false, "images": false, "snapshotsAnyLife": false, "snapshotListing": false, "templates": false,
            "sizes": [], "kept": true, "copies": true, "ownNetwork": false,
        });
        let c: Capabilities = reads(flags.clone());
        assert!(c.copies && !c.own_network);
        let no_copies = refused::<Capabilities>(without(&flags, "copies"));
        assert_eq!(no_copies.path, path!["copies"]);
        assert_eq!(no_copies.code, "invalid_type");
        let no_network = refused::<Capabilities>(without(&flags, "ownNetwork"));
        assert_eq!(no_network.path, path!["ownNetwork"]);
        assert_eq!(no_network.code, "invalid_type");
    }
}

mod the_init_job_view {
    use super::*;

    // init-job.test.ts:156
    #[test]
    fn the_setup_the_modal_opens_on_carries_the_provider_key_s_presence_alone_never_a_k() {
        let setup: InitSetup = reads(json!({
            "keys": { "solari": false }, "home": "/Users/me",
            "agents": [{ "id": "claude", "name": "Claude Code", "configured": true, "takesTools": true }],
            "pricing": { "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.11 }, "job": null,
        }));
        assert!(setup.job.is_none());
        assert_eq!(serde_json::to_value(&setup.keys).unwrap().as_object().unwrap().keys().collect::<Vec<_>>(), ["solari"]);
        // A key's own text is no presence flag: refused on the key's name (InitKeys is the views writer's type).
        let e = refused::<InitSetup>(
            json!({ "keys": { "solari": "slr_live_x" }, "home": "/Users/me", "agents": [], "pricing": null, "job": null }),
        );
        assert_eq!(e.path, path!["keys", "solari"]);
        assert_eq!(e.code, "invalid_type");
    }
}

mod how_many_forks_a_place_takes {
    use super::*;

    // machine-link.test.ts:158
    #[test]
    fn takes_the_smaller_of_what_the_memory_and_the_disk_allow() {
        // The memory is the smaller here: one fork of 1959 MB fits in 3000, and the disk would take two copies.
        assert_eq!(fork_room(&room(3000.0, 10.0 * GB), 1959.0, Some(4.2 * GB)), 1.0);
        // The disk is the smaller here: the memory would take four forks and the disk holds two more copies.
        assert_eq!(fork_room(&room(8000.0, 10.0 * GB), 2000.0, Some(4.0 * GB)), 2.0);
    }

    // machine-link.test.ts:165
    #[test]
    fn reads_the_memory_alone_where_no_image_is_there_to_measure_by() {
        assert_eq!(fork_room(&room(8000.0, 10.0 * GB), 2000.0, None), 4.0);
        assert_eq!(fork_room(&room(8000.0, 10.0 * GB), 2000.0, Some(0.0)), 4.0);
    }

    // machine-link.test.ts:170
    #[test]
    fn never_reads_below_none() {
        assert_eq!(fork_room(&room(0.0, 0.0), 2000.0, Some(4.0 * GB)), 0.0);
    }
}

mod runtime_wire_types {
    use super::*;

    fn b64<const N: usize>(fill: u8) -> String {
        Base64Bytes::<N>::from_bytes(&[fill; N]).as_str().to_owned()
    }

    // protocol.test.ts:973
    #[test]
    fn device_auth_carries_the_key_a_computer_on_the_account_proves_and_a_device_s_reco() {
        let frame = json!({ "id": 7, "op": "device.auth", "publicKey": b64::<44>(0), "name": "the laptop", "signature": b64::<64>(0) });
        match reads_with(parse_runtime_request, frame.clone()).op {
            RuntimeOp::DeviceAuth(req) => {
                assert_eq!(req.name, "the laptop");
                assert_eq!(req.public_key.as_str(), b64::<44>(0));
                assert_eq!(req.signature.as_str(), b64::<64>(0));
            }
            other => panic!("{other:?}"),
        }
        // A frame with no signature over the seal's transcript is no frame at all: the key alone names nobody.
        let unsigned = refused_with(parse_runtime_request, without(&frame, "signature"));
        assert_eq!(unsigned.path, path!["signature"]);
        assert_eq!(unsigned.code, "invalid_type");
        let nameless = refused_with(parse_runtime_request, with(frame, "name", json!("")));
        assert_eq!(nameless.path, path!["name"]);
        assert_eq!(nameless.code, "too_small");
        // One record for both roads: a code leaves the road absent, the account writes it, and the key rides along so
        // a device admitted here can sign for the next one.
        let via = json!({ "kind": "account", "relayDeviceId": "c_1", "fingerprint": "SHA256:aaa", "publicKey": "bbb", "admittedBy": "SHA256:ccc" });
        let device = json!({ "id": "d_1", "name": "the laptop", "createdAt": "2026-09-22T00:00:00.000Z", "lastSeenAt": "2026-09-22T00:00:00.000Z", "via": via });
        let read: DeviceView = reads(device.clone());
        assert_eq!(read.via.expect("via").kind, "account");
        let coded = without(&device, "via");
        let read: DeviceView = reads(coded);
        assert!(read.via.is_none());
        let mut bent = device;
        bent["via"]["kind"] = json!("code");
        let e = refused::<DeviceView>(bent);
        assert_eq!(e.path, path!["via", "kind"]);
        assert_eq!(e.code, "invalid_literal");
    }

    // protocol.test.ts:990
    #[test]
    fn an_admission_binds_the_key_admitted_the_key_that_signed_and_the_moment_and_no_ho() {
        let bytes = device_admission_transcript("SHA256:device", "SHA256:signer", "2026-09-22T00:00:00.000Z");
        assert_eq!(
            String::from_utf8(bytes.clone()).unwrap(),
            "wsp device admission v1\nSHA256:device\nSHA256:signer\n2026-09-22T00:00:00.000Z\n"
        );
        // Every part of it moves the bytes, so nothing a relay could swap on the way leaves a signature standing.
        for (device, by, at) in [
            ("SHA256:other", "SHA256:signer", "2026-09-22T00:00:00.000Z"),
            ("SHA256:device", "SHA256:other", "2026-09-22T00:00:00.000Z"),
            ("SHA256:device", "SHA256:signer", "2026-09-22T00:00:01.000Z"),
        ] {
            assert_ne!(device_admission_transcript(device, by, at), bytes);
        }
        // The time is the string the relay stores and hands back, byte for byte, since the bytes are built from it.
        let text = String::from_utf8(device_admission_transcript("a", "b", "2026-09-22T00:00:00Z")).unwrap();
        assert!(text.contains("2026-09-22T00:00:00Z"), "{text}");
    }
}

mod a_pairing_code_as_a_person_reads_it_and_as_the_host_takes_it {
    use super::*;

    // places.test.ts:312
    #[test]
    fn shows_in_two_halves_and_comes_back_as_the_letters_alone_whichever_screen_it_was() {
        assert_eq!(shown_pair_code("QW4K7PZX"), "QW4K-7PZX");
        assert_eq!(shown_pair_code("qw4k7pzx"), "QW4K-7PZX");
        assert_eq!(shown_pair_code("QW4K"), "QW4K");
        assert_eq!(sent_pair_code("QW4K-7PZX"), "QW4K7PZX");
        assert_eq!(sent_pair_code("qw4k-7pzx"), "QW4K7PZX");
        assert_eq!(sent_pair_code(&shown_pair_code("QW4K7PZX")), "QW4K7PZX");
    }
}

mod the_one_token_a_join_line_carries {
    use super::*;

    fn key() -> String {
        format!("SHA256:{}", "a".repeat(43))
    }

    // places.test.ts:325
    #[test]
    fn writes_the_code_as_a_screen_shows_it_and_the_key_beside_it_and_reads_both_back() {
        let k = key();
        assert_eq!(join_token("QW4K7PZX", &k), format!("QW4K-7PZX.{k}"));
        assert_eq!(read_join_token(&join_token("QW4K7PZX", &k)), JoinToken { code: "QW4K7PZX".into(), host_key: Some(k) });
    }

    // places.test.ts:330
    #[test]
    fn takes_the_code_the_way_every_other_screen_takes_one_and_leaves_the_key_exactly_a() {
        // The code is folded to the letters alone; the key is base64 and case is what tells two keys apart.
        let k = key();
        assert_eq!(read_join_token(&format!("  qw4k-7pzx.{k}  ")), JoinToken { code: "QW4K7PZX".into(), host_key: Some(k) });
        assert_eq!(read_join_token("QW4K-7PZX.SHA256:aB+/cD").host_key.as_deref(), Some("SHA256:aB+/cD"));
    }

    // places.test.ts:336
    #[test]
    fn answers_no_key_for_a_token_that_carries_none_which_is_what_the_join_refuses_on() {
        assert_eq!(read_join_token("QW4K-7PZX"), JoinToken { code: "QW4K7PZX".into(), host_key: None });
        assert_eq!(read_join_token("QW4K-7PZX."), JoinToken { code: "QW4K7PZX".into(), host_key: None });
        assert!(JOIN_NO_KEY_REFUSAL.contains("wsp add"));
    }

    // places.test.ts:342
    #[test]
    fn splits_at_the_first_mark_so_a_key_holding_one_is_read_whole() {
        assert_eq!(read_join_token("QW4K-7PZX.SHA256:a.b").host_key.as_deref(), Some("SHA256:a.b"));
    }
}

mod a_paste_that_is_a_pairing_code_and_one_that_is_not {
    use super::*;

    // places.test.ts:350
    #[test]
    fn reads_a_code_out_of_every_way_a_person_copies_one_off_wsp_host_pair_and_no_code() {
        let key = format!("SHA256:{}", "a".repeat(43));
        let codes = [
            format!("QW4K-7PZX.{key}"),
            format!("  QW4K-7PZX.{key}\n"),
            "QW4K7PZX".to_owned(),
            "QW4K-7PZX".to_owned(),
            " qw4k-7pzx ".to_owned(),
            format!("QW4K-7PZX.{key}\r\n"),
            "QW4K-7PZX.SHA256:aB".to_owned(),
            format!("QW4K-7PZX.{key}\nexpires     in 10m"),
        ];
        let missed: Vec<&String> = codes.iter().filter(|paste| !is_pair_code(&read_join_token(paste).code)).collect();
        assert!(missed.is_empty(), "{missed:?}");
        // A terminal's triple click takes the label too; a space for the dash and a ninth letter are typing slips.
        let none = [
            format!("code        QW4K-7PZX.{key}"),
            "QW4K 7PZX".to_owned(),
            "QW4K-7PZXA".to_owned(),
            "QW4K-7PZ".to_owned(),
            "QW4K-7PZO".to_owned(),
            String::new(),
        ];
        let taken: Vec<&String> = none.iter().filter(|paste| is_pair_code(&read_join_token(paste).code)).collect();
        assert!(taken.is_empty(), "{taken:?}");
        assert!(PAIR_CODE_SHAPE_REFUSAL.contains("wsp host pair"));
    }
}

// Edges no TS test pins, each printed by the TS module at a3b870a6e.
mod edges_the_ts_module_printed {
    use super::*;

    // devices.ts:70: dashes out, upper case by JavaScript's full mapping (ß is SS), the first eight UTF-16 units, and
    // a dash after the fourth only when more than four remain.
    #[test]
    fn shown_pair_code_groups_what_it_can_and_cuts_at_eight() {
        for (code, shown) in [
            ("", ""),
            ("abc", "ABC"),
            ("abcd", "ABCD"),
            ("abcde", "ABCD-E"),
            ("ab-cd-ef-gh", "ABCD-EFGH"),
            ("abcdefghijk", "ABCD-EFGH"),
            ("ÄÖ", "ÄÖ"),
            ("ß", "SS"),
        ] {
            assert_eq!(shown_pair_code(code), shown, "{code}");
        }
        assert_eq!(sent_pair_code(" a-b "), " AB ");
        assert_eq!(sent_pair_code("ß"), "SS");
    }

    // devices.ts:87: exactly eight characters of the alphabet, which has no I, L, O or U and no lower case.
    #[test]
    fn is_pair_code_takes_eight_characters_of_the_alphabet_alone() {
        for (code, takes) in [
            ("ABCDEFGH", true),
            ("ABCDEFG", false),
            ("ABCDEFGHI", false),
            ("ABCDEFGI", false),
            ("abcdefgh", false),
            ("ABCD-EFG", false),
            ("0123VWXY", true),
        ] {
            assert_eq!(is_pair_code(code), takes, "{code}");
        }
    }

    // devices.ts:233: trimmed whole, split at the first mark, the key trimmed and kept as typed, an empty key none;
    // the code half is taken as a screen's code is taken, so a space before the mark stays in it.
    #[test]
    fn read_join_token_splits_at_the_first_mark_and_keeps_the_key_as_typed() {
        let read = |t: &str| {
            let r = read_join_token(t);
            (r.code, r.host_key)
        };
        assert_eq!(read("  abcd-efgh.KEY/x+Y  "), ("ABCDEFGH".into(), Some("KEY/x+Y".into())));
        assert_eq!(read("abcd-efgh."), ("ABCDEFGH".into(), None));
        assert_eq!(read("abcdefgh"), ("ABCDEFGH".into(), None));
        assert_eq!(read(".key"), (String::new(), Some("key".into())));
        assert_eq!(read("a.b.c"), ("A".into(), Some("b.c".into())));
        assert_eq!(read("abcd-efgh . key "), ("ABCDEFGH ".into(), Some("key".into())));
        assert_eq!(read(""), (String::new(), None));
        assert_eq!(join_token("abcdefgh", "Fp+/="), "ABCD-EFGH.Fp+/=");
        assert_eq!(pair_token("ab", "k"), "AB.k");
    }

    // capabilities.ts:161: memory and disk each floor, the smaller wins, never below none. A missing or empty image
    // reads the memory alone; a zero machine size gives the disk's count; zero over zero is NaN, as the TS answers it.
    #[test]
    fn fork_room_at_its_edges() {
        let r = |mem_room: f64, disk: f64, mem: f64, image: Option<f64>| fork_room(&room(mem_room, disk), mem, image);
        assert_eq!(r(4000.0, 100.0, 1000.0, Some(30.0)), 3.0);
        assert_eq!(r(4000.0, 100.0, 1000.0, None), 4.0);
        assert_eq!(r(4000.0, 100.0, 1000.0, Some(0.0)), 4.0);
        assert_eq!(r(-10.0, 100.0, 1000.0, Some(30.0)), 0.0);
        assert_eq!(r(4000.0, 100.0, 0.0, Some(30.0)), 3.0);
        assert!(r(0.0, 100.0, 0.0, None).is_nan());
        assert_eq!(r(4000.0, -5.0, 1000.0, Some(30.0)), 0.0);
    }

    // capabilities.ts:133: a size is named when either half is there, a zero included.
    #[test]
    fn names_size_reads_either_half_a_zero_included() {
        assert!(!names_size(None, None));
        assert!(names_size(Some(0.0), None));
        assert!(names_size(None, Some(1.0)));
    }

    // devices.ts:19: a ticket socket's origin is the purpose's, never the frame's.
    #[test]
    fn ticket_origin_names_where_each_purpose_reached_the_host_from() {
        use wsp_wire::views::workspace::WorkspaceOrigin;
        assert_eq!(ticket_origin(TicketPurpose::Connect), WorkspaceOrigin::Here);
        assert_eq!(ticket_origin(TicketPurpose::Relay), WorkspaceOrigin::Relayed);
    }

    #[test]
    fn the_admission_transcript_and_the_cross_origin_refusal_are_their_bytes_and_words() {
        assert_eq!(device_admission_transcript("d", "b", "t"), b"wsp device admission v1\nd\nb\nt\n".to_vec());
        assert_eq!(cross_origin_refusal("http://a", "b:1"), "this request came from http://a and this host was reached at b:1; the page and its socket open from the address the host answers at");
    }

    // devices.ts:201: the page's frame keeps every key it sent, in its order, and may name any op but auth.
    #[test]
    fn a_daemon_frame_keeps_its_keys_in_order_and_never_names_auth() {
        let frame: DaemonFrame = reads_exact(json!({ "op": "fs.list", "path": "/root", "gitignore": true, "z": 1, "a": [2] }));
        assert_eq!(
            (frame.op.as_str(), frame.rest.keys().cloned().collect::<Vec<_>>()),
            ("fs.list", vec!["path".to_string(), "gitignore".into(), "z".into(), "a".into()])
        );
        let e = refused::<DaemonFrame>(json!({ "op": "auth", "token": "t" }));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("custom", path!["op"], "the host authenticates the channel"));
        assert_eq!(refused::<DaemonFrame>(json!({ "path": "/" })).path, path!["op"]);
    }
}
