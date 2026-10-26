// SPDX-License-Identifier: AGPL-3.0-only
//! Tests for what the fix round decided where the review's tests stop: how deep a line is kept, the spelling of a lone
//! surrogate beside a real private-use char, a bad escape in a row id, and a machine id whose host carries a query.
//! Each expected value is what node 22 answers for the TS line named.
use wsp_engine::golden_seam::parse_mcp_id;
use wsp_engine::project_state::resolver::rewrite_cwd;
use wsp_engine::provision_files::agent_state_file;
use wsp_engine::ssh_backend::{ssh_machine_id, SshReach};

async fn rewritten(text: &str) -> (Result<u64, String>, String) {
    let dir = tempfile::Builder::new().prefix("fix-").tempdir().unwrap();
    let file = dir.path().join("t.jsonl");
    std::fs::write(&file, text).unwrap();
    let n = rewrite_cwd(file.to_str().unwrap(), "/work/p", "/work/q", None).await.map_err(|e| e.to_string());
    (n, std::fs::read_to_string(&file).unwrap())
}

/// resolver.ts:219-225: node parses a line nested 3,400 deep and writes it back (`JSON.stringify` throws from about 3,650).
#[tokio::test]
async fn a_line_nested_3400_deep_is_rewritten_whole_on_a_small_stack() {
    let deep = format!("{}{}", "[".repeat(3400), "]".repeat(3400));
    let line = format!("{{\"cwd\":\"/work/p\",\"x\":{deep}}}");
    let (n, text) = rewritten(&format!("{line}\n")).await;
    assert_eq!(n, Ok(1));
    assert_eq!(text, format!("{{\"cwd\":\"/work/q\",\"x\":{deep}}}\n"));
}

/// resolver.ts:219-225 at 5,000 levels: node's `JSON.stringify` throws `RangeError: Maximum call stack size exceeded` where the
/// line must change, and a line that need not change is never stringified.
#[tokio::test]
async fn a_line_nested_5000_deep_fails_only_where_it_must_change() {
    let deep = format!("{}{}", "{\"a\":".repeat(5000), "1".to_string() + &"}".repeat(5000));
    let (n, text) = rewritten(&format!("{{\"cwd\":\"/work/p\",\"x\":{deep}}}\n")).await;
    assert_eq!(n, Err("Maximum call stack size exceeded".to_string()));
    assert!(text.contains("/work/p"));
    let (n, _) = rewritten(&format!("{{\"cwd\":\"/other\",\"x\":{deep}}}\n{{\"cwd\":\"/work/p\"}}\n")).await;
    assert_eq!(n, Ok(1));
}

/// resolver.ts:225: `JSON.stringify` writes a lone surrogate as its escape and every real character raw, U+10F7FF and
/// U+10F800 to U+10FFFF included.
#[tokio::test]
async fn a_lone_surrogate_and_the_private_use_chars_beside_it_keep_their_own_spelling() {
    let line = "{\"cwd\":\"/work/p\",\"a\":\"\\ud83d\",\"b\":\"\u{10F7FF}\u{10F7FF}\",\"c\":\"\u{10F83D}\",\"d\":\"\\udfff\u{10FFFF}\",\"\\ude00\":1}";
    let (n, text) = rewritten(&format!("{line}\n")).await;
    assert_eq!(n, Ok(1));
    let want = line.replace("/work/p", "/work/q");
    assert_eq!(text, format!("{want}\n"));
}

/// golden-mcp.ts:112: a folder that does not decode throws `URI malformed` out of `agentStateFile` (provision-files.ts:75) too.
#[test]
fn a_bad_escape_in_a_row_id_is_the_same_error_wherever_the_id_is_read() {
    let e = agent_state_file("agents/mcp/claude/@%/x", "/root/.claude.json", "/root").unwrap_err();
    assert_eq!(e.to_string(), "URI malformed");
    assert!(parse_mcp_id("agents/mcp/claude/@%41/x").unwrap().is_some());
}

/// ssh-backend.ts:43-46, node 22: `new URL("ssh://root@a?key=1:22")` then `searchParams.set("key", "/k")` writes the whole query again,
/// the host's own pair replaced: `ssh://root@a?key=%2Fk`.
#[test]
fn a_host_that_names_a_key_pair_of_its_own_has_it_replaced_by_the_key() {
    let r = SshReach { user: "root".into(), host: "a?key=1".into(), port: 22, key_path: Some("/k".into()) };
    assert_eq!(ssh_machine_id(&r).unwrap(), "ssh://root@a?key=%2Fk");
}
