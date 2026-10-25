// SPDX-License-Identifier: AGPL-3.0-only
//! Activity's rules held to the TS answers for the same snapshots: what a process is called, the names it runs by,
//! the stretch from a root to the agent, every map of a placement in the order the TS Maps hold it, each pid's kind,
//! need, End and reach, and the view's thread order, tree order, totals and ports. spec/activity.md lists the rule
//! each test pins.

#[path = "rules/oracle.rs"]
mod oracle;

use oracle::{cases, of, opt_str, text};
use serde_json::{json, Value};
use std::collections::HashMap;
use wsp_frames::{CgroupMemory, ProcEntry};
use wsp_wire::types::thread_cgroup::{cgroup_memory, in_cgroup};
use wsp_wire::types::JsNumber;
use wsp_wire::views::activity::*;
use wsp_wire::views::activity_names::*;

fn kind_word(k: wsp_frames::ProcKind) -> Value {
    serde_json::to_value(k).unwrap()
}

mod names {
    use super::*;

    #[test]
    fn a_process_is_called_by_the_package_a_runner_runs_a_tool_and_its_subcommand_or_its_script() {
        for (a, want) in cases("proc_name") {
            let p: ProcEntry = of(&a[0]);
            assert_eq!(proc_name(&p), text(&want), "{} / {}", p.comm, p.cmdline);
        }
    }

    #[test]
    fn the_names_a_process_runs_by_are_its_comm_argv0_and_the_script_a_runtime_runs_empty_ones_left_out() {
        for (a, want) in cases("proc_bins") {
            let p: ProcEntry = of(&a[0]);
            assert_eq!(proc_bins(&p), of::<Vec<String>>(&want), "{} / {}", p.comm, p.cmdline);
        }
    }

    #[test]
    fn the_agent_a_process_runs_is_the_first_whose_binary_it_runs_by() {
        for (a, want) in cases("agent_by_bin") {
            let p: ProcEntry = of(&a[0]);
            let agents: Vec<AgentBin> = a[1].as_array().unwrap().iter().map(|x| AgentBin { id: text(&x["id"]), bin: text(&x["bin"]) }).collect();
            assert_eq!(agent_by_bin(&p, &agents), opt_str(&want), "{}", p.cmdline);
        }
    }

    #[test]
    fn activity_names_an_agent_by_its_agent_the_daemon_and_the_host_by_role_and_the_rest_by_proc_name() {
        for (a, want) in cases("activity_name") {
            let p: ProcEntry = of(&a[0]);
            let facts = NameFacts { daemon: a[1]["daemon"].as_i64(), host: a[1]["host"].as_i64(), agent: opt_str(&a[1]["agent"]) };
            assert_eq!(activity_name(&p, &facts), text(&want), "{a:?}");
        }
    }

    #[test]
    fn a_run_script_and_its_input_pump_are_turn_plumbing_by_their_whole_line() {
        for (a, want) in cases("is_turn_plumbing") {
            assert_eq!(is_turn_plumbing(&text(&a[0])), want.as_bool().unwrap(), "{:?}", a[0]);
        }
    }
}

mod cgroups {
    use super::*;

    #[test]
    fn a_row_stands_in_a_cgroup_when_it_is_that_one_or_under_it_never_a_sibling_with_the_same_head() {
        for (a, want) in cases("in_cgroup") {
            assert_eq!(in_cgroup(a[0].as_str(), &text(&a[1])), want.as_bool().unwrap(), "{a:?}");
        }
    }

    #[test]
    fn a_cgroup_s_memory_is_the_first_entry_naming_it_and_none_without_a_cgroup_a_list_or_an_entry() {
        for (a, want) in cases("cgroup_memory") {
            let list: Option<Vec<CgroupMemory>> = if a[0].is_null() { None } else { Some(of(&a[0])) };
            assert_eq!(cgroup_memory(list.as_deref(), a[1].as_str()), want.as_u64(), "{a:?}");
        }
    }
}

mod the_stretch_to_the_agent {
    use super::*;

    #[test]
    fn the_path_runs_breadth_first_to_the_shallowest_agent_then_on_through_an_only_child_that_is_one_too() {
        for (a, want) in cases("agent_path") {
            let rows: Vec<ProcEntry> = of(&a[0]);
            let root = a[1].as_u64().unwrap() as u32;
            let by_pid: HashMap<u32, ProcEntry> = rows.iter().map(|p| (p.pid, p.clone())).collect();
            let kids = |pid: u32| rows.iter().filter(|p| p.ppid == pid && p.pid != pid).cloned().collect::<Vec<_>>();
            let is_agent = |p: &ProcEntry| p.comm == "claude";
            let got = agent_path(root, &kids, &is_agent, &by_pid);
            let want: Option<Vec<u32>> = if want.is_null() { None } else { Some(of(&want)) };
            assert_eq!(got, want, "root {root}");
        }
    }
}

mod a_placement {
    use super::*;

    fn input_of(v: &Value) -> (Vec<ProcEntry>, u32, Option<u32>, Vec<PlaceThread>, Vec<String>, Vec<u32>, HashMap<u32, AppPart>) {
        let procs: Vec<ProcEntry> = of(&v["procs"]);
        let threads = v["threads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| PlaceThread {
                id: text(&t["id"]),
                roots: of(&t["roots"]),
                cgroup: opt_str(&t["cgroup"]),
                bin: opt_str(&t["bin"]),
                agents: if t["agents"].is_null() { None } else { Some(of(&t["agents"])) },
                tabs: if t["tabs"].is_null() { None } else { Some(of(&t["tabs"])) },
            })
            .collect();
        let bins: Vec<String> = of(&v["agentBins"]);
        let listening: Vec<u32> = if v["listening"].is_null() { vec![] } else { of(&v["listening"]) };
        let app: HashMap<u32, AppPart> = v["app"]
            .as_array()
            .map(|l| l.iter().map(|e| (e[0].as_u64().unwrap() as u32, if e[1] == "tab" { AppPart::Tab } else { AppPart::App })).collect())
            .unwrap_or_default();
        (procs, v["daemon"].as_u64().unwrap() as u32, v["host"].as_u64().map(|h| h as u32), threads, bins, listening, app)
    }

    fn pairs(v: &Value) -> Vec<(u64, Value)> {
        v.as_array().unwrap().iter().map(|e| (e[0].as_u64().unwrap(), e[1].clone())).collect()
    }

    fn whose_word(w: &Whose) -> Value {
        match w {
            Whose::Thread(t) => json!({ "thread": t }),
            Whose::Wsp => json!("wsp"),
            Whose::Loose => json!("loose"),
        }
    }

    #[test]
    fn every_map_of_a_placement_holds_the_entries_and_the_order_the_ts_maps_hold() {
        // The maps are read in insertion order by what comes after (a group's first agent wins), so order is state.
        for (a, want) in cases("place_procs") {
            let name = text(&a[0]);
            let (procs, daemon, host, threads, bins, listening, app) = input_of(&a[1]);
            let listens = |pid: u32| listening.contains(&pid);
            let has_app = !app.is_empty();
            let input = PlaceInput { procs: &procs, daemon, host, threads: &threads, agent_bins: &bins, listens: &listens, app: if has_app { Some(&app) } else { None } };
            let placed = place_procs(&input);
            let live: Vec<u64> = placed.procs.iter().map(|p| p.pid as u64).collect();
            assert_eq!(live, of::<Vec<u64>>(&want["procs"]), "{name}: the live processes, zombies out, in the read's order");
            let whose: Vec<(u64, Value)> = placed.whose.iter().map(|(k, w)| (*k as u64, whose_word(w))).collect();
            assert_eq!(whose, pairs(&want["whose"]), "{name}: whose");
            let agent: Vec<(u64, Value)> = placed.agent.iter().map(|(k, t)| (*k as u64, json!(t))).collect();
            assert_eq!(agent, pairs(&want["agent"]), "{name}: agent");
            let folds: Vec<(u64, Value)> = placed.folds_into.iter().map(|(k, t)| (*k as u64, json!(t))).collect();
            assert_eq!(folds, pairs(&want["foldsInto"]), "{name}: foldsInto");
            let roots: Vec<(u64, Value)> = placed.root_of.iter().map(|(k, t)| (*k as u64, json!(t))).collect();
            assert_eq!(roots, pairs(&want["rootOf"]), "{name}: rootOf");
            for (pid, kids) in pairs(&want["kidsOf"]) {
                let got: Vec<u64> = placed.kids_of(pid as u32).iter().map(|p| p.pid as u64).collect();
                assert_eq!(got, of::<Vec<u64>>(&kids), "{name}: kids of {pid}");
            }
        }
    }

    #[test]
    fn each_pid_s_kind_need_end_and_reach_read_in_the_procs_order_are_the_ts_answers() {
        // Kinds are asked in the read's order, as the TS caches them: a parent loop reads `other` for the pid whose
        // kind is being worked out, and that answer is kept.
        for (a, want) in cases("place_procs") {
            let name = text(&a[0]);
            let (procs, daemon, host, threads, bins, listening, app) = input_of(&a[1]);
            let listens = |pid: u32| listening.contains(&pid);
            let has_app = !app.is_empty();
            let input = PlaceInput { procs: &procs, daemon, host, threads: &threads, agent_bins: &bins, listens: &listens, app: if has_app { Some(&app) } else { None } };
            let placed = place_procs(&input);
            for (pid, kind) in pairs(&want["kinds"]) {
                assert_eq!(kind_word(placed.kind_of(pid as u32)), kind, "{name}: kind of {pid}");
            }
            for (pid, needed) in pairs(&want["needed"]) {
                assert_eq!(json!(placed.needed(pid as u32)), needed, "{name}: needed {pid}");
            }
            for (pid, endable) in pairs(&want["endable"]) {
                assert_eq!(json!(placed.endable(pid as u32)), endable, "{name}: endable {pid}");
            }
            for (pid, reach) in pairs(&want["reachOf"]) {
                assert_eq!(placed.reach_of(pid as u32).map(Value::String).unwrap_or(Value::Null), reach, "{name}: reach of {pid}");
            }
        }
    }

    #[test]
    fn a_pid_outside_the_read_is_other_needed_by_nobody_ended_by_nobody_and_reached_by_none() {
        let procs = vec![of::<ProcEntry>(&json!({ "pid": 60, "ppid": 1, "user": "u", "state": "S", "comm": "wsp-daemon", "cmdline": "wsp-daemon serve", "cpu": 0, "rss": 1, "startedAt": 1 }))];
        let listens = |_: u32| true;
        let input = PlaceInput { procs: &procs, daemon: 60, host: None, threads: &[], agent_bins: &[], listens: &listens, app: None };
        let placed = place_procs(&input);
        assert_eq!(kind_word(placed.kind_of(4242)), json!("other"));
        assert!(!placed.needed(4242));
        assert!(!placed.endable(4242));
        assert_eq!(placed.reach_of(4242), None);
        assert!(placed.kids_of(4242).is_empty());
    }
}

mod the_view {
    use super::*;

    fn read_of(v: &Value) -> ActivityRead {
        let ports: Option<HashMap<u32, Vec<i64>>> = v["ports"].as_object().map(|m| m.iter().map(|(k, l)| (k.parse().unwrap(), of(l))).collect());
        ActivityRead {
            computer: text(&v["computer"]),
            at: JsNumber(v["at"].as_f64().unwrap()),
            procs: of(&v["procs"]),
            daemon: v["daemon"].as_u64().unwrap() as u32,
            host: v["host"].as_u64().unwrap() as u32,
            scopes: v["scopes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| ActivityScope {
                    thread_id: text(&s["threadId"]),
                    title: text(&s["title"]),
                    workspace_id: text(&s["workspaceId"]),
                    roots: of(&s["roots"]),
                    kept: s["kept"].as_u64().map(|k| k as u32),
                    cgroup: opt_str(&s["cgroup"]),
                    bin: opt_str(&s["bin"]),
                    agents: if s["agents"].is_null() { None } else { Some(of(&s["agents"])) },
                })
                .collect(),
            agent_bins: of(&v["agentBins"]),
            ports,
            cgroups: if v["cgroups"].is_null() { None } else { Some(of(&v["cgroups"])) },
        }
    }

    #[test]
    fn the_view_is_the_ts_view_key_for_key_thread_order_tree_order_depth_totals_ports_and_fallbacks() {
        for (a, want) in cases("activity_of") {
            let view = activity_of(&read_of(&a[1]));
            assert_eq!(serde_json::to_string(&view).unwrap(), serde_json::to_string(&want).unwrap(), "{}", text(&a[0]));
        }
    }

    #[test]
    fn what_a_thread_leaves_is_what_stands_under_no_agent_and_no_turn_plumbing_and_a_thread_left_none_is_dropped() {
        for (a, want) in cases("processes_left") {
            let view: ActivityView = of(&a[0]);
            let ids: Vec<String> = of(&a[1]);
            let left = processes_left(&view, &ids);
            assert_eq!(serde_json::to_string(&left).unwrap(), serde_json::to_string(&want).unwrap(), "{ids:?}");
        }
    }

    #[test]
    fn what_stood_after_a_stop_is_each_ended_row_still_left_in_the_order_ended() {
        for (a, want) in cases("standing_after_stop") {
            let ended: Vec<ActivityEnded> = of(&a[0]);
            assert_eq!(serde_json::to_string(&standing_after_stop(&ended)).unwrap(), serde_json::to_string(&want).unwrap());
        }
    }

    #[test]
    fn the_heads_of_an_end_are_the_rows_whose_parent_is_not_ending_in_the_order_given() {
        #[derive(Clone, Debug, PartialEq)]
        struct Pair(i64, i64);
        impl wsp_wire::views::activity_fold::PidRow for Pair {
            fn pid(&self) -> i64 {
                self.0
            }
            fn ppid(&self) -> i64 {
                self.1
            }
        }
        for (a, want) in cases("end_heads") {
            let rows: Vec<Pair> = a[0].as_array().unwrap().iter().map(|r| Pair(r["pid"].as_i64().unwrap(), r["ppid"].as_i64().unwrap())).collect();
            let got: Vec<i64> = end_heads(&rows).iter().map(|p| p.0).collect();
            let want: Vec<i64> = want.as_array().unwrap().iter().map(|r| r["pid"].as_i64().unwrap()).collect();
            assert_eq!(got, want);
        }
    }
}

mod the_lines {
    use super::*;

    #[test]
    fn each_line_reads_word_for_word_as_the_ts_writes_it() {
        let named: [(&str, fn(&str) -> String); 4] = [("boxEndLine", box_end_line), ("notJoinedActivityFix", not_joined_activity_fix), ("boxBehindActivityLine", box_behind_activity_line), ("notReadLine", not_read_line)];
        for (f, call) in named {
            for (a, want) in cases(f) {
                assert_eq!(call(&text(&a[0])), text(&want), "{f}");
            }
        }
        let by_pid: [(&str, fn(i64) -> String); 5] = [("endGoneSaid", end_gone_said), ("endGoneLine", end_gone_line), ("wspOwnLine", wsp_own_line), ("notThreadsLine", not_threads_line), ("notStartedLine", not_started_line)];
        for (f, call) in by_pid {
            for (a, want) in cases(f) {
                assert_eq!(call(a[0].as_i64().unwrap()), text(&want), "{f}");
            }
        }
    }

    #[test]
    fn a_reused_pid_s_line_writes_both_starts_as_iso_times() {
        for (a, want) in cases("endReusedLine") {
            let now: ProcEntry = of(&a[1]);
            assert_eq!(end_reused_line(a[0].as_i64().unwrap(), &now, a[2].as_i64().unwrap()), text(&want));
        }
    }

    #[test]
    fn the_fix_for_processes_left_names_the_person_s_pids_in_the_order_added_or_offers_stop_and_keep() {
        for (a, want) in cases("processesLeftFix") {
            let verb = if a[0] == "settle" { LeftVerb::Settle } else { LeftVerb::Delete };
            let theirs: Vec<i64> = of(&a[1]);
            assert_eq!(processes_left_fix(verb, &theirs), text(&want), "{a:?}");
        }
    }
}
