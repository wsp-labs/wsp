// SPDX-License-Identifier: AGPL-3.0-only
//! The exec put unit against the TS code's own answers (tests/fixtures/roads/exec_put.json and exec_paged.json, the
//! upload id pinned to abababababab). Spec: spec/exec_put.md.
mod roads_common;

use std::sync::Arc;

use base64::Engine;
use roads_common::*;
use serde_json::{json, Value};
use wsp_engine::exec_detached::*;
use wsp_engine::machine::Framing;

const UPLOAD: &str = "abababababab";

fn files_of(v: &Value) -> Vec<GuestWrite> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|f| GuestWrite {
            path: f["path"].as_str().unwrap().into(),
            text: f["text"].as_str().unwrap().into(),
            append: f["append"].as_bool().unwrap_or(false),
        })
        .collect()
}

fn lines(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(|l| l.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

fn framing_of(case: &Value) -> Option<Arc<dyn Framing>> {
    case["pad"].as_u64().map(|pad| {
        Arc::new(TestFraming { pad: "P".repeat(pad as usize), no_room: case["noRoom"].as_str().map(str::to_string) }) as Arc<dyn Framing>
    })
}

fn big() -> String {
    (0..200).map(|i| format!("prompt line {i} with a 'quote' and ünïcödé {}", "z".repeat(60))).collect::<Vec<_>>().join("\n")
}

#[test]
fn upload_sequences_match_the_ts_byte_for_byte() {
    for case in fixture("exec_put.json")["puts"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let framing = framing_of(case);
        let files = if framing.is_some() {
            vec![GuestWrite { path: "/p/f.in".into(), text: big(), append: false }]
        } else {
            files_of(&case["files"])
        };
        let got = upload_sequence(&files, &lines(&case["opts"]["before"]), &lines(&case["opts"]["after"]), UPLOAD, framing.as_deref());
        match case["res"].get("err") {
            Some(err) => assert_eq!(got.err().map(|e| e.to_string()).as_deref(), err.as_str(), "{name}"),
            None => {
                let want: Vec<String> = case["calls"].as_array().unwrap().iter().map(|c| c["cmd"].as_str().unwrap().to_string()).collect();
                assert_eq!(got.unwrap(), want, "{name}");
            }
        }
    }
}

#[tokio::test]
async fn put_files_sends_what_the_ts_sent() {
    for case in fixture("exec_put.json")["puts"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let answer = |cmd: &str, _o: &wsp_engine::machine::ExecOptions| {
            Ok(res(0, if cmd.contains("WSP_PIECE") { "WSP_PIECE\n" } else { "done\n" }, ""))
        };
        let machine = match framing_of(case) {
            Some(f) => ScriptedMachine::framed_by("m1", f, answer),
            None => ScriptedMachine::new("m1", answer),
        };
        let files = if case["pad"].is_u64() {
            vec![GuestWrite { path: "/p/f.in".into(), text: big(), append: false }]
        } else {
            files_of(&case["files"])
        };
        let opts = PutFilesOptions {
            before: lines(&case["opts"]["before"]),
            after: lines(&case["opts"]["after"]),
            timeout_ms: case["opts"]["timeoutMs"].as_u64(),
            stdin: case["opts"]["stdin"].as_array().map(|b| b.iter().map(|x| x.as_u64().unwrap() as u8).collect()),
        };
        let got = put_files(machine.as_ref(), &files, opts).await;
        let calls = machine.calls.lock().unwrap().clone();
        match case["res"].get("err") {
            Some(err) => {
                assert_eq!(got.err().map(|e| e.to_string()).as_deref(), err.as_str(), "{name}");
                assert!(calls.is_empty(), "{name}: nothing is sent before a refusal");
            }
            None => {
                let got = got.unwrap();
                let want = &case["res"]["ok"];
                assert_eq!(
                    json!({ "exitCode": got.exit_code, "stdout": got.stdout, "stderr": got.stderr, "pieces": got.pieces }),
                    *want,
                    "{name}"
                );
                // The upload id is random here; every key carries one id, the same in every exec of the upload.
                let id = calls[0].opts.idempotency_key.as_deref().unwrap().split('/').next().unwrap().to_string();
                assert!(id.len() == 12 && id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{name}: {id}");
                let wanted = case["calls"].as_array().unwrap();
                assert_eq!(calls.len(), wanted.len(), "{name}");
                for (call, want) in calls.iter().zip(wanted) {
                    assert_eq!(call.cmd, want["cmd"].as_str().unwrap().replace(UPLOAD, &id), "{name}");
                    assert_eq!(call.opts.timeout_ms, want["timeoutMs"].as_u64(), "{name}");
                    assert_eq!(
                        call.opts.idempotency_key.as_deref().map(|k| k.replace(&id, UPLOAD)),
                        want["idempotencyKey"].as_str().map(str::to_string),
                        "{name}"
                    );
                    let stdin = call.opts.stdin.as_ref().map(|b| base64::engine::general_purpose::STANDARD.encode(b));
                    assert_eq!(stdin.as_deref(), want["stdin"].as_str(), "{name}");
                }
            }
        }
    }
}

#[test]
fn a_framing_that_leaves_no_room_is_refused_in_its_own_words() {
    let file = [GuestWrite { path: "/p/f.in".into(), text: big(), append: false }];
    let tight = TestFraming { pad: "P".repeat(16_000), no_room: Some("no room for it".into()) };
    assert_eq!(upload_sequence(&file, &[], &[], UPLOAD, Some(&tight)).unwrap_err().to_string(), "no room for it");
    // Where every file is in pieces and the bare lines around them still do not fit, the framing is not what to
    // blame, whatever words it has.
    let roomy = TestFraming { pad: "P".repeat(10), no_room: Some("no room for it".into()) };
    let after = vec!["x".repeat(16_000)];
    assert_eq!(
        upload_sequence(&file, &[], &after, UPLOAD, Some(&roomy)).unwrap_err().to_string(),
        "the lines around the upload do not fit one exec body"
    );
    assert_eq!(
        upload_sequence(&file, &[], &after, UPLOAD, None).unwrap_err().to_string(),
        "the lines around the upload do not fit one exec body"
    );
}

#[test]
fn exec_fits_counts_utf8_bytes_and_the_envelope() {
    let d = fixture("exec_put.json");
    let longest = d["longestFit"].as_u64().unwrap() as usize;
    assert_eq!(longest, 16_384 - EXEC_ENVELOPE_BYTES);
    assert!(exec_fits(&"x".repeat(longest)));
    assert!(!exec_fits(&"x".repeat(longest + 1)));
    assert_eq!([exec_fits(&"é".repeat(7936)), exec_fits(&"é".repeat(7937))], [true, false]);
    assert!(exec_fits(""));
}

#[test]
fn machine_answer_matches_the_ts() {
    let want = lines(&fixture("exec_put.json")["machineAnswer"]);
    let got = [machine_answer(&res(0, "", "")), machine_answer(&res(1, " a \n", "\nb\n")), machine_answer(&res(3, "", "warn"))];
    assert_eq!(got.to_vec(), want);
}

#[test]
fn markers_of_reads_only_its_marker_lines() {
    let got = markers_of("M 0 yes\nM 1 two words\nX 2 no\nM\nM 3\n  M 4 padded  \nM 0 again", "M");
    let pairs: Vec<(String, String)> = got.into_iter().collect();
    // A later line with the same key replaces the value and keeps the key's first place.
    assert_eq!(
        pairs,
        [("0", "again"), ("1", "two words"), ("3", ""), ("4", "padded")].map(|(k, v)| (k.to_string(), v.to_string())).to_vec()
    );
    assert!(markers_of("", "M").is_empty());
    assert_eq!(append_marker("/p/x"), "/p/x.appended");
}

#[tokio::test]
async fn paged_reads_split_where_the_ts_split() {
    let d = fixture("exec_paged.json");
    let machine = ScriptedMachine::new("m1", |cmd, _o| Ok(res(0, cmd, "")));
    let rows: Vec<usize> = (0..19).collect();
    let line = |row: &usize, at: usize| {
        format!(
            "R {at} {row} {}",
            "w".repeat(if *row == 3 || *row == 4 {
                9000
            } else if *row == 10 {
                16_000
            } else {
                10
            })
        )
    };
    let pages = paged_reads(machine.as_ref(), &rows, line, "export PATH=/x").await.unwrap();
    let got: Vec<Vec<usize>> = pages.iter().map(|p| p.rows.clone()).collect();
    let want: Vec<Vec<usize>> = serde_json::from_value(d["pages"].clone()).unwrap();
    assert_eq!(got, want);
    let calls = machine.calls.lock().unwrap().clone();
    let wanted = d["calls"].as_array().unwrap();
    assert_eq!(calls.len(), wanted.len());
    for ((call, want), page) in calls.iter().zip(wanted).zip(&pages) {
        assert_eq!(call.cmd, want["cmd"].as_str().unwrap());
        assert_eq!(call.opts.timeout_ms, Some(INLINE_EXEC_MS));
        // Each page answers with the exec that carried it.
        assert_eq!(page.res.stdout, call.cmd);
    }
    // No rows, no exec.
    let quiet = ScriptedMachine::new("m1", |_c, _o| Ok(res(0, "", "")));
    assert!(paged_reads(quiet.as_ref(), &Vec::<usize>::new(), line, "h").await.unwrap().is_empty());
    assert!(quiet.cmds().is_empty());
}

#[tokio::test]
async fn a_paged_read_whose_exec_fails_fails_the_call() {
    let machine = ScriptedMachine::new("m1", |cmd, _o| if cmd.contains("R 0 8 ") { Err(plain("Bad Gateway")) } else { Ok(res(0, "", "")) });
    let rows: Vec<usize> = (0..12).collect();
    let e = paged_reads(machine.as_ref(), &rows, |row, at| format!("R {at} {row} "), "h").await.unwrap_err();
    assert_eq!(e.to_string(), "Bad Gateway");
    assert_eq!(machine.cmds().len(), 2);
}
