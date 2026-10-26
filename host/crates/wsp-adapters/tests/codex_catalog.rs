// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/catalog.test.ts: `catalogProbeCommand` and `parseCatalogProbe`. The describe "a probe on a
//! machine whose ChatGPT sign-in the host holds" runs in codex_adapter's file.

#[path = "common/codex_shell.rs"]
mod codex_shell;
#[path = "common/codex_wire.rs"]
mod codex_wire;

use regex::Regex;
use serde_json::{json, Value};
use wsp_adapters::codex::catalog::{catalog_probe_command, parse_catalog_probe};
use wsp_adapters::codex::command::{build_env, CodexEnvOptions};
use wsp_adapters::port::{HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe};
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;

const LOGIN: &str = "codex login --device-auth";
const SEP: &str = "__WSP_CATALOG_SEP__";
/// The one wait the probe carries: how long its reader gives the server for one more line.
const LINE_WAIT_MS: u128 = 10_000;

// codex-cli 0.153.0 on 2026-09-07: `codex --version`, `codex --help` and the app-server's answers to initialize,
// model/list, config/read and account/read on a CODEX_HOME with no sign-in, joined by the probe's separator. The
// answers come back out of order there, config/read last, which is why the parser reads each by its own id.
fn real() -> String {
    codex_wire::fixture_text("codex", "catalog-probe.txt")
}

// The same binary under a config routed to OpenRouter: config/read names a model no OpenAI catalog carries, and
// account/read wants no OpenAI sign-in. Only the app-server section differs, so it is spliced onto the head above.
fn route_server() -> String {
    codex_wire::fixture_text("codex", "catalog-probe-route.jsonl")
}

fn sections(stdout: &str) -> Vec<String> {
    stdout.split(SEP).map(String::from).collect()
}

/// The captured stdout with its app-server section replaced, so one reading of --version and --help serves every case.
fn probe_output(server: &[String], head: &str) -> String {
    let parts = sections(head);
    format!("{}{SEP}{}{SEP}\n{}\n", parts[0], parts[1], server.join("\n"))
}

fn server_lines(stdout: &str) -> Vec<String> {
    sections(stdout)[2].split('\n').filter(|l| l.starts_with('{')).map(String::from).collect()
}

/// The lists, or a failure naming what came back instead: every case here that is not about a refusal wants lists.
fn lists(stdout: &str) -> HarnessCatalogProbe {
    match parse_catalog_probe(stdout, LOGIN, None) {
        Some(HarnessCatalogAnswer::Probe(p)) => p,
        other => panic!("expected the binary's lists, got {other:?}"),
    }
}

/// The same output with model/list answering an empty catalog, which 0.153.0 does under no config we have seen.
fn without_models(account: &str) -> String {
    let mut kept: Vec<String> =
        server_lines(&real()).into_iter().filter(|l| !l.starts_with("{\"id\":2") && !l.starts_with("{\"id\":4")).collect();
    kept.push("{\"id\":2,\"result\":{\"data\":[]}}".to_string());
    if !account.is_empty() {
        kept.push(account.to_string());
    }
    probe_output(&kept, &real())
}

fn slugs(models: &[HarnessCatalogModelProbe]) -> Vec<&str> {
    models.iter().map(|m| m.slug.as_str()).collect()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

mod catalog_probe_command {
    use super::*;

    #[test]
    fn reads_the_base_env_a_turn_gets_off_its_input_path_and_codex_home_included_so_a_p() {
        let cmd = catalog_probe_command(None, None).unwrap();
        assert!(cmd.starts_with(&format!("cd ~ && {ENV_FROM_INPUT}; ")), "{cmd}");
        assert!(!Regex::new(r"PATH=|CODEX_HOME=").unwrap().is_match(&cmd), "{cmd}");
    }

    #[test]
    fn asks_for_the_version_the_help_and_the_four_app_server_requests_and_never_a_turn() {
        let cmd = catalog_probe_command(None, None).unwrap();
        assert!(cmd.contains("codex --version"));
        assert!(cmd.contains("codex --help"));
        assert!(cmd.contains("codex app-server"));
        for method in ["initialize", "model/list", "config/read", "account/read"] {
            assert!(cmd.contains(&format!("\"method\":\"{method}\"")), "{method}");
        }
        assert!(!Regex::new(r"codex exec|codex login|--dangerously").unwrap().is_match(&cmd));
        // No fixed wait anywhere: this line is awaited at a session start. The only wait is the reader's, per line.
        assert!(!Regex::new(r"\bsleep\b").unwrap().is_match(&cmd));
        assert!(cmd.contains(&format!("-t {} -u 4", LINE_WAIT_MS / 1000)));
        // The only pid killed is the one the shell recorded for the server, and never a bare 0, which is the group.
        assert!(cmd.contains("kill \"$WSP_APP_SERVER_PID\""));
        assert!(!Regex::new(r#"pkill|kill[a]ll|kill -|kill "?0"#).unwrap().is_match(&cmd));
        // Nothing from bash 4 or later: the Mac's own bash is 3.2 and this line has to prove itself there too.
        assert!(!Regex::new(r"\bcoproc\b").unwrap().is_match(&cmd));
    }

    #[test]
    fn holds_the_app_server_s_stdin_open_until_its_answers_are_in_and_lets_go_on_the_la() {
        // A fake codex on PATH that answers only once it has been sent all four requests, as the app-server does, and
        // then writes one line more. Every request reaching it proves the hold; the line after the fourth answer never
        // being read proves the probe let go there rather than waiting the server out.
        let cmd = catalog_probe_command(None, None).unwrap();
        let dir = codex_shell::TempDir::new("wsp-codex-probe-");
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let answers = ["{\"id\":1,\"result\":{}}", "{\"id\":2,\"result\":{}}", "{\"id\":4,\"result\":{}}", "{\"id\":3,\"result\":{}}"];
        let mut script = vec![
            "#!/bin/sh".to_string(),
            "if [ \"$1\" != \"app-server\" ]; then echo \"CALL $1\"; exit 0; fi".to_string(),
            "n=0; while [ \"$n\" -lt 4 ] && read -r line; do n=$((n+1)); echo \"GOT $line\"; done".to_string(),
            "echo \"SERVER $CODEX_HOME\"".to_string(),
        ];
        script.extend(answers.iter().map(|a| format!("echo '{a}'")));
        script.push("echo PAST_THE_LAST_ANSWER".to_string());
        script.push(String::new());
        codex_shell::write_stub(&bin.join("codex"), &script.join("\n"));
        let started = std::time::Instant::now();
        let env = codex_shell::pairs(&[("PATH", &format!("{}:/usr/bin:/bin", bin.to_string_lossy())), ("HOME", &dir.s())]);
        let built = build_env(&CodexEnvOptions { home: "/root/.codex".to_string(), ..Default::default() }).unwrap();
        let input = codex_shell::nul_env(built.iter());
        let out = codex_shell::run_shell("bash", &cmd, &env, &input).stdout;
        assert!(out.contains("CALL --version"), "{out}");
        assert!(out.contains("CALL --help"), "{out}");
        // The probe ran the server under the session's own home, and every request reached it while it was alive.
        assert!(out.contains("SERVER /root/.codex"), "{out}");
        for method in ["initialize", "model/list", "config/read", "account/read"] {
            assert!(out.contains(&format!("\"method\":\"{method}\"")), "{method}: {out}");
        }
        assert_eq!(out.lines().filter(|l| l.starts_with("GOT ")).count(), 4, "{out}");
        // Out of order on purpose: the reader counts answers, so id 3 landing last still ends it.
        for answer in answers {
            assert!(out.contains(answer), "{answer}: {out}");
        }
        assert!(!out.contains("PAST_THE_LAST_ANSWER"), "{out}");
        // Nothing here waits on a clock: the whole probe is well inside the one line the reader is allowed to wait for.
        assert!(started.elapsed().as_millis() < LINE_WAIT_MS);
    }
}

mod parse_catalog_probe {
    use super::*;

    #[test]
    fn reads_the_version_the_models_the_app_server_offers_with_their_reasoning_efforts() {
        let probe = lists(&real());
        assert_eq!(probe.version.as_deref(), Some("0.153.0"));
        assert_eq!(slugs(&probe.models), ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5", "gpt-5.2"]);
        let labels: Vec<&str> = probe.models.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(labels, ["GPT-5.6-Sol", "GPT-5.6-Terra", "GPT-5.6-Luna", "GPT-5.5", "GPT-5.2"]);
        let defaults: Vec<&str> = probe.models.iter().filter(|m| m.is_default).map(|m| m.slug.as_str()).collect();
        assert_eq!(defaults, ["gpt-5.6-sol"]);
        assert!(probe.models[0].description.as_deref().unwrap().contains("frontier agentic coding model"));
        assert_eq!(probe.models[0].efforts, Some(strs(&["low", "medium", "high", "xhigh", "max", "ultra"])));
        assert_eq!(probe.models[3].efforts, Some(strs(&["low", "medium", "high", "xhigh"])));
        // The effort each model runs at when a turn names none; the catalog marks the default model's.
        let default_efforts: Vec<Option<&str>> = probe.models.iter().map(|m| m.default_effort.as_deref()).collect();
        assert_eq!(default_efforts, [Some("low"), Some("medium"), Some("medium"), Some("medium"), Some("medium")]);
        // No codex model runs at two context windows, so no model offers one.
        assert!(probe.models.iter().all(|m| m.context_windows.is_empty()));
        assert_eq!(probe.efforts, strs(&["low", "medium", "high", "xhigh", "max", "ultra"]));
        assert_eq!(probe.permission_modes, strs(&["read-only", "workspace-write", "danger-full-access"]));
    }

    #[test]
    fn marks_a_model_fast_where_model_list_names_a_fast_speed_tier_for_it() {
        let models = json!({ "id": 2, "result": { "data": [
            { "id": "gpt-6-astra", "displayName": "GPT-6-Astra", "isDefault": true, "hidden": false, "supportedReasoningEfforts": [], "additionalSpeedTiers": ["fast"] },
            { "id": "gpt-5.2", "displayName": "GPT-5.2", "isDefault": false, "hidden": false, "supportedReasoningEfforts": [], "additionalSpeedTiers": [] },
        ] } });
        let mut server: Vec<String> = server_lines(&real()).into_iter().filter(|l| !l.starts_with("{\"id\":2")).collect();
        server.push(models.to_string());
        let probe = lists(&probe_output(&server, &real()));
        let fast: Vec<(&str, Option<bool>)> = probe.models.iter().map(|m| (m.slug.as_str(), m.fast)).collect();
        assert_eq!(fast, [("gpt-6-astra", Some(true)), ("gpt-5.2", None)]);
    }

    #[test]
    fn a_route_to_another_provider_leads_with_the_model_config_names_since_that_is_what() {
        let server: Vec<String> = route_server().split('\n').filter(|l| l.starts_with('{')).map(String::from).collect();
        let probe = lists(&probe_output(&server, &real()));
        // Which efforts the provider takes is not codex's to say, so the model names no list and keeps every one of the
        // catalog's; an empty list would read as "takes none" and refuse them all. No efforts and no default effort
        // are both `None` in the whole value.
        assert_eq!(
            probe.models[0],
            HarnessCatalogModelProbe {
                slug: "anthropic/claude-sonnet-4.5".to_string(),
                label: "anthropic/claude-sonnet-4.5".to_string(),
                description: None,
                efforts: None,
                default_effort: None,
                context_windows: vec![],
                is_default: true,
                fast: None,
            }
        );
        assert_eq!(probe.models.iter().filter(|m| m.is_default).count(), 1);
        assert_eq!(slugs(&probe.models)[1..], ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5", "gpt-5.2"]);
        assert_eq!(probe.efforts, strs(&["low", "medium", "high", "xhigh", "max", "ultra"]));
    }

    #[test]
    fn is_null_when_the_app_server_said_nothing_and_tolerates_a_missing_version_or_help() {
        assert_eq!(parse_catalog_probe("", LOGIN, None), None);
        assert_eq!(parse_catalog_probe(&format!("x{SEP}y{SEP}not json\n"), LOGIN, None), None);
        let bare = lists(&probe_output(&server_lines(&real()), &format!("{SEP}{SEP}")));
        assert_eq!(bare.version, None);
        assert_eq!(bare.permission_modes, Vec::<String>::new());
        assert_eq!(bare.models.len(), 5);
    }

    #[test]
    fn names_the_sign_in_when_the_app_server_answered_and_offered_no_model_for_want_of() {
        assert_eq!(
            parse_catalog_probe(&without_models("{\"id\":4,\"result\":{\"account\":null,\"requiresOpenaiAuth\":true}}"), LOGIN, None),
            Some(HarnessCatalogAnswer::Refused(format!("Codex is not signed in where this thread runs; run {LOGIN} there")))
        );
        // Signed in, or routed to a provider that wants no OpenAI login: an empty catalog is not a sign-in to ask for.
        assert_eq!(
            parse_catalog_probe(&without_models("{\"id\":4,\"result\":{\"account\":null,\"requiresOpenaiAuth\":false}}"), LOGIN, None),
            None
        );
        assert_eq!(
            parse_catalog_probe(
                &without_models("{\"id\":4,\"result\":{\"account\":{\"id\":\"a\"},\"requiresOpenaiAuth\":true}}"),
                LOGIN,
                None
            ),
            None
        );
        assert_eq!(parse_catalog_probe(&without_models(""), LOGIN, None), None);
    }

    #[test]
    fn leaves_out_a_model_the_app_server_hides_and_reads_each_answer_by_its_own_id_what() {
        let mut hidden: Vec<String> = server_lines(&real())
            .into_iter()
            .map(|line| {
                let mut message: Value = serde_json::from_str(&line).unwrap();
                if message["id"] != json!(2) {
                    return line;
                }
                if let Some(data) = message["result"]["data"].as_array_mut() {
                    for model in data {
                        if model["id"] == json!("gpt-5.5") {
                            model["hidden"] = json!(true);
                        }
                    }
                }
                message.to_string()
            })
            .collect();
        hidden.reverse();
        let probe = lists(&probe_output(&hidden, &real()));
        assert_eq!(slugs(&probe.models), ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.2"]);
        assert_eq!(probe.version.as_deref(), Some("0.153.0"));
    }
}

/// Rule CC1 of spec/codex_catalog.md, "Rules added for the port", printed by the TS at 65f6b89c7.
mod rules_added {
    use super::*;

    #[test]
    fn the_sandbox_modes_are_read_within_600_utf16_units_counted_from_the_flag_itself() {
        // CC1; catalog.ts:53-69. The flag is 24 units and `[possible values: a]` 20, so 556 units between them fit and
        // 576 do not; 285 emoji are 570 units; a bracket inside the list ends it.
        let head = sections(&real());
        let probe = |help: String| format!("{}{SEP}{help}{SEP}{}", head[0], head[2]);
        let flag = "--sandbox <SANDBOX_MODE>";
        let modes = |help: String| lists(&probe(help)).permission_modes;
        assert_eq!(modes(format!("\n{flag}{}[possible values: a]\n", "x".repeat(556))), vec!["a".to_string()]);
        assert_eq!(modes(format!("\n{flag}{}[possible values: a]\n", "x".repeat(576))), Vec::<String>::new());
        assert_eq!(modes(format!("\n{flag}{}[possible values: a]\n", "\u{1F600}".repeat(285))), Vec::<String>::new());
        assert_eq!(modes(format!("\n{flag} [possible values: a[b]c, d]\n")), vec!["a[b".to_string()]);
    }
}
