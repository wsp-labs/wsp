// SPDX-License-Identifier: AGPL-3.0-only
// The title Codex keeps for a thread, the rename through its app server, the title and the commit message asked of
// `codex exec`, and what a machine holds for a thread whose resume Codex refused. Ports session-title.test.ts,
// session-held.test.ts and one case of command.test.ts. The db is built with the columns of the real thread index measured
// on codex-cli 0.153.0 (state_5.sqlite, table threads): title is NOT NULL and carries the thread's opening words, name
// is null until the person names the thread.
#![allow(dead_code)]

#[path = "common/codex_shell.rs"]
mod codex_shell;

use std::path::Path;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_adapters::codex::catalog::catalog_probe_command;
use wsp_adapters::codex::command::{build_env, CodexEnvOptions};
use wsp_adapters::codex::session_held::{parse_session_held, session_held_command};
use wsp_adapters::codex::session_title::{
    draft_for_command, parse_draft_for, parse_rename, parse_session_title, parse_title_for, rename_command, session_title_command,
    title_for_command,
};
use wsp_adapters::port::{AgentLaunch, SessionRenameWrite};

use codex_shell::{bash, fake_app_server, nul_env, run_shell, sqlite, write_stub, FakeAppServer, TempDir};

const THREAD: &str = "01a079b6-6f04-7f73-84d6-40e9e6885ffd";
const SCHEMA: &str =
    "create table threads (id text primary key, rollout_path text not null, cwd text not null, title text not null, name text);";
const NOW: i64 = 1_791_900_000_000;

fn host_env() -> Vec<(String, String)> {
    std::env::vars().collect()
}

fn tmpdir() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

/// A CODEX_HOME with one state db per version named, each seeded with the rows given.
fn codex_home(dbs: &[(&str, Vec<String>)]) -> TempDir {
    let home = TempDir::new("wsp-codex-title-");
    for (name, rows) in dbs {
        let mut sql = vec![SCHEMA.to_string()];
        sql.extend(rows.iter().cloned());
        sqlite(&home.join(name), &sql.join("\n"));
    }
    home
}

fn row(id: &str, title: &str, name: Option<&str>) -> String {
    let name = match name {
        None => "null".to_string(),
        Some(n) => format!("'{n}'"),
    };
    format!(
        "insert into threads (id, rollout_path, cwd, title, name) values ('{id}', 'sessions/r.jsonl', '/root/work', '{title}', {name});"
    )
}

/// The command as the guest runs it: one bash line, its stdout read the way the adapter reads it.
fn title_of(home: &TempDir, thread_id: &str) -> Option<String> {
    let command = session_title_command(&home.s(), thread_id).unwrap();
    parse_session_title(&bash(&command, &host_env()).stdout)
}

/// A folder holding a `codex` that answers whatever the test wants, first on PATH: what is under test is a shell line, so
/// the binary it runs has to be a real one.
fn fake_codex(script: &str) -> TempDir {
    let root = TempDir::new("wsp-codex-bin-");
    write_stub(&root.join("codex"), &format!("#!/bin/sh\n{script}\n"));
    root
}

/// A question as the exec runs it: with a fake codex first on PATH, its variables on its input, which then closes.
fn ask(command: &str, bin: &TempDir) -> String {
    let path = format!("{}:{}", bin.s(), std::env::var("PATH").unwrap_or_default());
    let env = vec![("PATH".to_string(), path), ("HOME".to_string(), tmpdir())];
    run_shell("bash", command, &env, "").stdout
}

mod the_title_codex_keeps_for_a_thread {
    use super::*;

    #[test]
    fn is_the_title_its_index_derived_from_the_threads_opening_words() {
        let home = codex_home(&[("state_5.sqlite", vec![row(THREAD, "say hi", None)])]);
        assert_eq!(title_of(&home, THREAD), Some("say hi".to_string()));
    }

    #[test]
    fn is_the_name_the_person_gave_the_thread_once_it_has_one() {
        let home = codex_home(&[("state_5.sqlite", vec![row(THREAD, "say hi", Some("the codex thread"))])]);
        assert_eq!(title_of(&home, THREAD), Some("the codex thread".to_string()));
    }

    #[test]
    fn comes_out_of_the_highest_schema_version_not_the_last_name_in_the_folder() {
        let home = codex_home(&[
            ("state_5.sqlite", vec![row(THREAD, "old schema", None)]),
            ("state_10.sqlite", vec![row(THREAD, "live schema", None)]),
        ]);
        assert_eq!(title_of(&home, THREAD), Some("live schema".to_string()));
    }

    #[test]
    fn is_nothing_when_the_index_has_no_such_thread_and_nothing_when_the_home_has_no_st() {
        let home = codex_home(&[("state_5.sqlite", vec![row("01a079b6-bba9-77d1-8eed-a6f11fd839b4", "another thread", None)])]);
        assert_eq!(title_of(&home, THREAD), None);
        assert_eq!(title_of(&codex_home(&[]), THREAD), None);
    }

    #[test]
    fn survives_a_name_holding_a_quote_and_a_newline_which_is_why_the_row_comes_back_as() {
        let insert = format!("insert into threads (id, rollout_path, cwd, title, name) values ('{THREAD}', 'r', '/root', 'x', 'it''s' || char(10) || 'here');");
        let home = codex_home(&[("state_5.sqlite", vec![insert])]);
        assert_eq!(title_of(&home, THREAD), Some("it's\nhere".to_string()));
    }

    #[test]
    fn refuses_a_thread_id_that_is_not_a_plain_slug_so_nothing_rides_into_the_query_unq() {
        let err = session_title_command("/root/.codex", "x' or '1'='1").unwrap_err();
        assert!(err.contains("plain slug"), "{err}");
        let command = session_title_command("/root/it's here", THREAD).unwrap();
        assert!(command.contains(r"cd '/root/it'\''s here'"), "{command}");
    }
}

mod the_title_codex_makes_for_a_thread {
    use super::*;

    #[test]
    fn runs_the_question_as_a_read_only_turn_with_the_prompt_on_stdin_and_reads_the_ans() {
        let seen_dir = TempDir::new("wsp-codex-seen-");
        let seen = seen_dir.join("seen");
        let bin = fake_codex(&format!(
            "{{ printf '%s\\n' \"$*\"; cat; }} > {}\nprintf '%s\\n' '{}' '{}'",
            seen.display(),
            r#"{"type":"item.completed","item":{"id":"i1","type":"reasoning","text":"thinking"}}"#,
            r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"Seed thread titles here"}}"#,
        ));
        let prompt = "Name it. It's a thread's own \"words\"; nothing else.";
        let command = title_for_command(prompt, Some("gpt-5.2"), None, None, NOW).unwrap();
        let stdout = ask(&command, &bin);
        assert_eq!(parse_title_for(&stdout), Some("Seed thread titles here".to_string()));
        let text = std::fs::read_to_string(&seen).unwrap();
        let mut parts = text.split('\n');
        let argv = parts.next().unwrap();
        let rest: Vec<&str> = parts.collect();
        assert!(argv.contains(r#"sandbox_mode="read-only""#), "{argv}");
        assert!(argv.contains(r#"approval_policy="never""#), "{argv}");
        assert!(argv.contains("-m gpt-5.2"), "{argv}");
        assert!(!argv.contains("--dangerously-bypass-approvals-and-sandbox"), "{argv}");
        assert_eq!(rest.join("\n").trim(), prompt);
    }

    #[test]
    fn reads_no_title_out_of_a_turn_that_failed_said_nothing_or_explained_itself_over_s() {
        assert_eq!(parse_title_for(r#"{"type":"turn.failed","error":{"message":"401 Unauthorized"}}"#), None);
        assert_eq!(parse_title_for(""), None);
        assert_eq!(
            parse_title_for(r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"Here it is:\nA title"}}"#),
            None
        );
        assert_eq!(
            parse_title_for(r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"Seed thread titles here"}}"#),
            Some("Seed thread titles here".to_string())
        );
    }
}

const NO_ROLLOUT_ERROR: &str = "no rollout found for thread id 01a079b6-6f04-7f73-84d6-40e9e6885ffd";

/// The renamer's one shell line run under bash against the fake app server's home and PATH, the question's variables on
/// its input as the exec carries them.
fn rename_through(f: &FakeAppServer, title: &str) -> SessionRenameWrite {
    let command = rename_command(THREAD, title, None, None).unwrap();
    let mut base = IndexMap::new();
    base.insert("PATH".to_string(), Some(f.path.clone()));
    let env = build_env(&CodexEnvOptions { base: Some(base), home: f.home.clone(), ..Default::default() }).unwrap();
    let shell_env = vec![("PATH".to_string(), "/usr/bin:/bin".to_string()), ("HOME".to_string(), tmpdir())];
    let ran = run_shell("bash", &command, &shell_env, &nul_env(env.iter()));
    parse_rename(&ran.stdout)
}

mod naming_a_codex_thread_from_wsp {
    use super::*;

    #[test]
    fn asks_codexs_own_app_server_to_name_the_thread_which_writes_the_index_row_itself() {
        let f = fake_app_server(json!({ "thread/name/set": { "result": {} } }));
        assert_eq!(rename_through(&f, "the name he typed in wsp"), SessionRenameWrite::Written);
        let asked: Vec<(Value, Value)> = f.requests().iter().map(|r| (r["method"].clone(), r["params"].clone())).collect();
        assert_eq!(
            asked,
            vec![
                (json!("initialize"), json!({ "clientInfo": { "name": "wsp", "version": "0" } })),
                (json!("initialized"), json!({})),
                (json!("thread/name/set"), json!({ "threadId": THREAD, "name": "the name he typed in wsp" })),
            ]
        );
    }

    #[test]
    fn carries_a_name_holding_quotes_and_a_newline_as_its_own_bytes() {
        let f = fake_app_server(json!({ "thread/name/set": { "result": {} } }));
        let title = "it's \"here\"\n'); drop table threads; --";
        assert_eq!(rename_through(&f, title), SessionRenameWrite::Written);
        let set = f.requests().into_iter().find(|r| r["method"] == json!("thread/name/set")).unwrap();
        assert_eq!(set["params"], json!({ "threadId": THREAD, "name": title }));
    }

    #[test]
    fn says_no_session_when_codex_has_no_rollout_for_the_thread_yet_which_is_a_thread_w() {
        let f = fake_app_server(json!({ "thread/name/set": { "error": { "code": -32600, "message": NO_ROLLOUT_ERROR } } }));
        assert_eq!(rename_through(&f, "the name"), SessionRenameWrite::NoSession);
    }

    #[test]
    fn says_failed_with_codexs_own_words_for_any_other_refusal_and_when_the_server_neve() {
        let locked = fake_app_server(json!({ "thread/name/set": { "error": { "code": -32603, "message": "database is locked" } } }));
        assert_eq!(rename_through(&locked, "the name"), SessionRenameWrite::Failed { error: "database is locked".to_string() });
        let gone = fake_app_server(json!({ "thread/name/set": "exit" }));
        assert_eq!(
            rename_through(&gone, "the name"),
            SessionRenameWrite::Failed { error: "codex's app server did not answer the rename".to_string() }
        );
    }

    #[test]
    fn refuses_a_thread_id_that_is_not_a_plain_slug_before_anything_runs() {
        let err = rename_command("x' or '1'='1", "x", None, None).unwrap_err();
        assert!(err.contains("plain slug"), "{err}");
    }
}

mod the_commit_message_codex_drafts {
    use super::*;

    #[test]
    fn runs_the_question_from_the_file_as_a_read_only_turn_on_stdin_and_answers_the_las() {
        let dir = TempDir::new("wsp-codex-draft-");
        let seen = dir.join("seen");
        let asked = dir.join("it's the question.txt");
        std::fs::write(&asked, "Write a commit message.\n\nThe diff:\n+one\n").unwrap();
        let bin = fake_codex(&format!(
            "{{ printf '%s\\n' \"$*\"; cat; }} > {}\nprintf '%s\\n' '{}'",
            seen.display(),
            r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"Round the total once\n\nIt rounded per line."}}"#,
        ));
        let command = draft_for_command(&asked.to_string_lossy(), Some("gpt-5.2"), None, None, NOW).unwrap();
        let stdout = ask(&command, &bin);
        assert_eq!(parse_draft_for(&stdout), Some("Round the total once\n\nIt rounded per line.".to_string()));
        let text = std::fs::read_to_string(&seen).unwrap();
        let mut parts = text.split('\n');
        let argv = parts.next().unwrap();
        let rest: Vec<&str> = parts.collect();
        assert!(argv.contains(r#"sandbox_mode="read-only""#), "{argv}");
        assert!(argv.contains("-m gpt-5.2"), "{argv}");
        assert_eq!(rest.join("\n"), "Write a commit message.\n\nThe diff:\n+one\n");
    }

    #[test]
    fn reads_no_message_out_of_a_failed_turn_or_out_of_nothing() {
        assert_eq!(parse_draft_for(r#"{"type":"turn.failed","error":{"message":"401 Unauthorized"}}"#), None);
        assert_eq!(parse_draft_for(r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"  "}}"#), None);
        assert_eq!(parse_draft_for(""), None);
    }
}

const HELD_THREAD: &str = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";
const OTHER: &str = "01a1424b-fa29-7aa0-8c11-0d9e8f7a6b5c";
const DAY: &str = "sessions/2026/10/16";
const OPENING: &str = "Remember the word lantern.";

#[derive(Clone, Copy, PartialEq)]
enum Schema {
    V0163,
    Older,
}

/// A CODEX_HOME holding the rollout files named, each one line long, and a state db of the schema given.
fn held_home(files: &[&str], schema: Option<Schema>, rows: &[(&str, &str)]) -> TempDir {
    let home = TempDir::new("wsp-codex-held-");
    for file in files {
        let path = home.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, opening_line()).unwrap();
    }
    if let Some(schema) = schema {
        let mode = if schema == Schema::V0163 { ", history_mode text not null default 'legacy'" } else { "" };
        let mut sql = vec![format!("create table threads (id text primary key, rollout_path text not null, title text not null{mode});")];
        for (id, path) in rows {
            let abs = home.join(path);
            sql.push(if schema == Schema::V0163 {
                format!(
                    "insert into threads (id, rollout_path, title, history_mode) values ('{id}', '{}', '{OPENING}', 'paginated');",
                    abs.display()
                )
            } else {
                format!("insert into threads (id, rollout_path, title) values ('{id}', '{}', '{OPENING}');", abs.display())
            });
        }
        sqlite(&home.join("state_5.sqlite"), &sql.join("\n"));
    }
    home
}

fn opening_line() -> String {
    format!("{}\n", serde_json::to_string(&json!({ "type": "user_message", "text": OPENING })).unwrap())
}

fn held_in(home: &Path, thread_id: &str) -> String {
    let command = session_held_command(&home.to_string_lossy(), thread_id).unwrap();
    parse_session_held(&bash(&command, &host_env()).stdout)
}

mod what_the_machine_holds_for_a_session_codex_would_not_resume {
    use super::*;

    #[test]
    fn names_the_rows_path_and_says_the_file_is_missing_beside_every_rollout_file_that() {
        let named = format!("{DAY}/rollout-2026-10-16T01-19-52-{HELD_THREAD}.jsonl");
        let reverted = format!("{DAY}/rollout-2026-10-16T01-20-10-{HELD_THREAD}_0d9e8f7a.jsonl");
        let other = format!("{DAY}/rollout-2026-10-16T01-20-36-{OTHER}.jsonl");
        let home = held_home(&[&reverted, &other], Some(Schema::V0163), &[(HELD_THREAD, &named)]);
        let line = held_in(home.path(), HELD_THREAD);
        assert!(Regex::new(r"^boot (unread|[0-9a-f-]{36}); ").unwrap().is_match(&line), "{line}");
        let bytes = opening_line().len();
        assert!(line.contains(&format!("; rollout files: {} ({bytes} bytes); ", home.join(&reverted).display())), "{line}");
        assert!(
            line.contains(&format!(
                "; index row: state_5.sqlite {{\"path\":\"{}\",\"mode\":\"paginated\"}}, the file it names is missing",
                home.join(&named).display()
            )),
            "{line}"
        );
        assert!(!line.contains(OTHER), "{line}");
        assert!(!line.contains(OPENING), "{line}");
    }

    #[test]
    fn says_the_file_is_there_when_the_row_names_one_that_is() {
        let named = format!("{DAY}/rollout-2026-10-16T01-19-52-{HELD_THREAD}.jsonl");
        let home = held_home(&[&named], Some(Schema::V0163), &[(HELD_THREAD, &named)]);
        let line = held_in(home.path(), HELD_THREAD);
        assert!(
            line.contains(&format!(
                "; index row: state_5.sqlite {{\"path\":\"{}\",\"mode\":\"paginated\"}}, the file it names is there",
                home.join(&named).display()
            )),
            "{line}"
        );
    }

    #[test]
    fn says_none_when_no_file_and_no_row_carry_the_thread_and_reads_an_index_from_befor() {
        let other = held_home(&[], Some(Schema::V0163), &[(OTHER, "x.jsonl")]);
        let none = Regex::new(r"; rollout files: none; index row: no row in state_5\.sqlite$").unwrap();
        let line = held_in(other.path(), HELD_THREAD);
        assert!(none.is_match(&line), "{line}");
        let named = format!("{DAY}/rollout-2026-10-16T01-19-52-{HELD_THREAD}.jsonl");
        let older = held_home(&[&named], Some(Schema::Older), &[(HELD_THREAD, &named)]);
        let line = held_in(older.path(), HELD_THREAD);
        assert!(
            line.contains(&format!(
                "; index row: state_5.sqlite {{\"path\":\"{}\"}}, the file it names is there",
                older.join(&named).display()
            )),
            "{line}"
        );
        let empty = held_home(&[], None, &[]);
        let line = held_in(empty.path(), HELD_THREAD);
        assert!(Regex::new(r"; index row: no state db$").unwrap().is_match(&line), "{line}");
        let line = held_in(&std::env::temp_dir().join("wsp-no-such-codex-home"), HELD_THREAD);
        assert!(Regex::new(r"; index row: no Codex home$").unwrap().is_match(&line), "{line}");
    }

    #[test]
    fn says_the_index_was_not_read_never_that_it_holds_no_row_when_sqlite3_cannot_open() {
        let home = held_home(&[], None, &[]);
        std::fs::write(home.join("state_5.sqlite"), "lantern ".repeat(100)).unwrap();
        let line = held_in(home.path(), HELD_THREAD);
        assert!(Regex::new(r"; index row: not read from state_5\.sqlite .*file is not a database").unwrap().is_match(&line), "{line}");
        assert!(!line.contains("lantern"), "{line}");
    }

    #[test]
    fn says_a_reading_stopped_at_its_deadline_was_cut_short_not_that_the_home_holds_not() {
        assert_eq!(
            parse_session_held("boot 1f0c\nfile 12 /root/.codex/sessions/a.jsonl\n"),
            "boot 1f0c; rollout files: /root/.codex/sessions/a.jsonl (12 bytes), the listing cut short; index row: not read"
        );
    }

    #[test]
    fn refuses_an_id_that_is_not_a_plain_slug_before_anything_runs() {
        let err = session_held_command("/root/.codex", "x'; rm -rf /").unwrap_err();
        assert!(err.contains("plain slug"), "{err}");
    }
}

mod a_persons_setup_for_codex_on_a_computer {
    use super::*;

    #[test]
    fn the_probe_a_title_and_a_draft_run_the_program_and_none_of_the_launch_words() {
        let launch = AgentLaunch {
            program: Some("/opt/codex".to_string()),
            args: Some(vec!["-c".to_string(), "model_provider=\"ollama\"".to_string()]),
        };
        let bare = Regex::new(r"(^|[;&] *)codex ").unwrap();
        for line in [
            catalog_probe_command(Some(&launch), None).unwrap(),
            title_for_command("name it", None, Some(&launch), None, NOW).unwrap(),
            draft_for_command("/tmp/ask", None, Some(&launch), None, NOW).unwrap(),
        ] {
            assert!(line.contains("'/opt/codex' "), "{line}");
            assert!(!bare.is_match(&line), "{line}");
            assert!(!line.contains("model_provider"), "{line}");
        }
    }
}

/// Rules ST1 to ST4 of spec/codex_session_title.md, "Rules added for the port", each value printed by the TS at 65f6b89c7.
mod rules_added {
    use super::*;

    #[test]
    fn the_first_object_line_decides_the_title_even_one_holding_neither_field() {
        // ST1; session-title.ts:39-55.
        assert_eq!(parse_session_title("{} \n{\"name\":\"X\"}"), None);
        assert_eq!(parse_session_title(r#"{"name":5,"title":"T"}"#).as_deref(), Some("T"));
        assert_eq!(parse_session_title("{\"name\":\"a\"} trailing\n{\"name\":\"  b  \"}").as_deref(), Some("b"));
    }

    #[test]
    fn the_last_string_message_wins_even_empty_and_a_title_is_one_line_with_its_dot_dropped() {
        // ST2; session-title.ts:89-120, words/turn.ts:373.
        let msg = |t: Value| json!({"type": "item.completed", "item": {"type": "agent_message", "text": t}}).to_string();
        let one_then_empty = format!("{}\n{}", msg(json!("one")), msg(json!("")));
        assert_eq!((parse_title_for(&one_then_empty), parse_draft_for(&one_then_empty)), (None, None));
        assert_eq!(parse_title_for(&msg(json!("Fix the bug."))).as_deref(), Some("Fix the bug"));
        assert_eq!(parse_title_for(&msg(json!("two\nlines"))), None);
        assert_eq!(parse_draft_for(&msg(json!("  keep  \n  spaces \n"))).as_deref(), Some("  keep  \n  spaces \n"));
        assert_eq!(parse_title_for(&format!("{}\n{}", msg(json!("one")), msg(json!(5)))).as_deref(), Some("one"));
    }

    #[test]
    fn a_rename_s_answer_must_be_an_object_and_only_a_lowercase_no_rollout_found_start_is_no_session() {
        // ST3; session-title.ts:155-160.
        assert_eq!(
            parse_rename(r#"{"id":2,"result":[]}"#),
            SessionRenameWrite::Failed { error: "codex's app server did not answer the rename".into() }
        );
        assert_eq!(
            parse_rename(r#"{"id":2,"error":{"message":"No rollout found"}}"#),
            SessionRenameWrite::Failed { error: "No rollout found".into() }
        );
        assert_eq!(parse_rename(r#"{"id":2,"error":{"message":""}}"#), SessionRenameWrite::Failed { error: String::new() });
        assert_eq!(parse_rename(r#"{"id":2,"error":{"message":"no rollout found for thread x"}}"#), SessionRenameWrite::NoSession);
    }

    #[test]
    fn the_held_words_split_a_file_line_on_single_spaces_and_a_state_db_answer_outranks_a_row() {
        // ST4; lost-session.ts:35-58.
        assert_eq!(
            parse_session_held("boot T\nlisted\nfile  12  /a\nnodb\nrow {\"path\":\"/a\"}\n"),
            "boot T; rollout files: 12  /a ( bytes); index row: no state db"
        );
        assert_eq!(parse_session_held(""), "boot unread; rollout files: none, the listing cut short; index row: not read");
        assert_eq!(parse_session_held("boot T\nnohome\n"), "boot T; rollout files: none; index row: no Codex home");
    }
}
