// SPDX-License-Identifier: AGPL-3.0-only
//! Unit tools_loop's hard tests: each scenario in tests/fixtures/oracle/golden_tools_loop.json is a run of the TS
//! installTools against the scripted machine; the Rust loop must make the same calls in the same order, with the
//! same deadlines, say the same stage lines, hand the caller the same rows in the same order and answer the same.
//! spec/golden-tools_loop.md names the rule each scenario pins.
mod golden_common;

use std::sync::{Arc, Mutex};

use golden_common::{assert_calls, fixture, s, timeless, timeless_all, Scripted, Stages};
use serde_json::{json, Value};
use wsp_engine::golden_seam::{install_tools, install_tools_at, GoldenStage, InstallToolsOptions, OnTool, ToolInstall, ToolResult};

fn scenario(name: &str) -> Value {
    fixture("tools_loop").as_array().unwrap().iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no scenario {name}")).clone()
}

struct Ran {
    out: Result<Value, String>,
    on_tool: Value,
    stages: Value,
    calls: Value,
}

async fn replay(c: &Value) -> Ran {
    let m = Scripted::new(&c["rules"], false);
    let tools: Vec<ToolInstall> = serde_json::from_value(c["tools"].clone()).unwrap();
    let stages = Stages::default();
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let on_tool: OnTool = {
        let seen = seen.clone();
        Arc::new(move |r: ToolResult| {
            seen.lock().unwrap().push(timeless(&r));
            Box::pin(async {})
        })
    };
    let o = c.get("opts").cloned().unwrap_or(json!({}));
    let opts = InstallToolsOptions {
        present: o.get("present").map(|p| p.as_array().unwrap().iter().map(s).collect()).unwrap_or_default(),
        keep_caches: o.get("caches") == Some(&json!("keep")),
        floor: o.get("floor").and_then(Value::as_u64),
        path: o.get("path").and_then(Value::as_str).map(str::to_string),
        prefix: o.get("prefix").and_then(Value::as_str).map(str::to_string),
        on_tool: Some(on_tool),
    };
    let at: GoldenStage = serde_json::from_value(c.get("at").cloned().unwrap_or(json!("installing-tools"))).unwrap();
    let out = install_tools_at(&m, &tools, stages.tool_stage(), at, opts)
        .await
        .map(|o| json!({ "tools": timeless_all(&o.tools), "homebrew": o.homebrew.map(|h| json!({ "tag": h.tag, "commit": h.commit })) }))
        .map_err(|e| e.to_string());
    let on_tool = Value::Array(seen.lock().unwrap().clone());
    Ran { out, on_tool, stages: stages.value(), calls: m.calls() }
}

async fn check(name: &str) {
    let c = scenario(name);
    let ran = replay(&c).await;
    match (&ran.out, c["out"].get("error")) {
        (Err(e), Some(words)) => assert_eq!(e, words.as_str().unwrap(), "{name}"),
        (Ok(v), None) => {
            assert_eq!(v["tools"], c["out"]["tools"], "{name}: the rows");
            assert_eq!(v["homebrew"], c["out"]["homebrew"], "{name}: the Homebrew checkout");
        }
        (got, _) => panic!("{name}: {got:?} against {}", c["out"]),
    }
    assert_eq!(ran.on_tool, c["onTool"], "{name}: what the caller was handed, in order");
    assert_eq!(ran.stages, c["stages"], "{name}: the stage lines");
    assert_calls(name, &ran.calls, &c["calls"]);
}

#[tokio::test]
async fn two_steps_one_after_the_other_each_checked_after_the_loop() {
    check("two steps, the second after the first, each checked after the loop").await;
}

#[tokio::test]
async fn a_failure_is_named_by_its_error_line_and_skips_what_waits_on_it() {
    check("a failure is named by its Error line and skips what waits on it; the next one still runs").await;
}

#[tokio::test]
async fn a_step_already_on_the_machine_runs_nothing_and_lets_what_waits_on_it_run() {
    check("a step already on the machine runs nothing and lets what waits on it run").await;
}

#[tokio::test]
async fn under_the_floor_the_sweep_alone_runs_and_the_install_goes_on_once_it_clears() {
    check("under the floor with no Homebrew the sweep alone runs and the install goes on when it clears").await;
}

#[tokio::test]
async fn still_under_the_floor_after_the_cleanup_every_tool_left_is_skipped_with_the_reading() {
    check("under the floor and still under after the cleanup: every tool left is skipped with the reading").await;
}

#[tokio::test]
async fn df_unknown_after_the_cleanup_skips_with_both_readings() {
    check("df unknown after the cleanup").await;
}

#[tokio::test]
async fn a_full_disk_reads_zero_not_unknown() {
    check("a full disk reads zero, not unknown").await;
}

#[tokio::test]
async fn a_df_that_fails_warns_once_and_installs_without_the_floor() {
    check("a df that fails warns once and installs without the floor").await;
}

#[tokio::test]
async fn a_df_that_prints_junk_names_what_it_printed() {
    check("a df that exits 0 printing something it cannot use names what it printed").await;
}

#[tokio::test]
async fn an_install_too_big_for_the_room_above_the_floor_is_skipped_alone() {
    check("an install too big for the room above the floor is skipped alone, and what fits after it installs").await;
}

#[tokio::test]
async fn an_install_still_too_big_after_the_cleanup_stays_skipped_with_the_reading_kept() {
    check("a too-big install still too big after the cleanup stays skipped").await;
}

#[tokio::test]
async fn a_cellar_lock_waits_for_every_homebrew_lock_then_tries_once_more() {
    check("a cellar lock waits for every Homebrew lock, then tries once more").await;
}

#[tokio::test]
async fn a_download_road_at_its_limit_is_tried_once_more() {
    check("a download road that hits its limit is tried once more, and the second run is an install").await;
}

#[tokio::test]
async fn a_download_road_at_its_limit_twice_fails_with_both_and_a_compile_road_is_not_retried() {
    check("a download road that hits its limit twice is failed with both timeouts; a compile road is not tried again").await;
}

#[tokio::test]
async fn homebrew_installed_carries_its_checkout_and_the_housekeeping_says_what_it_freed() {
    check("Homebrew installs: the result carries its checkout and the housekeeping after the loop says what it freed").await;
}

#[tokio::test]
async fn a_homebrew_cleanup_that_fails_at_the_floor_is_named_in_the_rescue_line() {
    check("Homebrew cleanup that fails at the floor is named in the rescue's line").await;
}

#[tokio::test]
async fn an_install_that_leaves_no_command_on_path_fails_by_name_and_loses_its_road() {
    check("an install that exits 0 without its command on PATH is failed by name").await;
}

#[tokio::test]
async fn a_path_check_that_fails_counts_every_named_tool_as_failed() {
    check("a PATH check that itself fails counts every named tool as failed").await;
}

#[tokio::test]
async fn checks_read_a_page_at_a_time_and_a_page_that_could_not_run_fails_only_its_rows() {
    check("checks read a page at a time: a page that printed failures fails those rows, a page that could not run fails its own").await;
}

#[tokio::test]
async fn a_road_install_names_its_road_and_a_label_with_a_comma_is_quoted_in_the_tally() {
    check("a road install names the road in its result and the summary; a note and a label with a comma").await;
}

#[tokio::test]
async fn a_prefix_reads_the_volume_wsp_installs_onto_under_the_floor_the_caller_holds() {
    check("a prefix reads the volume wsp installs onto and runs on the probe PATH, with a floor the caller holds").await;
}

#[tokio::test]
async fn the_stage_named_by_the_caller_is_on_every_line() {
    check("the base stage's own name on every line").await;
}

#[tokio::test]
async fn a_free_read_that_rejects_ends_the_loop_with_its_error_before_anything_installs() {
    check("a free read whose exec rejects throws out of the loop before anything installs").await;
}

#[tokio::test]
async fn an_install_run_that_rejects_ends_the_loop_with_its_error() {
    check("an install run that rejects throws out of the loop").await;
}

#[tokio::test]
async fn skipped_rows_share_a_reason_line_in_the_order_first_met() {
    check("skipped rows share a reason line").await;
}

#[tokio::test]
async fn no_tools_still_sweeps_and_closes() {
    check("no tools at all").await;
}

#[tokio::test]
async fn every_scenario_in_the_fixture_has_a_test_here() {
    let named = [
        "two steps, the second after the first, each checked after the loop",
        "a failure is named by its Error line and skips what waits on it; the next one still runs",
        "a step already on the machine runs nothing and lets what waits on it run",
        "under the floor with no Homebrew the sweep alone runs and the install goes on when it clears",
        "under the floor and still under after the cleanup: every tool left is skipped with the reading",
        "df unknown after the cleanup",
        "a full disk reads zero, not unknown",
        "a df that fails warns once and installs without the floor",
        "a df that exits 0 printing something it cannot use names what it printed",
        "an install too big for the room above the floor is skipped alone, and what fits after it installs",
        "a too-big install still too big after the cleanup stays skipped",
        "a cellar lock waits for every Homebrew lock, then tries once more",
        "a download road that hits its limit is tried once more, and the second run is an install",
        "a download road that hits its limit twice is failed with both timeouts; a compile road is not tried again",
        "Homebrew installs: the result carries its checkout and the housekeeping after the loop says what it freed",
        "Homebrew cleanup that fails at the floor is named in the rescue's line",
        "an install that exits 0 without its command on PATH is failed by name",
        "a PATH check that itself fails counts every named tool as failed",
        "checks read a page at a time: a page that printed failures fails those rows, a page that could not run fails its own",
        "a road install names the road in its result and the summary; a note and a label with a comma",
        "a prefix reads the volume wsp installs onto and runs on the probe PATH, with a floor the caller holds",
        "the base stage's own name on every line",
        "a free read whose exec rejects throws out of the loop before anything installs",
        "an install run that rejects throws out of the loop",
        "skipped rows share a reason line",
        "no tools at all",
    ];
    let all: Vec<String> = fixture("tools_loop").as_array().unwrap().iter().map(|c| s(&c["name"])).collect();
    assert_eq!(all, named);
    // The wrapper provision calls is the same loop under installing-tools, answering only the rows.
    let c = scenario("two steps, the second after the first, each checked after the loop");
    let m = Scripted::new(&c["rules"], false);
    let tools: Vec<ToolInstall> = serde_json::from_value(c["tools"].clone()).unwrap();
    let rows = install_tools(&m, &tools, Stages::default().tool_stage(), InstallToolsOptions::default()).await.unwrap();
    assert_eq!(timeless_all(&rows), c["out"]["tools"]);
    assert_calls("install_tools", &m.calls(), &c["calls"]);
}
