// SPDX-License-Identifier: AGPL-3.0-only
//! The view and wire sentences no TS test reaches at a3b870a6e (wire/machine-link.ts, wire/place-link.ts,
//! wire/runtime.ts and views/place-setup-words.ts); every case below is pinned off the TS under node. The TS tests
//! that do reach them are ported in tests/place_setup.rs, tests/protocol_words.rs and tests/views_activity.rs.

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    #[allow(unused_imports)]
    use wsp_words::views::place_setup_words::*;

    use wsp_words::views::place_setup_words::{Packed, Shipped, SignInWay, SignInWayFacts};
    use wsp_words::wire::machine_link::{HeldRow, PlaceHeld};

    #[test]
    fn a_joined_computer_that_holds_work_is_forgotten_in_its_own_words() {
        assert_eq!(
            wsp_words::wire::place_link::place_still_installed_line("spoo"),
            "spoo is off this host, but the daemon on it is still installed; run wsp leave on that computer when it is back"
        ); // placeStillInstalledLine("spoo")
        assert_eq!(wsp_words::wire::place_link::place_forgotten_line("spoo", true), "spoo is forgotten here and nothing was done on it; whatever of wsp's is still there, the copies its forks ran in among it, comes off with wsp leave run on that computer"); // placeForgottenLine("spoo", true)
        assert_eq!(wsp_words::wire::place_link::place_forgotten_line("spoo", false), "spoo is forgotten here and nothing was done on it; whatever of wsp's is still there comes off with wsp leave run on that computer"); // placeForgottenLine("spoo", false)
        assert_eq!(
            wsp_words::wire::machine_link::place_not_a_workspace_line("spoo"),
            "spoo is a computer you joined, not a workspace; its forks are the workspaces"
        ); // placeNotAWorkspaceLine("spoo")
        assert_eq!(
            wsp_words::wire::machine_link::place_not_a_workspace_fix("spoo"),
            "Name a machine on it: wsp threads lists what runs on spoo."
        ); // placeNotAWorkspaceFix("spoo")
        assert_eq!(
            wsp_words::wire::machine_link::local_runs_one_line("spoo"),
            "this computer is already a workspace, spoo, the only one it can be"
        ); // localRunsOneLine("spoo")
        assert_eq!(
            wsp_words::wire::place_link::place_still_installed_line("it's"),
            "it's is off this host, but the daemon on it is still installed; run wsp leave on that computer when it is back"
        ); // placeStillInstalledLine("it's")
        assert_eq!(wsp_words::wire::place_link::place_forgotten_line("it's", true), "it's is forgotten here and nothing was done on it; whatever of wsp's is still there, the copies its forks ran in among it, comes off with wsp leave run on that computer"); // placeForgottenLine("it's", true)
        assert_eq!(wsp_words::wire::place_link::place_forgotten_line("it's", false), "it's is forgotten here and nothing was done on it; whatever of wsp's is still there comes off with wsp leave run on that computer"); // placeForgottenLine("it's", false)
        assert_eq!(
            wsp_words::wire::machine_link::place_not_a_workspace_line("it's"),
            "it's is a computer you joined, not a workspace; its forks are the workspaces"
        ); // placeNotAWorkspaceLine("it's")
        assert_eq!(
            wsp_words::wire::machine_link::place_not_a_workspace_fix("it's"),
            "Name a machine on it: wsp threads lists what runs on it's."
        ); // placeNotAWorkspaceFix("it's")
        assert_eq!(
            wsp_words::wire::machine_link::local_runs_one_line("it's"),
            "this computer is already a workspace, it's, the only one it can be"
        ); // localRunsOneLine("it's")
        assert_eq!(wsp_words::wire::machine_link::place_forget_line("spoo", &PlaceHeld { forks: vec![], projects: vec![] }, None), "spoo leaves this wsp, and nothing is done on spoo: whatever of wsp's stays there comes off with wsp leave run on that computer."); // placeForgetLine("spoo", {"forks":[],"projects":[]})
        assert_eq!(wsp_words::wire::machine_link::place_forget_line("spoo", &PlaceHeld { forks: vec![], projects: vec![] }, Some("root@10.0.0.2")), "wsp tries to take itself off spoo over root@10.0.0.2 first. Then spoo leaves this wsp, and nothing more is done on spoo: whatever of wsp's stays there comes off with wsp leave run on that computer."); // placeForgetLine("spoo", {"forks":[],"projects":[]}, "root@10.0.0.2")
        assert_eq!(
            wsp_words::wire::machine_link::place_holds_line(&PlaceHeld { forks: vec![], projects: vec![] })
                .unwrap_or_else(|| "none".into()),
            "none"
        ); // String(placeHoldsLine({"forks":[],"projects":[]}) ?? "none")
        assert_eq!(wsp_words::wire::machine_link::place_forget_line("spoo", &PlaceHeld { forks: vec![HeldRow { name: "web-1".into(), threads: 0.0 }], projects: vec![] }, None), "spoo leaves this wsp with its fork web-1, and nothing is done on spoo: whatever of wsp's stays there comes off with wsp leave run on that computer."); // placeForgetLine("spoo", {"forks":[{"name":"web-1","threads":0}],"projects":[]})
        assert_eq!(wsp_words::wire::machine_link::place_forget_line("spoo", &PlaceHeld { forks: vec![HeldRow { name: "web-1".into(), threads: 0.0 }], projects: vec![] }, Some("root@10.0.0.2")), "wsp tries to take itself off spoo over root@10.0.0.2 first. Then spoo leaves this wsp with its fork web-1, and nothing more is done on spoo: whatever of wsp's stays there comes off with wsp leave run on that computer."); // placeForgetLine("spoo", {"forks":[{"name":"web-1","threads":0}],"projects":[]}, "root@10.0.0.2")
        assert_eq!(
            wsp_words::wire::machine_link::place_holds_line(&PlaceHeld {
                forks: vec![HeldRow { name: "web-1".into(), threads: 0.0 }],
                projects: vec![]
            })
            .unwrap_or_else(|| "none".into()),
            "its fork web-1 deleted"
        ); // String(placeHoldsLine({"forks":[{"name":"web-1","threads":0}],"projects":[]}) ?? "none")
        assert_eq!(wsp_words::wire::machine_link::place_forget_line("spoo", &PlaceHeld { forks: vec![HeldRow { name: "a".into(), threads: 2.0 }, HeldRow { name: "b".into(), threads: 1.0 }], projects: vec![HeldRow { name: "web".into(), threads: 1.0 }] }, None), "spoo leaves this wsp with its 2 forks a, b with 3 threads and its project web with 1 thread, and nothing is done on spoo: whatever of wsp's stays there comes off with wsp leave run on that computer."); // placeForgetLine("spoo", {"forks":[{"name":"a","threads":2},{"name":"b","threads":1}],"projects":[{"name":"web","threads":1}]})
        assert_eq!(wsp_words::wire::machine_link::place_forget_line("spoo", &PlaceHeld { forks: vec![HeldRow { name: "a".into(), threads: 2.0 }, HeldRow { name: "b".into(), threads: 1.0 }], projects: vec![HeldRow { name: "web".into(), threads: 1.0 }] }, Some("root@10.0.0.2")), "wsp tries to take itself off spoo over root@10.0.0.2 first. Then spoo leaves this wsp with its 2 forks a, b with 3 threads and its project web with 1 thread, and nothing more is done on spoo: whatever of wsp's stays there comes off with wsp leave run on that computer."); // placeForgetLine("spoo", {"forks":[{"name":"a","threads":2},{"name":"b","threads":1}],"projects":[{"name":"web","threads":1}]}, "root@10.0.0.2")
        assert_eq!(
            wsp_words::wire::machine_link::place_holds_line(&PlaceHeld {
                forks: vec![HeldRow { name: "a".into(), threads: 2.0 }, HeldRow { name: "b".into(), threads: 1.0 }],
                projects: vec![HeldRow { name: "web".into(), threads: 1.0 }]
            })
            .unwrap_or_else(|| "none".into()),
            "its 2 forks a, b with 3 threads deleted, and its project web with 1 thread out of this wsp"
        ); // String(placeHoldsLine({"forks":[{"name":"a","threads":2},{"name":"b","threads":1}],"projects":[{"name":"web","threads":1}]}) ?? "none")
        assert_eq!(wsp_words::wire::runtime::thread_op_refusal("send", "01HABCDEFGHJ", None), "send is not a thread's to ask for; this request came in on the token of thread 01HABCDE, and a thread's token starts work only under its own root and reads only that tree"); // threadOpRefusal("send", "01HABCDEFGHJ")
        assert_eq!(wsp_words::wire::runtime::thread_op_refusal("rename", "01HABCDEFGHJ", Some("spoo")), "rename is not a thread's to ask for; this request came in on the token of thread 01HABCDE on spoo, and a thread's token starts work only under its own root and reads only that tree");
        // threadOpRefusal("rename", "01HABCDEFGHJ", "spoo")
    }

    #[test]
    fn setup_lines_name_the_computer_and_what_to_do() {
        assert_eq!(
            wsp_words::views::place_setup_words::place_syncing_line("spoo"),
            "spoo is syncing to its recipe; set it up again once that ends"
        ); // placeSyncingLine("spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::place_no_picks_line("spoo"),
            "spoo has nothing picked to go on it: choose in the app's Add a computer, or run wsp add spoo --resume --recipe <name>"
        ); // placeNoPicksLine("spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::place_no_home_line("spoo"),
            "spoo got no agents or tools: it reported no home folder for its login, so nothing on it could be reached"
        ); // placeNoHomeLine("spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::pending_held_fix("spoo"),
            "Finish it with wsp add spoo --resume --recipe <name>, or in the app."
        ); // pendingHeldFix("spoo")
        assert_eq!(wsp_words::views::place_setup_words::no_pending_refusal("spoo"), "no add of spoo is pending"); // noPendingRefusal("spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::pending_not_joined_line("spoo"),
            "spoo never joined, so there is nothing to set up"
        ); // pendingNotJoinedLine("spoo")
        assert_eq!(wsp_words::views::place_setup_words::pending_not_joined_fix("10.0.0.2"), "Add it again with wsp add 10.0.0.2."); // pendingNotJoinedFix("10.0.0.2")
        assert_eq!(wsp_words::views::place_setup_words::copied_from_line("this Mac"), "copied from this Mac"); // copiedFromLine("this Mac")
        assert_eq!(wsp_words::views::place_setup_words::waits_for_install_line("spoo"), "waits for spoo to install"); // waitsForInstallLine("spoo")
        assert_eq!(wsp_words::views::place_setup_words::no_copy_line("spoo", "this Mac"), "this Mac has no spoo sign-in to copy"); // noCopyLine("spoo", "this Mac")
        assert_eq!(
            wsp_words::views::place_setup_words::copied_not_signed_in_line("spoo", "this Mac"),
            "spoo still reads signed out there with the sign-in copied from this Mac"
        ); // copiedNotSignedInLine("spoo", "this Mac")
        assert_eq!(wsp_words::views::place_setup_words::sign_in_there_fix("spoo"), "Sign in on spoo instead."); // signInThereFix("spoo")
        assert_eq!(wsp_words::views::place_setup_words::token_held_line("this Mac"), "the token made on this Mac"); // tokenHeldLine("this Mac")
        assert_eq!(wsp_words::views::place_setup_words::key_held_line("this Mac"), "the key in ~/.wsp/.env on this Mac"); // keyHeldLine("this Mac")
        assert_eq!(
            wsp_words::views::place_setup_words::no_key_line("OPENAI_API_KEY", "this Mac"),
            "~/.wsp/.env on this Mac holds no OPENAI_API_KEY"
        ); // noKeyLine("OPENAI_API_KEY", "this Mac")
        assert_eq!(wsp_words::views::place_setup_words::was_there_line("spoo"), "spoo had it before wsp, so it stays"); // wasThereLine("spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::nothing_to_skip_line("codex", "spoo"),
            "nothing waits or failed under codex on spoo"
        ); // nothingToSkipLine("codex", "spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::no_setup_row_refusal("codex", "spoo"),
            "spoo's setup holds no row codex to run again"
        ); // noSetupRowRefusal("codex", "spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::folder_move_held_line("the daemon is behind"),
            "the daemon is behind, then retry to move it to the recipe's folder"
        ); // folderMoveHeldLine("the daemon is behind")
        assert_eq!(
            wsp_words::views::place_setup_words::folder_move_stands_line("web", "spoo"),
            "web still stands on spoo; remove or rename it there, then sync again to move the project to the recipe's folder"
        ); // folderMoveStandsLine("web", "spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::recipe_folder_gone_line("web"),
            "there is no folder at web on this computer; point the recipe at one that exists"
        ); // recipeFolderGoneLine("web")
        assert_eq!(
            wsp_words::views::place_setup_words::place_no_chip_line("spoo", "linux", "riscv64"),
            "spoo says it is linux riscv64, and this wsp carries no daemon built for it"
        ); // placeNoChipLine("spoo", "linux", "riscv64")
        assert_eq!(
            wsp_words::views::place_setup_words::edited_there_line("spoo", &[] as &[&str]),
            " were edited on spoo, so they stay and wsp no longer manages them"
        ); // editedThereLine("spoo", [])
        assert_eq!(
            wsp_words::views::place_setup_words::edited_there_line("spoo", &["CLAUDE.md"] as &[&str]),
            "CLAUDE.md was edited on spoo, so it stays and wsp no longer manages it"
        ); // editedThereLine("spoo", ["CLAUDE.md"])
        assert_eq!(
            wsp_words::views::place_setup_words::edited_there_line("spoo", &["a", "b", "c"] as &[&str]),
            "a, b, c were edited on spoo, so they stay and wsp no longer manages them"
        ); // editedThereLine("spoo", ["a","b","c"])
        assert!(!wsp_words::views::place_setup_words::says_no_key(None, "OPENAI_API_KEY")); // saysNoKey(undefined, "OPENAI_API_KEY")
        assert!(!wsp_words::views::place_setup_words::says_no_key(Some("OPENAI_API_KEY is not set"), "OPENAI_API_KEY")); // saysNoKey("OPENAI_API_KEY is not set", "OPENAI_API_KEY")
        assert!(!wsp_words::views::place_setup_words::says_no_key(Some("no key"), "OPENAI_API_KEY")); // saysNoKey("no key", "OPENAI_API_KEY")
        assert!(!wsp_words::views::place_setup_words::says_no_key(Some("openai_api_key missing"), "OPENAI_API_KEY")); // saysNoKey("openai_api_key missing", "OPENAI_API_KEY")
        assert!(wsp_words::views::place_setup_words::says_no_key(Some("~/.wsp/.env on spoo holds no OPENAI_API_KEY"), "OPENAI_API_KEY")); // saysNoKey("~/.wsp/.env on spoo holds no OPENAI_API_KEY", "OPENAI_API_KEY")
        assert!(!wsp_words::views::place_setup_words::says_no_key(
            Some("~/.wsp/.env on spoo holds no OPENAI_API_KEY"),
            "ANTHROPIC_API_KEY"
        )); // saysNoKey("~/.wsp/.env on spoo holds no OPENAI_API_KEY", "ANTHROPIC_API_KEY")
        assert!(!wsp_words::views::place_setup_words::says_no_key(Some("x ~/.wsp/.env on spoo holds no OPENAI_API_KEY"), "OPENAI_API_KEY"));
        // saysNoKey("x ~/.wsp/.env on spoo holds no OPENAI_API_KEY", "OPENAI_API_KEY")
    }

    #[test]
    fn a_provision_counts_steps_files_and_bytes() {
        assert_eq!(wsp_words::views::place_setup_words::setup_width(0.0), 1.0); // setupWidth(0)
        assert_eq!(wsp_words::views::place_setup_words::setup_width(1024.0), 1.0); // setupWidth(1024)
        assert_eq!(wsp_words::views::place_setup_words::setup_width(2048.0), 1.0); // setupWidth(2048)
        assert_eq!(wsp_words::views::place_setup_words::setup_width(4095.0), 1.0); // setupWidth(4095)
        assert_eq!(wsp_words::views::place_setup_words::setup_width(6144.0), 3.0); // setupWidth(6144)
        assert_eq!(wsp_words::views::place_setup_words::setup_width(8192.0), 4.0); // setupWidth(8192)
        assert_eq!(wsp_words::views::place_setup_words::setup_width(65536.0), 4.0); // setupWidth(65536)
        assert_eq!(wsp_words::views::place_setup_words::provision_landed_line(0.0), "landed: 0 files walked on that computer"); // provisionLandedLine(0)
        assert_eq!(wsp_words::views::place_setup_words::provision_list_read_line(0.0), "list read: 0 server keys"); // provisionListReadLine(0)
        assert_eq!(
            wsp_words::views::place_setup_words::provision_servers_line(0.0, 3.0),
            "servers merged: 0 of 3 servers written into the agents' own files"
        ); // provisionServersLine(0, 3)
        assert_eq!(wsp_words::views::place_setup_words::provision_landed_line(1.0), "landed: 1 file walked on that computer"); // provisionLandedLine(1)
        assert_eq!(wsp_words::views::place_setup_words::provision_list_read_line(1.0), "list read: 1 server key"); // provisionListReadLine(1)
        assert_eq!(
            wsp_words::views::place_setup_words::provision_servers_line(1.0, 3.0),
            "servers merged: 1 of 3 servers written into the agents' own files"
        ); // provisionServersLine(1, 3)
        assert_eq!(wsp_words::views::place_setup_words::provision_landed_line(7.0), "landed: 7 files walked on that computer"); // provisionLandedLine(7)
        assert_eq!(wsp_words::views::place_setup_words::provision_list_read_line(7.0), "list read: 7 server keys"); // provisionListReadLine(7)
        assert_eq!(
            wsp_words::views::place_setup_words::provision_servers_line(7.0, 3.0),
            "servers merged: 7 of 3 servers written into the agents' own files"
        ); // provisionServersLine(7, 3)
        assert_eq!(
            wsp_words::views::place_setup_words::provision_packed_line(1.0, &Packed { bytes: 2048.0, files: None, stood: None }),
            "packed on this computer: 1 path, 2 KB"
        ); // provisionPackedLine(1, {"bytes":2048})
        assert_eq!(
            wsp_words::views::place_setup_words::provision_packed_line(3.0, &Packed { bytes: 5242880.0, files: Some(120.0), stood: None }),
            "packed on this computer: 3 paths, 5 MB"
        ); // provisionPackedLine(3, {"bytes":5242880,"files":120})
        assert_eq!(
            wsp_words::views::place_setup_words::provision_packed_line(
                2.0,
                &Packed { bytes: 1048576.0, files: Some(10.0), stood: Some(4.0) }
            ),
            "packed on this computer: 2 paths, 4 files stand there already, 10 files packed, 1 MB"
        ); // provisionPackedLine(2, {"bytes":1048576,"files":10,"stood":4})
        assert_eq!(
            wsp_words::views::place_setup_words::provision_shipped_line(&Shipped {
                part: 1.0,
                parts: 1.0,
                bytes: 1048576.0,
                total: 1048576.0,
                pieces: None
            }),
            "shipped: 1 MB"
        ); // provisionShippedLine({"part":1,"parts":1,"bytes":1048576,"total":1048576})
        assert_eq!(
            wsp_words::views::place_setup_words::provision_shipped_line(&Shipped {
                part: 2.0,
                parts: 3.0,
                bytes: 1048576.0,
                total: 3145728.0,
                pieces: None
            }),
            "shipped part 2 of 3: 1 MB of 3 MB"
        ); // provisionShippedLine({"part":2,"parts":3,"bytes":1048576,"total":3145728})
        assert_eq!(
            wsp_words::views::place_setup_words::provision_shipped_line(&Shipped {
                part: 1.0,
                parts: 1.0,
                bytes: 10.0,
                total: 10.0,
                pieces: Some(4.0)
            }),
            "shipped: 10 B in 4 pieces"
        ); // provisionShippedLine({"part":1,"parts":1,"bytes":10,"total":10,"pieces":4})
        assert_eq!(
            wsp_words::views::place_setup_words::provision_log_line(1792920605000.0, "npm i -g codex"),
            "2026-10-25T09:30:05Z npm i -g codex"
        ); // provisionLogLine(1792920605000, "npm i -g codex")
    }

    #[test]
    fn the_sign_in_ways_a_computer_offers() {
        assert_eq!(wsp_words::views::place_setup_words::sign_in_way_label(&SignInWay::Token, "spoo"), "Make a token"); // signInWayLabel("token", "spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_way_does(
                &SignInWay::Token,
                &SignInWayFacts {
                    name: "Codex".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: None,
                    key_env: None,
                    field: None
                }
            ),
            "Codex's own command runs on this Mac and asks your browser once"
        ); // signInWayDoes("token", {"name":"Codex","here":"this Mac","box":"spoo"})
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_way_does(
                &SignInWay::Token,
                &SignInWayFacts {
                    name: "Claude Code".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: Some("https://console.anthropic.com/settings/keys".into()),
                    key_env: Some("ANTHROPIC_API_KEY".into()),
                    field: Some(true)
                }
            ),
            "https://console.anthropic.com/settings/keys runs on this Mac and asks your browser once"
        ); // signInWayDoes("token", {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
        assert_eq!(wsp_words::views::place_setup_words::sign_in_way_label(&SignInWay::Key, "spoo"), "Use an API key"); // signInWayLabel("key", "spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_way_does(
                &SignInWay::Key,
                &SignInWayFacts {
                    name: "Codex".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: None,
                    key_env: None,
                    field: None
                }
            ),
            "put the key in ~/.wsp/.env on this Mac first, and wsp reads it from there without showing it"
        ); // signInWayDoes("key", {"name":"Codex","here":"this Mac","box":"spoo"})
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_way_does(
                &SignInWay::Key,
                &SignInWayFacts {
                    name: "Claude Code".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: Some("https://console.anthropic.com/settings/keys".into()),
                    key_env: Some("ANTHROPIC_API_KEY".into()),
                    field: Some(true)
                }
            ),
            "paste your ANTHROPIC_API_KEY; wsp saves it to ~/.wsp/.env without showing it"
        ); // signInWayDoes("key", {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
        assert_eq!(wsp_words::views::place_setup_words::sign_in_way_label(&SignInWay::Machine, "spoo"), "Sign in on spoo"); // signInWayLabel("machine", "spoo")
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_way_does(
                &SignInWay::Machine,
                &SignInWayFacts {
                    name: "Codex".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: None,
                    key_env: None,
                    field: None
                }
            ),
            "Codex's own sign-in runs there"
        ); // signInWayDoes("machine", {"name":"Codex","here":"this Mac","box":"spoo"})
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_way_does(
                &SignInWay::Machine,
                &SignInWayFacts {
                    name: "Claude Code".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: Some("https://console.anthropic.com/settings/keys".into()),
                    key_env: Some("ANTHROPIC_API_KEY".into()),
                    field: Some(true)
                }
            ),
            "Claude Code's own sign-in runs there"
        ); // signInWayDoes("machine", {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
        assert_eq!(
            wsp_words::views::place_setup_words::sign_in_ways_fix(
                &[SignInWay::Token],
                &SignInWayFacts {
                    name: "Claude Code".into(),
                    here: "this Mac".into(),
                    r#box: "spoo".into(),
                    mint: Some("https://console.anthropic.com/settings/keys".into()),
                    key_env: Some("ANTHROPIC_API_KEY".into()),
                    field: Some(true)
                }
            ),
            "Make a token: https://console.anthropic.com/settings/keys runs on this Mac and asks your browser once."
        ); // signInWaysFix(["token"], {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
        assert_eq!(wsp_words::views::place_setup_words::sign_in_ways_fix(&[SignInWay::Token, SignInWay::Key], &SignInWayFacts { name: "Claude Code".into(), here: "this Mac".into(), r#box: "spoo".into(), mint: Some("https://console.anthropic.com/settings/keys".into()), key_env: Some("ANTHROPIC_API_KEY".into()), field: Some(true) }), "Make a token: https://console.anthropic.com/settings/keys runs on this Mac and asks your browser once. Use an API key: paste your ANTHROPIC_API_KEY; wsp saves it to ~/.wsp/.env without showing it."); // signInWaysFix(["token","key"], {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
        assert_eq!(wsp_words::views::place_setup_words::sign_in_ways_fix(&[SignInWay::Key, SignInWay::Machine], &SignInWayFacts { name: "Claude Code".into(), here: "this Mac".into(), r#box: "spoo".into(), mint: Some("https://console.anthropic.com/settings/keys".into()), key_env: Some("ANTHROPIC_API_KEY".into()), field: Some(true) }), "Use an API key: paste your ANTHROPIC_API_KEY; wsp saves it to ~/.wsp/.env without showing it. Sign in on spoo: Claude Code's own sign-in runs there."); // signInWaysFix(["key","machine"], {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
        assert_eq!(wsp_words::views::place_setup_words::sign_in_ways_fix(&[SignInWay::Token, SignInWay::Key, SignInWay::Machine], &SignInWayFacts { name: "Claude Code".into(), here: "this Mac".into(), r#box: "spoo".into(), mint: Some("https://console.anthropic.com/settings/keys".into()), key_env: Some("ANTHROPIC_API_KEY".into()), field: Some(true) }), "Make a token: https://console.anthropic.com/settings/keys runs on this Mac and asks your browser once. Use an API key: paste your ANTHROPIC_API_KEY; wsp saves it to ~/.wsp/.env without showing it. Sign in on spoo: Claude Code's own sign-in runs there.");
        // signInWaysFix(["token","key","machine"], {"name":"Claude Code","here":"this Mac","box":"spoo","mint":"https://console.anthropic.com/settings/keys","keyEnv":"ANTHROPIC_API_KEY","field":true})
    }
}
