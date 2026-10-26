// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of unit claude-launch (spec/claude_launch.md). Every expected value is what the TS functions at
//! a3b870a6e printed for the same input.

use indexmap::IndexMap;
use serde_json::json;
use wsp_adapters::claude::landmines::{
    build_command, build_env, forwards_subagent_text, person_settings, sign_in_guard, terminal_resume_command, user_message_line, BaseEnv,
    BrokerKind, BrokeredSignIn, BuildCommandOptions, ClaudeEnvOptions, PersonSettings,
};
use wsp_adapters::port::{AgentLaunch, TurnImage};

const U: &str = "e16ed170-8257-4668-879e-fe836341633c";
const PID_WORD: &str =
    r#"sh -c '[ -z "${WSP_AGENT_PID_FILE:-}" ] || echo $$ > "$WSP_AGENT_PID_FILE"; unset WSP_AGENT_PID_FILE; exec "$0" "$@"' claude"#;

fn base(pairs: &[(&str, Option<&str>)]) -> BaseEnv {
    pairs.iter().map(|(k, v)| (k.to_string(), v.map(String::from))).collect()
}

fn entries(env: &IndexMap<String, String>) -> Vec<(&str, &str)> {
    env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

const OVERRIDES: [(&str, &str); 5] = [
    ("CLAUDE_CODE_AUTO_CONNECT_IDE", "0"),
    ("CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", "1"),
    ("CLAUDE_CODE_ENABLE_TODO_TOOLS", "1"),
    ("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "0"),
    ("CLAUDE_CODE_MCP_STARTUP_WAIT_MS", "0"),
];

#[test]
fn env_keeps_the_ts_key_order() {
    let env = build_env(&ClaudeEnvOptions {
        base: Some(base(&[
            ("PATH", Some("/bin")),
            ("CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS", Some("5000")),
            ("CLAUDE_CODE_X", Some("1")),
            ("CLAUDE_CODE_PLUGIN_SEED_DIR", Some("/seed")),
            ("ANTHROPIC_API_KEY", Some("sk-in")),
            ("CLAUDE_CODE_OAUTH_TOKEN", Some("tok-in")),
            ("NOPE", None),
            ("HOME", Some("/root")),
        ])),
        project_dir_name: Some("-work".into()),
        ..Default::default()
    });
    let mut want = vec![
        ("CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS", "5000"),
        ("PATH", "/bin"),
        ("CLAUDE_CODE_PLUGIN_SEED_DIR", "/seed"),
        ("HOME", "/root"),
    ];
    want.extend(OVERRIDES);
    want.extend([("CLAUDE_CODE_OAUTH_TOKEN", "tok-in"), ("CLAUDE_CODE_PROJECT_DIR_NAME", "-work")]);
    assert_eq!(
        entries(&env),
        want,
        "a base wait keeps first place with its own value; the inherited token outranks the key; the project key comes last"
    );
}

#[test]
fn inherited_sign_in_is_a_fallback_the_vault_replaces() {
    let vaulted = build_env(&ClaudeEnvOptions {
        base: Some(base(&[("ANTHROPIC_API_KEY", Some("sk-in")), ("CLAUDE_CODE_OAUTH_TOKEN", Some(""))])),
        oauth_token: Some("vault-tok".into()),
        ..Default::default()
    });
    let mut want = vec![("CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS", "30000")];
    want.extend(OVERRIDES);
    want.push(("CLAUDE_CODE_OAUTH_TOKEN", "vault-tok"));
    assert_eq!(entries(&vaulted), want, "an empty inherited token is dropped as a mark and the vault's stands alone");
    let key_only = build_env(&ClaudeEnvOptions { base: Some(base(&[("ANTHROPIC_API_KEY", Some("sk-in"))])), ..Default::default() });
    let mut want = vec![("CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS", "30000")];
    want.extend(OVERRIDES);
    want.push(("ANTHROPIC_API_KEY", "sk-in"));
    assert_eq!(entries(&key_only), want, "the inherited key moves behind the overrides");
    let gateway = build_env(&ClaudeEnvOptions {
        base: Some(base(&[("ANTHROPIC_BASE_URL", Some("x")), ("PATH", Some("/bin"))])),
        brokered: Some(BrokeredSignIn { token: "T".into(), kind: BrokerKind::Gateway }),
        ..Default::default()
    });
    let mut want = vec![("PATH", "/bin")];
    want.extend(OVERRIDES);
    want.extend([
        ("ANTHROPIC_BASE_URL", "http://127.0.0.1:7071"),
        ("ANTHROPIC_AUTH_TOKEN", "T"),
        ("ANTHROPIC_API_KEY", ""),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
        ("CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "1"),
        ("DISABLE_UPDATES", "1"),
        ("API_TIMEOUT_MS", "3600000"),
    ]);
    assert_eq!(
        entries(&gateway),
        want,
        "a gateway carries no first-byte wait and no tool search, and the door's URL replaces the inherited one"
    );
}

#[test]
fn the_guard_asks_auth_status_with_the_inherited_values_out_of_the_way() {
    let guard = sign_in_guard(
        &ClaudeEnvOptions { base: Some(base(&[("ANTHROPIC_API_KEY", Some("sk"))])), ..Default::default() },
        Some(&AgentLaunch { program: Some("/opt/my claude".into()), args: None }),
    );
    assert_eq!(
        guard,
        r#"wsp_auth=$(env -u ANTHROPIC_API_KEY -u CLAUDE_CODE_OAUTH_TOKEN '/opt/my claude' auth status 2>/dev/null); case "$wsp_auth" in *'"authMethod": "api_key_helper"'*) ;; *'"loggedIn": true'*) unset ANTHROPIC_API_KEY CLAUDE_CODE_OAUTH_TOKEN ;; esac; unset wsp_auth; "#
    );
    assert_eq!(
        sign_in_guard(&ClaudeEnvOptions { base: Some(base(&[("ANTHROPIC_API_KEY", Some(""))])), ..Default::default() }, None),
        "",
        "an empty key is nothing inherited"
    );
    assert_eq!(
        sign_in_guard(
            &ClaudeEnvOptions { base: Some(base(&[("ANTHROPIC_API_KEY", Some("sk"))])), api_key: Some("v".into()), ..Default::default() },
            None
        ),
        ""
    );
}

#[test]
fn build_command_refuses_in_the_ts_order() {
    let refused = |o: BuildCommandOptions| build_command(&o).expect_err("refused").0;
    let fresh = || BuildCommandOptions { session_id: Some(U.into()), ..Default::default() };
    assert_eq!(refused(BuildCommandOptions::default()), "buildCommand needs exactly one of sessionId or resume");
    assert_eq!(refused(BuildCommandOptions { resume: Some(U.into()), ..fresh() }), "buildCommand needs exactly one of sessionId or resume");
    assert_eq!(
        refused(BuildCommandOptions { session_id: Some("nope".into()), ..Default::default() }),
        "session identifier must be a UUID, got \"nope\""
    );
    assert_eq!(
        refused(BuildCommandOptions { copy_of: Some("x".into()), ..fresh() }),
        "a copy opens a new session of a session named by its UUID, got \"x\""
    );
    assert_eq!(refused(BuildCommandOptions { aside: true, ..fresh() }), "a side question resumes the copy it is asked on");
    assert_eq!(refused(BuildCommandOptions { model: Some("a b".into()), ..fresh() }), "model must be a plain slug, got \"a b\"");
    assert_eq!(refused(BuildCommandOptions { context_window: Some("1m".into()), ..fresh() }), "contextWindow needs a model to ride on");
    assert_eq!(
        refused(BuildCommandOptions {
            model: Some("m".into()),
            effort: Some("bad effort".into()),
            context_window: Some("2m".into()),
            ..fresh()
        }),
        "contextWindow must be \"200k\" or \"1m\", got \"2m\"",
        "the window is read before the effort"
    );
    assert_eq!(
        refused(BuildCommandOptions { permission_mode: Some("no mode".into()), context_window: Some("2m".into()), ..fresh() }),
        "permissionMode must be a plain slug, got \"no mode\"",
        "the mode is read before the window"
    );
}

#[test]
fn settings_fold_into_one_flag_in_the_ts_key_order() {
    let line = build_command(&BuildCommandOptions {
        session_id: Some(U.into()),
        fast: Some(true),
        memory_dir: Some("/mem".into()),
        off_servers: Some(vec!["gh".into()]),
        launch: Some(AgentLaunch {
            program: None,
            args: Some(vec!["--settings".into(), r#"{"fastMode":false,"x":1}"#.into(), "--debug".into()]),
        }),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        line,
        format!(
            r#"{{ mkdir -p /etc/claude-code/managed-settings.d && printf '%s' '{{"deniedMcpServers":[{{"serverName":"gh"}}]}}' > /etc/claude-code/managed-settings.d/wsp-servers-off.json; }} 2>/dev/null; cd ~ && {PID_WORD} -p '--debug' --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions --permission-prompt-tool stdio --settings '{{"fastMode":false,"autoMemoryDirectory":"/mem","x":1,"deniedMcpServers":[{{"serverName":"gh"}}]}}' --session-id {U}"#
        ),
        "the person's fastMode overrides the turn's in its place"
    );
    let aside = build_command(&BuildCommandOptions {
        resume: Some(U.into()),
        aside: true,
        launch: Some(AgentLaunch { program: None, args: Some(vec!["--settings".into(), "/etc/s.json".into()]) }),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        aside,
        format!(
            r#"cd ~ && {PID_WORD} -p --input-format stream-json --output-format stream-json --verbose --include-partial-messages --permission-mode 'default' --permission-prompt-tool stdio --settings '{{"disableAllHooks":true}}' --max-turns 2 --resume {U}"#
        ),
        "a side question leaves the person's settings file out and keeps its hooks off"
    );
    let none_off = build_command(&BuildCommandOptions {
        session_id: Some(U.into()),
        off_servers: Some(vec![]),
        cwd: Some("/w s".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        none_off,
        format!(
            r#"rm -f /etc/claude-code/managed-settings.d/wsp-servers-off.json 2>/dev/null; cd '/w s' && {PID_WORD} -p --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions --permission-prompt-tool stdio --session-id {U}"#
        )
    );
}

#[test]
fn person_settings_reads_both_forms_and_gives_way_to_a_file() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        person_settings(&s(&[r#"--settings={"a":1}"#, "-x", "--settings", r#"{"b":2}"#])),
        PersonSettings { words: s(&["-x"]), settings: json!({"b": 2}).as_object().unwrap().clone(), file: false }
    );
    assert_eq!(
        person_settings(&s(&["--settings", "[1]", "-x"])),
        PersonSettings { words: s(&["--settings", "[1]", "-x"]), settings: Default::default(), file: true }
    );
    assert_eq!(
        person_settings(&s(&["--settings"])),
        PersonSettings { words: s(&["--settings"]), settings: Default::default(), file: true }
    );
    assert_eq!(person_settings(&[]), PersonSettings { words: vec![], settings: Default::default(), file: false });
}

#[test]
fn subagent_text_starts_at_2_1_270() {
    let cases = [
        ("2.1.269", false),
        ("2.1.270", true),
        ("2.2.0", true),
        ("3.0.0", true),
        ("2.1.270-beta", true),
        ("v2.1.300", false),
        ("10.0.0", true),
        ("2.0.999", false),
    ];
    for (version, takes) in cases {
        assert_eq!(forwards_subagent_text(Some(version)), takes, "{version}");
    }
    assert!(!forwards_subagent_text(None));
}

#[test]
fn a_user_line_is_byte_for_byte_the_ts_one() {
    let images = [
        TurnImage { media_type: "image/png".into(), bytes: "AAA".into(), path: None },
        TurnImage { media_type: "image/jpeg".into(), bytes: "BBB".into(), path: Some("/p".into()) },
    ];
    assert_eq!(
        user_message_line("héllo \"x\" </b>", U, &images, Some("u-1")),
        format!(
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"AAA"}}}},{{"type":"image","source":{{"type":"base64","media_type":"image/jpeg","data":"BBB"}}}},{{"type":"text","text":"héllo \"x\" </b>"}}]}},"parent_tool_use_id":null,"session_id":"{U}","uuid":"u-1"}}"#
        )
    );
    assert_eq!(
        terminal_resume_command(Some(&base(&[("CLAUDE_CONFIG_DIR", Some("/root/.claude cfg"))])), None),
        "CLAUDE_CONFIG_DIR='/root/.claude cfg' claude --resume"
    );
    assert_eq!(terminal_resume_command(Some(&base(&[("CLAUDE_CONFIG_DIR", None)])), None), "claude --resume");
}

const HEAD: &str = "--input-format stream-json --output-format stream-json --verbose";

// Rule L1: a side question drops every --settings pair where any of them names a file, the JSON ones too.
#[test]
fn a_side_question_drops_every_settings_pair_where_one_is_a_file() {
    let args = ["--settings", r#"{"a":1}"#, "--settings", "/f.json", "-x"].map(String::from).to_vec();
    let line = build_command(&BuildCommandOptions {
        resume: Some(U.into()),
        aside: true,
        launch: Some(AgentLaunch { program: None, args: Some(args) }),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        line,
        format!(
            r#"cd ~ && {PID_WORD} -p '-x' {HEAD} --include-partial-messages --permission-mode 'default' --permission-prompt-tool stdio --settings '{{"disableAllHooks":true}}' --max-turns 2 --resume {U}"#
        )
    );
}

// Rule L2: a side question always runs at the default mode, so its own permission mode is never read, a bad one included.
#[test]
fn a_side_question_never_reads_its_permission_mode() {
    let line = build_command(&BuildCommandOptions {
        resume: Some(U.into()),
        aside: true,
        permission_mode: Some("bad mode".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        line,
        format!(
            r#"cd ~ && {PID_WORD} -p {HEAD} --include-partial-messages --permission-mode 'default' --permission-prompt-tool stdio --settings '{{"disableAllHooks":true}}' --max-turns 2 --resume {U}"#
        )
    );
}

// Rule L3: the model slug is checked with the window's suffix on; an empty slug, name or folder are values like any.
#[test]
fn empty_values_are_given_values() {
    let refused = |o: BuildCommandOptions| build_command(&o).expect_err("refused").0;
    assert_eq!(
        refused(BuildCommandOptions {
            session_id: Some(U.into()),
            model: Some("bad model".into()),
            context_window: Some("1m".into()),
            ..Default::default()
        }),
        r#"model must be a plain slug, got "bad model[1m]""#
    );
    assert_eq!(
        refused(BuildCommandOptions { session_id: Some(U.into()), model: Some("".into()), ..Default::default() }),
        r#"model must be a plain slug, got """#
    );
    let line = build_command(&BuildCommandOptions {
        session_id: Some(U.into()),
        name: Some("".into()),
        cwd: Some("".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        line,
        format!("cd '' && {PID_WORD} -p {HEAD} --dangerously-skip-permissions --permission-prompt-tool stdio --name '' --session-id {U}")
    );
}

// Rule L4: a --settings value that is JSON but no object is a file to the TS: the words stay and nothing folds.
#[test]
fn settings_json_that_is_no_object_reads_as_a_file() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    for value in ["null", "\"abc\"", "5"] {
        assert_eq!(
            person_settings(&s(&["--settings", value])),
            PersonSettings { words: s(&["--settings", value]), settings: Default::default(), file: true },
            "{value}"
        );
    }
    assert_eq!(
        person_settings(&s(&["--settings", "--debug", "x"])),
        PersonSettings { words: s(&["--settings", "--debug", "x"]), settings: Default::default(), file: true }
    );
    assert_eq!(
        person_settings(&s(&["--settings", "{}", "-a"])),
        PersonSettings { words: s(&["-a"]), settings: Default::default(), file: false }
    );
}

// Rule L5: an empty key or project name is given: it is set, and the inherited token is not added behind it.
#[test]
fn an_empty_vault_key_is_still_the_key() {
    let fixed = [
        ("CLAUDE_CODE_AUTO_CONNECT_IDE", "0"),
        ("CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", "1"),
        ("CLAUDE_CODE_ENABLE_TODO_TOOLS", "1"),
        ("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "0"),
        ("CLAUDE_CODE_MCP_STARTUP_WAIT_MS", "0"),
    ];
    let keyed = build_env(&ClaudeEnvOptions {
        base: Some(base(&[("CLAUDE_CODE_OAUTH_TOKEN", Some("t")), ("HOME", Some("/root"))])),
        api_key: Some("".into()),
        ..Default::default()
    });
    let mut want = vec![("CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS", "30000"), ("HOME", "/root")];
    want.extend(fixed);
    want.push(("ANTHROPIC_API_KEY", ""));
    assert_eq!(entries(&keyed), want);
    let named = build_env(&ClaudeEnvOptions { project_dir_name: Some("".into()), ..Default::default() });
    let mut want = vec![("CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS", "30000")];
    want.extend(fixed);
    want.push(("CLAUDE_CODE_PROJECT_DIR_NAME", ""));
    assert_eq!(entries(&named), want);
}

// Rule L6: the version is read as the TS reads it, each part a JS number: leading zeros and a part past u64 read, a
// leading space or a short version do not.
#[test]
fn subagent_text_reads_each_part_as_a_js_number() {
    let cases = [
        ("2.1.99999999999999999999", true),
        ("02.1.270", true),
        ("2.1.0270", true),
        ("2.1.270.5", true),
        (" 2.1.270", false),
        ("2.1", false),
        ("2.1.9a", false),
        ("2.1.27", false),
    ];
    for (version, takes) in cases {
        assert_eq!(forwards_subagent_text(Some(version)), takes, "{version:?}");
    }
}

// Rule L7: settings fold as a JS object does: keys that read as array indexes go first, ascending, then the rest in the
// order they were set.
#[test]
fn settings_put_index_keys_first_as_a_js_object_does() {
    let args = ["--settings", r#"{"b":1,"2":"x","1":"y"}"#].map(String::from).to_vec();
    let line = build_command(&BuildCommandOptions {
        session_id: Some(U.into()),
        fast: Some(true),
        memory_dir: Some("/m".into()),
        launch: Some(AgentLaunch { program: None, args: Some(args) }),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        line,
        format!(
            r#"cd ~ && {PID_WORD} -p {HEAD} --dangerously-skip-permissions --permission-prompt-tool stdio --settings '{{"1":"y","2":"x","fastMode":true,"autoMemoryDirectory":"/m","b":1}}' --session-id {U}"#
        )
    );
}
