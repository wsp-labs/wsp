// SPDX-License-Identifier: AGPL-3.0-only
//! release.ts has no test file of its own at a3b870a6e (protocol.test.ts:2076 is ported in tests/protocol_words.rs);
//! every case below is pinned off the TS under node.

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::release::*;

    use serde_json::json;
    use wsp_wire::types::release::ReleaseView;

    fn rel(v: serde_json::Value) -> ReleaseView {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_release_is_ahead_only_of_a_version_that_runs() {
        assert_eq!(wsp_words::release::release_above(&rel(json!({"state":"checking"})), &["0.3.8"] as &[&str]), false); // releaseAbove({"state":"checking"}, ...["0.3.8"])
        assert_eq!(wsp_words::release::release_above(&rel(json!({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})), &["0.3.8"] as &[&str]), true); // releaseAbove({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}, ...["0.3.8"])
        assert_eq!(wsp_words::release::release_above(&rel(json!({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})), &["0.3.9"] as &[&str]), false); // releaseAbove({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}, ...["0.3.9"])
        assert_eq!(wsp_words::release::release_above(&rel(json!({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})), &["0.3.10","0.3.8"] as &[&str]), true); // releaseAbove({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}, ...["0.3.10","0.3.8"])
        assert_eq!(wsp_words::release::release_above(&rel(json!({"state":"unreached"})), &[] as &[&str]), false); // releaseAbove({"state":"unreached"}, ...[])
        assert_eq!(wsp_words::release::release_above(&rel(json!({"state":"read","latest":{"version":"0.4.0-rc.2","tag":"v0.4.0-rc.2","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})), &["0.4.0-rc.10"] as &[&str]), false); // releaseAbove({"state":"read","latest":{"version":"0.4.0-rc.2","tag":"v0.4.0-rc.2","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}, ...["0.4.0-rc.10"])
    }

    #[test]
    fn a_release_reads_its_version_else_its_state() {
        assert_eq!(wsp_words::release::release_word(&rel(json!({"state":"checking"}))), "checking"); // releaseWord({"state":"checking"})
        assert_eq!(wsp_words::release::release_word(&rel(json!({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}))), "0.3.9"); // releaseWord({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})
        assert_eq!(wsp_words::release::release_word(&rel(json!({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}))), "0.3.9"); // releaseWord({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})
        assert_eq!(wsp_words::release::release_word(&rel(json!({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}))), "0.3.9"); // releaseWord({"state":"read","latest":{"version":"0.3.9","tag":"v0.3.9","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})
        assert_eq!(wsp_words::release::release_word(&rel(json!({"state":"unreached"}))), "unreached"); // releaseWord({"state":"unreached"})
        assert_eq!(wsp_words::release::release_word(&rel(json!({"state":"read","latest":{"version":"0.4.0-rc.2","tag":"v0.4.0-rc.2","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}}))), "0.4.0-rc.2"); // releaseWord({"state":"read","latest":{"version":"0.4.0-rc.2","tag":"v0.4.0-rc.2","url":"https://x","publishedAt":"2026-10-20T00:00:00.000Z"}})
    }
}
