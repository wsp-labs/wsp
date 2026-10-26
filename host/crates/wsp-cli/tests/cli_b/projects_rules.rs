// SPDX-License-Identifier: AGPL-3.0-only
// spec/projects.md's rules on the words a projects line prints, held outside a whole line, and the order a remove
// asks in.

use serde_json::Value;
use wsp_cli::verbs::projects::{project_seed_text, project_set_lines};
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::workspace::ProjectSeed;

use crate::common::case;

fn reply_of(unit: &str, tool: &str, name: &str, op: &str) -> Value {
    let c = case(unit, tool, name);
    serde_json::from_str(&c.replies[op]).unwrap()
}

/// The seed's text is the line the recorded run printed: head, a blank line, the menu indented two spaces and a blank
/// line where it has rows, then the foot.
#[test]
fn seed_text_lays_out_head_menu_and_foot() {
    for name in
        ["the menu with nothing picked yet, nothing saved", "a path kept, one cut and the picks saved, one saved path gone from the folder"]
    {
        let c = case("projects", "projects_seed", name);
        let seed: ProjectSeed = serde_json::from_value(reply_of("projects", "projects_seed", name, "projects.seed")).unwrap();
        assert_eq!(project_seed_text(&seed), c.text.stdout.join("\n"), "{name}");
        for line in c.text.stdout[0].lines().skip(2).take_while(|l| !l.is_empty()) {
            assert!(line.starts_with("  "), "a menu row is indented: {line:?}");
        }
    }
}

#[test]
fn seed_text_with_no_rows_has_no_menu_and_one_blank_line() {
    let c = case("projects", "hand", "seed with an empty menu");
    let seed: ProjectSeed = serde_json::from_value(reply_of("projects", "hand", "seed with an empty menu", "projects.seed")).unwrap();
    let text = project_seed_text(&seed);
    assert_eq!(text, c.text.stdout.join("\n"));
    assert_eq!(text.matches("\n\n").count(), 1, "{text:?}");
}

#[test]
fn set_lines_head_then_defaults_then_the_command_and_trackers_only_when_answered() {
    let c = case("projects", "projects_set", "set");
    let set: Value = serde_json::from_str(&c.json.stdout[0]).unwrap();
    let defaults: ThreadDefaults = serde_json::from_value(set["defaults"].clone()).unwrap();
    let name = set["project"]["name"].as_str().unwrap();
    let lines = project_set_lines(name, &defaults, None, None);
    assert_eq!(lines.join("\n"), c.text.stdout.join("\n"));
    let more = project_set_lines(name, &defaults, Some("pnpm build"), Some(&["acme/docs".to_string()]));
    assert_eq!(more.len(), lines.len() + 2, "the command and the trackers each add one line");
    assert!(more[lines.len()].contains("pnpm build"));
    assert!(more[lines.len() + 1].contains("acme/docs"));
    let none = project_set_lines(name, &defaults, None, Some(&[]));
    assert_eq!(none.len(), lines.len() + 1, "an empty tracker list the host answered still says so");
}

/// The host's own refusals come first: a remove that would be refused is never asked about, and off a terminal the
/// question is refused after the check went out and before the remove did.
#[test]
fn remove_asks_only_after_the_check() {
    let c = case("projects", "hand", "remove off a terminal asks after the host's check");
    assert_eq!(c.text.frames.len(), 2);
    assert!(c.text.frames[1].contains("\"check\":true"));
    crate::common::holds(&c);
}
