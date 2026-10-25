// SPDX-License-Identifier: AGPL-3.0-only
//! The pinned cases of packages/protocol/src/workspace-state.ts, the other half of workspace_state.rs.

mod support;

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::workspace_state::*;

    use serde_json::{json, Value};
    use wsp_wire::views::place::PlaceBack;
    use wsp_wire::views::workspace::{ReachState, WorkspaceView};
    use wsp_wire::views::workspace_kind::WorkspaceKind;
    use wsp_wire::wire::capabilities::PauseMode;

    fn from<T: serde::de::DeserializeOwned>(v: Value) -> T {
        serde_json::from_value(v).unwrap()
    }
    fn kind(k: &str) -> WorkspaceKind {
        from(json!(k))
    }
    fn state(s: &str) -> WorkspaceState {
        from(json!(s))
    }
    fn pause(p: &str) -> PauseMode {
        from(json!(p))
    }
    fn block(b: &str) -> SendBlock {
        from(json!(b))
    }
    fn reach(r: &str) -> ReachState {
        from(json!(r))
    }
    fn reach_word(r: ReachState) -> String {
        serde_json::to_value(r).unwrap().as_str().unwrap().to_string()
    }
    fn back(v: Value) -> PlaceBack {
        from(v)
    }
    fn road(v: Value) -> PlaceRoad {
        PlaceRoad { ssh: v["ssh"].as_str().map(Into::into), from: v["from"].as_str().map(Into::into), back: None }
    }
    fn view(v: Value) -> WorkspaceView {
        from(crate::support::over(crate::support::workspace_view(), v))
    }
    fn input(v: Value) -> WorkspaceStateInput {
        WorkspaceStateInput {
            phase: from(v["phase"].clone()),
            machine_state: v.get("machineState").map(|m| from(m.clone())),
            reach: v.get("reach").map(|r| from(r.clone())),
            wake_refused: v["wakeRefused"].as_str().map(Into::into),
        }
    }

    #[test]
    fn the_sign_in_road_and_its_refusal_follow_the_kind() {
        assert_eq!(sign_in_road(None), "sign this workspace in from the Workspace panel"); // signInRoad({})
        assert_eq!(sign_in_refusal_line(None), "sign this workspace in from the Workspace panel, then send again"); // signInRefusalLine({})
        assert_eq!(sign_in_road(Some(&kind("cloud"))), "sign this workspace in from the Workspace panel"); // signInRoad({"kind":"cloud"})
        assert_eq!(sign_in_refusal_line(Some(&kind("cloud"))), "sign this workspace in from the Workspace panel, then send again"); // signInRefusalLine({"kind":"cloud"})
        assert_eq!(sign_in_road(Some(&kind("local"))), "sign in from a terminal on this computer"); // signInRoad({"kind":"local"})
        assert_eq!(sign_in_refusal_line(Some(&kind("local"))), "sign in from a terminal on this computer, then send again"); // signInRefusalLine({"kind":"local"})
        assert_eq!(sign_in_road(Some(&kind("place"))), "sign in from a terminal on the computer the thread runs on"); // signInRoad({"kind":"place"})
        assert_eq!(
            sign_in_refusal_line(Some(&kind("place"))),
            "sign in from a terminal on the computer the thread runs on, then send again"
        ); // signInRefusalLine({"kind":"place"})
        assert_eq!(signed_out_line("Codex", "spoo"), "Codex is not signed in on spoo; sign in from a terminal there before you send");
        // signedOutLine("Codex", "spoo")
    }

    #[test]
    fn each_kind_says_its_own_machine_and_what_a_delete_takes() {
        assert_eq!(agents_kind_refusal(&kind("cloud")), "agents on a machine wsp forks cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer"); // agentsKindRefusal("cloud")
        assert_eq!(machine_word(&kind("cloud")), "a machine wsp forks"); // machineWord("cloud")
        assert_eq!(thread_place(&kind("cloud"), "spoo"), "a machine wsp forks"); // threadPlace("cloud", "spoo")
        assert_eq!(
            delete_notice(1.0, &kind("cloud"), None, None, None),
            "Its computer is deleted in the cloud; its record and 1 thread leave this computer."
        ); // deleteNotice(1, "cloud")
        assert_eq!(
            delete_notice(3.0, &kind("cloud"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None),
            "Its worktree at /r/wt/a is removed and the project folder is left as it is; its record and 3 threads leave this computer."
        ); // deleteNotice(3, "cloud", {path:"/r/wt/a"})
        assert_eq!(
            delete_notice(0.0, &kind("cloud"), None, Some(""), None),
            "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."
        ); // deleteNotice(0, "cloud", undefined, "")
        assert_eq!(
            delete_notice(2.0, &kind("cloud"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })),
            "Its copy on spoo is deleted; its record and 2 threads leave this computer."
        ); // deleteNotice(2, "cloud", undefined, "m1", {name:"web", computer:"spoo"})
        assert_eq!(on_delete_of(&kind("cloud"), None, None, None).done_for("m-7"), "computer m-7 is gone in the cloud"); // onDeleteOf("cloud").done("m-7")
        assert_eq!(
            on_delete_of(&kind("cloud"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })).done_for("m1"),
            "web is deleted from spoo"
        ); // onDeleteOf("cloud", undefined, "m1", {name:"web", computer:"spoo"}).done("m1")
        assert_eq!(
            on_delete_of(&kind("cloud"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None).done_for("m1"),
            "its worktree at /r/wt/a is removed and the project folder is left as it is"
        ); // onDeleteOf("cloud", {path:"/r/wt/a"}).done("m1")
        assert_eq!(on_delete_of(&kind("cloud"), None, Some(""), None).done_for("x"), "its create had made no computer"); // onDeleteOf("cloud", undefined, "").done("x")
        assert_eq!(agents_kind_refusal(&kind("local")), "agents on this computer cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer"); // agentsKindRefusal("local")
        assert_eq!(machine_word(&kind("local")), "this computer"); // machineWord("local")
        assert_eq!(thread_place(&kind("local"), "spoo"), "spoo"); // threadPlace("local", "spoo")
        assert_eq!(
            delete_notice(1.0, &kind("local"), None, None, None),
            "Its computer is left as it is; its record and 1 thread leave this computer."
        ); // deleteNotice(1, "local")
        assert_eq!(
            delete_notice(3.0, &kind("local"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None),
            "Its worktree at /r/wt/a is removed and the project folder is left as it is; its record and 3 threads leave this computer."
        ); // deleteNotice(3, "local", {path:"/r/wt/a"})
        assert_eq!(
            delete_notice(0.0, &kind("local"), None, Some(""), None),
            "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."
        ); // deleteNotice(0, "local", undefined, "")
        assert_eq!(
            delete_notice(2.0, &kind("local"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })),
            "Its copy on spoo is deleted; its record and 2 threads leave this computer."
        ); // deleteNotice(2, "local", undefined, "m1", {name:"web", computer:"spoo"})
        assert_eq!(on_delete_of(&kind("local"), None, None, None).done_for("m-7"), "its computer is left as it is"); // onDeleteOf("local").done("m-7")
        assert_eq!(
            on_delete_of(&kind("local"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })).done_for("m1"),
            "web is deleted from spoo"
        ); // onDeleteOf("local", undefined, "m1", {name:"web", computer:"spoo"}).done("m1")
        assert_eq!(
            on_delete_of(&kind("local"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None).done_for("m1"),
            "its worktree at /r/wt/a is removed and the project folder is left as it is"
        ); // onDeleteOf("local", {path:"/r/wt/a"}).done("m1")
        assert_eq!(on_delete_of(&kind("local"), None, Some(""), None).done_for("x"), "its create had made no computer"); // onDeleteOf("local", undefined, "").done("x")
        assert_eq!(agents_kind_refusal(&kind("place")), "agents on a computer you joined cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer"); // agentsKindRefusal("place")
        assert_eq!(machine_word(&kind("place")), "a computer you joined"); // machineWord("place")
        assert_eq!(thread_place(&kind("place"), "spoo"), "spoo"); // threadPlace("place", "spoo")
        assert_eq!(
            delete_notice(1.0, &kind("place"), None, None, None),
            "Its computer is left as it is; its record and 1 thread leave this computer."
        ); // deleteNotice(1, "place")
        assert_eq!(
            delete_notice(3.0, &kind("place"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None),
            "Its worktree at /r/wt/a is removed and the project folder is left as it is; its record and 3 threads leave this computer."
        ); // deleteNotice(3, "place", {path:"/r/wt/a"})
        assert_eq!(
            delete_notice(0.0, &kind("place"), None, Some(""), None),
            "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."
        ); // deleteNotice(0, "place", undefined, "")
        assert_eq!(
            delete_notice(2.0, &kind("place"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })),
            "Its copy on spoo is deleted; its record and 2 threads leave this computer."
        ); // deleteNotice(2, "place", undefined, "m1", {name:"web", computer:"spoo"})
        assert_eq!(on_delete_of(&kind("place"), None, None, None).done_for("m-7"), "its computer is left as it is"); // onDeleteOf("place").done("m-7")
        assert_eq!(
            on_delete_of(&kind("place"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })).done_for("m1"),
            "web is deleted from spoo"
        ); // onDeleteOf("place", undefined, "m1", {name:"web", computer:"spoo"}).done("m1")
        assert_eq!(
            on_delete_of(&kind("place"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None).done_for("m1"),
            "its worktree at /r/wt/a is removed and the project folder is left as it is"
        ); // onDeleteOf("place", {path:"/r/wt/a"}).done("m1")
        assert_eq!(on_delete_of(&kind("place"), None, Some(""), None).done_for("x"), "its create had made no computer");
        // onDeleteOf("place", undefined, "").done("x")
    }

    #[test]
    fn each_state_reads_one_word_and_refuses_in_it() {
        assert_eq!(workspace_word(&state("running"), None), "Running"); // workspaceWord("running")
        assert_eq!(workspace_word(&state("running"), Some(&pause("memory"))), "Running"); // workspaceWord("running", "memory")
        assert_eq!(workspace_state_line("web", &state("running"), None), "web running"); // workspaceStateLine("web", "running")
        assert!(!paused_or_pausing(&state("running"))); // pausedOrPausing("running")
        assert!(is_billing(&state("running"))); // isBilling("running")
        assert_eq!(action_refusal(&state("running"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "null"); // String(actionRefusal("running", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("running"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "null"
        ); // String(actionRefusal("running", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("running")), None, None).unwrap_or_else(|| "null".into()), "null"); // String(sendRefusal("running") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("running"), GoneRoad::Rebuild, None),
            "This one is running, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("running", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("running"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is running"
        ); // goneRoadRefusal("running", "forget", "disk")
        assert_eq!(workspace_word(&state("pausing"), None), "Stopping"); // workspaceWord("pausing")
        assert_eq!(workspace_word(&state("pausing"), Some(&pause("memory"))), "Pausing"); // workspaceWord("pausing", "memory")
        assert_eq!(workspace_state_line("web", &state("pausing"), None), "web stopping"); // workspaceStateLine("web", "pausing")
        assert!(paused_or_pausing(&state("pausing"))); // pausedOrPausing("pausing")
        assert!(!is_billing(&state("pausing"))); // isBilling("pausing")
        assert_eq!(
            action_refusal(&state("pausing"), "open a terminal", None, None).unwrap_or_else(|| "null".into()),
            "Workspace is pausing; wake it to open a terminal"
        ); // String(actionRefusal("pausing", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("pausing"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "Workspace is pausing; wake it to open a terminal"
        ); // String(actionRefusal("pausing", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::State(state("pausing")), None, None).unwrap_or_else(|| "null".into()),
            "Workspace is pausing; wake it to send"
        ); // String(sendRefusal("pausing") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("pausing"), GoneRoad::Rebuild, None),
            "This one is stopping, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("pausing", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("pausing"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is stopping"
        ); // goneRoadRefusal("pausing", "forget", "disk")
        assert_eq!(workspace_word(&state("paused"), None), "Stopped"); // workspaceWord("paused")
        assert_eq!(workspace_word(&state("paused"), Some(&pause("memory"))), "Paused"); // workspaceWord("paused", "memory")
        assert_eq!(workspace_state_line("web", &state("paused"), None), "web stopped"); // workspaceStateLine("web", "paused")
        assert!(paused_or_pausing(&state("paused"))); // pausedOrPausing("paused")
        assert!(!is_billing(&state("paused"))); // isBilling("paused")
        assert_eq!(
            action_refusal(&state("paused"), "open a terminal", None, None).unwrap_or_else(|| "null".into()),
            "Workspace is paused; wake it to open a terminal"
        ); // String(actionRefusal("paused", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("paused"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "Workspace is paused; wake it to open a terminal"
        ); // String(actionRefusal("paused", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::State(state("paused")), None, None).unwrap_or_else(|| "null".into()),
            "Workspace is paused; wake it to send"
        ); // String(sendRefusal("paused") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("paused"), GoneRoad::Rebuild, None),
            "This one is stopped, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("paused", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("paused"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is stopped"
        ); // goneRoadRefusal("paused", "forget", "disk")
        assert_eq!(workspace_word(&state("waking"), None), "Waking"); // workspaceWord("waking")
        assert_eq!(workspace_word(&state("waking"), Some(&pause("memory"))), "Waking"); // workspaceWord("waking", "memory")
        assert_eq!(workspace_state_line("web", &state("waking"), None), "web waking"); // workspaceStateLine("web", "waking")
        assert!(!paused_or_pausing(&state("waking"))); // pausedOrPausing("waking")
        assert!(!is_billing(&state("waking"))); // isBilling("waking")
        assert_eq!(
            action_refusal(&state("waking"), "open a terminal", None, None).unwrap_or_else(|| "null".into()),
            "Workspace is waking; open a terminals open when it is running"
        ); // String(actionRefusal("waking", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("waking"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "Workspace is waking; open a terminals open when it is running"
        ); // String(actionRefusal("waking", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::State(state("waking")), None, None).unwrap_or_else(|| "null".into()),
            "Workspace is waking; sends open when it is running"
        ); // String(sendRefusal("waking") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("waking"), GoneRoad::Rebuild, None),
            "This one is waking, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("waking", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("waking"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is waking"
        ); // goneRoadRefusal("waking", "forget", "disk")
        assert_eq!(workspace_word(&state("unreachable"), None), "Unreachable"); // workspaceWord("unreachable")
        assert_eq!(workspace_word(&state("unreachable"), Some(&pause("memory"))), "Unreachable"); // workspaceWord("unreachable", "memory")
        assert_eq!(workspace_state_line("web", &state("unreachable"), None), "web is up and not answering yet"); // workspaceStateLine("web", "unreachable")
        assert!(!paused_or_pausing(&state("unreachable"))); // pausedOrPausing("unreachable")
        assert!(is_billing(&state("unreachable"))); // isBilling("unreachable")
        assert_eq!(
            action_refusal(&state("unreachable"), "open a terminal", None, None).unwrap_or_else(|| "null".into()),
            "Workspace is unreachable; open a terminals open when the machine answers"
        ); // String(actionRefusal("unreachable", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("unreachable"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "Workspace is unreachable; open a terminals open when the machine answers"
        ); // String(actionRefusal("unreachable", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::State(state("unreachable")), None, None).unwrap_or_else(|| "null".into()),
            "Workspace is unreachable; sends open when the machine answers"
        ); // String(sendRefusal("unreachable") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("unreachable"), GoneRoad::Rebuild, None),
            "This one is not answering yet; the rebuild is offered once it is gone"
        ); // goneRoadRefusal("unreachable", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("unreachable"), GoneRoad::Forget, Some(&pause("disk"))),
            "This one is not answering yet; the forget is offered once it is gone"
        ); // goneRoadRefusal("unreachable", "forget", "disk")
        assert_eq!(workspace_word(&state("gone"), None), "Gone"); // workspaceWord("gone")
        assert_eq!(workspace_word(&state("gone"), Some(&pause("memory"))), "Gone"); // workspaceWord("gone", "memory")
        assert_eq!(workspace_state_line("web", &state("gone"), None), "web gone"); // workspaceStateLine("web", "gone")
        assert!(!paused_or_pausing(&state("gone"))); // pausedOrPausing("gone")
        assert!(!is_billing(&state("gone"))); // isBilling("gone")
        assert_eq!(action_refusal(&state("gone"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open a terminal, which brings back its home folder from the last saved nap"); // String(actionRefusal("gone", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("gone"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "web's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open a terminal, which brings back its home folder from the last saved nap (its disk went with it)"); // String(actionRefusal("gone", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("gone")), None, None).unwrap_or_else(|| "null".into()), "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to send, which brings back its home folder from the last saved nap"); // String(sendRefusal("gone") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("gone"), GoneRoad::Rebuild, None),
            "This one is gone, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("gone", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("gone"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is gone"
        ); // goneRoadRefusal("gone", "forget", "disk")
        assert_eq!(workspace_word(&state("waiting"), None), "Waiting"); // workspaceWord("waiting")
        assert_eq!(workspace_word(&state("waiting"), Some(&pause("memory"))), "Waiting"); // workspaceWord("waiting", "memory")
        assert_eq!(workspace_state_line("web", &state("waiting"), None), "web waiting"); // workspaceStateLine("web", "waiting")
        assert!(!paused_or_pausing(&state("waiting"))); // pausedOrPausing("waiting")
        assert!(!is_billing(&state("waiting"))); // isBilling("waiting")
        assert_eq!(
            action_refusal(&state("waiting"), "open a terminal", None, None).unwrap_or_else(|| "null".into()),
            "Workspace is waiting for a machine; open a terminals open when it is running"
        ); // String(actionRefusal("waiting", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("waiting"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "Workspace is waiting for a machine; open a terminals open when it is running"
        ); // String(actionRefusal("waiting", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("waiting")), None, None).unwrap_or_else(|| "null".into()), "null"); // String(sendRefusal("waiting") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("waiting"), GoneRoad::Rebuild, None),
            "This one is waiting, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("waiting", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("waiting"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is waiting"
        ); // goneRoadRefusal("waiting", "forget", "disk")
        assert_eq!(workspace_word(&state("released"), None), "Deleted"); // workspaceWord("released")
        assert_eq!(workspace_word(&state("released"), Some(&pause("memory"))), "Deleted"); // workspaceWord("released", "memory")
        assert_eq!(workspace_state_line("web", &state("released"), None), "web deleted"); // workspaceStateLine("web", "released")
        assert!(!paused_or_pausing(&state("released"))); // pausedOrPausing("released")
        assert!(!is_billing(&state("released"))); // isBilling("released")
        assert_eq!(
            action_refusal(&state("released"), "open a terminal", None, None).unwrap_or_else(|| "null".into()),
            "This workspace's machine was deleted once its sub-thread was done, so there is none to open a terminal on"
        ); // String(actionRefusal("released", "open a terminal") ?? "null")
        assert_eq!(
            action_refusal(&state("released"), "open a terminal", Some("its disk went with it"), Some("web"))
                .unwrap_or_else(|| "null".into()),
            "web's machine was deleted once its sub-thread was done, so there is none to open a terminal on"
        ); // String(actionRefusal("released", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::State(state("released")), None, None).unwrap_or_else(|| "null".into()),
            "This workspace's machine was deleted once its sub-thread was done, so there is none to send on"
        ); // String(sendRefusal("released") ?? "null")
        assert_eq!(
            gone_road_refusal(&state("released"), GoneRoad::Rebuild, None),
            "This one is deleted, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
        ); // goneRoadRefusal("released", "rebuild")
        assert_eq!(
            gone_road_refusal(&state("released"), GoneRoad::Forget, Some(&pause("disk"))),
            "Only a workspace whose computer is gone can be forgotten; this one is deleted"
        ); // goneRoadRefusal("released", "forget", "disk")
        assert_eq!(
            send_refusal(&SendRefusalKind::Block(block("connecting")), None, Some("web")).unwrap_or_else(|| "null".into()),
            "Connecting to wsp"
        ); // String(sendRefusal("connecting", undefined, "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::Block(block("reconnecting")), None, Some("web")).unwrap_or_else(|| "null".into()),
            "wsp is not running, reconnecting"
        ); // String(sendRefusal("reconnecting", undefined, "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::Block(block("closed")), None, Some("web")).unwrap_or_else(|| "null".into()),
            "wsp is not running"
        ); // String(sendRefusal("closed", undefined, "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::Block(block("not-found")), None, Some("web")).unwrap_or_else(|| "null".into()),
            "Task not found"
        ); // String(sendRefusal("not-found", undefined, "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::Block(block("loading")), None, Some("web")).unwrap_or_else(|| "null".into()),
            "Loading transcript"
        ); // String(sendRefusal("loading", undefined, "web") ?? "null")
        assert_eq!(
            send_refusal(&SendRefusalKind::Block(block("no-agents")), None, Some("web")).unwrap_or_else(|| "null".into()),
            "the agents here have not answered yet"
        ); // String(sendRefusal("no-agents", undefined, "web") ?? "null")
        assert_eq!(not_answering_yet(GoneRoad::Rebuild), "This one is not answering yet; the rebuild is offered once it is gone"); // notAnsweringYet("rebuild")
        assert_eq!(not_answering_yet(GoneRoad::Forget), "This one is not answering yet; the forget is offered once it is gone"); // notAnsweringYet("forget")
        assert_eq!(gone_refusal(None, "open it", None), "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open it, which brings back its home folder from the last saved nap"); // goneRefusal(undefined, "open it", undefined)
        assert_eq!(gone_refusal(Some("web"), "open it", Some("")), "web's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open it, which brings back its home folder from the last saved nap"); // goneRefusal("web", "open it", "")
        assert_eq!(gone_refusal(Some("web"), "open it", Some("disk lost")), "web's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open it, which brings back its home folder from the last saved nap (disk lost)");
        // goneRefusal("web", "open it", "disk lost")
    }

    #[test]
    fn the_state_reads_off_phase_machine_and_reach() {
        assert!(!daemon_silent(None)); // daemonSilent(undefined)
        assert!(!daemon_silent(Some(&reach("reachable")))); // daemonSilent("reachable")
        assert!(daemon_silent(Some(&reach("no-daemon")))); // daemonSilent("no-daemon")
        assert!(daemon_silent(Some(&reach("unreachable")))); // daemonSilent("unreachable")
        assert!(!daemon_silent(Some(&reach("napping")))); // daemonSilent("napping")
        assert!(!daemon_silent(Some(&reach("unsupported")))); // daemonSilent("unsupported")
        assert!(!daemon_silent(Some(&reach("gone")))); // daemonSilent("gone")
        assert!(!daemon_silent(Some(&reach("slow")))); // daemonSilent("slow")
        assert!(!daemon_silent(Some(&reach("zombie")))); // daemonSilent("zombie")
        assert_eq!(reach_word(reach_shown(None, &reach("reachable"))), "reachable"); // reachShown(undefined, "reachable")
        assert_eq!(reach_word(reach_shown(Some(&reach("unreachable")), &reach("slow"))), "slow"); // reachShown("unreachable", "slow")
        assert_eq!(reach_word(reach_shown(Some(&reach("reachable")), &reach("slow"))), "slow"); // reachShown("reachable", "slow")
        assert_eq!(reach_word(reach_shown(Some(&reach("slow")), &reach("unreachable"))), "slow"); // reachShown("slow", "unreachable")
        assert_eq!(reach_word(reach_shown(Some(&reach("no-daemon")), &reach("slow"))), "slow"); // reachShown("no-daemon", "slow")
        assert!(start_failed(&view(json!({"phase":"gone","machineId":""})))); // startFailed({"phase":"gone","machineId":""})
        assert!(!start_failed(&view(json!({"phase":"gone","machineId":"m1"})))); // startFailed({"phase":"gone","machineId":"m1"})
        assert!(!start_failed(&view(json!({"phase":"running","machineId":""})))); // startFailed({"phase":"running","machineId":""})
        assert!(!needs_rebuild(&input(json!({"phase":"running"})))); // needsRebuild({"phase":"running"})
        assert!(needs_rebuild(&input(json!({"phase":"gone"})))); // needsRebuild({"phase":"gone"})
        assert!(needs_rebuild(&input(json!({"phase":"running","reach":"zombie"})))); // needsRebuild({"phase":"running","reach":"zombie"})
        assert!(needs_rebuild(&input(json!({"phase":"running","wakeRefused":"no room"})))); // needsRebuild({"phase":"running","wakeRefused":"no room"})
        assert!(!needs_rebuild(&input(json!({"phase":"running","wakeRefused":null})))); // needsRebuild({"phase":"running","wakeRefused":null})
        assert_eq!(workspace_state(&input(json!({"phase":"running"}))).as_str(), "running"); // workspaceState({"phase":"running"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","machineState":"paused"}))).as_str(), "paused"); // workspaceState({"phase":"running","machineState":"paused"})
        assert_eq!(
            workspace_state(&input(json!({"phase":"running","machineState":"running","reach":"unreachable"}))).as_str(),
            "unreachable"
        ); // workspaceState({"phase":"running","machineState":"running","reach":"unreachable"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","reach":"gone"}))).as_str(), "running"); // workspaceState({"phase":"running","reach":"gone"})
        assert_eq!(workspace_state(&input(json!({"phase":"waking"}))).as_str(), "waking"); // workspaceState({"phase":"waking"})
        assert_eq!(workspace_state(&input(json!({"phase":"gone"}))).as_str(), "gone"); // workspaceState({"phase":"gone"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","wakeRefused":"no room"}))).as_str(), "running");
        // workspaceState({"phase":"running","wakeRefused":"no room"})
    }

    #[test]
    fn an_absent_computer_reads_its_road_and_its_last_word() {
        assert_eq!(back_url(2222.0), "http://127.0.0.1:2222"); // backUrl(2222)
        assert_eq!(dials_back_word(2222.0, "spoo"), "dials back over ssh (127.0.0.1:2222 on spoo)"); // dialsBackWord(2222, "spoo")
        assert_eq!(back_url(0.0), "http://127.0.0.1:0"); // backUrl(0)
        assert_eq!(dials_back_word(0.0, "spoo"), "dials back over ssh (127.0.0.1:0 on spoo)"); // dialsBackWord(0, "spoo")
        assert_eq!(linked_over("10.0.0.2", None, &[] as &[&str]).unwrap_or_else(|| "none".into()), "none"); // String(linkedOver("10.0.0.2", undefined, []) ?? "none")
        assert_eq!(
            linked_over("10.0.0.2", Some(&back(json!({"boxPort":2222}))), &["10.0.0.2"] as &[&str]).unwrap_or_else(|| "none".into()),
            "at 10.0.0.2"
        ); // String(linkedOver("10.0.0.2", {"boxPort":2222}, ["10.0.0.2"]) ?? "none")
        assert_eq!(
            linked_over("10.0.0.2", Some(&back(json!({"boxPort":2222}))), &["10.0.0.9"] as &[&str]).unwrap_or_else(|| "none".into()),
            "none"
        ); // String(linkedOver("10.0.0.2", {"boxPort":2222}, ["10.0.0.9"]) ?? "none")
        assert_eq!(
            linked_over(&back_url(2222.0), Some(&back(json!({"boxPort":2222}))), &[] as &[&str]).unwrap_or_else(|| "none".into()),
            "over ssh"
        ); // String(linkedOver(backUrl(2222), {boxPort: 2222}, []) ?? "none")
        assert_eq!(last_known("2.1 GB", None, None), "2.1 GB"); // lastKnown("2.1 GB", null)
        assert_eq!(last_known("2.1 GB", Some(90000.0), None), "2.1 GB, last seen 1 min ago"); // lastKnown("2.1 GB", 90000)
        assert_eq!(last_known("2.1 GB", Some(7200000.0), Some("read")), "2.1 GB, read 2 h ago"); // lastKnown("2.1 GB", 7200000, "read")
        assert_eq!(last_known("x", Some(0.0), None), "x, last seen 0 min ago"); // lastKnown("x", 0)
        assert_eq!(waited_line(0.0), "Waited 0 s for your computer"); // waitedLine(0)
        assert_eq!(waited_line(4999.0), "Waited 5 s for your computer"); // waitedLine(4999)
        assert_eq!(waited_line(5000.0), "Waited 5 s for your computer"); // waitedLine(5000)
        assert_eq!(waited_line(65000.0), "Waited 1 min for your computer"); // waitedLine(65000)
        assert_eq!(waited_line(3700000.0), "Waited 62 min for your computer"); // waitedLine(3700000)
        assert_eq!(composer_held_line("spoo"), "held until spoo answers"); // composerHeldLine("spoo")
        assert_eq!(
            place_no_dial_line("spoo"),
            "wsp has no road to dial spoo: it joined by typing a code, so it connects on its own when it is on."
        ); // placeNoDialLine("spoo")
        assert_eq!(away_ms_of(None, 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(awayMsOf({}, 1792920600000) ?? "null")
        assert_eq!(
            away_ms_of(Some("2026-10-25T09:00:00.000Z"), 1792920600000.0)
                .map(wsp_words::js::number_string)
                .unwrap_or_else(|| "null".into()),
            "1800000"
        ); // String(awayMsOf({"lastSeenAt":"2026-10-25T09:00:00.000Z"}, 1792920600000) ?? "null")
        assert_eq!(
            away_ms_of(Some("not a date"), 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()),
            "null"
        ); // String(awayMsOf({"lastSeenAt":"not a date"}, 1792920600000) ?? "null")
        assert_eq!(ssh_road_of(None).unwrap_or_else(|| "none".into()), "none"); // String(sshRoadOf(undefined) ?? "none")
        assert_eq!(ssh_road_of(Some(&road(json!({})))).unwrap_or_else(|| "none".into()), "none"); // String(sshRoadOf({}) ?? "none")
        assert_eq!(ssh_road_of(Some(&road(json!({"ssh":""})))).unwrap_or_else(|| "none".into()), "none"); // String(sshRoadOf({"ssh":""}) ?? "none")
        assert_eq!(ssh_road_of(Some(&road(json!({"ssh":"root@10.0.0.2"})))).unwrap_or_else(|| "none".into()), "root@10.0.0.2");
        // String(sshRoadOf({"ssh":"root@10.0.0.2"}) ?? "none")
    }
}
