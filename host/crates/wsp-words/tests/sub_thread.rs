// SPDX-License-Identifier: AGPL-3.0-only
//! sub-thread.ts has no test file of its own at a3b870a6e: packages/protocol/test/push-refused.test.ts is ported
//! whole below, and every other case is pinned off the TS under node.

use wsp_words::sub_thread::{is_git_push, push_refused_of};

mod push_refused_of {
    use super::*;

    #[test]
    fn names_the_remote_s_own_words_over_git_s_fatal_line_measured_on_git_2_43_against() {
        assert_eq!(push_refused_of("remote: Permission to acme/web.git denied to wsp-bot.\nfatal: unable to access 'https://github.com/acme/web.git/': The requested URL returned error: 403").as_deref(), Some("Permission to acme/web.git denied to wsp-bot"));
        assert_eq!(push_refused_of("remote: protected branch        \nTo ../bare\n ! [remote rejected] HEAD -> main (pre-receive hook declined)\nerror: failed to push some refs to '../bare'").as_deref(), Some("protected branch"));
    }

    #[test]
    fn falls_back_to_the_rejected_ref_then_git_s_fatal_or_error_line_skipping_a_remote() {
        assert_eq!(push_refused_of("To github.com:acme/web.git\n ! [rejected]        HEAD -> main (fetch first)\nerror: failed to push some refs\nhint: Updates were rejected").as_deref(), Some("[rejected] HEAD -> main (fetch first)"));
        assert_eq!(push_refused_of("remote: Enumerating objects: 5, done.\nremote: Total 3 (delta 0)\nfatal: Authentication failed for 'https://github.com/acme/web.git/'").as_deref(), Some("fatal: Authentication failed for 'https://github.com/acme/web.git/'"));
        assert_eq!(push_refused_of("Everything up-to-date"), None);
    }

    #[test]
    fn knows_a_git_push_among_the_commands_an_agent_runs() {
        assert!(is_git_push("{\"command\":\"git push -u origin wsp/fix\"}"));
        assert!(is_git_push("git -C /root/web push origin HEAD"));
        assert!(!is_git_push("git status && git log"));
        assert!(!is_git_push("echo push"));
        // Claude Code's input is JSON: its description is words about the call, not the call.
        assert!(!is_git_push("{\"command\":\"git status\",\"description\":\"Check the tree before git push\"}"));
        assert!(is_git_push("{\"command\":\"git push -u origin wsp/fix\",\"description\":\"Push the branch\"}"));
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::sub_thread::*;

    use wsp_words::exit::HostError;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }
    fn err(e: HostError) -> String {
        format!("{} | {} | {}", e.message, e.fix.unwrap_or_else(|| "-".into()), e.kind.map(|k| k.as_str().to_string()).unwrap_or_else(|| "-".into()))
    }

    #[test]
    fn sizes_are_read_named_and_stepped_within_what_a_plan_sells() {
        assert_eq!(sub_size_word(&CpuMem { cpu: 2.0, mem_mb: 4096.0 }), "2x4"); // subSizeWord({"cpu":2,"memMb":4096})
        assert_eq!(sub_size_word(&CpuMem { cpu: 4.0, mem_mb: 1536.0 }), "4x1.5"); // subSizeWord({"cpu":4,"memMb":1536})
        assert_eq!(sub_size_word(&CpuMem { cpu: 1.0, mem_mb: 512.0 }), "1x0.5"); // subSizeWord({"cpu":1,"memMb":512})
        assert_eq!(sub_size_of("2x4").map(|s| js(&s)).unwrap_or_else(|| "null".into()), "{\"cpu\":2,\"memMb\":4096}"); // JSON.stringify(subSizeOf("2x4") ?? null)
        assert_eq!(sub_size_of("4 vCPU 8 GB").map(|s| js(&s)).unwrap_or_else(|| "null".into()), "null"); // JSON.stringify(subSizeOf("4 vCPU 8 GB") ?? null)
        assert_eq!(sub_size_of("nope").map(|s| js(&s)).unwrap_or_else(|| "null".into()), "null"); // JSON.stringify(subSizeOf("nope") ?? null)
        assert_eq!(sub_size_of("").map(|s| js(&s)).unwrap_or_else(|| "null".into()), "null"); // JSON.stringify(subSizeOf("") ?? null)
        assert_eq!(sub_size_of("16x32").map(|s| js(&s)).unwrap_or_else(|| "null".into()), "{\"cpu\":16,\"memMb\":32768}"); // JSON.stringify(subSizeOf("16x32") ?? null)
        assert_eq!(sub_size_words(&SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }), "small, 2 vCPU, 4 GB"); // subSizeWords({"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05})
        assert_eq!(sub_size_words(&SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }), "medium, 4 vCPU, 8 GB"); // subSizeWords({"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1})
        assert_eq!(sub_size_words(&SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }), "large, 8 vCPU, 16 GB"); // subSizeWords({"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2})
        assert_eq!(size_name_of(&SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }), "medium"); // sizeNameOf({"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1})
        assert_eq!(size_name_of(&CpuMem { cpu: 2.0, mem_mb: 4096.0 }), "2x4"); // sizeNameOf({cpu: 2, memMb: 4096})
        assert_eq!(sub_next_size(4096.0, &[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }]).map(|s| s.name).unwrap_or_else(|| "none".into()), "medium"); // subNextSize({memMb: 4096}, [{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}])?.name ?? "none"
        assert_eq!(sub_next_size(8192.0, &[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }]).map(|s| s.name).unwrap_or_else(|| "none".into()), "large"); // subNextSize({memMb: 8192}, [{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}])?.name ?? "none"
        assert_eq!(sub_next_size(16384.0, &[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }]).map(|s| s.name).unwrap_or_else(|| "none".into()), "none"); // subNextSize({memMb: 16384}, [{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}])?.name ?? "none"
        assert_eq!(sub_next_size(100.0, &[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }]).map(|s| s.name).unwrap_or_else(|| "none".into()), "small"); // subNextSize({memMb: 100}, [{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}])?.name ?? "none"
        assert_eq!(sub_sizes_sold(&[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }], None).iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(","), "small,medium,large"); // subSizesSold([{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}], undefined).map(s => s.name).join(",")
        assert_eq!(sub_sizes_sold(&[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }], Some(&["2x4","8x16"] as &[&str])).iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(","), "small,large"); // subSizesSold([{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}], {"sizes":["2x4","8x16"]}).map(s => s.name).join(",")
        assert_eq!(sub_sizes_sold(&[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }], Some(&[] as &[&str])).iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(","), ""); // subSizesSold([{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}], {"sizes":[]}).map(s => s.name).join(",")
    }

    #[test]
    fn folders_and_branches_are_named_off_the_remote_and_the_machine() {
        assert_eq!(sub_folder_of("https://github.com/acme/web.git"), "/root/web"); // subFolderOf("https://github.com/acme/web.git")
        assert_eq!(sub_folder_of("git@github.com:acme/web.git"), "/root/web"); // subFolderOf("git@github.com:acme/web.git")
        assert_eq!(sub_folder_of("https://gitlab.com/a/b/c"), "/root/c"); // subFolderOf("https://gitlab.com/a/b/c")
        assert_eq!(sub_folder_of("/root/repo"), "/root/repo"); // subFolderOf("/root/repo")
        assert_eq!(sub_folder_of("web"), "/root/web"); // subFolderOf("web")
        assert_eq!(sub_branch_of("fk_abc123"), "wsp/fk-abc123"); // subBranchOf("fk_abc123")
        assert_eq!(sub_branch_of("m-1"), "wsp/m-1"); // subBranchOf("m-1")
        assert_eq!(sub_branch_of(""), "wsp/sub-thread"); // subBranchOf("")
        assert_eq!(sub_branch_name_ok("wsp/fix"), true); // subBranchNameOk("wsp/fix")
        assert_eq!(sub_branch_name_ok("fix"), true); // subBranchNameOk("fix")
        assert_eq!(sub_branch_name_ok("-x"), false); // subBranchNameOk("-x")
        assert_eq!(sub_branch_name_ok("a..b"), false); // subBranchNameOk("a..b")
        assert_eq!(sub_branch_name_ok("a b"), false); // subBranchNameOk("a b")
        assert_eq!(sub_branch_name_ok("main"), true); // subBranchNameOk("main")
        assert_eq!(sub_branch_name_ok("wsp/"), false); // subBranchNameOk("wsp/")
        assert_eq!(sub_branch_name_ok("x.lock"), false); // subBranchNameOk("x.lock")
        assert_eq!(sub_branch_name_ok("ok_1-2/3"), true); // subBranchNameOk("ok_1-2/3")
    }

    #[test]
    fn every_refusal_says_what_happened_what_to_do_and_its_kind() {
        assert_eq!(err(sub_beside_refusal("x")), "--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it cannot also take x. Drop x, or run the work without --on. | Drop x, or run the work without --on. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subBesideRefusal("x")))
        assert_eq!(err(sub_other_project_refusal("Boat", "web", "api")), "run --on Boat starts a sub-thread of the thread asking, on that thread's project api, and web is another project. Leave the project out, or run the work on web without --on. | Leave the project out, or run the work on web without --on. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subOtherProjectRefusal("Boat", "web", "api")))
        assert_eq!(err(sub_no_thread_refusal("Boat")), "run --on Boat starts a sub-thread of the thread that asks, and this line came from no thread. Start it from a thread, or run the work on this computer with wsp run <project>. | Start it from a thread, or run the work on this computer with wsp run <project>. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoThreadRefusal("Boat")))
        assert_eq!(err(sub_size_no_cloud_refusal()), "--size sizes a sub-thread's machine, and this line starts none. Add --on boat. | Add --on boat. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subSizeNoCloudRefusal()))
        assert_eq!(err(sub_from_no_cloud_refusal()), "--from names the commit a sub-thread's machine starts from, and this line starts none. Add --on boat. | Add --on boat. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subFromNoCloudRefusal()))
        assert_eq!(err(sub_lead_on_machine_refusal("Boat", "01HAB", "fk_1")), "run --on Boat starts a sub-thread of a thread on this computer or on a computer you added, and thread 01HAB runs on fk_1, a machine wsp forked. Start it from a thread on this computer. | Start it from a thread on this computer. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subLeadOnMachineRefusal("Boat", "01HAB", "fk_1")))
        assert_eq!(err(sub_starts_nothing_refusal("01HABCDEFG", "01HZZZZZZZ")), "sub-thread 01HABCDE starts no threads of its own. Send the work to thread 01HZZZZZ, which started it, with wsp send 01HZZZZZ \"<message>\". | Send the work to thread 01HZZZZZ, which started it, with wsp send 01HZZZZZ \"<message>\". | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subStartsNothingRefusal("01HABCDEFG", "01HZZZZZZZ")))
        assert_eq!(err(sub_no_cloud_refusal("Boat", &[] as &[&str])), "Boat is not a cloud this host holds. wsp add boat adds it. It holds: none. | wsp add boat adds it. It holds: none. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoCloudRefusal("Boat", [])))
        assert_eq!(err(sub_no_cloud_refusal("Boat", &["Hetzner","Solari"] as &[&str])), "Boat is not a cloud this host holds. wsp add boat adds it. It holds: Hetzner, Solari. | wsp add boat adds it. It holds: Hetzner, Solari. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoCloudRefusal("Boat", ["Hetzner", "Solari"])))
        assert_eq!(err(sub_no_remote_refusal("web", "Boat")), "web has no remote, so a machine on Boat has nothing to clone. Push it to GitHub or GitLab, add the remote with git remote add origin <url>, then start the sub-thread again. | Push it to GitHub or GitLab, add the remote with git remote add origin <url>, then start the sub-thread again. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoRemoteRefusal("web", "Boat")))
        assert_eq!(err(sub_other_host_refusal("web", "gitlab.com", "Boat")), "web's remote is on gitlab.com, and a machine on Boat signs in to GitHub and GitLab only. Run this work on this computer. | Run this work on this computer. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subOtherHostRefusal("web", "gitlab.com", "Boat")))
        assert_eq!(err(brokered_git_hub_only_refusal("web", "gitlab.com", "Boat")), "web's remote is on gitlab.com, and a machine on Boat reaches GitHub alone, through this host. Run this work on this computer. | Run this work on this computer. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(brokeredGitHubOnlyRefusal("web", "gitlab.com", "Boat")))
        assert_eq!(err(branch_of_record_unread_refusal("acme/web", "timeout")), "gh on this computer could not read acme/web (timeout), and a machine whose credentials this host keeps pushes only to a repository GitHub names. Run gh auth status on this computer, then start it again. | Run gh auth status on this computer, then start it again. | -"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(branchOfRecordUnreadRefusal("acme/web", "timeout")))
        assert_eq!(err(fork_head_gone_refusal(12.0)), "pull request #12's fork was deleted, so there is no branch to push to. Start the work from an issue or a branch instead. | Start the work from an issue or a branch instead. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(forkHeadGoneRefusal(12)))
        assert_eq!(err(clone_no_thread_refusal("web")), "web has no thread yet, so wsp cannot reach GitHub for it. Start a thread on it first. | Start a thread on it first. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(cloneNoThreadRefusal("web")))
        assert_eq!(err(fork_not_pushable_refusal("zed", 12.0)), "can't push to zed's fork: they did not allow edits from maintainers. Ask zed to allow edits from maintainers on pull request #12, or review it with wsp review instead. | Ask zed to allow edits from maintainers on pull request #12, or review it with wsp review instead. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(forkNotPushableRefusal("zed", 12)))
        assert_eq!(err(sub_detached_refusal("web", "main")), "web is on no branch. Check one out and push it, or start it from a ref the remote holds with --from main. | Check one out and push it, or start it from a ref the remote holds with --from main. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subDetachedRefusal("web", "main")))
        assert_eq!(err(sub_no_upstream_refusal("wsp/x", "main")), "wsp/x is not on the remote yet. Run git push -u origin wsp/x and start the sub-thread again, or start it from a ref the remote holds with --from main. | Run git push -u origin wsp/x and start the sub-thread again, or start it from a ref the remote holds with --from main. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoUpstreamRefusal("wsp/x", "main")))
        assert_eq!(err(sub_ahead_refusal("wsp/x", 1.0, "main")), "wsp/x has 1 commit the remote does not. Run git push and start the sub-thread again, or start it from a ref the remote holds with --from main. | Run git push and start the sub-thread again, or start it from a ref the remote holds with --from main. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subAheadRefusal("wsp/x", 1, "main")))
        assert_eq!(err(sub_ahead_refusal("wsp/x", 3.0, "main")), "wsp/x has 3 commits the remote does not. Run git push and start the sub-thread again, or start it from a ref the remote holds with --from main. | Run git push and start the sub-thread again, or start it from a ref the remote holds with --from main. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subAheadRefusal("wsp/x", 3, "main")))
        assert_eq!(err(sub_ahead_unknown_refusal("wsp/x", "main")), "git could not count wsp/x's commits against the remote, so some of them may not be on it. Run git fetch, then git push, and start the sub-thread again, or start it from a ref the remote holds with --from main. | Run git fetch, then git push, and start the sub-thread again, or start it from a ref the remote holds with --from main. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subAheadUnknownRefusal("wsp/x", "main")))
        assert_eq!(err(sub_from_not_on_remote_refusal("abc123", None)), "abc123 is not on the remote. Run git fetch if a branch holding it was pushed since, else push one with git push -u origin <branch>, then start the sub-thread again. | Run git fetch if a branch holding it was pushed since, else push one with git push -u origin <branch>, then start the sub-thread again. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subFromNotOnRemoteRefusal("abc123")))
        assert_eq!(err(sub_from_not_on_remote_refusal("abc123", Some("my-branch"))), "abc123 is not on the remote. Run git push -u origin my-branch, then start the sub-thread again. | Run git push -u origin my-branch, then start the sub-thread again. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subFromNotOnRemoteRefusal("abc123", "my-branch")))
        assert_eq!(err(sub_from_unread_refusal("abc", "no such ref")), "the remote did not say where abc stands (no such ref). Check the remote answers with git ls-remote origin, then start the sub-thread again. | Check the remote answers with git ls-remote origin, then start the sub-thread again. | -"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subFromUnreadRefusal("abc", "no such ref")))
        assert_eq!(err(sub_no_image_refusal("Boat")), "Boat holds no image of yours yet. Build one in Settings > Computers > Boat, or run wsp init --on boat. | Build one in Settings > Computers > Boat, or run wsp init --on boat. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoImageRefusal("Boat")))
        assert_eq!(err(sub_no_sign_in_refusal("Boat", &GitHostWords { label: "GitHub".into(), cli: "gh".into(), login: "gh login".into() }, "acme/web")), "the image on Boat holds no GitHub sign-in (gh), so a sub-thread could neither clone acme/web nor push its branch. Run wsp init --on boat again with the gh login set to copy. | Run wsp init --on boat again with the gh login set to copy. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoSignInRefusal("Boat", {label: "GitHub", cli: "gh", login: "gh login"}, "acme/web")))
        assert_eq!(err(sub_host_no_sign_in_refusal("Boat", "acme/web")), "gh on this computer is not signed in to GitHub, so a sub-thread on Boat could neither clone acme/web nor push its branch. Run gh auth login in a terminal on this computer, then start the sub-thread again. | Run gh auth login in a terminal on this computer, then start the sub-thread again. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subHostNoSignInRefusal("Boat", "acme/web")))
        assert_eq!(err(sub_host_other_git_refusal("Boat", "gitlab.com", "acme/web")), "a sub-thread on Boat reaches GitHub alone through this computer, so it could not clone acme/web on gitlab.com. Run this work on this computer. | Run this work on this computer. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subHostOtherGitRefusal("Boat", "gitlab.com", "acme/web")))
        assert_eq!(err(sub_size_refusal("Boat", "3x3", &[SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, SubSize { name: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.1 }, SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }])), "Boat sells small 2x4 ($0.050/h), medium 4x8 ($0.100/h) and large 8x16 ($0.200/h) on this plan; 3x3 is not one of them. Name one of them, or leave the size out for Boat's own. | Name one of them, or leave the size out for Boat's own. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subSizeRefusal("Boat", "3x3", [{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05},{"name":"medium","cpu":4,"memMb":8192,"rateUsdPerHour":0.1},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}])))
        assert_eq!(err(sub_size_refusal("Boat", "3x3", &[])), "Boat sells nothing on this plan; 3x3 is not one of them. Name one of them, or leave the size out for Boat's own. | Name one of them, or leave the size out for Boat's own. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subSizeRefusal("Boat", "3x3", [])))
        assert_eq!(err(sub_branch_name_refusal("a b")), "a b is not a name git takes for a branch. Name another, or leave --branch out for wsp/<slug>. | Name another, or leave --branch out for wsp/<slug>. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subBranchNameRefusal("a b")))
        assert_eq!(err(sub_lead_branch_refusal("main")), "the sub-thread pushes main, and that is the lead's own branch. Name another, or leave --branch out for wsp/<slug>. | Name another, or leave --branch out for wsp/<slug>. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subLeadBranchRefusal("main")))
        assert_eq!(err(sub_machine_taken_refusal("fk_1", "01HABCDEFG")), "fk_1 is the machine of sub-thread 01HABCDE and runs that thread alone. Send to 01HABCDE to continue it. | Send to 01HABCDE to continue it. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subMachineTakenRefusal("fk_1", "01HABCDEFG")))
        assert_eq!(err(sub_waiting_refusal("fk_1")), "fk_1 is waiting for a machine, and its sub-thread's first message waits with it. Send again once it starts. | Send again once it starts. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subWaitingRefusal("fk_1")))
        assert_eq!(err(sub_no_agent_refusal("Boat", "opencode", &["claude","codex"] as &[&str])), "the image on Boat carries no opencode. Add it to the recipe and build the image again, or name one it carries: claude, codex. | Add it to the recipe and build the image again, or name one it carries: claude, codex. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoAgentRefusal("Boat", "opencode", ["claude", "codex"])))
        assert_eq!(err(sub_no_agent_refusal("Boat", "opencode", &[] as &[&str])), "the image on Boat carries no opencode. Add it to the recipe and build the image again, or name one it carries: . | Add it to the recipe and build the image again, or name one it carries: . | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subNoAgentRefusal("Boat", "opencode", [])))
        assert_eq!(err(sub_seed_too_big_refusal("web", "3 GB", "1 GB", &["node_modules",".venv"] as &[&str])), "the files picked for web come to 3 GB, over the 1 GB a sub-thread carries; the largest are node_modules, .venv. Pick again with wsp projects seed web. | Pick again with wsp projects seed web. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subSeedTooBigRefusal("web", "3 GB", "1 GB", ["node_modules", ".venv"])))
        assert_eq!(err(sub_twice_refusal(&SubFault::Unusable { r#box: "fk_1".into(), why: "no disk".into(), request_id: None }, &SubFault::Cancelled { r#box: "fk_4".into(), why: "spot reclaimed".into() }, "fk_9", "Boat")), "Boat gave two machines in a row that failed before the sub-thread started (fk_1: no disk; fk_4: spot reclaimed). Start it again in a few minutes. | Start it again in a few minutes. | -"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subTwiceRefusal({"kind":"unusable","box":"fk_1","why":"no disk"}, {"kind":"cancelled","box":"fk_4","why":"spot reclaimed"}, "fk_9", "Boat")))
        assert_eq!(err(sub_twice_refusal(&SubFault::Env { r#box: "fk_3".into() }, &SubFault::Env { r#box: "fk_3".into() }, "fk_9", "Boat")), "Boat's machines are slow to take a setup right now (fk_3, fk_3, exit 124 on both). Start it again in a few minutes. | Start it again in a few minutes. | -"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subTwiceRefusal({"kind":"env","box":"fk_3"}, {"kind":"env","box":"fk_3"}, "fk_9", "Boat")))
        assert_eq!(err(sub_held_delete_refusal("01HABCDEFG", &SubUnpushed { files: 2.0, commits: 1.0, read_failed: None, unread: None }, "wsp/x")), "sub-thread 01HABCDE holds 1 commit and 2 changed files not pushed on wsp/x. Send it a push, or ask the person to delete it. | Send it a push, or ask the person to delete it. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subHeldDeleteRefusal("01HABCDEFG", {"files":2,"commits":1}, "wsp/x")))
        assert_eq!(err(sub_held_delete_refusal("01HABCDEFG", &SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: None }, "wsp/x")), "sub-thread 01HABCDE holds work not pushed on wsp/x. Send it a push, or ask the person to delete it. | Send it a push, or ask the person to delete it. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subHeldDeleteRefusal("01HABCDEFG", {"files":0,"commits":0}, "wsp/x")))
        assert_eq!(err(sub_held_delete_refusal("01HABCDEFG", &SubUnpushed { files: 0.0, commits: 0.0, read_failed: Some("git broke".into()), unread: None }, "wsp/x")), "sub-thread 01HABCDE's machine could not be read (git broke), so work on wsp/x may not be pushed. Send it a push, or ask the person to delete it. | Send it a push, or ask the person to delete it. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subHeldDeleteRefusal("01HABCDEFG", {"files":0,"commits":0,"readFailed":"git broke"}, "wsp/x")))
        assert_eq!(err(sub_held_delete_refusal("01HABCDEFG", &SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: Some(true) }, "wsp/x")), "sub-thread 01HABCDE's machine has not been read since its last turn, so work on wsp/x may not be pushed. Send it a push, or ask the person to delete it. | Send it a push, or ask the person to delete it. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subHeldDeleteRefusal("01HABCDEFG", {"files":0,"commits":0,"unread":true}, "wsp/x")))
        assert_eq!(err(sub_cloud_gone_refusal("Boat")), "Boat is no longer on this host. Add it with wsp add boat and send again. | Add it with wsp add boat and send again. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subCloudGoneRefusal("Boat")))
        assert_eq!(err(sub_released_refusal("01HABCDEFG", "Boat")), "sub-thread 01HABCDE's machine on Boat was deleted after it was done, so there is none to wake. Send to it with wsp send 01HABCDE \"<message>\", which starts a new one. | Send to it with wsp send 01HABCDE \"<message>\", which starts a new one. | usage"); // ((e => `${e.message} | ${e.fix ?? "-"} | ${e.kind ?? "-"}`)(subReleasedRefusal("01HABCDEFG", "Boat")))
    }

    #[test]
    fn the_lines_of_a_start_a_fault_and_a_copy() {
        assert_eq!(cloud_added_line("Boat"), "Boat was added since this thread started: run with on boat starts a sub-thread there."); // cloudAddedLine("Boat")
        assert_eq!(cloud_removed_line("Boat"), "Boat was removed since this thread started: run with on boat starts nothing there now."); // cloudRemovedLine("Boat")
        assert_eq!(sub_session_too_large_line("40 MB"), "the agent's session was too large to keep (40 MB); a send continues from the transcript"); // subSessionTooLargeLine("40 MB")
        assert_eq!(sub_session_not_kept_line("the disk was full"), "the agent's session was not kept (the disk was full); a send continues from the transcript"); // subSessionNotKeptLine("the disk was full")
        assert_eq!(sub_opening("PRE", "do the thing"), "PRE\n\ndo the thing"); // subOpening("PRE", "do the thing")
        assert_eq!(sub_replaced_line(&SubFault::Unusable { r#box: "fk_1".into(), why: "no disk".into(), request_id: None }, "fk_9", "Boat"), "Boat gave fk_1 with nothing able to run on it (no disk); wsp deleted it and started another."); // subReplacedLine({"kind":"unusable","box":"fk_1","why":"no disk"}, "fk_9", "Boat")
        assert_eq!(sub_replaced_line(&SubFault::Unusable { r#box: "fk_2".into(), why: "no disk".into(), request_id: Some("req_9".into()) }, "fk_9", "Boat"), "Boat gave fk_2 with nothing able to run on it (no disk); wsp deleted it and started another."); // subReplacedLine({"kind":"unusable","box":"fk_2","why":"no disk","requestId":"req_9"}, "fk_9", "Boat")
        assert_eq!(sub_replaced_line(&SubFault::Env { r#box: "fk_3".into() }, "fk_9", "Boat"), "Boat's machine fk_3 did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another."); // subReplacedLine({"kind":"env","box":"fk_3"}, "fk_9", "Boat")
        assert_eq!(sub_replaced_line(&SubFault::Cancelled { r#box: "fk_4".into(), why: "spot reclaimed".into() }, "fk_9", "Boat"), "Boat could not get a machine for fk_9 and cancelled the create (spot reclaimed); wsp started another."); // subReplacedLine({"kind":"cancelled","box":"fk_4","why":"spot reclaimed"}, "fk_9", "Boat")
        assert_eq!(sub_oom_line("fk_9", "Boat", &SubSize { name: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.05 }, Some("medium")), "the agent on fk_9 was killed for memory: its machine is small, 2 vCPU, 4 GB, and Boat's own processes hold part of that. Start the work again with size medium."); // subOomLine("fk_9", "Boat", {"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.05}, "medium")
        assert_eq!(sub_oom_line("fk_9", "Boat", &SubSize { name: "large".into(), cpu: 8.0, mem_mb: 16384.0, rate_usd_per_hour: 0.2 }, None), "the agent on fk_9 was killed for memory: its machine is large, 8 vCPU, 16 GB, and Boat's own processes hold part of that. Boat sells no bigger size, so the work has to use less memory."); // subOomLine("fk_9", "Boat", {"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.2}, undefined)
        assert_eq!(sub_uncommitted_line(1.0, "web", "main", "abcdef1234"), "1 uncommitted change in web did not travel; the sub-thread starts at main as pushed (abcdef1)."); // subUncommittedLine(1, "web", "main", "abcdef1234")
        assert_eq!(sub_uncommitted_line(4.0, "web", "main", "abcdef1234"), "4 uncommitted changes in web did not travel; the sub-thread starts at main as pushed (abcdef1)."); // subUncommittedLine(4, "web", "main", "abcdef1234")
        assert_eq!(sub_nothing_picked_why("web"), "nothing is picked for web"); // subNothingPickedWhy("web")
        assert_eq!(sub_picked_elsewhere_why("web", "spoo"), "web lives on spoo, and picks are read from folders on this computer only"); // subPickedElsewhereWhy("web", "spoo")
        assert_eq!(sub_not_sent_line("the copy failed", "web"), "no git-ignored files were sent: the copy failed."); // subNotSentLine("the copy failed", "web")
        assert_eq!(sub_stayed_line(&[".env"] as &[&str]), ".env stayed on this computer: a sub-thread's machine is a provider's, which gets no login, file of values or key."); // subStayedLine([".env"])
        assert_eq!(sub_stayed_line(&[".env","secrets.json"] as &[&str]), ".env, secrets.json stayed on this computer: a sub-thread's machine is a provider's, which gets no login, file of values or key."); // subStayedLine([".env","secrets.json"])
        assert_eq!(sub_stayed_line(&[] as &[&str]), " stayed on this computer: a sub-thread's machine is a provider's, which gets no login, file of values or key."); // subStayedLine([])
        assert_eq!(sub_copying_line(1.0), "copying 1 file"); // subCopyingLine(1)
        assert_eq!(sub_copying_line(3.0), "copying 3 files"); // subCopyingLine(3)
        assert_eq!(sub_opened_line("01HABCDEFG", "Boat", "web"), "thread 01HABCDE on boat in web"); // subOpenedLine("01HABCDEFG", "Boat", "web")
    }

    #[test]
    fn a_push_and_an_oom_are_read_off_their_own_lines() {
        assert_eq!(is_git_push("{\"command\":\"git push -u origin wsp/fix\"}"), true); // isGitPush("{\"command\":\"git push -u origin wsp/fix\"}")
        assert_eq!(is_git_push("git -C /root/web push origin HEAD"), true); // isGitPush("git -C /root/web push origin HEAD")
        assert_eq!(is_git_push("git status && git log"), false); // isGitPush("git status && git log")
        assert_eq!(is_git_push("echo push"), false); // isGitPush("echo push")
        assert_eq!(is_git_push("{\"command\":\"git status\",\"description\":\"Check the tree before git push\"}"), false); // isGitPush("{\"command\":\"git status\",\"description\":\"Check the tree before git push\"}")
        assert_eq!(is_git_push("git  push"), true); // isGitPush("git  push")
        assert_eq!(is_git_push("gitpush"), false); // isGitPush("gitpush")
        assert_eq!(is_git_push("cd x; git push --force"), true); // isGitPush("cd x; git push --force")
        assert_eq!(push_refused_of("remote: Permission to acme/web.git denied to wsp-bot.\nfatal: unable to access 'https://github.com/acme/web.git/': The requested URL returned error: 403").unwrap_or_else(|| "none".into()), "Permission to acme/web.git denied to wsp-bot"); // String(pushRefusedOf("remote: Permission to acme/web.git denied to wsp-bot.\nfatal: unable to access 'https://github.com/acme/web.git/': The requested URL returned error: 403") ?? "none")
        assert_eq!(push_refused_of("Everything up-to-date").unwrap_or_else(|| "none".into()), "none"); // String(pushRefusedOf("Everything up-to-date") ?? "none")
        assert_eq!(push_refused_of("error: src refspec main does not match any").unwrap_or_else(|| "none".into()), "error: src refspec main does not match any"); // String(pushRefusedOf("error: src refspec main does not match any") ?? "none")
        assert_eq!(push_refused_of("").unwrap_or_else(|| "none".into()), "none"); // String(pushRefusedOf("") ?? "none")
        assert_eq!(sub_start_again_prompt("Fix it", "Boat", "medium"), "Start the work of the sub-thread \"Fix it\" again on Boat at size medium: its agent was killed for memory."); // subStartAgainPrompt("Fix it", "Boat", "medium")
        assert_eq!(sub_push_again_prompt("wsp/x"), "Push wsp/x again, and say what the remote answers."); // subPushAgainPrompt("wsp/x")
        assert_eq!(is_sub_oom_line("killed for memory on fk_1"), false); // isSubOomLine("killed for memory on fk_1")
        assert_eq!(is_sub_oom_line("Out of memory: Killed process 12"), false); // isSubOomLine("Out of memory: Killed process 12")
        assert_eq!(is_sub_oom_line("nothing"), false); // isSubOomLine("nothing")
        assert_eq!(is_sub_oom_line(""), false); // isSubOomLine("")
    }

    #[test]
    fn work_not_pushed_holds_the_machine() {
        assert_eq!(disk_may_go_line("spoo", "Friday"), "spoo may delete its disk from Friday"); // diskMayGoLine("spoo", "Friday")
        assert_eq!(sub_not_pushed_reason(&Held { files: 2.0, commits: 0.0 }, "wsp/x", None), "2 changed files on wsp/x are not pushed, so its machine stays."); // subNotPushedReason({"files":2,"commits":0}, "wsp/x")
        assert_eq!(sub_not_pushed_reason(&Held { files: 0.0, commits: 3.0 }, "wsp/x", Some(&KeptOn { on: "spoo".into(), day: "Friday".into() })), "spoo may delete its disk from Friday, with 3 commits on wsp/x not pushed."); // subNotPushedReason({"files":0,"commits":3}, "wsp/x", {"on":"spoo","day":"Friday"})
        assert_eq!(sub_not_pushed_reason(&Held { files: 1.0, commits: 1.0 }, "wsp/x", None), "1 commit and 1 changed file on wsp/x are not pushed, so its machine stays."); // subNotPushedReason({"files":1,"commits":1}, "wsp/x")
        assert_eq!(sub_unpushed_line(&SubUnpushed { files: 2.0, commits: 1.0, read_failed: None, unread: None }, "wsp/x"), "1 commit and 2 changed files on wsp/x are not pushed and go with it."); // subUnpushedLine({"files":2,"commits":1}, "wsp/x")
        assert_eq!(sub_held_line("01HABCDEFG", &SubUnpushed { files: 2.0, commits: 1.0, read_failed: None, unread: None }, "wsp/x"), "sub-thread 01HABCDE holds 1 commit and 2 changed files not pushed on wsp/x"); // subHeldLine("01HABCDEFG", {"files":2,"commits":1}, "wsp/x")
        assert_eq!(sub_unpushed_line(&SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: None }, "wsp/x"), "work on wsp/x are not pushed and go with it."); // subUnpushedLine({"files":0,"commits":0}, "wsp/x")
        assert_eq!(sub_held_line("01HABCDEFG", &SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: None }, "wsp/x"), "sub-thread 01HABCDE holds work not pushed on wsp/x"); // subHeldLine("01HABCDEFG", {"files":0,"commits":0}, "wsp/x")
        assert_eq!(sub_unpushed_line(&SubUnpushed { files: 0.0, commits: 0.0, read_failed: Some("git broke".into()), unread: None }, "wsp/x"), "its machine could not be read (git broke), so work on wsp/x may not be pushed and would go with it."); // subUnpushedLine({"files":0,"commits":0,"readFailed":"git broke"}, "wsp/x")
        assert_eq!(sub_held_line("01HABCDEFG", &SubUnpushed { files: 0.0, commits: 0.0, read_failed: Some("git broke".into()), unread: None }, "wsp/x"), "sub-thread 01HABCDE's machine could not be read (git broke), so work on wsp/x may not be pushed"); // subHeldLine("01HABCDEFG", {"files":0,"commits":0,"readFailed":"git broke"}, "wsp/x")
        assert_eq!(sub_unpushed_line(&SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: Some(true) }, "wsp/x"), "its machine has not been read since its last turn, so work on wsp/x may not be pushed and would go with it."); // subUnpushedLine({"files":0,"commits":0,"unread":true}, "wsp/x")
        assert_eq!(sub_held_line("01HABCDEFG", &SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: Some(true) }, "wsp/x"), "sub-thread 01HABCDE's machine has not been read since its last turn, so work on wsp/x may not be pushed"); // subHeldLine("01HABCDEFG", {"files":0,"commits":0,"unread":true}, "wsp/x")
        assert_eq!(sub_delete_question(&SubDeleteQuestion { thread_id: "01HABCDEFG".into(), title: "Fix it".into(), on: "Boat".into(), branch: "wsp/x".into(), held: None }), "Delete sub-thread 01HABCDE (Fix it) and its machine on Boat?\nIts turns leave this computer and its machine is deleted; what it pushed to wsp/x stays on the remote."); // subDeleteQuestion({threadId: "01HABCDEFG", title: "Fix it", on: "Boat", branch: "wsp/x"})
        assert_eq!(sub_delete_question(&SubDeleteQuestion { thread_id: "01HABCDEFG".into(), title: "Fix it".into(), on: "Boat".into(), branch: "wsp/x".into(), held: Some(SubUnpushed { files: 2.0, commits: 1.0, read_failed: None, unread: None }) }), "Delete sub-thread 01HABCDE (Fix it) and its machine on Boat?\n1 commit and 2 changed files on wsp/x are not pushed and go with it."); // subDeleteQuestion({threadId: "01HABCDEFG", title: "Fix it", on: "Boat", branch: "wsp/x", held: {"files":2,"commits":1}})
        assert_eq!(sub_held_on_remove_line(&[] as &[&str]).unwrap_or_else(|| "none".into()), "none"); // String(subHeldOnRemoveLine([]) ?? "none")
        assert_eq!(sub_held_on_remove_line(&["01HABCDEFG"] as &[&str]).unwrap_or_else(|| "none".into()), "Work not pushed goes with its machine: 01HABCDEFG."); // String(subHeldOnRemoveLine(["01HABCDEFG"]) ?? "none")
        assert_eq!(sub_held_on_remove_line(&["01HABCDEFG","01HZZZZZZZ"] as &[&str]).unwrap_or_else(|| "none".into()), "Work not pushed goes with their machines: 01HABCDEFG; 01HZZZZZZZ."); // String(subHeldOnRemoveLine(["01HABCDEFG","01HZZZZZZZ"]) ?? "none")
    }

    #[test]
    fn the_end_of_a_machine() {
        assert_eq!(sub_cloud_gone_line("Boat"), "Boat is no longer on this host"); // subCloudGoneLine("Boat")
        assert_eq!(sub_delete_refused_line("Boat", "fk_1", "busy", 1.0), "Boat would not delete fk_1 (busy); it is tried again in 1 minutes"); // subDeleteRefusedLine("Boat", "fk_1", "busy", 1)
        assert_eq!(sub_delete_refused_line("Boat", "fk_1", "busy", 5.0), "Boat would not delete fk_1 (busy); it is tried again in 5 minutes"); // subDeleteRefusedLine("Boat", "fk_1", "busy", 5)
        assert_eq!(cloud_said("Boat"), "Boat"); // cloudSaid("Boat")
        assert_eq!(sub_deleted_line("01HABCDEFG", "Boat"), "deleted sub-thread 01HABCDE and its machine on Boat"); // subDeletedLine("01HABCDEFG", "Boat")
    }

    #[test]
    fn strip_and_spawn_words() {
        assert_eq!({ let w = sub_strip::waiting_slot("Boat", 3.0, 3.0); format!("{} / {}", w.title, w.note) }, "Waiting for a machine on Boat / 3 of 3 machines are running. It starts on its own when one frees."); // SUB_STRIP.waitingSlot("Boat", 3, 3).title + " / " + SUB_STRIP.waitingSlot("Boat", 3, 3).note
        assert_eq!(sub_strip::not_pushed(&Held { files: 1.0, commits: 2.0 }, "wsp/x", Some("denied"), Some(&KeptOn { on: "spoo".into(), day: "Friday".into() })).note, "The push to wsp/x was refused: denied. This machine stays until the work is pushed or copied to your computer, but spoo may delete its disk from Friday."); // SUB_STRIP.notPushed({files: 1, commits: 2}, "wsp/x", "denied", {on: "spoo", day: "Friday"}).note
        assert_eq!(sub_strip::not_pushed(&Held { files: 1.0, commits: 0.0 }, "wsp/x", None, None).title, "1 changed file not pushed"); // SUB_STRIP.notPushed({files: 1, commits: 0}, "wsp/x").title
        assert_eq!(sub_strip::oom("killed for memory on fk_1").note, "Killed for memory on fk_1"); // SUB_STRIP.oom("killed for memory on fk_1").note
        assert_eq!(sub_strip::start_again("medium"), "Start again at medium"); // SUB_STRIP.startAgain("medium")
        assert_eq!(format!("{} / {}", sub_spawn_words::files(1.0, true), sub_spawn_words::files(3.0, false)), "Copied 1 picked file / Copying 3 picked files"); // SUB_SPAWN_WORDS.files(1, true) + " / " + SUB_SPAWN_WORDS.files(3, false)
        assert_eq!(sub_spawn_words::agent_fact(None, None).unwrap_or_else(|| "none".into()), "none"); // String(SUB_SPAWN_WORDS.agentFact(undefined, undefined) ?? "none")
        assert_eq!(sub_spawn_words::agent_fact(Some("opus"), Some("high")).unwrap_or_else(|| "none".into()), "opus, high"); // String(SUB_SPAWN_WORDS.agentFact("opus", "high") ?? "none")
        assert_eq!(sub_spawn_words::machine_fact(Some("fk_1"), &["booting", "ready"]).unwrap_or_else(|| "none".into()), "fk_1, booting, ready"); // String(SUB_SPAWN_WORDS.machineFact("fk_1", ["booting", "ready"]) ?? "none")
        assert_eq!(sub_spawn_words::branch_fact("wsp/x", "abcdef1234"), "wsp/x is pushed at abcdef1"); // SUB_SPAWN_WORDS.branchFact("wsp/x", "abcdef1234")
    }
}
