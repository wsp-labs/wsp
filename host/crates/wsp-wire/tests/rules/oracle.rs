// SPDX-License-Identifier: AGPL-3.0-only
//! The answers the TS functions gave for chosen inputs (fixtures/rules/oracle.json, written by running the TS at
//! a3b870a6e): each rule test calls the Rust function on the same arguments and wants the same answer.
#![allow(dead_code)]

use serde::de::DeserializeOwned;
use serde_json::Value;

const ORACLE: &str = include_str!("../fixtures/rules/oracle.json");

/// Every case of one function: its arguments, and what the TS answered (null for undefined).
pub fn cases(name: &str) -> Vec<(Vec<Value>, Value)> {
    let all: Value = serde_json::from_str(ORACLE).expect("oracle is JSON");
    let list = all[name].as_array().unwrap_or_else(|| panic!("no oracle cases for {name}"));
    assert!(!list.is_empty(), "no oracle cases for {name}");
    list.iter().map(|c| (c["args"].as_array().expect("args").clone(), c["want"].clone())).collect()
}

/// A JSON value as a Rust value, through plain serde (not the crate's parse, so the input side never waits on it).
pub fn of<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("fixture {v} does not read as the Rust type: {e}"))
}

pub fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

pub fn text(v: &Value) -> String {
    v.as_str().unwrap_or_else(|| panic!("{v} is not text")).to_string()
}
