// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/manifest.test.ts.
use serde_json::{json, Value};
use wsp_collect::manifest::{parse_entry, parse_manifest, safe_parse_manifest, Linux, Rung, LINUX, LOGIN_CHOICES, RUNGS};
use wsp_wire::types::init_job::{LoginChoice, LOGIN_CHOICES as PROTOCOL_LOGIN_CHOICES};

fn entry() -> Value {
    json!({
        "rung": "identity",
        "id": "identity/git-user",
        "label": "git name and email",
        "paths": ["~/.gitconfig"],
        "bytes": 512,
        "default": "bring",
        "required": true,
    })
}

/// The TS `{ ...base, key: value }`.
fn with(base: &Value, changes: &[(&str, Value)]) -> Value {
    let mut out = base.clone();
    for (key, value) in changes {
        out[*key] = value.clone();
    }
    out
}

fn round_trip(input: &Value) -> Value {
    serde_json::to_value(parse_entry(input).unwrap_or_else(|e| panic!("{input} did not parse: {e:?}"))).unwrap()
}

mod manifest_schema {
    use super::*;

    #[test]
    fn a_choice_sits_on_a_logins_row_or_a_consent_row_never_on_a_plain_one() {
        let token = json!({ "rung": "agents", "id": "agents/mcp/claude/github", "label": "github", "paths": [], "bytes": 0, "default": "bring", "consent": true, "choice": "copy" });
        let parsed = parse_manifest(&json!({ "entries": [token.clone()] })).unwrap();
        assert_eq!(parsed.entries[0].choice, Some(LoginChoice::Copy));
        // `consent: undefined` is a row with no consent key.
        let mut plain = token;
        plain.as_object_mut().unwrap().remove("consent");
        let err = parse_manifest(&json!({ "entries": [plain] })).unwrap_err();
        assert!(err.message.contains("entries.0.choice: only a logins row or a consent row carries a choice"), "{}", err.message);
    }

    #[test]
    fn lists_the_six_rungs_in_ladder_order_and_the_six_login_choices_which_the_protocol() {
        assert_eq!(RUNGS, [Rung::Identity, Rung::Shell, Rung::Toolchains, Rung::Tools, Rung::Agents, Rung::Logins]);
        // The TS rungs are the words themselves.
        assert_eq!(RUNGS.map(Rung::as_str), ["identity", "shell", "toolchains", "tools", "agents", "logins"]);
        assert_eq!(LOGIN_CHOICES, ["copy", "machine", "later", "key", "skip", "token"]);
        assert_eq!(LOGIN_CHOICES, PROTOCOL_LOGIN_CHOICES);
    }

    #[test]
    fn accepts_a_fresh_entry_and_every_recipe_field_wsp_init_writes_back() {
        let entry = entry();
        assert_eq!(round_trip(&entry), entry);
        let answered = json!({
            "rung": "logins",
            "id": "logins/gh",
            "label": "GitHub CLI login",
            "paths": ["~/.config/gh/hosts.yml"],
            "bytes": 200,
            "default": "bring",
            "group": "CLI logins",
            "bring": true,
            "choice": "copy",
        });
        assert_eq!(round_trip(&answered), answered);
    }

    #[test]
    fn a_tools_row_may_say_whether_it_runs_on_linux() {
        let formula = json!({ "rung": "tools", "id": "tools/brew/mas", "label": "mas", "group": "Homebrew", "paths": [], "bytes": 0, "default": "skip", "reason": "no Linux bottle", "linux": "no" });
        assert_eq!(round_trip(&formula), formula);
        assert_eq!(LINUX, [Linux::Yes, Linux::No, Linux::Unknown]);
        let pinned = json!({ "rung": "tools", "id": "tools/uv/ruff", "label": "ruff 0.6.3", "group": "uv tools", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "0.6.3" });
        assert_eq!(round_trip(&pinned), pinned);
    }

    #[test]
    fn rejects_each_row_that_breaks_a_rule() {
        let entry = entry();
        let rows = [
            ("an unknown rung", with(&entry, &[("rung", json!("fonts"))])),
            ("an empty id", with(&entry, &[("id", json!(""))])),
            ("an id that does not start with its rung", with(&entry, &[("id", json!("shell/git-user"))])),
            ("a default outside bring or skip", with(&entry, &[("default", json!("maybe"))])),
            ("a negative size", with(&entry, &[("bytes", json!(-1))])),
            ("a non-integer size", with(&entry, &[("bytes", json!(1.5))])),
            ("a choice outside copy, machine, skip", with(&entry, &[("choice", json!("later"))])),
            ("a choice on a row that is not a login", with(&entry, &[("choice", json!("copy"))])),
            ("a required row that defaults to skip", with(&entry, &[("required", json!(true)), ("default", json!("skip"))])),
            (
                "a linux marker outside yes, no, unknown",
                with(&entry, &[("rung", json!("tools")), ("id", json!("tools/brew/x")), ("linux", json!("maybe"))]),
            ),
            ("a linux marker on a row that is not a tool", with(&entry, &[("linux", json!("yes"))])),
            ("a version on a row that is not a tool", with(&entry, &[("version", json!("1.0.0"))])),
            ("a login shell on a row that is not a shell row", with(&entry, &[("login", json!("zsh"))])),
            ("a terminal font on a row that is not a shell row", with(&entry, &[("font", json!("Hack"))])),
            ("sourced files on a row that is not a shell row", with(&entry, &[("sources", json!(["~/.cargo/env"]))])),
        ];
        for (name, bad) in rows {
            assert!(parse_entry(&bad).is_err(), "rejects {name}");
        }
    }

    #[test]
    fn parses_the_envelope_and_reports_the_failing_row() {
        let entry = entry();
        let parsed = parse_manifest(&json!({ "entries": [entry.clone()] })).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), json!({ "entries": [entry.clone()] }));
        let err = parse_manifest(&json!({ "entries": [entry.clone(), with(&entry, &[("rung", json!(3))])] })).unwrap_err();
        assert!(err.message.contains("entries.1.rung"), "{}", err.message);
        let err = parse_manifest(&json!({ "entries": "nope" })).unwrap_err();
        assert!(err.message.contains("entries"), "{}", err.message);
        assert!(safe_parse_manifest(&json!(null)).is_err());
    }

    #[test]
    fn rejects_two_rows_with_the_same_id() {
        let entry = entry();
        let err = parse_manifest(&json!({ "entries": [entry.clone(), entry] })).unwrap_err();
        assert!(err.message.contains("duplicate id identity/git-user"), "{}", err.message);
    }
}
