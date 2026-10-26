// SPDX-License-Identifier: AGPL-3.0-only
//! Ticket ruling 3: `parse_mcp_id` on a bad `%` escape returns an error with the TS's words, where the TS throws
//! (golden-mcp.ts:112 `decodeURIComponent`, node 22: `URIError: URI malformed`).
//!
//! This is written against the ruled signature, `parse_mcp_id(&str) -> Result<Option<McpId>, EngineError>`. The code
//! today returns `Option<McpId>`, so this file does not compile until the signature carries the error. It is its own
//! test target on purpose: run it as `cargo test --test review_eng_golden_ruling3`; no other target includes it.
//! TS answers, from node 22 on the cited line:
//!   parseMcpId "agents/mcp/claude/@%E0%A4%A/x" {"error":"URI malformed"}
//!   parseMcpId "agents/mcp/claude/@%/x" {"error":"URI malformed"}
//!   parseMcpId "agents/mcp/claude/@%FF/x" {"error":"URI malformed"}
//!   parseMcpId "agents/mcp/claude/@%41/x" {"ok":{"agent":"claude","home":false,"folder":"A","name":"x"}}

use wsp_engine::golden_seam::parse_mcp_id;

#[test]
fn a_folder_that_does_not_decode_is_an_error_with_the_words_uri_malformed() {
    for id in ["agents/mcp/claude/@%E0%A4%A/x", "agents/mcp/claude/@%/x", "agents/mcp/claude/@%FF/x"] {
        match parse_mcp_id(id) {
            Err(e) => assert_eq!(e.to_string(), "URI malformed", "{id}"),
            Ok(row) => panic!("{id}: the TS throws URI malformed, got {}", if row.is_some() { "a row" } else { "no row" }),
        }
    }
}

#[test]
fn a_folder_that_decodes_is_still_read() {
    let row = parse_mcp_id("agents/mcp/claude/@%41/x").unwrap().unwrap();
    assert_eq!((row.agent.as_str(), row.home, row.folder.as_deref(), row.name.as_str()), ("claude", false, Some("A"), "x"));
}
