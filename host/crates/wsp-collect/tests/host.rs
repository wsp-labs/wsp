// SPDX-License-Identifier: AGPL-3.0-only
//! host.ts's two path helpers, and the fake every other test reads the computer through.
use wsp_collect::host::{expand, tilde};

#[test]
fn expand_turns_a_tilde_path_into_one_under_home_and_leaves_any_other_alone() {
    assert_eq!(expand("/Users/dev", "~"), "/Users/dev");
    assert_eq!(expand("/Users/dev", "~/.zshrc"), "/Users/dev/.zshrc");
    assert_eq!(expand("/Users/dev", "/etc/shells"), "/etc/shells");
    // `~user` and a bare `~x` are not this home's.
    assert_eq!(expand("/Users/dev", "~other/x"), "~other/x");
    assert_eq!(expand("/Users/dev", ""), "");
}

#[test]
fn tilde_turns_a_path_under_home_into_its_tilde_form_and_leaves_any_other_alone() {
    assert_eq!(tilde("/Users/dev", "/Users/dev"), "~");
    assert_eq!(tilde("/Users/dev", "/Users/dev/.ssh/config"), "~/.ssh/config");
    // A sibling that shares the prefix is not under home.
    assert_eq!(tilde("/Users/dev", "/Users/devops/x"), "/Users/devops/x");
    assert_eq!(tilde("/Users/dev", "/tmp/x"), "/tmp/x");
}
