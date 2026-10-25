// SPDX-License-Identifier: AGPL-3.0-only
//! A harness catalog's picks held to the TS answers for one catalog: marked defaults, the efforts and windows a model
//! takes, every model a start may name, the picks a thread's rows carry and keep, the checks a start meets, and the
//! adapter answers a catalog stands in for. spec/harness.md lists the rule each test pins.

#[path = "rules/oracle.rs"]
mod oracle;

use oracle::{cases, of, opt_str, text};
use serde_json::{json, Value};
use wsp_wire::types::harness_picks::*;
use wsp_wire::views::harness::*;
use wsp_wire::views::session::SessionView;

/// The catalog every case reads, as zod parsed it.
fn catalog() -> HarnessCatalog {
    of(&cases("catalog")[0].1)
}

fn model_arg(cat: &HarnessCatalog, v: &Value) -> Option<HarnessModel> {
    model_of(cat, v.as_str())
}

fn opt_json<T: serde::Serialize>(v: Option<T>) -> Value {
    v.map(|x| serde_json::to_value(x).unwrap()).unwrap_or(Value::Null)
}

#[test]
fn a_model_is_the_catalog_s_by_value_a_slug_it_lacks_is_named_by_itself_and_none_is_none() {
    let cat = catalog();
    for (a, want) in cases("model_of") {
        assert_eq!(opt_json(model_of(&cat, a[0].as_str())), want, "{a:?}");
    }
}

#[test]
fn every_model_is_the_current_ones_then_the_legacy_then_the_hidden_in_that_order() {
    let cat = catalog();
    let (_, want) = &cases("every_model")[0];
    assert_eq!(serde_json::to_value(every_model(&cat)).unwrap(), *want);
}

#[test]
fn a_model_s_efforts_narrow_the_catalog_s_in_its_order_and_its_own_default_takes_every_other_mark() {
    // The opus case names defaultEffort "low": the catalog's mark on "high" goes, and "low" alone is marked.
    let cat = catalog();
    for (a, want) in cases("efforts_for") {
        let model = model_arg(&cat, &a[0]);
        let got = efforts_for(&cat, model.as_ref());
        assert_eq!(serde_json::to_value(&got).unwrap(), want, "{a:?}");
        let marked: Vec<&str> = got.iter().filter(|o| o.is_default == Some(true)).map(|o| o.value.as_str()).collect();
        let want_marked: Vec<&str> =
            want.as_array().unwrap().iter().filter(|o| o["isDefault"] == json!(true)).map(|o| o["value"].as_str().unwrap()).collect();
        assert_eq!(marked, want_marked, "the marks, {a:?}");
    }
}

#[test]
fn a_model_s_context_windows_narrow_the_catalog_s_and_no_model_has_none() {
    let cat = catalog();
    for (a, want) in cases("context_windows_for") {
        let model = model_arg(&cat, &a[0]);
        assert_eq!(serde_json::to_value(context_windows_for(&cat, model.as_ref())).unwrap(), want, "{a:?}");
    }
}

#[test]
fn the_marked_default_is_the_first_option_marked_and_none_where_none_is() {
    let cat = catalog();
    for (a, want) in cases("marked_default") {
        let got = match text(&a[0]).as_str() {
            "efforts" => opt_json(marked_default(&cat.efforts)),
            "permissionModes" => opt_json(marked_default(&cat.permission_modes)),
            "models" => opt_json(marked_default(&cat.models)),
            _ => opt_json(marked_default(&cat.context_windows)),
        };
        assert_eq!(got, want, "{a:?}");
    }
}

#[test]
fn a_kept_pick_stands_only_where_the_list_carries_it() {
    let cat = catalog();
    for (a, want) in cases("listed_pick") {
        assert_eq!(listed_pick(&cat.efforts, a[0].as_str()), opt_str(&want), "{a:?}");
    }
}

#[test]
fn a_model_s_1m_suffix_is_its_window_unless_a_window_is_named_and_comes_off_the_model() {
    for (a, want) in cases("model_picks") {
        let got = model_picks(&text(&a[0]), a[1].as_str());
        assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap(), "{a:?}");
    }
}

#[test]
fn what_a_thread_ran_with_is_each_pick_s_latest_turn_that_named_one_model_and_window_from_one_row() {
    for (a, want) in cases("recorded_picks") {
        let rows: Vec<SessionView> = of(&a[0]);
        let got = recorded_picks(&rows);
        assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap(), "{a:?}");
    }
}

#[test]
fn a_resume_keeps_the_thread_s_model_only_where_it_takes_what_the_start_names_and_drops_what_the_list_lost() {
    let cat = catalog();
    for (a, want) in cases("kept_picks") {
        let ran: RanPicks = of(&a[1]);
        let named: KeptPicks = of(&a[2]);
        let got = kept_picks(if a[0].is_null() { None } else { Some(&cat) }, &ran, &named);
        assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap(), "{a:?}");
    }
}

#[test]
fn a_start_fills_what_opens_a_thread_from_the_marks_and_refuses_a_value_no_list_carries_or_a_fast_no_model_has() {
    let cat = catalog();
    for (a, want) in cases("start_picks") {
        let picks: StartPicks = of(&a[1]);
        let got = start_picks(if a[0].is_null() { None } else { Some(&cat) }, &picks, a[2].as_bool().unwrap(), a[3].as_str());
        match (&got, want.get("ok")) {
            (Ok(p), Some(ok)) => assert_eq!(serde_json::to_string(p).unwrap(), serde_json::to_string(ok).unwrap(), "{a:?}"),
            (Err(e), None) => {
                assert_eq!(e.message, text(&want["err"]["message"]), "{a:?}");
                assert_eq!(e.kind, text(&want["err"]["kind"]), "{a:?}");
            }
            _ => panic!("{a:?}: got {got:?}, the TS answered {want}"),
        }
    }
}

#[test]
fn servers_named_for_an_agent_that_takes_none_are_refused_in_its_name_and_none_named_pass() {
    for (a, want) in cases("mcp_servers_blocked") {
        let servers: Option<indexmap::IndexMap<String, McpServerSpec>> = if a[0].is_null() { None } else { Some(of(&a[0])) };
        assert_eq!(mcp_servers_blocked(servers.as_ref(), a[1].as_bool().unwrap(), "cursor"), opt_str(&want), "{a:?}");
    }
    assert_eq!(no_mcp_servers_line("cursor"), text(&cases("no_mcp_servers_line")[0].1));
    assert_eq!(no_fast_line("Opus"), text(&cases("no_fast_line")[0].1));
}

#[test]
fn no_catalog_or_the_runtime_s_table_answers_yes_to_renames_and_images_and_no_to_servers() {
    let cat = catalog();
    let mut table = cat.clone();
    table.source = HarnessCatalogSource::Table;
    for (fname, f) in [
        ("keeps_rename", keeps_rename as fn(Option<&HarnessCatalog>) -> bool),
        ("reads_images", reads_images),
        ("takes_mcp_servers", takes_mcp_servers),
    ] {
        for (a, want) in cases(fname) {
            let arg = match a[0].as_str() {
                None => None,
                Some("table") => Some(&table),
                Some(_) => Some(&cat),
            };
            assert_eq!(f(arg), want.as_bool().unwrap(), "{fname} {a:?}");
        }
    }
}
