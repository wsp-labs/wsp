// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/machine-link.test.ts, places.test.ts, protocol.test.ts, hosts-menu.test.ts and
//! place-state.test.ts: machine_link.
mod common;
use common::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wsp_wire::views::place::{MachineShare, WorkspaceCopy};
use wsp_wire::wire::limits::EXEC_BODY_MAX;
use wsp_wire::wire::machine_link::*;

/// `{...v, key: val}`: a copy of an object with one key set.
fn with(v: &Value, key: &str, val: Value) -> Value {
    let mut out = v.clone();
    out.as_object_mut().unwrap().insert(key.to_string(), val);
    out
}

fn create(spec: Value) -> Value {
    json!({"id": 1, "op": "machine.create", "spec": spec})
}

mod the_machine_ops_on_a_place_link {
    use super::*;

    // machine-link.test.ts:20
    #[test]
    fn parses_every_op_the_host_sends() {
        let frames = [
            json!({"id": 1, "op": "machine.backend"}),
            json!({"id": 2, "op": "machine.capacity"}),
            json!({"id": 3, "op": "machine.checkKey"}),
            json!({"id": 4, "op": "machine.create", "spec": {"kind": "sandbox", "template": "wsp/dev:template", "cpu": 2, "memMb": 4096}}),
            json!({"id": 5, "op": "machine.get", "machineId": "c1"}),
            json!({"id": 6, "op": "machine.list", "labels": {"wsp": "1"}}),
            json!({"id": 13, "op": "machine.exec", "machineId": "c1", "cmd": "echo hi", "timeoutMs": 1000}),
            json!({"id": 15, "op": "machine.pause", "machineId": "c1"}),
            json!({"id": 16, "op": "machine.resume", "machineId": "c1"}),
            json!({"id": 17, "op": "machine.kill", "machineId": "c1"}),
            json!({"id": 18, "op": "machine.state", "machineId": "c1"}),
            json!({"id": 19, "op": "machine.describe", "machineId": "c1"}),
            json!({"id": 20, "op": "machine.facts", "machineId": "c1"}),
            json!({"id": 21, "op": "machine.metrics", "machineId": "c1"}),
            json!({"id": 22, "op": "machine.daemonAnswers", "machineId": "c1", "timeoutMs": 500}),
            json!({"id": 23, "op": "machine.previewUrl", "machineId": "c1", "port": 7070}),
            json!({"id": 24, "op": "machine.downloadUrl", "machineId": "c1", "path": "/root/a"}),
            json!({"id": 25, "op": "machine.uploadUrl", "machineId": "c1", "path": "/root/a"}),
            json!({"id": 26, "op": "machine.putBytes", "machineId": "c1", "path": "/root/a", "uploadId": "u1", "seq": 0, "last": true, "data": "AAAA"}),
        ];
        for frame in frames {
            let got = reads_with(parse_machine_link_request, frame.clone());
            assert_eq!(serde_json::to_value(&got).unwrap()["op"], frame["op"], "{frame}");
        }
    }

    // machine-link.test.ts:45
    #[test]
    fn refuses_the_eight_frames_a_layer_store_answered_a_place_keeps_no_image_to_save_nam() {
        let gone = [
            json!({"id": 1, "op": "machine.snapshot", "machineId": "c1", "name": "v1", "life": {"firstLife": true}}),
            json!({"id": 2, "op": "machine.snapshotJob", "job": "3f9a1c2b4d5e6f70"}),
            json!({"id": 3, "op": "machine.deleteSnapshot", "snapshotId": "sha256:aa"}),
            json!({"id": 4, "op": "machine.listSnapshots"}),
            json!({"id": 5, "op": "machine.promoteSnapshot", "snapshotId": "sha256:aa", "name": "v1"}),
            json!({"id": 6, "op": "machine.getTemplate", "templateId": "wsp/dev:template"}),
            json!({"id": 7, "op": "machine.listTemplates"}),
            json!({"id": 8, "op": "machine.deleteTemplate", "templateId": "wsp/dev:template"}),
        ];
        for frame in gone {
            let e = refused_with(parse_machine_link_request, frame.clone());
            assert_eq!(e.code, "invalid_union_discriminator", "{frame}");
            assert_eq!(e.path, path!["op"], "{frame}");
        }
    }

    // machine-link.test.ts:59
    #[test]
    fn takes_a_create_that_brings_a_project_of_the_computer_s_own_and_refuses_one_whose_p() {
        let copy = json!({"from": "/wsp/projects/wsp/checkout", "at": "/Users/maya/wsp"});
        let ok = |spec: Value| {
            reads_with(parse_machine_link_request, create(spec));
        };
        ok(json!({"kind": "sandbox", "copy": copy}));
        // A workspace of the computer with no project in it is the same frame without the field.
        ok(json!({"kind": "sandbox"}));
        let _: WorkspaceCopy = reads(copy.clone());
        // Both sides are absolute and plain: a relative path, a path that is not one, and an empty one are refused
        // at the wire rather than quoted in a mount command on the far side.
        // A folder of the computer's bound into the workspace is read by the same rule the copy and the shares are.
        let bind = json!({"source": "/wsp/projects/pr_1/memory", "target": "/root/.claude-cfg/projects/-root-wsp/memory"});
        ok(json!({"kind": "sandbox", "binds": [bind]}));
        ok(json!({"kind": "sandbox", "binds": [with(&bind, "readOnly", json!(true))]}));
        let no = |spec: Value, at: Value, code: &str| {
            let e = refused_with(parse_machine_link_request, create(spec.clone()));
            assert_eq!(e.path, vec![json!("spec")].into_iter().chain(at.as_array().unwrap().clone()).collect::<Vec<_>>(), "{spec}");
            assert_eq!(e.code, code, "{spec}");
        };
        no(
            json!({"kind": "sandbox", "binds": [with(&bind, "source", json!("projects/pr_1/memory"))]}),
            json!(["binds", 0, "source"]),
            "custom",
        );
        no(
            json!({"kind": "sandbox", "binds": [with(&bind, "source", json!("/wsp/projects/../../root/.ssh"))]}),
            json!(["binds", 0, "source"]),
            "custom",
        );
        no(json!({"kind": "sandbox", "binds": [with(&bind, "target", json!("/root/../etc"))]}), json!(["binds", 0, "target"]), "custom");
        no(json!({"kind": "sandbox", "binds": [{"source": bind["source"]}]}), json!(["binds", 0, "target"]), "invalid_type");
        no(json!({"kind": "sandbox", "copy": with(&copy, "from", json!("projects/wsp/checkout"))}), json!(["copy", "from"]), "custom");
        no(json!({"kind": "sandbox", "copy": with(&copy, "at", json!("wsp"))}), json!(["copy", "at"]), "custom");
        no(json!({"kind": "sandbox", "copy": with(&copy, "at", json!("/Users/maya/wsp; rm -rf /"))}), json!(["copy", "at"]), "custom");
        no(json!({"kind": "sandbox", "copy": with(&copy, "from", json!(""))}), json!(["copy", "from"]), "too_small");
        no(json!({"kind": "sandbox", "copy": {"from": copy["from"]}}), json!(["copy", "at"]), "invalid_type");
        // A path that walks up out of itself is refused on both halves: what it resolves to is a folder on the
        // computer, and a bind mount over one of those cannot be taken back.
        no(json!({"kind": "sandbox", "copy": with(&copy, "at", json!("/Users/../../etc"))}), json!(["copy", "at"]), "custom");
        no(
            json!({"kind": "sandbox", "copy": with(&copy, "from", json!("/wsp/projects/../../root/.ssh"))}),
            json!(["copy", "from"]),
            "custom",
        );
        no(json!({"kind": "sandbox", "copy": with(&copy, "at", json!("/Users/./wsp"))}), json!(["copy", "at"]), "custom");
        // A folder whose name begins with a dot is a folder, and half a home is made of them.
        ok(json!({"kind": "sandbox", "copy": with(&copy, "at", json!("/root/.claude/projects"))}));
    }

    // machine-link.test.ts:90
    #[test]
    fn takes_a_create_that_shares_the_computer_s_own_logins_into_the_workspace_and_refuse() {
        let share = json!({"source": "/wsp/logins/codex/auth.json", "target": "/root/.codex/auth.json"});
        let ok = |spec: Value| {
            reads_with(parse_machine_link_request, create(spec));
        };
        ok(json!({"kind": "sandbox", "shares": [share]}));
        // A workspace that shares none is the same frame without the field, and the empty list is no shares either.
        ok(json!({"kind": "sandbox"}));
        ok(json!({"kind": "sandbox", "shares": []}));
        let _: MachineShare = reads(share.clone());
        // Both sides are absolute and plain, and neither walks up out of itself: where the file on the computer has
        // to live is the daemon's own wall behind this one, and a bind mount is what cannot be taken back.
        let no = |spec: Value, at: Value, code: &str| {
            let e = refused_with(parse_machine_link_request, create(spec.clone()));
            assert_eq!(e.path, vec![json!("spec")].into_iter().chain(at.as_array().unwrap().clone()).collect::<Vec<_>>(), "{spec}");
            assert_eq!(e.code, code, "{spec}");
        };
        no(
            json!({"kind": "sandbox", "shares": [with(&share, "source", json!("logins/codex/auth.json"))]}),
            json!(["shares", 0, "source"]),
            "custom",
        );
        no(
            json!({"kind": "sandbox", "shares": [with(&share, "source", json!("/wsp/logins/../../root/.ssh/id"))]}),
            json!(["shares", 0, "source"]),
            "custom",
        );
        no(
            json!({"kind": "sandbox", "shares": [with(&share, "target", json!("/root/../etc/passwd"))]}),
            json!(["shares", 0, "target"]),
            "custom",
        );
        no(json!({"kind": "sandbox", "shares": [with(&share, "target", json!(""))]}), json!(["shares", 0, "target"]), "too_small");
        no(json!({"kind": "sandbox", "shares": [{"source": share["source"]}]}), json!(["shares", 0, "target"]), "invalid_type");
    }

    // machine-link.test.ts:107
    #[test]
    fn reads_where_a_computer_keeps_the_logins_it_shares_and_nothing_that_is_not_a_path() {
        let facts = json!({
            "offer": "runtime",
            "capabilities": {
                "liveCloneForks": false,
                "replacesMachine": true,
                "previewUrls": false,
                "signedUrls": false,
                "callbackRelay": false,
                "diskSnapshots": true,
                "images": true,
                "snapshotsAnyLife": false,
                "snapshotListing": true,
                "templates": true,
                "sizes": [],
                "kept": false,
                "copies": true,
                "ownNetwork": true,
            },
            "pricing": {"defaultSize": {"cpu": 2, "memMb": 4096}, "snapshotStorage": {"freeGb": 0, "usdPerGbMonth": 0, "billedFrom": ""}},
        });
        // A provider shares none, so the field is absent there and present on a computer the person owns.
        let bare: BackendFacts = reads(facts.clone());
        assert_eq!(bare.logins, None);
        let kept: BackendFacts = reads(with(&facts, "logins", json!("/wsp/logins")));
        assert_eq!(kept.logins.as_deref(), Some("/wsp/logins"));
        let e = refused::<BackendFacts>(with(&facts, "logins", json!("logins")));
        assert_eq!((e.code.as_str(), e.path), ("custom", path!["logins"]));
        let e = refused::<BackendFacts>(with(&facts, "logins", json!("")));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["logins"]));
    }

    // machine-link.test.ts:135
    #[test]
    fn refuses_an_op_it_does_not_carry_a_part_out_of_range_and_a_command_over_the_body_ca() {
        let e = refused_with(parse_machine_link_request, json!({"id": 1, "op": "machine.nonsense"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_union_discriminator", path!["op"]));
        let e = refused_with(
            parse_machine_link_request,
            json!({"id": 1, "op": "machine.putBytes", "machineId": "c1", "path": "/a", "uploadId": "u", "seq": -1, "last": false, "data": ""}),
        );
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["seq"]));
        let e = refused_with(
            parse_machine_link_request,
            json!({"id": 1, "op": "machine.exec", "machineId": "c1", "cmd": "x".repeat(EXEC_BODY_MAX as usize + 1)}),
        );
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["cmd"]));
    }

    // machine-link.test.ts:141
    #[test]
    fn carries_a_refusal_s_kind_and_status_and_still_reads_a_refusal_with_neither() {
        let r: DaemonErrorResponse = reads(json!({"id": 1, "ok": false, "error": "no such container", "kind": "missing", "status": 404}));
        assert_eq!(r.kind, Some(MachineErrorKind::Missing));
        assert_eq!(r.status, Some(404));
        let bare: DaemonErrorResponse = reads(json!({"id": 1, "ok": false, "error": "no"}));
        assert_eq!(bare.kind, None);
        assert_eq!(bare.status, None);
    }

    // machine-link.test.ts:148
    // The constant alone passes without a stub, so the part is also read as a putBytes frame whose data is the base64
    // of one whole part.
    #[test]
    fn sends_a_file_in_parts_of_four_mebibytes() {
        let bytes = MACHINE_PUT_PART_BYTES as usize;
        let data = "A".repeat(bytes.div_ceil(3) * 4);
        let frame = json!({"id": 1, "op": "machine.putBytes", "machineId": "c1", "path": "/root/a", "uploadId": "u1", "seq": 0, "last": false, "data": data});
        let _ = reads_with(parse_machine_link_request, frame);
        assert_eq!(MACHINE_PUT_PART_BYTES, 4 * 1024 * 1024);
    }
}

mod the_one_word_a_row_says_about_the_daemon_a_place_runs {
    use super::*;

    // places.test.ts:419
    #[test]
    fn says_which_computer_answers_a_workspace_that_runs_no_daemon_of_its_own_rather_than() {
        assert_eq!(
            place_serves_daemon_line("landing-a", "spoo"),
            "landing-a has no daemon of its own: spoo answers its files and git through this host"
        );
    }

    // places.test.ts:423
    #[test]
    fn says_whose_the_ports_and_the_load_are_where_a_pane_asks_a_workspace_for_readings_i() {
        assert_eq!(
            place_watches_itself_line("spoo"),
            "spoo watches its own ports and load, which are that computer's rather than one workspace's"
        );
    }

    // places.test.ts:427
    #[test]
    fn says_a_workspace_s_processes_on_a_computer_somebody_owns_are_not_readable_from_her() {
        assert_eq!(fork_procs_unread_line("landing-a", "spoo"), "landing-a's processes on spoo are not readable from here yet");
    }

    // places.test.ts:431
    #[test]
    fn says_a_workspace_on_a_computer_somebody_owns_is_driven_from_here_by_its_shells_fil() {
        assert_eq!(fork_op_refused_line("exec", "landing-a", "spoo"), "spoo answers landing-a's shells, files and git from here, not exec");
    }
}

mod the_one_rule_that_decides_whether_a_computer_can_be_a_place {
    use super::*;

    const CONTROLLERS: &str = "cpuset cpu io memory pids\n";
    const FILESYSTEMS: &str = "nodev sysfs\next4\nnodev overlay\n";

    /// A kernel, as the rule reads one: the files it asks for, and nothing else on this machine.
    fn cgroup() -> HashMap<String, String> {
        HashMap::from([
            (CGROUP_CONTROLLERS_PATH.to_string(), CONTROLLERS.to_string()),
            (PROC_FILESYSTEMS_PATH.to_string(), FILESYSTEMS.to_string()),
        ])
    }

    fn at(files: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
        move |path| files.get(path).cloned()
    }

    fn unasked(args: &[&str]) -> ModprobeRun {
        panic!("modprobe {} was asked", args.join(" "))
    }

    fn blocked(read: &dyn Fn(&str) -> Option<String>, modprobe: &dyn Fn(&[&str]) -> ModprobeRun, euid: Option<u32>) -> Option<String> {
        workspaces_blocked_by(&KernelAt { platform: "linux", read, modprobe, euid })
    }

    // places.test.ts:586
    #[test]
    fn takes_a_linux_box_with_cgroup_v2_the_two_controllers_a_cap_needs_an_overlay_and_ro() {
        let files = cgroup();
        assert_eq!(blocked(&at(&files), &unasked, Some(0)), None);
    }

    // places.test.ts:590
    #[test]
    fn turns_down_anything_that_is_not_linux_before_it_reads_a_file_at_all() {
        let files = cgroup();
        let asked = Cell::new(0);
        let counting = |path: &str| {
            asked.set(asked.get() + 1);
            files.get(path).cloned()
        };
        let said = workspaces_blocked_by(&KernelAt { platform: "darwin", read: &counting, modprobe: &unasked, euid: Some(0) });
        assert_eq!(said.as_deref(), Some("wsp runs workspaces on a Linux computer"));
        assert_eq!(asked.get(), 0);
    }

    // places.test.ts:600
    #[test]
    fn names_cgroup_v1_when_the_controllers_file_is_not_there_since_that_is_what_mountin() {
        let files = HashMap::from([(PROC_FILESYSTEMS_PATH.to_string(), FILESYSTEMS.to_string())]);
        let said = blocked(&at(&files), &unasked, Some(0)).expect("a reason");
        assert!(said.contains("cgroup v1"), "{said}");
        assert!(said.contains("systemd.unified_cgroup_hierarchy=1"), "{said}");
    }

    // places.test.ts:606
    #[test]
    fn names_the_controller_a_cap_needs_and_does_not_have_one_at_a_time() {
        let without = |drop: &str| {
            let mut files = cgroup();
            files.insert(CGROUP_CONTROLLERS_PATH.to_string(), CONTROLLERS.replace(&format!("{drop} "), ""));
            files
        };
        let files = without("memory");
        assert_eq!(
            blocked(&at(&files), &unasked, Some(0)).as_deref(),
            Some("this computer's cgroup root offers no memory controller, which wsp needs to run workspaces here")
        );
        let files = without("cpu");
        assert_eq!(
            blocked(&at(&files), &unasked, Some(0)).as_deref(),
            Some("this computer's cgroup root offers no cpu controller, which wsp needs to run workspaces here")
        );
    }

    // places.test.ts:612
    #[test]
    fn loads_an_overlay_module_the_kernel_does_not_list_yet_and_reads_the_list_again_befo() {
        let mut start = cgroup();
        start.insert(PROC_FILESYSTEMS_PATH.to_string(), "nodev sysfs\next4\n".to_string());
        let files = RefCell::new(start);
        let asked: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
        let read = |path: &str| files.borrow().get(path).cloned();
        let modprobe = |args: &[&str]| {
            asked.borrow_mut().push(args.iter().map(|a| a.to_string()).collect());
            files.borrow_mut().get_mut(PROC_FILESYSTEMS_PATH).unwrap().push_str("nodev\toverlay\n");
            ModprobeRun::Ran { ok: true, stderr: String::new() }
        };
        assert_eq!(blocked(&read, &modprobe, Some(0)), None);
        assert_eq!(*asked.borrow(), vec![vec!["overlay".to_string()]]);
    }

    // places.test.ts:624
    #[test]
    fn refuses_a_kernel_with_no_overlay_module_to_load_in_the_doctor_s_words_whether_the() {
        // kmod 31's answer for a module that is not on disk, to the load and to the dry run alike (measured on 6.8.0-117).
        let asked: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
        let modprobe = |args: &[&str]| {
            asked.borrow_mut().push(args.iter().map(|a| a.to_string()).collect());
            ModprobeRun::Ran {
                ok: false,
                stderr: "modprobe: FATAL: Module overlay not found in directory /lib/modules/6.8.0-146-generic\n".to_string(),
            }
        };
        let absent = "this computer's kernel has no overlay filesystem and no overlay module to load, which a workspace here reads this computer's own directories through";
        let only_controllers = HashMap::from([(CGROUP_CONTROLLERS_PATH.to_string(), CONTROLLERS.to_string())]);
        assert_eq!(blocked(&at(&only_controllers), &modprobe, Some(0)).as_deref(), Some(absent));
        let mut unlisted = cgroup();
        unlisted.insert(PROC_FILESYSTEMS_PATH.to_string(), "nodev sysfs\next4\n".to_string());
        assert_eq!(blocked(&at(&unlisted), &modprobe, Some(0)).as_deref(), Some(absent));
        let want: Vec<Vec<String>> = [vec!["overlay"], vec!["-n", "overlay"], vec!["overlay"], vec!["-n", "overlay"]]
            .iter()
            .map(|c| c.iter().map(|a| a.to_string()).collect())
            .collect();
        assert_eq!(*asked.borrow(), want);
    }

    // places.test.ts:637
    #[test]
    fn refuses_a_module_that_will_not_load_in_the_doctor_s_words_with_modprobe_s_own_last() {
        let modprobe = |args: &[&str]| {
            if args[0] == "-n" {
                ModprobeRun::Ran { ok: true, stderr: String::new() }
            } else {
                ModprobeRun::Ran { ok: false, stderr: "modprobe: ERROR: could not insert 'overlay': Operation not permitted\n".to_string() }
            }
        };
        let mut files = cgroup();
        files.insert(PROC_FILESYSTEMS_PATH.to_string(), "nodev sysfs\next4\n".to_string());
        let read = at(&files);
        assert_eq!(
            blocked(&read, &modprobe, Some(0)).as_deref(),
            Some("this computer's kernel has an overlay module that would not load (modprobe: ERROR: could not insert 'overlay': Operation not permitted), and a workspace here reads this computer's own directories through an overlay")
        );
        let loads_and_still_lists = |_: &[&str]| ModprobeRun::Ran { ok: true, stderr: String::new() };
        assert_eq!(
            blocked(&read, &loads_and_still_lists, Some(0)).as_deref(),
            Some("this computer's kernel has an overlay module that would not load (modprobe loaded it and the kernel still lists no overlay), and a workspace here reads this computer's own directories through an overlay")
        );
        let cannot_run = |_: &[&str]| ModprobeRun::Failed { error: "ENOENT".to_string() };
        assert_eq!(
            blocked(&read, &cannot_run, Some(0)),
            Some(format!("this computer's kernel has an overlay module that would not load ({MODPROBE_PATH} could not run: ENOENT), and a workspace here reads this computer's own directories through an overlay"))
        );
    }

    // places.test.ts:652
    #[test]
    fn says_the_refusal_of_a_named_computer_with_its_and_it_never_it_s_or_this_computer_w() {
        let overlay = "this computer's kernel has no overlay filesystem and no overlay module to load, which a workspace here reads this computer's own directories through";
        assert_eq!(place_cannot_boot_line("hetzner", Some(overlay)), "hetzner cannot run wsp workspaces: its kernel has no overlay filesystem and no overlay module to load, which a workspace here reads its own directories through");
        assert_eq!(
            place_cannot_boot_line("hetzner", Some("wsp runs workspaces on this computer as root, and this daemon is not root")),
            "hetzner cannot run wsp workspaces: wsp runs workspaces on it as root, and this daemon is not root"
        );
    }

    // places.test.ts:658
    #[test]
    fn names_root_last_so_a_box_that_has_everything_else_reads_the_one_thing_a_person_ca() {
        let files = cgroup();
        assert_eq!(
            blocked(&at(&files), &unasked, Some(1000)).as_deref(),
            Some("wsp runs workspaces on this computer as root, and this daemon is not root")
        );
        let said = blocked(&at(&files), &unasked, None).expect("a reason");
        assert!(said.contains("as root"), "{said}");
    }

    // places.test.ts:663
    #[test]
    fn asks_in_one_order_so_the_reason_a_person_reads_is_the_first_thing_missing_rather_() {
        // A Mac with none of it reads the platform, not the cgroup; a Linux box missing both cgroup and overlay reads
        // the cgroup. Every sentence above is reachable, and only the first one that applies is ever said.
        let none = HashMap::new();
        let mac = workspaces_blocked_by(&KernelAt { platform: "darwin", read: &at(&none), modprobe: &unasked, euid: Some(1000) })
            .expect("a reason");
        assert!(mac.contains("Linux computer"), "{mac}");
        let linux = blocked(&at(&none), &unasked, Some(1000)).expect("a reason");
        assert!(linux.contains("cgroup v1"), "{linux}");
        let controllers_only = HashMap::from([(CGROUP_CONTROLLERS_PATH.to_string(), "cpu memory\n".to_string())]);
        let modprobe = |_: &[&str]| ModprobeRun::Ran { ok: false, stderr: String::new() };
        let overlay = blocked(&at(&controllers_only), &modprobe, Some(1000)).expect("a reason");
        assert!(overlay.contains("overlay"), "{overlay}");
    }
}

mod the_sentences_a_vault_archive_is_refused_with {
    use super::*;

    // protocol.test.ts:1435
    #[test]
    fn the_member_refusal_names_the_member_the_reason_and_what_that_refusal_did_in_the_ro() {
        let why = "it lands at /etc/cron.d/x, which is not one of the paths the seal asked for or under one";
        for road in [VaultRoad::Seal, VaultRoad::Import] {
            let line = vault_member_refusal(road, "etc/cron.d/x", why);
            assert!(line.contains("etc/cron.d/x"), "{line}");
            assert!(line.contains("not one of the paths the seal asked for"), "{line}");
        }
        // Nothing was ever going to be imported at the seal, and no copy is what the import stopped.
        let seal = vault_member_refusal(VaultRoad::Seal, "etc/cron.d/x", why);
        assert!(seal.contains("the seal is refused and no version is recorded"), "{seal}");
        assert!(!seal.contains("imported"), "{seal}");
        let import = vault_member_refusal(VaultRoad::Import, "etc/cron.d/x", why);
        assert!(import.contains("nothing of it was imported"), "{import}");
        assert!(!import.contains("the seal is refused"), "{import}");
    }

    // protocol.test.ts:1448
    #[test]
    fn the_unlisted_refusal_names_the_image_and_its_version_and_says_to_cut_the_next_one() {
        let line = vault_unlisted_refusal("default", 1);
        assert!(line.contains("default v1"), "{line}");
        assert!(line.contains("cut the next version"), "{line}");
    }

    // protocol.test.ts:1454
    #[test]
    fn neither_sentence_carries_a_path_of_the_computer_the_record_sits_on() {
        for line in [
            vault_member_refusal(VaultRoad::Seal, "root/.codex/auth.json", "it points at /etc"),
            vault_member_refusal(VaultRoad::Import, "root/.codex/auth.json", "it points at /etc"),
            vault_unlisted_refusal("default", 2),
        ] {
            for path in ["/Users/", "/home/", ".wsp", "state.json"] {
                assert!(!line.contains(path), "{line} carries {path}");
            }
        }
    }
}

mod what_the_computers_table_says_about_a_computer {
    use super::*;

    // hosts-menu.test.ts:100
    #[test]
    fn names_the_place_in_the_forks_nowhere_line_which_is_a_record_that_went_and_never_a() {
        // Every computer on the list forks: a box whose kernel cannot is turned down at the join and again at every
        // link, so the only way to reach this line is a place the host stopped holding.
        assert_eq!(place_forks_nowhere_line("laptop"), "laptop is no longer a place in this wsp, so nothing forks there");
    }
}

mod the_word_a_place_s_row_says {
    use super::*;

    // place-state.test.ts:252
    // Only the blocked word and the sentence a blocked place carries; the state word itself is the views writer's.
    #[test]
    fn puts_blocked_then_not_answering_then_at_limit_then_full_then_the_setup_running_th() {
        assert_eq!(place_blocked("solari", false, Some("no overlay")).as_deref(), Some("solari cannot run wsp workspaces: no overlay"));
        assert_eq!(place_blocked("solari", true, Some("no overlay")), None);
        assert_eq!(PLACE_BLOCKED_WORD, "can't run threads");
    }
}

// Edges no TS test pins, each printed by the TS module at a3b870a6e.
mod edges_the_ts_module_printed {
    use wsp_wire::wire::machine_link::*;

    // machine-link.ts:456: "this computer's" reads "its" and "this computer" reads "it", every one of them, lower
    // case only; an empty reason still takes the colon, and no reason at all the other sentence.
    #[test]
    fn the_cannot_boot_line_turns_this_computer_into_it_everywhere_and_keeps_an_empty_reason() {
        assert_eq!(place_cannot_boot_line("box", None), "box cannot run wsp workspaces, so it cannot be a place");
        assert_eq!(
            place_cannot_boot_line("box", Some("this computer mounts cgroup v1 and this computer's root is this computer")),
            "box cannot run wsp workspaces: it mounts cgroup v1 and its root is it"
        );
        assert_eq!(place_cannot_boot_line("box", Some("")), "box cannot run wsp workspaces: ");
        assert_eq!(place_cannot_boot_line("box", Some("This computer x")), "box cannot run wsp workspaces: This computer x");
        assert_eq!(place_blocked("box", true, Some("x")), None);
        assert_eq!(place_blocked("box", false, None).as_deref(), Some("box cannot run wsp workspaces, so it cannot be a place"));
        assert_eq!(place_blocked("box", false, Some("no root")).as_deref(), Some("box cannot run wsp workspaces: no root"));
    }

    // machine-link.ts:469: only a door the report says is false gets a line; absent is a daemon older than the door.
    #[test]
    fn the_no_tools_line_speaks_only_of_a_door_that_said_it_is_shut() {
        assert_eq!(place_no_tools_line("box", None, None, None), None);
        assert_eq!(place_no_tools_line("box", Some(true), Some("x"), None), None);
        assert_eq!(
            place_no_tools_line("box", Some(false), None, None).as_deref(),
            Some("threads on box get no wsp tools: its daemon opened no door for them; remove box and add it again")
        );
        assert_eq!(
            place_no_tools_line("box", Some(false), Some("socket in use"), Some("wsp.service")).as_deref(),
            Some("threads on box get no wsp tools: socket in use; restart wsp's daemon there with systemctl restart wsp.service")
        );
    }

    #[test]
    fn the_two_part_refusals_and_the_lists_inside_sentences() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(place_drop_stopped_line("why", &[]), "why");
        assert_eq!(place_drop_stopped_line("why", &names(&["a", "b"])), "why; a, b had already gone with the remove");
        let unsaved = place_unsaved_refusal("box", &names(&["x; y", "z"]));
        assert_eq!(unsaved.said, "box holds work no remote has, which a remove would lose: x; y; z");
        assert_eq!(unsaved.fix, "Keep a fork's work with wsp export or by pushing its branch, and copy a project folder's off box from the path named; then remove box again, or wsp remove box --force removes it anyway.");
        let away = place_away_refusal("box", "box is off");
        assert_eq!(away.said, "box is off, and its forks and projects go over its link");
        assert_eq!(away.fix, "Turn box on and remove it again once it answers; if it never will, wsp remove box --forget takes it, its forks, projects and threads out of this wsp, with nothing done on it beyond a try at wsp's own leave over ssh.");
        let forget = place_forget_answers_refusal("box");
        assert_eq!(
            (forget.said.as_str(), forget.fix.as_str()),
            ("box is answering, so there is nothing to forget", "wsp remove box takes it out and takes wsp off it.")
        );
        let project = project_unsaved_refusal("p", "u");
        assert_eq!((project.said.as_str(), project.fix.as_str()), ("u, which removing p would lose", "Copy that work off its computer from the path named or push its branches, then remove p again; wsp projects remove p --force removes it anyway."));
        assert_eq!(no_such_place_refusal("x", &[]), "no place named x; you have ");
    }

    // machine-link.ts:488: a computer row, and not the one the host runs on (id "here").
    #[test]
    fn is_joined_computer_is_a_computer_row_that_is_not_here() {
        assert!(!is_joined_computer("here", "computer"));
        assert!(is_joined_computer("p", "computer"));
        assert!(!is_joined_computer("p", "cloud"));
    }

    // machine-link.ts:155 and 510: the error words in their TS order, which a refusal's `kind` and `code` carry.
    #[test]
    fn the_machine_error_kinds_and_daemon_error_codes_are_the_wire_words() {
        let kinds = ["concurrency", "plan", "missing", "conflict", "snapshotUnavailable", "transient", "auth", "unknown", "absent"];
        for word in kinds {
            let read: MachineErrorKind = super::common::reads(serde_json::json!(word));
            assert_eq!(serde_json::to_value(read).unwrap(), serde_json::json!(word));
        }
        let codes = [
            "unsupported",
            "outside-root",
            "not-found",
            "not-a-directory",
            "not-a-file",
            "not-a-git-repo",
            "bad-request",
            "forbidden",
            "no-host-cli",
            "no-git-credential",
            "rate-limited",
            "on-default-branch",
        ];
        for word in codes {
            let read: DaemonErrorCode = super::common::reads(serde_json::json!(word));
            assert_eq!(serde_json::to_value(read).unwrap(), serde_json::json!(word));
        }
        assert_eq!(super::common::refused::<DaemonErrorCode>(serde_json::json!("gone")).code, "invalid_enum_value");
    }

    // machine-link.ts:113: a snapshot's parent is absent, null at a root, or the snapshot it was taken under, and each
    // writes back as it came.
    #[test]
    fn a_snapshot_row_keeps_an_absent_a_null_and_a_named_parent_apart() {
        let root: SnapshotRow = super::common::reads_exact(serde_json::json!({ "id": "s1", "sizeBytes": 10, "parent": null }));
        assert_eq!(root.parent, Some(None));
        let child: SnapshotRow = super::common::reads_exact(serde_json::json!({ "id": "s2", "sizeBytes": 10, "parent": "s1" }));
        assert_eq!(child.parent, Some(Some("s1".to_string())));
        let unknown: SnapshotRow = super::common::reads_exact(serde_json::json!({ "id": "s3", "sizeBytes": 10.5 }));
        assert_eq!((unknown.parent, unknown.size_bytes.0), (None, 10.5));
    }
}
