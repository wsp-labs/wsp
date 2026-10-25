// SPDX-License-Identifier: AGPL-3.0-only
// spec/agents-skills.md and spec/agents-servers.md rules beyond the recorded lines: a secret typed at the terminal
// or read off the environment reaches the host in its frame and is printed nowhere, and the disable/enable
// templates make the same entries but for their word.

use wsp_cli::verbs::agents::agent_verbs;
use wsp_cli::verbs::client::{has_tool, Verb};

use crate::common::{case, holds, run_line};

/// The secrets each case carries, which must cross the socket and never reach stdout or stderr, as text or JSON.
const SECRET_CASES: &[(&str, &[&str])] = &[
    ("agents key at a terminal, the token typed", &["sk-ant-oat01-SECRET"]),
    ("agents key at a terminal, an API key", &["sk-proj-SECRET"]),
    ("agents setup at a terminal, two variables typed", &["foo-SECRET", "bar-SECRET"]),
    ("servers add with a variable read off the environment", &["lin-SECRET"]),
    ("servers add an address with a header", &["auth-SECRET"]),
];

#[test]
fn secrets_reach_the_host_and_are_never_printed() {
    for (name, secrets) in SECRET_CASES {
        let c = case("agents_servers", "hand", name);
        holds(&c);
        for json in [false, true] {
            let run = run_line(&c, json);
            for secret in *secrets {
                assert!(run.frames.iter().any(|f| f.contains(secret)), "{name}: {secret} reaches the host");
                for line in run.stdout.iter().chain(&run.stderr) {
                    assert!(!line.contains(secret), "{name}: {secret} printed in {line:?}");
                }
                for q in run.asked.iter().flatten() {
                    assert!(!q.contains(secret), "{name}: a question carries {secret}");
                }
            }
        }
    }
}

/// A variable's value is asked for where nothing echoes it, never taken from the line: off a terminal the setup
/// refuses before anything is sent.
#[test]
fn a_variable_value_is_never_taken_off_a_terminal() {
    let c = case("agents_servers", "hand", "setup a variable off a terminal");
    holds(&c);
    assert!(c.text.frames.is_empty());
}

fn entry(name: &str) -> Verb {
    agent_verbs().into_iter().find(|v| v.name() == name).unwrap_or_else(|| panic!("{name}"))
}

/// The disable/enable entries come from one template each (agents.ts:194-218, 573-603, 626-652): with the words
/// swapped they read the same, flags and tool inputs included.
#[test]
fn toggle_templates_make_the_same_entry_but_for_their_word() {
    for noun in ["skills", "servers", "plugins"] {
        let (off, on) = (entry(&format!("{noun} disable")), entry(&format!("{noun} enable")));
        let (off_line, on_line) = (off.line().unwrap(), on.line().unwrap());
        assert_eq!(off_line.usage().replace("disable", "enable"), on_line.usage(), "{noun}");
        assert_eq!(off_line.options(), on_line.options(), "{noun}: flags");
        assert_eq!(off_line.page(), on_line.page(), "{noun}: page");
        let names = |v: &Verb| has_tool(v).unwrap().input.iter().map(|i| i.name).collect::<Vec<_>>();
        assert_eq!(names(&off), names(&on), "{noun}: tool inputs");
    }
}
