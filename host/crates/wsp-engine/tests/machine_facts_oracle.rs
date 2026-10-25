// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of machine_facts against the TS's own answers (spec/machine_facts.md).
#[path = "oracle/mod.rs"]
mod oracle;

use indexmap::IndexMap;
use serde_json::Value;
use wsp_engine::machine_facts::*;

fn values_of(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn every_read_line_is_the_ts_string_byte_for_byte() {
    let o = oracle::load("machine_facts.json");
    let r = &o["reads"];
    let os: Vec<&str> = r["OS_READ"].as_array().unwrap().iter().map(oracle::s).collect();
    assert_eq!(OS_READ.to_vec(), os);
    let up: Vec<&str> = r["UPTIME_READ"].as_array().unwrap().iter().map(oracle::s).collect();
    assert_eq!(UPTIME_READ.to_vec(), up);
    assert_eq!(HOME_READ, oracle::s(&r["HOME_READ"]));
    assert_eq!(SYSTEM_READ, oracle::s(&r["SYSTEM_READ"]));
    assert_eq!(ARCH_READ, oracle::s(&r["ARCH_READ"]));
    assert_eq!(MEM_READ, oracle::s(&r["MEM_READ"]));
    assert_eq!(SHELL_READ, oracle::s(&r["SHELL_READ"]));
}

#[test]
fn read_values_keeps_each_keys_last_value_in_first_seen_order_and_read_lists_keeps_them_all() {
    for case in oracle::load("machine_facts.json")["readValues"].as_array().unwrap() {
        let input = oracle::s(&case["in"]);
        let want: Vec<(String, String)> =
            case["values"].as_array().unwrap().iter().map(|p| (oracle::s(&p[0]).to_string(), oracle::s(&p[1]).to_string())).collect();
        let got: Vec<(String, String)> = read_values(input).into_iter().collect();
        assert_eq!(got, want, "read_values({input:?})");
        let want_lists: Vec<(String, Vec<String>)> = case["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (oracle::s(&p[0]).to_string(), p[1].as_array().unwrap().iter().map(|v| oracle::s(v).to_string()).collect()))
            .collect();
        let got_lists: Vec<(String, Vec<String>)> = read_lists(input).into_iter().collect();
        assert_eq!(got_lists, want_lists, "read_lists({input:?})");
    }
}

#[test]
fn a_line_with_a_leading_space_or_no_space_is_no_answer_and_a_carriage_return_is_trimmed() {
    let got = read_values("nospace\n leading x\nc  spaced  \r\n");
    assert_eq!(got.into_iter().collect::<Vec<_>>(), vec![("c".to_string(), "spaced".to_string())]);
}

#[test]
fn mem_mb_of_reads_the_figure_as_javascripts_number_does() {
    for case in oracle::load("machine_facts.json")["memMbOf"].as_array().unwrap() {
        let input = oracle::s(&case["in"]);
        let want = case["out"].as_u64();
        assert_eq!(mem_mb_of(&values_of(&[("memkb", input)])), want, "memkb {input:?}");
    }
    assert_eq!(mem_mb_of(&IndexMap::new()), None);
}

#[test]
fn uptime_ms_of_takes_procs_figure_first_and_the_boot_moment_otherwise() {
    for case in oracle::load("machine_facts.json")["uptimeMsOf"].as_array().unwrap() {
        let mut v = IndexMap::new();
        if let Some(u) = case["uptime"].as_str() {
            v.insert("uptime".to_string(), u.to_string());
        }
        v.insert("boot".to_string(), oracle::s(&case["boot"]).to_string());
        let now = case["now"].as_i64().unwrap();
        assert_eq!(uptime_ms_of(&v, now), case["out"].as_i64(), "{case}");
    }
}

#[test]
fn arch_and_system_read_an_empty_answer_as_none() {
    for case in oracle::load("machine_facts.json")["archOf"].as_array().unwrap() {
        let v = match case["in"].as_str() {
            Some(a) => values_of(&[("arch", a), ("system", a)]),
            None => IndexMap::new(),
        };
        let want = case["out"].as_str().map(str::to_string);
        assert_eq!(arch_of(&v), want);
        assert_eq!(system_of(&v), want);
    }
}

#[test]
fn os_name_prefers_the_mac_version_then_the_distribution_then_the_kernel() {
    let v = values_of(&[("pretty", "Ubuntu"), ("mac", "26.4"), ("kernel", "Darwin 25")]);
    assert_eq!(os_name_of(&v).as_deref(), Some("macOS 26.4"));
    let v = values_of(&[("pretty", ""), ("mac", ""), ("kernel", "")]);
    assert_eq!(os_name_of(&v), None);
    let _ = Value::Null;
}
