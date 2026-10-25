// SPDX-License-Identifier: AGPL-3.0-only
//! project-look.ts has no TS test of its own at a3b870a6e; the one sentence is pinned off the TS under node.

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::project_look::*;

    #[test]
    fn an_icon_the_host_refused_says_why_and_what_to_do() {
        assert_eq!({ let r = wsp_words::project_look::project_icon_refusal("it is 9 MB"); format!("{} | {}", r.said, r.fix) }, "The host did not take this image: it is 9 MB. | Pick it again; if it is refused again, pick another file."); // projectIconRefusal("it is 9 MB").said + " | " + projectIconRefusal("it is 9 MB").fix
    }
}
