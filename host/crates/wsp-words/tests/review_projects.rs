// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the `projects` area of wsp-words against the TS host (packages/protocol/src at a3b870a6e).
//! Every test fails today for the reason in its name. Each expected value is what node printed for the TS function
//! (`/root/rvw2445/ts/bundle.mjs`), and each test says which TS function it ran.

use std::process::Command;

use serde_json::json;
use wsp_words::app_ports::ports_asked;
use wsp_words::exit::json_line;
use wsp_words::js::number_string;
use wsp_words::project_seed::seed_bytes;
use wsp_words::recipe_file::{recipe_canon, ResolvedRecipe};
use wsp_words::start::{github_link_of, review_from_reply, with_closes};

fn fenced(json_text: &str) -> String {
    format!("```json\n{json_text}\n```")
}

/// start.ts:207 `reviewFromReply`, start.rs:693. V8's JSON.parse is iterative and reads a million brackets; the Rust
/// reader recurses once per bracket (and serde_json::Value drops recursively), so a deep reply aborts the process.
/// node: reviewFromReply("```json\n" + "[".repeat(200000) + "]".repeat(200000) + "\n```")
///   => { ok: false, why: "the json block's shape is wrong: Expected object, received array" }
/// The call runs in a child process so the abort fails this test and not the whole run.
#[test]
fn review_from_reply_survives_two_hundred_thousand_nested_brackets() {
    const NAME: &str = "review_from_reply_survives_two_hundred_thousand_nested_brackets";
    if std::env::var("REVIEW_PROJECTS_CHILD").is_ok() {
        let depth = 200_000;
        let text = fenced(&format!("{}{}", "[".repeat(depth), "]".repeat(depth)));
        let said = match review_from_reply(&text) {
            Ok(_) => "ok".to_string(),
            Err(why) => why,
        };
        println!("RESULT:{said}");
        return;
    }
    let out = Command::new(std::env::current_exe().unwrap())
        .args([NAME, "--exact", "--nocapture", "--test-threads=1"])
        .env("REVIEW_PROJECTS_CHILD", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the call aborted the process: {:?}\n{}",
        out.status,
        stderr.lines().take(3).collect::<Vec<_>>().join("\n")
    );
    let result = stdout.lines().find_map(|line| line.split_once("RESULT:").map(|(_, said)| said)).unwrap_or("");
    assert_eq!(result, "the json block's shape is wrong: Expected object, received array");
}

/// start.ts:213 `JSON.parse` inside `reviewFromReply`, start.rs:417. V8 reads 1e999 as Infinity; the Rust reader hands
/// the text to serde_json, which refuses it, so a number any key holds turns a readable reply into a parse failure.
/// node: reviewFromReply(fence of {"verdict":"comment","summary":"s","comments":[{"path":"a","line":1e999,"side":"RIGHT","body":"b"}]})
///   => "the json block's comments.0.line is wrong: Expected integer, received float"
#[test]
fn review_from_reply_reads_a_number_past_the_double_range_as_infinity() {
    let text = fenced(r#"{"verdict":"comment","summary":"s","comments":[{"path":"a","line":1e999,"side":"RIGHT","body":"b"}]}"#);
    assert_eq!(review_from_reply(&text).unwrap_err(), "the json block's comments.0.line is wrong: Expected integer, received float");
}

/// start.ts:219 zod's message through `reviewFromReply`, start.rs:706 (wsp_wire check). A non-integer number is
/// "received number" to zod for every shape but integer; the Rust says "received float".
/// node: reviewFromReply(fence("1.5")) => "the json block's shape is wrong: Expected object, received number"
#[test]
fn review_from_reply_says_received_number_for_a_fractional_number() {
    assert_eq!(review_from_reply(&fenced("1.5")).unwrap_err(), "the json block's shape is wrong: Expected object, received number");
}

/// start.ts:215 V8's line and column in a JSON.parse message, start.rs:266. V8 ends a line at a lone "\r".
/// node: reviewFromReply(fence("\r{x")) => "the json block does not parse: Expected property name or '}' in JSON at position 2 (line 2 column 2)"
#[test]
fn review_from_reply_counts_a_lone_carriage_return_as_a_line_end() {
    assert_eq!(
        review_from_reply(&fenced("\r{x")).unwrap_err(),
        "the json block does not parse: Expected property name or '}' in JSON at position 2 (line 2 column 2)"
    );
}

/// start.ts:215 V8's message for a bad escape, start.rs:363. After a backslash V8 names a character above U+00FF an
/// unexpected token; only U+0000 to U+00FF read "Bad escaped character".
/// node: reviewFromReply(fence('"\\\u0100"')) => "the json block does not parse: Unexpected token 'Ā', \"\"\\Ā\"\" is not valid JSON"
#[test]
fn review_from_reply_reports_a_backslash_before_a_wide_character_as_an_unexpected_token() {
    assert_eq!(
        review_from_reply(&fenced("\"\\\u{100}\"")).unwrap_err(),
        "the json block does not parse: Unexpected token 'Ā', \"\"\\Ā\"\" is not valid JSON"
    );
}

/// app-ports.ts:158 `portsAsked`, app_ports.rs (js_number). `Number("0x10000000000000000")` is 18446744073709552000;
/// the Rust reads the digits as a u64 and answers NaN past 64 bits (hex, octal and binary alike).
/// node: portsAsked({ port: "0x10000000000000000" }).port => 18446744073709552000
#[test]
fn ports_asked_reads_a_hex_port_wider_than_64_bits() {
    let asked = ports_asked(Some("0x10000000000000000"), None);
    assert_eq!(number_string(asked.port), "18446744073709552000");
}

/// start.ts:146 `CLOSES`, start.rs:561. The TS puts the number into the regex raw, so 1.5 reads "1" any character "5";
/// the Rust escapes it, so "closes #1x5" is not seen and the Closes line is added.
/// node: withCloses("closes #1x5", 1.5) => "closes #1x5"
#[test]
fn with_closes_reads_the_number_as_regex_source_like_the_ts() {
    assert_eq!(with_closes("closes #1x5", 1.5), "closes #1x5");
}

/// start.ts:26 `githubLinkOf`, start.rs:50. A link whose number has 400 digits is `Number("99..9")` = Infinity in the
/// TS, which still builds the link; the Rust goes through a JSON value, where infinity is null, and answers None.
/// node: githubLinkOf("https://github.com/a/b/issues/" + "9".repeat(400)).url => "https://github.com/a/b/issues/99...9"
#[test]
fn github_link_of_keeps_a_link_whose_number_is_infinity() {
    let digits = "9".repeat(400);
    let link = github_link_of(&format!("https://github.com/a/b/issues/{digits}"));
    assert_eq!(link.map(|l| l.url), Some(format!("https://github.com/a/b/issues/{digits}")));
}

/// recipe-file.ts:147 `recipeCanon`, recipe_file.rs `recipe_canon`. The TS sorts the keys and rebuilds each object with
/// Object.fromEntries, and a JS object lists integer-like keys first in numeric order, so "9" comes before "10"; the
/// Rust keeps the sort by UTF-16 units and writes "10" first. The text is what the recipe hash is taken of.
/// node: recipeCanon({ file: { ...skills: { "10": {from:"a"}, "9": {from:"b"} } }, items: {} })
#[test]
fn recipe_canon_lists_integer_like_keys_first_in_numeric_order() {
    let file = serde_json::from_value(json!({
        "name": "r", "agents": {}, "mcp": {}, "clis": {},
        "skills": { "10": { "from": "a" }, "9": { "from": "b" } },
        "plugins": {}, "folders": {}, "configs": {}
    }))
    .unwrap();
    let canon = recipe_canon(&ResolvedRecipe { file, items: indexmap::IndexMap::new() });
    assert_eq!(
        canon,
        r#"{"file":{"agents":{},"clis":{},"configs":{},"folders":{},"mcp":{},"name":"r","plugins":{},"skills":{"9":{"from":"b"},"10":{"from":"a"}}},"items":{}}"#
    );
}

/// exit.ts:54 `jsonLine`, exit.rs:`json_line`. `JSON.stringify(value, null, 11)` indents by at most 10.
/// node: jsonLine({ a: 1 }, 11) => '{\n          "a": 1\n}' (ten spaces)
#[test]
fn json_line_clamps_the_indent_to_ten() {
    assert_eq!(json_line(&json!({ "a": 1 }), Some(11)), "{\n          \"a\": 1\n}");
}

/// project-seed.ts:161 `seedBytes`, project_seed.rs:420. The TS reduce starts at 0; Rust's f64 `sum()` starts at -0.0
/// (since 1.83), so a choice naming no file answers negative zero, which a serializer writes as "-0.0".
/// node: Object.is(seedBytes(plan, { files: [], memory: false, commits: false }), 0) => true
#[test]
fn seed_bytes_of_no_file_is_positive_zero() {
    let plan = serde_json::from_value(json!({
        "source": "/s", "remote": null, "branch": "main", "defaultBranch": null, "unpushed": null, "uncommitted": 0,
        "memory": null, "remembered": false,
        "files": [{ "path": "a", "dir": false, "bytes": 5, "kind": "config", "ticked": true }]
    }))
    .unwrap();
    let choice = serde_json::from_value(json!({ "files": [], "memory": false, "commits": false })).unwrap();
    assert_eq!(seed_bytes(&plan, &choice).to_bits(), 0u64);
}
