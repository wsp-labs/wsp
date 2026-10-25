// SPDX-License-Identifier: AGPL-3.0-only
// The verb table's shared helpers (client.ts:736-1102) held to the answers the TS gave: fixture client.json, the keys
// releases, failed, toolFailures and misc. The keys asked, oneOf, tables and formatter belong to the rows and output
// half of client.ts (453-735) and are not read here.

mod common;

use serde_json::{json, Value};

use common::{fixture, strings, RecordingIo};
use wsp_cli::verbs::client::{
    absolute_folder, absolute_path, access_word_of, cell, common as common_flags, failed, flag, flag_for, flag_list, flag_set, host_platform,
    host_schema_refusal, opened_thread_line, opened_thread_said, other_version, pick, pick_options, printable, release_here_fix, release_refusal,
    release_words, send_options, setting_resets, tool_failure, under, FlagKind, FlagTable, FlagValue, Flags, OpenedFn, ReleaseHost,
};
use wsp_words::exit::{usage_refusal, HostError, Kind};

fn client() -> Value {
    fixture("client", false)
}

fn word_of_kind(kind: &Option<Kind>) -> Option<String> {
    kind.as_ref().map(|k| k.as_str().to_owned())
}

/// The argv the oracle's process had: a script under a global node_modules, so the npm road, which is what every
/// recorded "newer host" fix says. process.argv[1] of a vitest run is a file of node_modules/vitest.
fn oracle_argv() -> Vec<String> {
    vec!["node".to_owned(), "/root/wsp/node_modules/vitest/vitest.mjs".to_owned()]
}

fn host_of(v: &Value) -> ReleaseHost {
    match v.get("state").and_then(Value::as_str) {
        Some(state) => ReleaseHost::State(state.to_owned()),
        None => ReleaseHost::Where { r#where: v["where"].as_str().unwrap().to_owned(), here: v["here"].as_bool().unwrap() },
    }
}

/// The half of a refusal's message before its fix: what `refusalSaid` gives back.
fn said_of(message: &str, fix: &str) -> String {
    message.strip_suffix(&format!(" {fix}")).unwrap_or_else(|| panic!("{message:?} does not end in its fix {fix:?}")).to_owned()
}

// ---------------------------------------------------------------------------------------------------------------
// releases
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn release_refusal_names_both_releases_and_what_to_run_on_the_older_end() {
    let fx = client();
    let rows = fx["releases"].as_array().unwrap();
    assert_eq!(rows.len(), 8);
    for row in rows {
        let (mine, theirs) = (row["mine"].as_str().unwrap(), row["theirs"].as_str().unwrap());
        let host = host_of(&row["host"]);
        let road = row["road"].as_str();
        let at = format!("{mine} vs {theirs}, host {}, road {road:?}", row["host"]);
        let e = release_refusal(mine, theirs, &host, road, &oracle_argv());
        assert_eq!(e.message, row["message"].as_str().unwrap(), "release_refusal message for {at}");
        assert_eq!(word_of_kind(&e.kind), row["kind"].as_str().map(str::to_owned), "release_refusal kind for {at}");
        assert_eq!(e.fix.as_deref(), row["fix"].as_str(), "release_refusal fix for {at}");
    }
}

#[test]
fn release_refusal_of_a_newer_host_reads_this_processs_road_for_its_fix() {
    // The oracle's argv sits under node_modules (npm road). The other two roads' lines are read from daemon-fix.ts
    // (ROAD_LINES), not recorded: a checkout says a pull and a build, the app says its own update.
    let fx = client();
    let mut newer = 0;
    for row in fx["releases"].as_array().unwrap() {
        let recorded_fix = row["fix"].as_str().unwrap();
        if !recorded_fix.starts_with("npm i -g ") {
            continue;
        }
        newer += 1;
        let (mine, theirs) = (row["mine"].as_str().unwrap(), row["theirs"].as_str().unwrap());
        let host = host_of(&row["host"]);
        let said = said_of(row["message"].as_str().unwrap(), recorded_fix);
        let roads = [
            (vec!["node".to_owned(), "/usr/local/bin/wsp".to_owned()], format!("a pull of the checkout and a build on this computer brings this line to {theirs}.")),
            (vec!["node".to_owned(), "/Applications/wsp.app/Contents/Resources/wsp".to_owned()], format!("updating the wsp app on this computer brings this line to {theirs}.")),
            (vec!["node".to_owned(), "/usr/lib/node_modules/@wsp-labs/wsp/bin/wsp".to_owned()], format!("npm i -g @wsp-labs/wsp@{theirs} on this computer brings this line to {theirs}.")),
        ];
        for (argv, fix) in roads {
            let e = release_refusal(mine, theirs, &host, row["road"].as_str(), &argv);
            assert_eq!(e.fix.as_deref(), Some(fix.as_str()), "release_refusal fix for {mine} vs {theirs} on argv {argv:?}");
            assert_eq!(e.message, format!("{said} {fix}"), "release_refusal message for {mine} vs {theirs} on argv {argv:?}");
            assert_eq!(word_of_kind(&e.kind).as_deref(), Some("usage"), "release_refusal kind for {mine} vs {theirs} on argv {argv:?}");
        }
    }
    assert_eq!(newer, 2, "the fixture holds two rows where the host is the newer end");
}

#[test]
fn release_words_say_the_pieces_the_refusal_is_built_from() {
    let fx = client();
    for row in fx["releases"].as_array().unwrap() {
        let (mine, theirs) = (row["mine"].as_str().unwrap(), row["theirs"].as_str().unwrap());
        let message = row["message"].as_str().unwrap();
        let fix = row["fix"].as_str().unwrap();
        let host = match host_of(&row["host"]) {
            ReleaseHost::State(state) => release_words::host_here(&state),
            ReleaseHost::Where { r#where, .. } => release_words::host_at(&r#where),
        };
        assert_eq!(release_words::said(mine, &host, theirs), said_of(message, fix), "release_words::said for {mine} vs {theirs}, host {}", row["host"]);
    }
    assert_eq!(release_words::RESTART, "restart that host with wsp restart.");
    assert_eq!(release_words::reopen_app("0.3.8"), "quit and reopen the app that holds it once the app is on wsp 0.3.8.");
    assert_eq!(release_words::restart_up(), "Ctrl-C the terminal running wsp up and run it again.");
    assert_eq!(release_words::update_there("0.3.8"), "update wsp to 0.3.8 on that computer and restart its host there.");
    assert_eq!(release_words::update_here("npm i -g @wsp-labs/wsp@0.3.10", "0.3.10"), "npm i -g @wsp-labs/wsp@0.3.10 on this computer brings this line to 0.3.10.");
    assert_eq!(release_words::host_here("/s/state.json"), "the host serving /s/state.json");
    assert_eq!(release_words::host_at("mini"), "the host at mini");
}

#[test]
fn release_here_fix_picks_the_roads_line() {
    for row in client()["misc"]["releaseHereFix"].as_array().unwrap() {
        let road = row[0].as_str();
        assert_eq!(release_here_fix("0.3.8", road), row[1].as_str().unwrap(), "release_here_fix for road {road:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// failed
// ---------------------------------------------------------------------------------------------------------------

/// The Errors the oracle built, in its order; every one is two rows of `failed`, json off then on.
fn failures() -> Vec<HostError> {
    let mut slate = HostError::with_kind("slate", Kind::Usage);
    slate.errors = Some(vec![json!({ "code": "V1" })]);
    slate.warnings = Some(vec![]);
    slate.left = Some(vec![json!({ "id": "x" })]);
    vec![
        HostError::new("plain"),
        HostError::with_kind("wsp exec: names itself", Kind::Usage),
        HostError::with_kind("no thread nope", Kind::NotFound),
        HostError::with_kind("refused token", Kind::Auth),
        slate,
        // The oracle gave this one `errors: "x"`, a string where lists are arrays: no list reaches the failure.
        HostError::new("lists not arrays"),
    ]
}

#[test]
fn failed_prints_one_line_on_stderr_and_answers_the_classs_exit_code() {
    let fx = client();
    let rows = fx["failed"].as_array().unwrap();
    let errs = failures();
    assert_eq!(rows.len(), errs.len() * 2);
    for (at, row) in rows.iter().enumerate() {
        let e = &errs[at / 2];
        let json = row["json"].as_bool().unwrap();
        assert_eq!(json, at % 2 == 1, "failed row {at}: the oracle runs json off then on");
        assert_eq!(e.message, row["message"].as_str().unwrap(), "failed row {at}: the error this row stands for");
        let io = RecordingIo::new();
        let code = failed(&*io, json, e, "wsp exec: ");
        let label = format!("failed row {at} ({:?}, json {json})", e.message);
        assert_eq!(code as i64, row["code"].as_i64().unwrap(), "{label}: exit code");
        assert_eq!(io.errors(), strings(&row["errors"]), "{label}: stderr lines");
        assert_eq!(io.lines(), strings(&row["lines"]), "{label}: stdout lines");
        assert!(io.lines().is_empty(), "{label}: nothing on stdout");
    }
}

#[test]
fn failed_says_the_prefix_once_where_the_message_opens_with_the_commands_name() {
    // sayOnce (thread.ts:418): the message opens with the prefix's name followed by a space, so it is printed alone.
    let io = RecordingIo::new();
    let code = failed(&*io, false, &HostError::with_kind("wsp exec takes a thread", Kind::Usage), "wsp exec: ");
    assert_eq!(code, 3);
    assert_eq!(io.errors(), ["wsp exec takes a thread"], "a sentence that names itself is not given a second name");
    let io = RecordingIo::new();
    assert_eq!(failed(&*io, false, &HostError::new("boom"), ""), 1);
    assert_eq!(io.errors(), ["boom"], "no prefix: the message alone");
}

#[test]
fn tool_failure_is_the_text_and_the_object_marked_as_an_error() {
    let fx = client();
    let mut errs = failures();
    errs.push(HostError::new("the request did not match: [{\"path\":[\"thread\"],\"message\":\"Required\"}]"));
    let rows = fx["toolFailures"].as_array().unwrap();
    assert_eq!(rows.len(), errs.len() * 2);
    for (at, row) in rows.iter().enumerate() {
        let e = &errs[at / 2];
        let usage = if at % 2 == 0 { None } else { Some("wsp exec <thread> -- <command...>") };
        assert_eq!(row["usage"].as_str(), usage, "toolFailures row {at}: the oracle gives no usage then one");
        assert_eq!(e.message, row["message"].as_str().unwrap(), "toolFailures row {at}: the error this row stands for");
        let have = serde_json::to_string(&tool_failure(e, usage)).unwrap();
        let want = serde_json::to_string(&row["out"]).unwrap();
        assert_eq!(have, want, "toolFailures row {at} ({:?}, usage {usage:?})", e.message);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// misc
// ---------------------------------------------------------------------------------------------------------------

fn check_refusal<T: std::fmt::Debug + PartialEq>(got: Result<T, HostError>, out: &Value, label: &str, ok: impl Fn(&Value) -> T) {
    match (got, out.get("ok"), out.get("err")) {
        (Ok(v), Some(want), None) => assert_eq!(v, ok(want), "{label}: answer"),
        (Err(e), None, Some(err)) => {
            assert_eq!(e.message, err["error"].as_str().unwrap(), "{label}: message");
            assert_eq!(e.fix.as_deref(), err["fix"].as_str(), "{label}: fix");
            assert_eq!(word_of_kind(&e.kind), err["kind"].as_str().map(str::to_owned), "{label}: kind");
            assert_eq!(e.exit_code() as i64, err["exit"].as_i64().unwrap(), "{label}: exit code");
        }
        (got, ok, err) => panic!("{label}: answered {got:?}; the oracle's ok {ok:?}, err {err:?}"),
    }
}

#[test]
fn flag_for_names_the_flag_for_a_computer_where_one_is_named() {
    for row in client()["misc"]["flagFor"].as_array().unwrap() {
        let (name, on) = (row[0].as_str().unwrap(), row[1].as_str());
        assert_eq!(flag_for(name, on), row[2].as_str().unwrap(), "flag_for {name:?} on {on:?}");
    }
}

#[test]
fn absolute_path_refuses_a_path_that_does_not_open_with_a_slash() {
    for row in client()["misc"]["absolutePath"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        check_refusal(absolute_path("--cwd is a path on the machine", path), &row["out"], &format!("absolute_path {path:?}"), |v| v.as_str().unwrap().to_owned());
    }
}

#[test]
fn absolute_folder_passes_nothing_and_refuses_a_relative_folder() {
    for row in client()["misc"]["absoluteFolder"].as_array().unwrap() {
        let path = row["path"].as_str();
        check_refusal(absolute_folder(path), &row["out"], &format!("absolute_folder {path:?}"), |v| v.as_str().map(str::to_owned));
    }
}

#[test]
fn access_word_of_takes_one_of_the_four_words_and_refuses_the_rest() {
    for row in client()["misc"]["accessWordOf"].as_array().unwrap() {
        let word = row["word"].as_str().unwrap();
        let got = access_word_of(word).map(|choice| serde_json::to_value(choice).unwrap());
        check_refusal(got, &row["out"], &format!("access_word_of {word:?}"), |v| v.clone());
    }
}

#[test]
fn under_is_the_folder_or_a_path_inside_it() {
    for row in client()["misc"]["under"].as_array().unwrap() {
        let (path, folder) = (row[0].as_str().unwrap(), row[1].as_str().unwrap());
        assert_eq!(under(path, folder), row[2].as_bool().unwrap(), "under {path:?} {folder:?}");
    }
}

#[test]
fn other_version_names_the_op_the_host_answered_in_a_shape_this_wsp_does_not_read() {
    assert_eq!(other_version("sessions.start"), client()["misc"]["otherVersion"].as_str().unwrap());
}

#[test]
fn printable_drops_every_control_character_but_tab_and_newline() {
    for row in client()["misc"]["printable"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        assert_eq!(printable(input), row[1].as_str().unwrap(), "printable {input:?}");
    }
}

#[test]
fn cell_puts_a_name_on_one_line_without_control_characters() {
    for row in client()["misc"]["cell"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        assert_eq!(cell(input), row[1].as_str().unwrap(), "cell {input:?}");
    }
}

#[test]
fn pick_drops_only_the_absent_fields_and_keeps_their_order() {
    let have = pick(vec![("a", Some(json!(1))), ("b", None), ("c", Some(Value::Null)), ("d", Some(json!(""))), ("e", Some(json!(false)))]);
    let fx = client();
    let want = &fx["misc"]["pick"][0];
    assert_eq!(serde_json::to_string(&Value::Object(have)).unwrap(), serde_json::to_string(want).unwrap(), "pick of a, b absent, c null, d empty, e false");
}

#[test]
fn opened_thread_line_and_said_name_the_thread_and_its_prefix_word() {
    let fx = client();
    let want = fx["misc"]["opened"].as_array().unwrap();
    assert_eq!(want.len(), 4);
    let with_folder: OpenedFn<'_> = &|id: &str, folder: Option<&str>| format!("{id} in {}", folder.expect("a folder"));
    let dash: OpenedFn<'_> = &|id: &str, folder: Option<&str>| format!("{id} in {}", folder.unwrap_or("-"));
    assert_eq!(opened_thread_line("t-1", None, None), want[0].as_str().unwrap(), "opened_thread_line, no opening");
    assert_eq!(opened_thread_line("t-1", Some(with_folder), Some("/x")), want[1].as_str().unwrap(), "opened_thread_line, an opening and a folder");
    assert_eq!(opened_thread_said("t-1", None, None), want[2].as_str().unwrap(), "opened_thread_said, no opening");
    assert_eq!(opened_thread_said("t-1", Some(dash), None), want[3].as_str().unwrap(), "opened_thread_said, an opening and no folder");
}

#[test]
fn host_schema_refusal_leaves_a_sentence_alone() {
    for row in client()["misc"]["schemaRefusal"].as_array().unwrap() {
        let message = row["message"].as_str().unwrap();
        let e = HostError::new(message);
        assert_eq!(row["out"], json!({ "ok": null }), "the oracle answers nothing for {message:?}");
        assert!(host_schema_refusal(&e, "wsp exec <thread>").is_none(), "host_schema_refusal for {message:?}");
    }
}

#[test]
fn host_schema_refusal_turns_the_validators_issue_list_into_one_usage_line() {
    // The oracle's two cases both answer nothing (its issue list carries no `code`); these two are read from
    // validatorRefusal (thread.ts:79) and hostSchemaRefusal (client.ts:1099), not recorded.
    let usage = "wsp exec <thread>";
    let cases = [
        (r#"[{"code":"invalid_union_discriminator","path":["op"]}]"#, "the host does not serve this line; it runs another version of wsp, restart it with wsp restart"),
        (r#"[{"code":"invalid_type","path":["zzz-not-a-request-field"]}]"#, "the host would not read this line"),
    ];
    for (message, said) in cases {
        let want = usage_refusal(said, &format!("usage: {usage}"));
        let got = host_schema_refusal(&HostError::new(message), usage).unwrap_or_else(|| panic!("host_schema_refusal for {message}: nothing"));
        assert_eq!(got.message, want.message, "host_schema_refusal message for {message}");
        assert_eq!(got.fix, want.fix, "host_schema_refusal fix for {message}");
        assert_eq!(got.kind, want.kind, "host_schema_refusal kind for {message}");
        let failure = tool_failure(&HostError::new(message), Some(usage));
        assert_eq!(failure.content[0].text, want.message, "tool_failure of {message} reads the refusal");
        assert_eq!(failure.is_error, Some(true));
    }
}

#[test]
fn setting_resets_are_the_place_setting_words_in_order() {
    assert_eq!(setting_resets(), strings(&client()["misc"]["settingResets"]));
}

fn table_matches(table: &FlagTable, want: &Value, label: &str) {
    let rows = want.as_array().unwrap();
    let have: Vec<&str> = table.0.iter().map(|(n, _)| *n).collect();
    let names: Vec<&str> = rows.iter().map(|r| r[0].as_str().unwrap()).collect();
    assert_eq!(have, names, "{label}: names in order");
    for ((name, row), fx) in table.0.iter().zip(rows) {
        let o = &fx[1];
        let kind = match o["type"].as_str().unwrap() {
            "string" => FlagKind::String,
            "boolean" => FlagKind::Boolean,
            other => panic!("{label}: {name} type {other}"),
        };
        assert_eq!(row.kind, kind, "{label}: {name} kind");
        assert_eq!(row.multiple, o.get("multiple").and_then(Value::as_bool) == Some(true), "{label}: {name} multiple");
        assert_eq!(row.short.map(|c| c.to_string()).as_deref(), o.get("short").and_then(Value::as_str), "{label}: {name} short");
        assert_eq!(row.value_with, None, "{label}: {name} value_with");
    }
}

#[test]
fn common_is_host_state_json_and_help() {
    table_matches(&common_flags(), &client()["misc"]["common"], "COMMON");
}

#[test]
fn pick_options_and_send_options_are_string_flags() {
    let fx = client();
    table_matches(&pick_options(), &fx["misc"]["pickOptions"], "PICK_OPTIONS");
    table_matches(&send_options(), &fx["misc"]["sendOptions"], "SEND_OPTIONS");
}

#[test]
fn flag_reads_a_string_value_alone_and_flag_list_a_list_alone() {
    let fx = client();
    let misc = &fx["misc"];
    let mut with_all = Flags::new();
    with_all.insert("a".to_owned(), FlagValue::Str("x".to_owned()));
    with_all.insert("b".to_owned(), FlagValue::Bool(true));
    with_all.insert("c".to_owned(), FlagValue::List(vec!["y".to_owned()]));
    let empty = Flags::new();
    let flag_want = misc["flag"].as_array().unwrap();
    assert_eq!(flag(&with_all, "a"), flag_want[0].as_str(), "flag of a string");
    assert_eq!(flag(&with_all, "b"), flag_want[1].as_str(), "flag of a boolean");
    assert_eq!(flag(&empty, "a"), flag_want[2].as_str(), "flag of nothing");
    let mut two = Flags::new();
    two.insert("c".to_owned(), FlagValue::List(vec!["y".to_owned(), "z".to_owned()]));
    two.insert("a".to_owned(), FlagValue::Str("x".to_owned()));
    let list_want = misc["flagList"].as_array().unwrap();
    assert_eq!(flag_list(&two, "c"), strings(&list_want[0]), "flag_list of a list of two");
    assert_eq!(flag_list(&with_all, "a"), strings(&list_want[1]), "flag_list of a string");
    assert_eq!(flag_list(&empty, "c"), strings(&list_want[2]), "flag_list of nothing");
    // flags[name] === true, read from client.ts:972 and not recorded.
    assert!(flag_set(&with_all, "b"), "flag_set of a true boolean");
    assert!(!flag_set(&with_all, "a"), "flag_set of a string");
    assert!(!flag_set(&empty, "b"), "flag_set of nothing");
    let mut off = Flags::new();
    off.insert("b".to_owned(), FlagValue::Bool(false));
    assert!(!flag_set(&off, "b"), "flag_set of a false boolean");
}

#[test]
fn host_platform_is_darwin_on_a_mac_and_linux_elsewhere() {
    let want = if cfg!(target_os = "macos") { "darwin" } else { "linux" };
    assert_eq!(host_platform(), want);
    if !cfg!(target_os = "macos") {
        assert_eq!(client()["misc"]["hostPlatform"].as_str().unwrap(), host_platform(), "the oracle's own platform word");
    }
}
