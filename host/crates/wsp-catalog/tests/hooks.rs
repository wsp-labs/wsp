// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/hooks.test.ts.
use std::cell::RefCell;

use regex::Regex;
use serde_json::{json, Value};
use wsp_catalog::catalog::{catalog_agents, catalog_entry};
use wsp_catalog::hooks::{on_machine, Carried, CarriedHooks, HookCarry, CLAUDE_SETTINGS_FILE, CODEX_CONFIG_FILE};
use wsp_catalog::jsonc::parse_jsonc;

const HOME: &str = "/Users/dev";

fn place(abs: &str) -> Option<String> {
    let here = [
        (format!("{HOME}/.claude/hooks/remind"), "/root/.claude-cfg/hooks/remind"),
        (format!("{HOME}/.codync/notify.sh"), "/root/.codync/notify.sh"),
        (format!("{HOME}/bin/say it"), "/root/bin/say it"),
        (format!("{HOME}/x.sh"), "/root/x.sh"),
        (format!("{HOME}/.codex/notify.py"), "/root/.codex/notify.py"),
    ];
    here.iter().find(|(from, _)| from == abs).map(|(_, to)| to.to_string())
}

fn text(v: &Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(v).unwrap())
}

fn carried(from: &str, to: &str) -> Carried {
    Carried { from: from.to_string(), to: to.to_string() }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn claude(text: &str) -> CarriedHooks {
    HookCarry::Claude.carry(text, HOME, &place).unwrap()
}

fn codex(text: &str) -> CarriedHooks {
    HookCarry::Codex.carry(text, HOME, &place).unwrap()
}

fn untouched(text: &str) -> CarriedHooks {
    CarriedHooks { text: text.to_string(), carried: vec![], left: vec![] }
}

fn stop_commands(out_text: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(out_text).unwrap();
    v["hooks"]["Stop"][0]["hooks"].as_array().unwrap().iter().map(|h| h["command"].as_str().unwrap().to_string()).collect()
}

mod the_catalog_s_hook_carries {
    use super::*;

    #[test]
    fn are_registered_on_the_agent_entries_whose_settings_run_hooks_each_naming_a_file() {
        let agents = catalog_agents();
        let carriers: Vec<_> = agents.iter().filter(|a| a.hooks.is_some()).collect();
        let ids: Vec<&str> = carriers.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex"]);
        for a in &carriers {
            let file = a.hooks.unwrap().file().to_string();
            assert!(a.config_paths.contains(&file), "{}", a.id);
        }
        assert_eq!(HookCarry::Claude.file(), CLAUDE_SETTINGS_FILE);
        assert_eq!(HookCarry::Codex.file(), CODEX_CONFIG_FILE);
        assert!(catalog_entry("gemini").unwrap().as_agent().unwrap().hooks.is_none());
    }
}

mod claude_code_s_hooks {
    use super::*;

    #[test]
    fn rewrites_every_path_word_the_placer_knows_written_as_home_home_or_in_full() {
        let before = json!({
            "model": "opus",
            "hooks": {
                "SessionStart": [{"hooks": [
                    {"type": "command", "command": "~/.claude/hooks/remind"},
                    {"type": "command", "command": format!("{HOME}/.codync/notify.sh --quiet"), "timeout": 10},
                ]}],
                "Stop": [{"matcher": "", "hooks": [
                    {"type": "command", "command": "$HOME/.codync/notify.sh done"},
                    {"type": "command", "command": "bash \"${HOME}/bin/say it\" now"},
                ]}],
            },
        });
        let out = claude(&text(&before));
        let after: Value = serde_json::from_str(&out.text).unwrap();
        assert_eq!(
            after,
            json!({
                "model": "opus",
                "hooks": {
                    "SessionStart": [{"hooks": [
                        {"type": "command", "command": "/root/.claude-cfg/hooks/remind"},
                        {"type": "command", "command": "/root/.codync/notify.sh --quiet", "timeout": 10},
                    ]}],
                    "Stop": [{"matcher": "", "hooks": [
                        {"type": "command", "command": "/root/.codync/notify.sh done"},
                        {"type": "command", "command": "bash \"/root/bin/say it\" now"},
                    ]}],
                },
            })
        );
        assert_eq!(
            out.carried,
            vec![
                carried(&format!("{HOME}/.claude/hooks/remind"), "/root/.claude-cfg/hooks/remind"),
                carried(&format!("{HOME}/.codync/notify.sh"), "/root/.codync/notify.sh"),
                carried(&format!("{HOME}/bin/say it"), "/root/bin/say it"),
            ]
        );
        assert_eq!(out.left, Vec::<String>::new());
    }

    #[test]
    fn takes_out_a_hook_whose_script_the_placer_refuses_by_the_path_as_written() {
        let before = json!({
            "hooks": {
                "SessionStart": [
                    {"hooks": [
                        {"type": "command", "command": "~/.claude/hooks/remind"},
                        {"type": "command", "command": "/opt/homebrew/bin/terminal-notifier -title done"},
                    ]},
                    {"matcher": "resume", "hooks": [{"type": "command", "command": "~/.claude/hooks/gone"}]},
                ],
                "Stop": [{"hooks": [{"type": "command", "command": format!("{HOME}/.claude/hooks/gone --now")}]}],
            },
        });
        let out = claude(&text(&before));
        let after: Value = serde_json::from_str(&out.text).unwrap();
        assert_eq!(
            after,
            json!({"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "/root/.claude-cfg/hooks/remind"}]}]}})
        );
        assert_eq!(
            out.left,
            vec![
                "/opt/homebrew/bin/terminal-notifier".to_string(),
                "~/.claude/hooks/gone".to_string(),
                format!("{HOME}/.claude/hooks/gone")
            ]
        );
        assert_eq!(out.carried, vec![carried(&format!("{HOME}/.claude/hooks/remind"), "/root/.claude-cfg/hooks/remind")]);
        let only = claude(&text(
            &json!({"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "~/.claude/hooks/gone"}]}]}}),
        ));
        assert_eq!(only.text, "{\n  \"model\": \"opus\"\n}\n");
        assert_eq!(only.left, strings(&["~/.claude/hooks/gone"]));
    }

    #[test]
    fn a_path_the_machine_has_passes_through_unchanged_and_is_never_asked_of_the_placer() {
        let asked = RefCell::new(Vec::<String>::new());
        let placer = |abs: &str| {
            asked.borrow_mut().push(abs.to_string());
            place(abs)
        };
        let before = json!({
            "hooks": {
                "Stop": [
                    {"hooks": [
                        {"type": "command", "command": "/usr/bin/env jq -r .tool_input"},
                        {"type": "command", "command": "/bin/sh -c 'echo done'"},
                        {"type": "command", "command": "/usr/local/bin/notify done"},
                        {"type": "command", "command": "/usr/sbin/logger done"},
                        {"type": "command", "command": "~/.claude/hooks/remind"},
                        {"type": "command", "command": "/Users/dev/.codync/notify.sh"},
                        {"type": "command", "command": "/opt/homebrew/bin/terminal-notifier -title done"},
                        {"type": "command", "command": "/Applications/Say.app/Contents/MacOS/say done"},
                        {"type": "command", "command": "/Users/other/bin/ping"},
                    ]},
                ],
            },
        });
        let out = HookCarry::Claude.carry(&text(&before), HOME, &placer).unwrap();
        assert_eq!(
            stop_commands(&out.text),
            strings(&[
                "/usr/bin/env jq -r .tool_input",
                "/bin/sh -c 'echo done'",
                "/usr/local/bin/notify done",
                "/usr/sbin/logger done",
                "/root/.claude-cfg/hooks/remind",
                "/root/.codync/notify.sh",
            ])
        );
        assert_eq!(
            out.left,
            strings(&["/opt/homebrew/bin/terminal-notifier", "/Applications/Say.app/Contents/MacOS/say", "/Users/other/bin/ping"])
        );
        assert_eq!(
            *asked.borrow(),
            vec![
                format!("{HOME}/.claude/hooks/remind"),
                format!("{HOME}/.codync/notify.sh"),
                "/opt/homebrew/bin/terminal-notifier".to_string(),
                "/Applications/Say.app/Contents/MacOS/say".to_string(),
                "/Users/other/bin/ping".to_string(),
            ]
        );
        assert!(on_machine("/usr/bin/env"));
        assert!(!on_machine("/usr/bin"));
        assert!(!on_machine("/usr/binx/y"));
        assert!(!on_machine("/opt/homebrew/bin/jq"));
    }

    #[test]
    fn asks_the_placer_about_the_first_word_of_each_simple_command_only_redirect_target() {
        let asked = RefCell::new(Vec::<String>::new());
        let placer = |abs: &str| {
            asked.borrow_mut().push(abs.to_string());
            place(abs)
        };
        let before = json!({
            "hooks": {
                "Stop": [
                    {"hooks": [
                        {"type": "command", "command": "~/.claude/hooks/remind > /dev/null 2>&1"},
                        {"type": "command", "command": "~/.claude/hooks/remind >> /tmp/hooks.log"},
                        {"type": "command", "command": "/usr/bin/env bash ~/x.sh"},
                        {"type": "command", "command": "cd /opt/homebrew && ~/.codync/notify.sh; jq . /Users/dev/.claude/x.json | /usr/bin/tee /tmp/out || ~/.codync/notify.sh"},
                        {"type": "command", "command": "FOO=1 ~/.claude/hooks/remind --flag /Users/dev/.claude/hooks/gone"},
                        {"type": "command", "command": "node ~/.claude/hooks/gone"},
                    ]},
                ],
            },
        });
        let out = HookCarry::Claude.carry(&text(&before), HOME, &placer).unwrap();
        assert_eq!(
            stop_commands(&out.text),
            strings(&[
                "/root/.claude-cfg/hooks/remind > /dev/null 2>&1",
                "/root/.claude-cfg/hooks/remind >> /tmp/hooks.log",
                "/usr/bin/env bash /root/x.sh",
                "cd /opt/homebrew && /root/.codync/notify.sh; jq . /Users/dev/.claude/x.json | /usr/bin/tee /tmp/out || /root/.codync/notify.sh",
                "FOO=1 /root/.claude-cfg/hooks/remind --flag /Users/dev/.claude/hooks/gone",
            ])
        );
        assert_eq!(out.left, strings(&["~/.claude/hooks/gone"]));
        assert_eq!(
            out.carried,
            vec![
                carried(&format!("{HOME}/.claude/hooks/remind"), "/root/.claude-cfg/hooks/remind"),
                carried(&format!("{HOME}/x.sh"), "/root/x.sh"),
                carried(&format!("{HOME}/.codync/notify.sh"), "/root/.codync/notify.sh"),
            ]
        );
        assert_eq!(
            *asked.borrow(),
            vec![
                format!("{HOME}/.claude/hooks/remind"),
                format!("{HOME}/.claude/hooks/remind"),
                format!("{HOME}/x.sh"),
                format!("{HOME}/.codync/notify.sh"),
                format!("{HOME}/.codync/notify.sh"),
                format!("{HOME}/.claude/hooks/remind"),
                format!("{HOME}/.claude/hooks/gone"),
            ]
        );
    }

    #[test]
    fn carries_the_hooks_of_a_settings_file_with_comments_in_place_every_comment_outsid() {
        let before = [
            "{",
            "  // my settings",
            "  \"model\": \"opus\", // the big one",
            "  \"hooks\": {",
            "    /* on start */",
            "    \"SessionStart\": [{ \"hooks\": [",
            "      { \"type\": \"command\", \"command\": \"~/.claude/hooks/remind\" }, // reminds me",
            "      // gone soon",
            "      { \"type\": \"command\", \"command\": \"~/.claude/hooks/gone\" }",
            "    ] }],",
            "    \"Stop\": [{ \"hooks\": [{ \"type\": \"command\", \"command\": \"~/.claude/hooks/gone\" }] }] // stop",
            "  }",
            "}",
            "// the end",
            "",
        ]
        .join("\n");
        let out = claude(&before);
        for c in ["// my settings", "// the big one", "/* on start */", "// reminds me", "// the end"] {
            assert!(out.text.contains(c), "{c}");
        }
        assert_eq!(
            parse_jsonc(&out.text).unwrap(),
            json!({"model": "opus", "hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "/root/.claude-cfg/hooks/remind"}]}]}})
        );
        assert_eq!(out.left, strings(&["~/.claude/hooks/gone", "~/.claude/hooks/gone"]));
        assert_eq!(out.carried, vec![carried(&format!("{HOME}/.claude/hooks/remind"), "/root/.claude-cfg/hooks/remind")]);
    }

    #[test]
    fn carries_a_notify_in_a_config_toml_full_of_comments_and_leaves_every_comment_it_d() {
        let before = format!("# my codex\nmodel = \"gpt-5.5\" # the model\nnotify = [\"python3\", \"~/.codex/notify.py\"] # after a turn\n\n# trusted\n[projects.\"{HOME}\"]\ntrust_level = \"trusted\" # yes\n");
        let out = codex(&before);
        assert_eq!(out.text, before.replace("~/.codex/notify.py", "/root/.codex/notify.py"));
    }

    #[test]
    fn leaves_a_command_that_names_no_file_a_prompt_hook_a_file_that_is_not_json_or_has() {
        let plain = "{\"model\": \"opus\", \"hooks\": {\"PreToolUse\": [{\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\", \"command\": \"jq -r .tool_input.command\"}, {\"type\": \"prompt\", \"prompt\": \"check it\"}]}]}}";
        assert_eq!(claude(plain), untouched(plain));
        for t in ["{\"model\": \"opus\"}", "{not json", "{\"hooks\": \"none\"}", "[]"] {
            assert_eq!(claude(t), untouched(t), "{t}");
        }
    }
}

mod codex_s_notify {
    use super::*;

    fn mac_app() -> String {
        format!("{HOME}/.codex/computer-use/Codex Computer Use.app/Contents/MacOS/SkyComputerUseClient")
    }

    fn config(notify: &str) -> String {
        format!("model = \"gpt-5.5\"\n{notify}\n\n[projects.\"{HOME}\"]\ntrust_level = \"trusted\"\n\n[tui]\npet = \"fireball\"\n")
    }

    const WITHOUT_NOTIFY: &str =
        "model = \"gpt-5.5\"\n\n[projects.\"/Users/dev\"]\ntrust_level = \"trusted\"\n\n[tui]\npet = \"fireball\"\n";

    #[test]
    fn takes_out_a_notify_whose_program_is_neither_on_the_image_nor_a_bare_command_name() {
        let app = mac_app();
        let out = codex(&config(&format!("notify = [\"{app}\", \"turn-ended\"]")));
        assert_eq!(out.text, WITHOUT_NOTIFY);
        assert_eq!(out.left, vec![app]);
        assert_eq!(out.carried, Vec::<Carried>::new());
    }

    #[test]
    fn takes_out_a_notify_whose_array_closes_on_a_later_line_with_its_comment() {
        let app = mac_app();
        let out = codex(&format!("notify = [\n  \"{app}\", # the Mac app\n  \"turn-ended\",\n]\nmodel = \"gpt-5.5\"\n"));
        assert_eq!(out.text, "model = \"gpt-5.5\"\n");
        assert_eq!(out.left, vec![app]);
    }

    #[test]
    fn leaves_a_notify_the_machine_can_run_a_bare_command_name_and_a_notify_key_under_a() {
        for notify in
            ["notify = [\"/usr/bin/notify-send\", \"turn-ended\"]", "notify = [\"notify-send\"]", "notify = []", "notify = \"/bin/echo\""]
        {
            let text = config(notify);
            assert_eq!(codex(&text), untouched(&text), "{notify}");
        }
        let under = format!("model = \"x\"\n\n[tui]\nnotify = [\"{}\"]\n", mac_app());
        assert_eq!(codex(&under), untouched(&under));
    }

    #[test]
    fn puts_the_script_an_interpreter_runs_through_the_placer_so_it_travels_to_its_gues() {
        let asked = RefCell::new(Vec::<String>::new());
        let placer = |abs: &str| {
            asked.borrow_mut().push(abs.to_string());
            place(abs)
        };
        let out = HookCarry::Codex.carry(&config(&format!("notify = [\"python3\", \"{HOME}/.codex/notify.py\"]")), HOME, &placer).unwrap();
        assert_eq!(out.text, config("notify = [\"python3\", \"/root/.codex/notify.py\"]"));
        assert_eq!(out.carried, vec![carried(&format!("{HOME}/.codex/notify.py"), "/root/.codex/notify.py")]);
        assert_eq!(out.left, Vec::<String>::new());
        assert_eq!(*asked.borrow(), vec![format!("{HOME}/.codex/notify.py")]);
        // The same shape written with ~, with a flag before the script, and with the interpreter by a path the image has.
        let script = Regex::new(r#""[^"]*notify\.py""#).unwrap();
        for notify in [
            "notify = [\"python3\", \"-u\", \"~/.codex/notify.py\"]",
            "notify = [\"/usr/bin/python3\", \"~/.codex/notify.py\"]",
            "notify = [\"node\", \"$HOME/.codex/notify.py\"]",
        ] {
            let one = codex(&config(notify));
            let rewritten = script.replace(notify, "\"/root/.codex/notify.py\"");
            assert_eq!(one.text, config(&rewritten), "{notify}");
            assert_eq!(one.carried, vec![carried(&format!("{HOME}/.codex/notify.py"), "/root/.codex/notify.py")], "{notify}");
        }
        // A script the placer will not take: the key goes, listed by the script's path, not the interpreter's.
        let gone = codex(&config("notify = [\"python3\", \"~/.codex/gone.py\"]"));
        assert_eq!(gone.text, WITHOUT_NOTIFY);
        assert_eq!(gone.left, strings(&["~/.codex/gone.py"]));
        assert_eq!(gone.carried, Vec::<Carried>::new());
        // An interpreter with nothing to run, and one whose argument names no path, are left as written.
        for notify in ["notify = [\"python3\"]", "notify = [\"python3\", \"-c\", \"-q\"]", "notify = [\"node\", \"notify.js\"]"] {
            let text = config(notify);
            assert_eq!(codex(&text), untouched(&text), "{notify}");
        }
    }

    #[test]
    fn takes_the_comment_lines_above_the_key_out_with_it_so_the_machine_s_config_never() {
        let app = mac_app();
        let out = codex(&format!("# fires after every turn\n# the Mac app\nnotify = [\"{app}\"]\nmodel = \"x\"\n"));
        assert_eq!(out.text, "model = \"x\"\n");
        assert_eq!(out.left, vec![app]);
    }

    #[test]
    fn never_asks_the_placer_about_the_program_itself_since_a_binary_from_this_computer() {
        let app = mac_app();
        let asked = RefCell::new(Vec::<String>::new());
        let placer = |abs: &str| {
            asked.borrow_mut().push(abs.to_string());
            place(abs)
        };
        let out = HookCarry::Codex.carry(&config(&format!("notify = [\"{app}\"]")), HOME, &placer).unwrap();
        assert_eq!(*asked.borrow(), Vec::<String>::new());
        assert_eq!(out.left, vec![app.clone()]);
        let unclosed = format!("notify = [\n  \"{app}\",\n");
        for t in ["model = \"gpt-5.5\"\n", "", "not toml at all", "[mcp_servers.a]\ncommand = \"npx\"\n", unclosed.as_str()] {
            assert_eq!(codex(t), untouched(t), "{t}");
        }
    }
}
