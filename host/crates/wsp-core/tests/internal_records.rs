// SPDX-License-Identifier: AGPL-3.0-only
//! internal.ts against the TS oracle (tests/fixtures/core_types/internal.json): the constants, the key and log
//! line builders, the stopped-prepare error, the refusals, the readers of stored rows, the launch match, and the
//! records read off JSON and written back as they came.

mod common;

use common::{fixture, json_of, wire};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_core::types::internal::*;
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_words::exit::Kind;
use wsp_words::words::computer::DefaultBranchRoad;

fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_json(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same_json(v, w))),
        _ => a == b,
    }
}

fn round_trip<T: DeserializeOwned + Serialize>(doc: &Value) -> Value {
    let record: T = wire(doc);
    let out = serde_json::to_value(&record).expect("writes");
    assert!(same_json(&out, doc), "written back differently:\n  read    {doc}\n  written {out}");
    out
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object().expect("an object").keys().map(String::as_str).collect()
}

fn text(v: &Value) -> &str {
    v.as_str().expect("a string")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("an array").iter().map(|s| text(s).to_owned()).collect()
}

// --- constants -----------------------------------------------------------------------------------------------------

macro_rules! same_str {
    ($c:expr, $($name:ident),+ $(,)?) => {$(
        assert_eq!($name, text(&$c[stringify!($name)]), stringify!($name));
    )+};
}

macro_rules! same_num {
    ($c:expr, $($name:ident),+ $(,)?) => {$(
        assert_eq!($name as u64, $c[stringify!($name)].as_u64().expect("a whole number"), stringify!($name));
    )+};
}

// internal.ts:44-137
#[test]
fn constants_collection_names_match_the_ts() {
    let f = fixture("internal");
    let c = &f["constants"];
    same_str!(
        c,
        WORKSPACES,
        PROJECTS,
        SEED_CHOICES,
        GOLDENS,
        GOLDEN_RECIPES,
        PROJECT_GOLDENS,
        IMAGES,
        IMAGE_VAULTS,
        TRANSCRIPTS,
        TRANSCRIPT_HEADS,
        TRANSCRIPT_INDEX,
        ATTACHMENTS,
        ATTACHMENT_KEYS,
        SESSIONS,
        NOTIFY_OWED,
        HELD_STARTS,
        SUB_STARTS,
        WORKSPACE_NAMES,
        DROPPED,
        UNRECORDED,
        PREFERENCES,
        PREFERENCES_ID,
        READS,
        READS_ID,
        AGENT_LISTS,
        BUILDERS,
        OWNER,
        CREATES,
    );
}

// internal.ts:61-64,161-164
#[test]
fn constants_sentences_match_the_ts() {
    let f = fixture("internal");
    let c = &f["constants"];
    same_str!(c, NO_COPY_RECIPE, NO_SEED_WIRING, PAUSED_REASON, DELETED_REASON, RESTARTED_REASON, GONE_REASON);
}

// internal.ts:99-233,847-877
#[test]
fn constants_times_and_caps_match_the_ts() {
    let f = fixture("internal");
    let c = &f["constants"];
    same_num!(
        c,
        OWED_RETRY_MS,
        OWED_RETRY_MAX_MS,
        OWED_FOR_MS,
        DROPPED_WATCH_MS,
        UNRECORDED_DELETE_MS,
        DAEMON_TOKEN_MISS_TTL_MS,
        TRANSCRIPT_CAP,
        TRANSCRIPT_BYTES,
        TOOL_RESULT_KEPT,
        TRANSCRIPTS_HELD,
        PENDING_FLUSH_BYTES,
        MAX_TIMER_MS,
        WORDS_HELD,
        SESSION_INDEX_CAP,
        TRANSCRIPT_FLUSH_MS,
        KEPT_CLOSE_WAIT_MS,
        KEY_PURPOSE_MAX,
        LOST_CREATE_MS,
        HELD_TTL_MS,
        HEARTBEAT_MS,
    );
}

// internal.ts:296
#[test]
fn constants_session_facts_are_cwd_permission_mode_model_effort_in_that_order() {
    let f = fixture("internal");
    assert_eq!(SESSION_FACTS.to_vec(), strings(&f["constants"]["SESSION_FACTS"]));
}

// internal.ts:1055
#[test]
fn constants_optional_backend_calls_are_the_ten_names_in_the_ts_order() {
    let f = fixture("internal");
    assert_eq!(OPTIONAL_BACKEND_CALLS.to_vec(), strings(&f["constants"]["OPTIONAL_BACKEND_CALLS"]));
}

// --- keys ----------------------------------------------------------------------------------------------------------

// internal.ts:69
#[test]
fn copy_key_joins_place_and_name_with_a_slash() {
    let f = fixture("internal");
    assert_eq!(copy_key("solari", "default"), text(&f["keys"]["copyKey"]));
}

fn parts_row(key: &str) -> (CopyKeyParts, CopyKeyParts) {
    let f = fixture("internal");
    let row = f["keys"]["parts"].as_array().unwrap().iter().find(|r| text(&r[0]) == key).unwrap_or_else(|| panic!("no row for {key}"));
    let want = CopyKeyParts { place: row[1].get("place").map(|p| text(p).to_owned()), name: text(&row[1]["name"]).to_owned() };
    (copy_key_parts(key), want)
}

// internal.ts:72-75
#[test]
fn copy_key_parts_splits_at_the_first_slash() {
    let (got, want) = parts_row("solari/default");
    assert_eq!(got, want);
    assert_eq!(got, CopyKeyParts { place: Some("solari".into()), name: "default".into() });
}

// internal.ts:72-75
#[test]
fn copy_key_parts_keeps_later_slashes_in_the_name() {
    let (got, want) = parts_row("a/b/c");
    assert_eq!(got, want);
    assert_eq!(got, CopyKeyParts { place: Some("a".into()), name: "b/c".into() });
}

// internal.ts:72-75
#[test]
fn copy_key_parts_of_a_key_with_no_slash_names_no_place() {
    let (got, want) = parts_row("default");
    assert_eq!(got, want);
    assert_eq!(got, CopyKeyParts { place: None, name: "default".into() });
}

// internal.ts:72-75
#[test]
fn copy_key_parts_of_a_leading_slash_is_an_empty_place_not_no_place() {
    let (got, want) = parts_row("/x");
    assert_eq!(got, want);
    assert_eq!(got, CopyKeyParts { place: Some(String::new()), name: "x".into() });
}

// internal.ts:72-75
#[test]
fn copy_key_parts_of_a_trailing_slash_is_an_empty_name() {
    let (got, want) = parts_row("x/");
    assert_eq!(got, want);
    assert_eq!(got, CopyKeyParts { place: Some("x".into()), name: String::new() });
}

// internal.ts:76
#[test]
fn recipe_key_is_the_name_at_v_and_the_version() {
    let f = fixture("internal");
    assert_eq!(recipe_key("default", 3), text(&f["keys"]["recipeKey"]));
}

// internal.ts:77
#[test]
fn vault_key_is_the_name_at_v_and_the_version() {
    let f = fixture("internal");
    assert_eq!(vault_key("img", 12), text(&f["keys"]["vaultKey"]));
}

// --- lines ---------------------------------------------------------------------------------------------------------

fn line(name: &str) -> String {
    text(&fixture("internal")["lines"][name]).to_owned()
}

// internal.ts:167
#[test]
fn label_of_is_the_record_name_where_no_thread_named_the_verb() {
    let f = fixture("internal");
    assert_eq!(label_of("box", None), text(&f["lines"]["labelOf"][0]));
}

// internal.ts:167
#[test]
fn label_of_a_thread_is_thread_and_its_id_cut_to_eight() {
    let f = fixture("internal");
    assert_eq!(label_of("box", Some("0123456789abcdef")), text(&f["lines"]["labelOf"][1]));
}

// internal.ts:169
#[test]
fn gone_log_line_says_the_workspace_is_gone_and_why() {
    assert_eq!(gone_log_line("w1", "404"), line("gone"));
}

// internal.ts:171-172
#[test]
fn swept_runs_log_line_names_one_run_in_the_singular() {
    assert_eq!(swept_runs_log_line("w1", &["r1".into()]), line("swept1"));
}

// internal.ts:171-172
#[test]
fn swept_runs_log_line_joins_two_runs_with_a_comma_and_pluralizes() {
    assert_eq!(swept_runs_log_line("w1", &["r1".into(), "r2".into()]), line("swept2"));
}

// internal.ts:171-172
#[test]
fn swept_runs_log_line_of_no_runs_still_pluralizes_and_ends_after_the_colon_space() {
    assert_eq!(swept_runs_log_line("w1", &[]), line("swept0"));
}

// internal.ts:177-178
#[test]
fn server_values_unread_line_names_the_agent_and_the_reason_in_brackets() {
    assert_eq!(server_values_unread_line("Claude Code", "timed out"), line("unread"));
}

// internal.ts:181-182
#[test]
fn server_values_left_line_carries_the_line_after_a_colon() {
    assert_eq!(server_values_left_line("Codex", "config.toml is a link out of its folder"), line("left"));
}

// internal.ts:185-186
#[test]
fn server_values_cut_line_joins_two_files_with_and() {
    assert_eq!(server_values_cut_line("Codex", &["a.json".into(), "b.toml".into()]), line("cut"));
}

// internal.ts:187-188
#[test]
fn no_title_log_line_cuts_the_session_id_to_eight_units() {
    assert_eq!(no_title_log_line("0123456789", "w1", "gone"), line("noTitle"));
}

// internal.ts:187-188
#[test]
fn no_title_log_line_cuts_the_session_id_at_eight_utf16_units_so_five_emoji_keep_four() {
    assert_eq!(no_title_log_line("😀😀😀😀😀", "w1", "gone"), line("noTitleEmoji"));
}

// internal.ts:191-192
#[test]
fn no_made_title_log_line_cuts_the_thread_id_to_eight() {
    assert_eq!(no_made_title_log_line("abcdefghij", "w1", "slow"), line("noMadeTitle"));
}

// internal.ts:194-195
#[test]
fn no_checkpoint_log_line_cuts_the_thread_id_to_eight() {
    assert_eq!(no_checkpoint_log_line("abcdefghij", "w1", "dirty"), line("noCheckpoint"));
}

// internal.ts:198-199
#[test]
fn no_name_write_log_line_keeps_a_short_session_id_whole() {
    assert_eq!(no_name_write_log_line("abc", "w1", "refused"), line("noNameWrite"));
}

// internal.ts:202-203
#[test]
fn lost_session_log_line_keeps_the_session_id_whole_and_cuts_the_thread_id() {
    assert_eq!(lost_session_log_line("sess-full-id", "thread-long-id", "w1", "no file"), line("lostSession"));
}

// internal.ts:205-206
#[test]
fn stop_log_line_without_a_task_or_leftover_cuts_the_thread_id_and_ends_at_the_outcome() {
    assert_eq!(stop_log_line("thread-long-id", None, "stopped", None), line("stop"));
}

// internal.ts:205-206
#[test]
fn stop_log_line_with_a_task_and_leftover_cuts_both_ids_and_puts_the_leftover_in_brackets() {
    assert_eq!(stop_log_line("thread-long-id", Some("task-long-id"), "gone", Some("3 left")), line("stopTask"));
}

fn restart_line(i: usize) -> String {
    text(&fixture("internal")["lines"]["restart"][i]).to_owned()
}

// internal.ts:208
#[test]
fn restart_cut_line_of_zero_ms_reads_zero_minutes_zero_seconds() {
    assert_eq!(restart_cut_line(0.0), restart_line(0));
}

// internal.ts:208
#[test]
fn restart_cut_line_of_999_ms_reads_one_second() {
    assert_eq!(restart_cut_line(999.0), restart_line(1));
}

// internal.ts:208
#[test]
fn restart_cut_line_of_61_seconds_reads_one_minute_one_second() {
    assert_eq!(restart_cut_line(61_000.0), restart_line(2));
}

// internal.ts:208
#[test]
fn restart_cut_line_past_an_hour_reads_hours_minutes_seconds() {
    assert_eq!(restart_cut_line(3_605_000.0), restart_line(3));
}

// internal.ts:293
#[test]
fn transcript_unread_line_names_the_workspace_and_why_it_does_not_read() {
    assert_eq!(transcript_unread_line("w1", "bad json"), line("transcriptUnread"));
}

// --- prepare -------------------------------------------------------------------------------------------------------

fn prepare_row(i: usize) -> (PrepareStoppedError, Value) {
    let f = fixture("internal");
    let row = &f["prepare"][i];
    let id = row["id"].as_str().map(str::to_owned);
    let outcome = match (row["o"].get("kept"), row["o"].get("left")) {
        (Some(_), _) => Some(PrepareOutcome::Kept),
        (_, Some(left)) => Some(PrepareOutcome::Left(text(left).to_owned())),
        _ => None,
    };
    (PrepareStoppedError::new(id, outcome), row.clone())
}

fn assert_prepare_row(i: usize) {
    let (e, row) = prepare_row(i);
    assert_eq!(e.message(), text(&row["message"]));
    assert_eq!(e.builder_id, row["builderId"].as_str().map(str::to_owned));
    assert_eq!(e.kept, row["kept"].as_bool().unwrap());
    assert_eq!(e.left, row["left"].as_str().map(str::to_owned));
}

// internal.ts:950
#[test]
fn prepare_stopped_with_no_builder_says_it_stopped_before_a_machine_existed() {
    assert_prepare_row(0);
    let (e, _) = prepare_row(0);
    assert_eq!((e.builder_id, e.kept, e.left), (None, false, None));
}

// internal.ts:952
#[test]
fn prepare_stopped_with_no_outcome_says_the_builder_was_killed() {
    assert_prepare_row(1);
    let (e, _) = prepare_row(1);
    assert_eq!((e.builder_id, e.kept, e.left), (Some("b1".into()), false, None));
}

// internal.ts:954
#[test]
fn prepare_stopped_with_a_kept_builder_says_it_was_left_running_with_a_seal_in_it() {
    assert_prepare_row(2);
    let (e, _) = prepare_row(2);
    assert_eq!((e.builder_id, e.kept, e.left), (Some("b1".into()), true, None));
}

// internal.ts:956
#[test]
fn prepare_stopped_with_a_left_reason_says_the_builder_did_not_stop_and_why() {
    assert_prepare_row(3);
    let (e, _) = prepare_row(3);
    assert_eq!((e.builder_id, e.kept, e.left), (Some("b1".into()), false, Some("provider said no".into())));
}

// --- refusals ------------------------------------------------------------------------------------------------------

fn refusal_row(i: usize) -> (DaemonRefusal, (bool, bool, bool), String) {
    let f = fixture("internal");
    let row = &f["refusals"]["codes"][i];
    let refusal = DaemonRefusal { code: row["code"].as_str().map(str::to_owned), message: text(&row["message"]).to_owned() };
    let flags = (row["host"].as_bool().unwrap(), row["cred"].as_bool().unwrap(), row["def"].as_bool().unwrap());
    (refusal, flags, text(&row["message"]).to_owned())
}

fn assert_refusal_row(i: usize) {
    let (r, want, message) = refusal_row(i);
    assert_eq!((is_no_host_cli(&r), is_no_git_credential(&r), is_on_default_branch(&r)), want);
    assert_eq!(r.to_string(), message);
}

// internal.ts:1082
#[test]
fn refusal_code_no_host_cli_is_only_no_host_cli() {
    assert_refusal_row(0);
    assert_eq!(refusal_row(0).1, (true, false, false));
}

// internal.ts:1083
#[test]
fn refusal_code_no_git_credential_is_only_no_git_credential() {
    assert_refusal_row(1);
    assert_eq!(refusal_row(1).1, (false, true, false));
}

// internal.ts:1085
#[test]
fn refusal_code_on_default_branch_is_only_on_default_branch() {
    assert_refusal_row(2);
    assert_eq!(refusal_row(2).1, (false, false, true));
}

// internal.ts:1082-1085
#[test]
fn refusal_with_no_code_is_none_of_the_three() {
    assert_refusal_row(3);
    assert_eq!(refusal_row(3).1, (false, false, false));
}

// internal.ts:1082-1085
#[test]
fn refusal_with_another_code_is_none_of_the_three() {
    assert_refusal_row(4);
    assert_eq!(refusal_row(4).1, (false, false, false));
}

fn assert_default_branch_row(i: usize, road: DefaultBranchRoad, road_word: &str) {
    let f = fixture("internal");
    let row = &f["refusals"]["defaultBranch"][i];
    assert_eq!(text(&row["road"]), road_word);
    let e = default_branch_refused(road, "my-copy", "main");
    assert_eq!(e.message, text(&row["message"]));
    assert_eq!(e.fix.as_deref(), Some(text(&row["fix"])));
    assert_eq!(text(&row["kind"]), "usage");
    assert_eq!(e.kind, Some(Kind::Usage));
}

// internal.ts:1089-1090
#[test]
fn default_branch_refused_on_the_fork_road_says_to_fork_the_branch_again() {
    assert_default_branch_row(0, DefaultBranchRoad::Fork, "fork");
}

// internal.ts:1089-1090
#[test]
fn default_branch_refused_on_the_run_road_says_to_start_the_thread_again() {
    assert_default_branch_row(1, DefaultBranchRoad::Run, "run");
}

// internal.ts:1089-1090
#[test]
fn default_branch_refused_on_the_bring_back_road_says_to_bring_it_back_again() {
    assert_default_branch_row(2, DefaultBranchRoad::BringBack, "bring back");
}

fn provider_refusal(kind: ErrorKind) -> EngineError {
    EngineError::Provider(WspError { kind, status: 429, code: None, message: "cap".into(), request_id: None, window: None, said: None })
}

// internal.ts:879
#[test]
fn is_cap_refusal_is_true_for_an_answered_refusal_of_kind_concurrency() {
    let f = fixture("internal");
    assert_eq!(f["refusals"]["capRefusal"][0], json!(true));
    assert!(is_cap_refusal(&provider_refusal(ErrorKind::Concurrency)));
}

// internal.ts:879
#[test]
fn is_cap_refusal_is_false_for_an_answered_refusal_of_another_kind() {
    let f = fixture("internal");
    assert_eq!(f["refusals"]["capRefusal"][1], json!(false));
    assert!(!is_cap_refusal(&provider_refusal(ErrorKind::Plan)));
}

// internal.ts:879
#[test]
fn is_cap_refusal_is_false_for_a_plain_error_with_no_kind() {
    let f = fixture("internal");
    assert_eq!(f["refusals"]["capRefusal"][2], json!(false));
    assert!(!is_cap_refusal(&EngineError::Other { message: "x".into(), code: None }));
}

// --- reading -------------------------------------------------------------------------------------------------------

fn row(section: &str, i: usize) -> (Value, Value) {
    let f = fixture("internal");
    let r = &f["reading"][section][i];
    (r[0].clone(), r[1].clone())
}

fn json_or_null<T: Serialize>(v: Option<T>) -> Value {
    v.map_or(Value::Null, |v| serde_json::to_value(v).expect("writes"))
}

fn assert_steered_row(i: usize) {
    let (raw, want) = row("steered", i);
    let got = read_steered(&raw);
    assert_eq!(json_or_null(got.as_ref()), want, "read_steered({raw})");
    if let (Some(got), Some(want)) = (got, want.as_object()) {
        assert_eq!(got.keys().map(String::as_str).collect::<Vec<_>>(), want.keys().map(String::as_str).collect::<Vec<_>>());
        for (id, entry) in &got {
            assert_eq!(keys(&serde_json::to_value(entry).unwrap()), keys(&want[id]), "key order of {id}");
        }
    }
}

// internal.ts:619-632
#[test]
fn read_steered_of_null_is_none() {
    assert_steered_row(0);
}

// internal.ts:619-632
#[test]
fn read_steered_of_an_array_is_none() {
    assert_steered_row(1);
}

// internal.ts:619-632
#[test]
fn read_steered_of_a_string_is_none() {
    assert_steered_row(2);
}

// internal.ts:619-632
#[test]
fn read_steered_of_an_empty_object_is_an_empty_map_not_none() {
    assert_steered_row(3);
    assert_eq!(read_steered(&json!({})), Some(IndexMap::new()));
}

// internal.ts:619-632
#[test]
fn read_steered_keeps_an_entry_with_a_prompt_and_a_known_opener() {
    assert_steered_row(4);
}

// internal.ts:619-632
#[test]
fn read_steered_drops_an_entry_whose_opener_is_not_one_of_the_three() {
    assert_steered_row(5);
}

// internal.ts:619-632
#[test]
fn read_steered_drops_an_entry_with_no_prompt() {
    assert_steered_row(6);
}

// internal.ts:619-632
#[test]
fn read_steered_keeps_scope_road_and_request_id_and_leaves_out_an_empty_attachment_list() {
    assert_steered_row(7);
}

// internal.ts:619-632
#[test]
fn read_steered_drops_an_entry_whose_scope_does_not_read() {
    assert_steered_row(8);
}

// internal.ts:619-632
#[test]
fn read_steered_drops_an_entry_whose_road_is_not_a_road() {
    assert_steered_row(9);
}

// internal.ts:619-632
#[test]
fn read_steered_ignores_a_request_id_that_is_not_a_string_and_attachments_that_are_not_a_list() {
    assert_steered_row(10);
}

// internal.ts:619-632
#[test]
fn read_steered_skips_a_null_entry_and_keeps_the_next() {
    assert_steered_row(11);
}

// internal.ts:619-632
#[test]
fn read_steered_keeps_entries_in_the_order_the_row_wrote_them() {
    let one = json!({"prompt": "p", "startedBy": "person"});
    let raw = json!({"z": one.clone(), "a": one.clone(), "m": one});
    let got = read_steered(&raw).expect("a map");
    assert_eq!(got.keys().map(String::as_str).collect::<Vec<_>>(), vec!["z", "a", "m"]);
}

fn assert_asked_row(i: usize) {
    let (raw, want) = row("asked", i);
    assert_eq!(json_or_null(read_asked(&raw)), want, "read_asked({raw})");
}

// internal.ts:741-745
#[test]
fn read_asked_of_null_is_none() {
    assert_asked_row(0);
}

// internal.ts:741-745
#[test]
fn read_asked_of_a_bare_string_is_none() {
    assert_asked_row(1);
}

// internal.ts:741-745
#[test]
fn read_asked_with_a_prompt_that_is_not_a_string_is_none() {
    assert_asked_row(2);
}

// internal.ts:741-745
#[test]
fn read_asked_of_an_array_is_none() {
    assert_asked_row(6);
}

// internal.ts:741-745
#[test]
fn read_asked_with_only_a_prompt_keeps_only_the_prompt() {
    assert_asked_row(3);
    let (raw, _) = row("asked", 3);
    let got = read_asked(&raw).expect("reads");
    assert_eq!(got.prompt, "p");
}

// internal.ts:741-745
#[test]
fn read_asked_keeps_every_field_it_names() {
    assert_asked_row(4);
    let (raw, _) = row("asked", 4);
    let got = read_asked(&raw).expect("reads");
    assert_eq!(
        (got.effort.as_deref(), got.typed.as_deref(), got.request_id.as_deref(), got.owed, got.awaits_prompt),
        (Some("high"), Some("t"), Some("r"), Some(true), Some(true))
    );
}

// internal.ts:741-745
#[test]
fn read_asked_drops_fields_of_the_wrong_type_and_flags_that_are_not_true() {
    assert_asked_row(5);
    let (raw, _) = row("asked", 5);
    let got = read_asked(&raw).expect("reads");
    assert_eq!((got.effort, got.typed, got.owed, got.awaits_prompt), (None, None, None, None));
}

fn assert_scope_row(i: usize) {
    let (raw, want) = row("scope", i);
    if raw.is_null() {
        assert_eq!(json_or_null(read_scope(None)), want, "read_scope(absent)");
        assert_eq!(json_or_null(read_scope(Some(&Value::Null))), want, "read_scope(null)");
    } else {
        assert_eq!(json_or_null(read_scope(Some(&raw))), want, "read_scope({raw})");
    }
}

// internal.ts:749-753
#[test]
fn read_scope_of_an_absent_value_is_none() {
    let (raw, want) = row("scope", 0);
    assert_eq!((raw, want), (Value::Null, Value::Null));
    assert_eq!(read_scope(None), None);
}

// internal.ts:749-753
#[test]
fn read_scope_of_a_json_null_is_none() {
    let (raw, want) = row("scope", 1);
    assert_eq!((raw, want), (Value::Null, Value::Null));
    assert_eq!(read_scope(Some(&Value::Null)), None);
}

// internal.ts:749-753
#[test]
fn read_scope_reads_a_thread_scope_whole() {
    assert_scope_row(2);
    let (raw, _) = row("scope", 2);
    let got = read_scope(Some(&raw)).expect("reads");
    assert_eq!((got.thread_id.as_str(), got.workspace_id.as_str(), got.root_thread_id.as_str()), ("t1", "w1", "t0"));
}

// internal.ts:749-753
#[test]
fn read_scope_of_a_device_kind_is_none() {
    assert_scope_row(3);
}

// internal.ts:749-753
#[test]
fn read_scope_with_a_thread_id_that_is_not_a_string_is_none() {
    assert_scope_row(4);
}

fn assert_road_row(i: usize) {
    let (raw, want) = row("road", i);
    if raw.is_null() {
        assert_eq!(json_or_null(read_road(None)), want, "read_road(absent)");
        assert_eq!(json_or_null(read_road(Some(&Value::Null))), want, "read_road(null)");
    } else {
        assert_eq!(json_or_null(read_road(Some(&raw))), want, "read_road({raw})");
    }
}

// internal.ts:758-762
#[test]
fn read_road_of_an_absent_value_is_none() {
    assert_road_row(0);
    assert_eq!(read_road(None), None);
}

// internal.ts:758-762
#[test]
fn read_road_of_a_json_null_is_none() {
    assert_road_row(1);
    assert_eq!(read_road(Some(&Value::Null)), None);
}

// internal.ts:758-762
#[test]
fn read_road_reads_relayed() {
    assert_road_row(2);
    assert_eq!(json_or_null(read_road(Some(&json!("relayed")))), json!("relayed"));
}

// internal.ts:758-762
#[test]
fn read_road_reads_here() {
    assert_road_row(3);
    assert_eq!(json_or_null(read_road(Some(&json!("here")))), json!("here"));
}

// internal.ts:758-762
#[test]
fn read_road_reads_paired() {
    assert_road_row(4);
    assert_eq!(json_or_null(read_road(Some(&json!("paired")))), json!("paired"));
}

// internal.ts:758-762
#[test]
fn read_road_of_a_word_that_is_no_road_is_none() {
    assert_road_row(5);
    assert_eq!(read_road(Some(&json!("nowhere"))), None);
}

// --- launches ------------------------------------------------------------------------------------------------------

fn launch(v: &Value) -> KeptLaunch {
    KeptLaunch { fixed: text(&v["fixed"]).to_owned(), picks: wire::<IndexMap<String, Value>>(&v["picks"]) }
}

fn assert_launch_row(i: usize, want: bool) {
    let f = fixture("internal");
    let r = &f["launches"][i];
    assert_eq!(r[2], json!(want), "fixture row {i}");
    assert_eq!(launches_as(&launch(&r[0]), &launch(&r[1])), want);
}

// internal.ts:688-690
#[test]
fn launches_as_two_launches_with_no_picks_match() {
    assert_launch_row(0, true);
}

// internal.ts:688-690
#[test]
fn launches_as_a_send_that_names_fewer_picks_than_the_kept_launch_matches() {
    assert_launch_row(1, true);
}

// internal.ts:688-690
#[test]
fn launches_as_a_send_that_names_a_pick_the_kept_launch_lacks_does_not_match() {
    assert_launch_row(2, false);
}

// internal.ts:688-690
#[test]
fn launches_as_equal_picks_match() {
    assert_launch_row(3, true);
}

// internal.ts:688-690
#[test]
fn launches_as_a_different_fixed_part_does_not_match() {
    assert_launch_row(4, false);
}

// internal.ts:688-690
#[test]
fn launches_as_a_false_pick_equals_a_false_pick() {
    assert_launch_row(5, true);
}

// internal.ts:688-690
#[test]
fn launches_as_a_false_pick_does_not_equal_a_null_pick() {
    assert_launch_row(6, false);
}

// --- records -------------------------------------------------------------------------------------------------------

fn scope() -> Value {
    json!({"kind": "thread", "threadId": "t1", "workspaceId": "w1", "rootThreadId": "t0"})
}

// internal.ts:772-818
#[test]
fn record_thread_record_with_every_optional_key_writes_back_equal() {
    let doc = json!({
        "harness": "claude", "permissionMode": "plan", "parentThreadId": "t0", "rootThreadId": "t00",
        "readAt": 1700000000001u64, "settledAt": 1700000000002u64, "settleNamedAt": 1700000000003u64,
        "pinnedAt": 1700000000004u64, "order": 2.5, "foldedAt": 1700000000005u64, "replaces": "t-old",
        "snoozedUntil": 1700000000006u64, "section": {"name": "working", "whileState": "x"}, "cutOwed": "a1",
        "forkedFrom": {"threadId": "t9", "turnId": "u9", "title": "Old"},
        "limitResume": {"at": 1700000000007u64, "turnId": "u1"}, "rewound": {"before": "c1", "at": 1700000000008u64},
        "filesIn": ["/a", "/b"], "worked": false, "ended": "failed", "clouds": ["solari"],
    });
    round_trip::<ThreadRecord>(&doc);
}

// internal.ts:772-818
#[test]
fn record_thread_record_with_only_a_harness_writes_no_other_key() {
    let doc = json!({"harness": "claude"});
    let out = round_trip::<ThreadRecord>(&doc);
    assert_eq!(keys(&out), vec!["harness"]);
}

// internal.ts:772-818
#[test]
fn record_thread_record_whole_number_times_write_without_a_decimal_point_as_json_stringify_does() {
    let doc = json!({"harness": "claude", "readAt": 1700000000001u64, "settledAt": 1700000000002u64});
    let record: ThreadRecord = wire(&doc);
    assert_eq!(serde_json::to_value(&record).unwrap(), doc);
    assert_eq!(json_of(&record), r#"{"harness":"claude","readAt":1700000000001,"settledAt":1700000000002}"#);
}

// internal.ts:823-837
#[test]
fn record_session_index_record_with_every_optional_key_writes_back_equal() {
    let doc = json!({
        "workspaceId": "w1", "viewed": {"a.ts": "b1", "c.ts": "b2"},
        "sessions": [{
            "id": "s1", "workspaceId": "w1", "harness": "claude", "status": "running", "threadId": "t1", "turnId": "u1",
            "notify": ["t0"], "notifyBy": scope(), "notifyRoad": "relayed", "reply": "completed", "told": 2, "toldLast": true,
            "steered": {"r1": {"prompt": "p", "startedBy": "person"}}, "waited": 5, "waitingSince": 6, "run": "run1",
            "from": 10, "asked": {"prompt": "p"}, "turnToken": "tok", "scopeDeviceId": "d1", "snapshot": "abc",
        }],
        "threads": {"t1": {"harness": "claude", "permissionMode": "plan"}},
    });
    round_trip::<SessionIndexRecord>(&doc);
}

// internal.ts:823-837
#[test]
fn record_session_index_record_with_only_its_required_keys_writes_no_other_key() {
    let doc = json!({"workspaceId": "w1", "sessions": [{"id": "s1", "workspaceId": "w1", "harness": "claude", "status": "completed", "turnId": "u1"}]});
    let out = round_trip::<SessionIndexRecord>(&doc);
    assert_eq!(keys(&out), vec!["workspaceId", "sessions"]);
    assert_eq!(keys(&out["sessions"][0]), vec!["id", "workspaceId", "harness", "status", "turnId"]);
}

// transcripts.ts persistSessions: `{ workspaceId, sessions, threads, viewed? }`
#[test]
fn record_session_index_record_writes_its_keys_in_the_order_persist_sessions_does() {
    let doc = json!({
        "workspaceId": "w1",
        "sessions": [{"id": "s1", "workspaceId": "w1", "harness": "claude", "status": "completed", "turnId": "u1"}],
        "threads": {"t1": {"harness": "claude"}},
        "viewed": {"a.ts": "b1"},
    });
    let out = round_trip::<SessionIndexRecord>(&doc);
    assert_eq!(keys(&out), vec!["workspaceId", "sessions", "threads", "viewed"]);
}

// internal.ts:106-117
#[test]
fn record_held_start_record_with_every_optional_key_writes_back_equal() {
    let doc = json!({
        "workspaceId": "w1", "threadId": "t1", "turnId": "u1", "sessionId": "s1", "harness": "claude", "place": "p1",
        "prompt": "go", "notify": ["t0"], "notifyBy": scope(), "notifyRoad": "paired",
    });
    round_trip::<HeldStartRecord>(&doc);
}

// internal.ts:106-117
#[test]
fn record_held_start_record_with_only_its_required_keys_writes_no_other_key() {
    let doc = json!({"workspaceId": "w1", "threadId": "t1", "turnId": "u1", "sessionId": "s1", "harness": "claude", "place": "p1"});
    let out = round_trip::<HeldStartRecord>(&doc);
    assert_eq!(keys(&out), vec!["workspaceId", "threadId", "turnId", "sessionId", "harness", "place"]);
}

// internal.ts:122-133
#[test]
fn record_sub_start_record_with_every_optional_key_writes_back_equal() {
    let doc = json!({
        "workspaceId": "w2", "threadId": "t2", "turnId": "u2", "at": "2026-10-01T00:00:00.000Z",
        "opts": {
            "prompt": "go", "harness": "codex", "model": "gpt", "effort": "high", "permissionMode": "plan", "access": "auto-edit",
            "contextWindow": "1m", "fast": true, "startedBy": "agent", "requestId": "r1", "attempt": "a1", "title": "T", "cwd": "/w",
            "attachments": [{"mediaType": "image/png", "bytes": "AAAA", "name": "a.png"}],
        },
        "tree": {"parentThreadId": "t1", "rootThreadId": "t0"},
        "notify": ["t1"], "by": scope(), "road": "relayed",
    });
    round_trip::<SubStartRecord>(&doc);
}

// internal.ts:122-133
#[test]
fn record_sub_start_record_with_only_its_required_keys_writes_no_other_key() {
    let doc = json!({"workspaceId": "w2", "threadId": "t2", "turnId": "u2", "at": "2026-10-01T00:00:00.000Z", "opts": {"prompt": "go"}, "tree": {}});
    let out = round_trip::<SubStartRecord>(&doc);
    assert_eq!(keys(&out), vec!["workspaceId", "threadId", "turnId", "at", "opts", "tree"]);
    assert_eq!(keys(&out["opts"]), vec!["prompt"]);
    assert_eq!(keys(&out["tree"]), Vec::<&str>::new());
}

// internal.ts:848-861
#[test]
fn record_pending_create_with_every_optional_key_writes_back_equal() {
    let doc = json!({
        "key": "k1", "createdAt": "2026-10-01T00:00:00.000Z", "body": "abc", "host": "h1", "pid": 4242,
        "request": "{\"kind\":\"sandbox\"}", "image": "img", "place": "p1",
    });
    round_trip::<PendingCreate>(&doc);
}

// internal.ts:848-861
#[test]
fn record_pending_create_with_only_its_required_keys_writes_no_other_key() {
    let doc = json!({"key": "k1", "createdAt": "2026-10-01T00:00:00.000Z", "body": "abc", "host": "h1", "pid": 4242});
    let out = round_trip::<PendingCreate>(&doc);
    assert_eq!(keys(&out), vec!["key", "createdAt", "body", "host", "pid"]);
}

fn builder_full() -> Value {
    json!({
        "id": "b1", "name": "img", "kind": "sandbox", "baseTemplate": "tpl", "setupSha": "sha", "createdAt": "2026-10-01T00:00:00.000Z",
        "size": {"cpu": 2, "memMb": 4096}, "streamUrl": "https://s", "firstLife": true,
        "heldBy": {"host": "h1", "pid": 7, "heartbeat": "2026-10-01T00:01:00.000Z"}, "building": true, "import": {},
        "base": [{"name": "node", "version": "22"}], "parentSnapshotId": "snap1", "retired": [{"id": "g0", "name": "old"}],
        "sealed": {"at": "2026-10-01T00:02:00.000Z", "version": 3}, "place": "p1",
    })
}

// internal.ts:890-921
#[test]
fn record_builder_record_with_every_optional_key_writes_back_equal() {
    round_trip::<BuilderRecord>(&builder_full());
}

// internal.ts:890-921
#[test]
fn record_builder_record_with_only_its_required_keys_writes_no_other_key() {
    let doc = json!({
        "id": "b1", "name": "img", "kind": "desktop", "baseTemplate": "tpl", "setupSha": "sha", "createdAt": "2026-10-01T00:00:00.000Z",
        "size": {"cpu": 2, "memMb": 4096}, "firstLife": false,
    });
    let out = round_trip::<BuilderRecord>(&doc);
    assert_eq!(keys(&out), vec!["id", "name", "kind", "baseTemplate", "setupSha", "createdAt", "size", "firstLife"]);
}

// internal.ts:1248
#[test]
fn record_stored_builder_with_every_optional_key_writes_back_equal() {
    round_trip::<StoredBuilder>(&builder_full());
}

// internal.ts:1248
#[test]
fn record_stored_builder_from_before_size_and_first_life_were_kept_writes_neither() {
    let doc = json!({"id": "b1", "name": "img", "kind": "sandbox", "baseTemplate": "tpl", "setupSha": "sha", "createdAt": "2026-10-01T00:00:00.000Z"});
    let out = round_trip::<StoredBuilder>(&doc);
    assert_eq!(keys(&out), vec!["id", "name", "kind", "baseTemplate", "setupSha", "createdAt"]);
}

// internal.ts:236-251
#[test]
fn record_thread_words_with_an_open_message_writes_open_back_between_lines_and_last() {
    let doc = json!({"lines": ["a"], "open": "u1:m1", "last": 3, "size": 1});
    let out = round_trip::<ThreadWords>(&doc);
    assert_eq!(keys(&out), vec!["lines", "open", "last", "size"]);
    assert_eq!(out["open"], json!("u1:m1"));
}

// internal.ts:236-251
#[test]
fn record_thread_words_with_no_open_message_writes_no_open_key() {
    let doc = json!({"lines": [], "last": 0, "size": 0});
    let record: ThreadWords = wire(&doc);
    assert_eq!(record.open, None);
    let out = round_trip::<ThreadWords>(&doc);
    assert_eq!(keys(&out), vec!["lines", "last", "size"]);
}

// internal.ts:236-251
#[test]
fn record_thread_words_whole_number_last_writes_without_a_decimal_point_as_json_stringify_does() {
    let doc = json!({"lines": ["a"], "open": "u1:m1", "last": 3, "size": 1});
    let record: ThreadWords = wire(&doc);
    assert_eq!(json_of(&record), r#"{"lines":["a"],"open":"u1:m1","last":3,"size":1}"#);
}
