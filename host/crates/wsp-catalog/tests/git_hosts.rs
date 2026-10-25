// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/git-hosts.test.ts.
// The one reading of the host a remote names, which the row lookup and every sentence about a remote's host share.
use wsp_catalog::git_hosts::{git_host_of, remote_host, GitHostId};

mod the_host_a_remote_names {
    use super::*;

    #[test]
    fn reads_the_scp_form_an_ssh_url_and_an_https_url_alike_a_login_and_a_port_being_no() {
        for remote in ["git@github.com:o/r.git", "ssh://git@GitHub.com/o/r", "https://github.com/o/r", "https://user@github.com:443/o/r.git"] {
            assert_eq!(remote_host(remote).as_deref(), Some("github.com"), "{remote}");
        }
        assert_eq!(remote_host("git@gitlab.example.com:o/r.git").as_deref(), Some("gitlab.example.com"));
        assert_eq!(remote_host(""), None);
    }

    #[test]
    fn is_what_the_row_lookup_reads_so_a_host_with_a_row_and_one_without_are_told_apart() {
        assert_eq!(git_host_of("https://github.com/o/r").map(|h| h.id), Some(GitHostId::Github));
        assert_eq!(git_host_of("git@gitlab.example.com:o/r.git").map(|h| h.id), None);
    }
}
