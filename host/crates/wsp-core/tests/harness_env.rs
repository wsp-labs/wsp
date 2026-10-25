// SPDX-License-Identifier: AGPL-3.0-only
//! The login environments of harness.ts against the TS oracle: every fixture value is an ordered list of
//! [key, value] pairs, so the key order is part of what a test pins.

mod common;

use common::fixture;
use serde_json::Value;
use wsp_adapters::port::Env;
use wsp_core::types::harness::{guest_login_env, login_env_on, machine_sandbox_env, place_login_env};

fn pairs(env: &Env) -> Vec<(String, String)> {
    env.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()
}

fn expected(fixture: &Value, key: &str) -> Vec<(String, String)> {
    fixture[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is an array"))
        .iter()
        .map(|p| (p[0].as_str().expect("a key").to_owned(), p[1].as_str().expect("a value").to_owned()))
        .collect()
}

// harness.ts:78
#[test]
fn machine_sandbox_env_is_the_sandbox_flag_then_the_autoupdater_flag() {
    let f = fixture("harness");
    assert_eq!(pairs(&machine_sandbox_env()), expected(&f, "machineSandbox"));
}

// harness.ts:82
#[test]
fn guest_login_env_is_home_user_path_then_the_sandbox_flags() {
    let f = fixture("harness");
    assert_eq!(pairs(&guest_login_env()), expected(&f, "guestLogin"));
}

// harness.ts:89-94
#[test]
fn place_login_env_puts_the_place_path_in_the_guest_path_slot_and_adds_the_tool_homes_last() {
    let f = fixture("harness");
    assert_eq!(pairs(&place_login_env()), expected(&f, "placeLogin"));
}

// harness.ts:98-101
#[test]
fn login_env_on_no_place_is_the_guest_environment() {
    let f = fixture("harness");
    assert_eq!(pairs(&login_env_on(None, None)), expected(&f, "onGuest"));
}

// harness.ts:98-101
#[test]
fn login_env_on_a_place_is_the_place_environment() {
    let f = fixture("harness");
    assert_eq!(pairs(&login_env_on(Some("p1"), None)), expected(&f, "onPlace"));
}

// harness.ts:98-101
#[test]
fn login_env_on_the_guest_with_npm_prepends_the_agent_bin_and_npm_bin_to_path_in_place() {
    let f = fixture("harness");
    assert_eq!(pairs(&login_env_on(None, Some("/opt/npm/bin"))), expected(&f, "onGuestNpm"));
}

// harness.ts:98-101
#[test]
fn login_env_on_a_place_with_npm_prepends_the_agent_bin_and_npm_bin_to_path_in_place() {
    let f = fixture("harness");
    assert_eq!(pairs(&login_env_on(Some("p1"), Some("/opt/npm/bin"))), expected(&f, "onPlaceNpm"));
}

// harness.ts:98-101
#[test]
fn login_env_on_an_empty_place_name_reads_as_a_place_not_the_guest() {
    let f = fixture("harness");
    assert_eq!(pairs(&login_env_on(Some(""), None)), expected(&f, "onEmptyPlace"));
}
