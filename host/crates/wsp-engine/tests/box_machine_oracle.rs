// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of box_backend::machine's text against the TS's own answers (spec/box_machine.md).
#[path = "oracle/mod.rs"]
mod oracle;

use indexmap::IndexMap;
use wsp_engine::box_backend::*;

#[test]
fn the_environment_script_is_the_ts_script_for_each_set_of_envs() {
    for case in oracle::load("box.json")["envLandingScript"].as_array().unwrap() {
        let envs: IndexMap<String, String> = case["in"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), oracle::s(v).to_string())).collect();
        assert_eq!(env_landing_script(&envs), oracle::s(&case["out"]), "{}", case["in"]);
    }
}

#[test]
fn every_command_runs_as_root_under_the_ts_wrapper() {
    for case in oracle::load("box.json")["sudoCommand"].as_array().unwrap() {
        assert_eq!(sudo_command(oracle::s(&case["in"])), oracle::s(&case["out"]));
    }
}

#[test]
fn the_daemon_exits_command_is_the_ts_script() {
    assert_eq!(DAEMON_EXITS_CMD, oracle::s(&oracle::load("box.json")["DAEMON_EXITS_CMD"]));
}

#[test]
fn daemon_exits_of_reads_every_shape_the_ts_reads() {
    for case in oracle::load("box.json")["daemonExitsOf"].as_array().unwrap() {
        assert_eq!(daemon_exits_of(oracle::s(&case["in"])).as_deref(), case["out"].as_str(), "{:?}", case["in"]);
    }
}

#[test]
fn exit_words_name_each_result_and_pass_any_other_through() {
    assert_eq!(exit_words("exit-code", "3"), "exit status 3");
    assert_eq!(exit_words("signal", "15"), "killed by signal 15");
    assert_eq!(exit_words("core-dump", "6"), "killed by signal 6");
    assert_eq!(exit_words("oom-kill", "9"), "killed for want of memory");
    assert_eq!(exit_words("timeout", "0"), "timeout");
}
