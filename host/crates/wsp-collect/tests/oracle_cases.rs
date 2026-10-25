// SPDX-License-Identifier: AGPL-3.0-only
//! Whole functions of credential signals, the Ghostty reader, the seed menu and the project scan against what the TS
//! answered for the same inputs (tests/fixtures/oracle/*.json, written by oracle/rest.oracle.ts).
mod common;

use std::cell::Cell;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::credential_signals::{bare_urls, file_signals, keys_signal, name_signal, pem_signal, top_level_keys, FileShape};
use wsp_collect::ghostty_config::{parse_ghostty_directives, read_ghostty_config, theme_for};
use wsp_collect::host::Platform;
use wsp_collect::project::scan_project;
use wsp_collect::seed::{seed_menu, SeedOptions};
use wsp_wire::views::project_bundle::TerminalScheme;

fn oracle(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/oracle/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn laptop_of(files: &Value) -> FakeLaptop {
    let mut laptop = FakeLaptop::new();
    for (path, text) in files.as_object().unwrap() {
        laptop = laptop.file(path, text.as_str().unwrap());
    }
    laptop
}

fn scheme(v: &Value) -> TerminalScheme {
    if v == "light" {
        TerminalScheme::Light
    } else {
        TerminalScheme::Dark
    }
}

mod credential_signals {
    use super::*;

    #[test]
    fn every_name_of_the_oracle_is_or_is_not_credential_shaped_as_the_ts_said() {
        for case in oracle("credential-signals.json")["names"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            assert_eq!(name_signal(name), case["signal"].as_bool().unwrap(), "{name:?}");
        }
    }

    #[test]
    fn every_text_of_the_oracle_gives_the_keys_pem_and_bare_urls_the_ts_gave() {
        for case in oracle("credential-signals.json")["texts"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let keys: Vec<String> = case["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_string()).collect();
            assert_eq!(top_level_keys(text), keys, "keys of {text:?}");
            assert_eq!(keys_signal(text), case["keysSignal"].as_bool().unwrap(), "keys signal of {text:?}");
            assert_eq!(pem_signal(text), case["pem"].as_bool().unwrap(), "pem of {text:?}");
            let bare = bare_urls(text);
            assert_eq!(bare.text, case["bare"]["text"].as_str().unwrap(), "bare text of {text:?}");
            let urls: Vec<String> = case["bare"]["urls"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_string()).collect();
            assert_eq!(bare.urls, urls, "urls of {text:?}");
        }
    }

    #[test]
    fn file_signals_reads_only_what_it_may_and_answers_what_the_ts_answered() {
        for case in oracle("file-signals.json").as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let text = case["text"].as_str().map(str::to_string);
            let reads = Cell::new(0);
            let e = FileShape { bytes: case["bytes"].as_u64().unwrap(), mode: case["mode"].as_u64().unwrap() as u32 };
            let got = block_on(file_signals(
                name,
                e,
                || {
                    reads.set(reads.get() + 1);
                    let t = text.clone();
                    async move { t }
                },
                case["shallow"].as_bool().unwrap(),
            ));
            let got = serde_json::to_value(got).unwrap();
            assert_eq!(got, case["signals"], "{case}");
            assert_eq!(reads.get(), case["reads"].as_u64().unwrap(), "reads of {case}");
        }
    }
}

mod ghostty {
    use super::*;

    #[test]
    fn every_text_of_the_oracle_gives_the_directives_the_ts_gave() {
        for case in oracle("ghostty.json")["directives"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let got: Vec<Value> = parse_ghostty_directives(text).into_iter().map(|d| json!({"key": d.key, "value": d.value})).collect();
            assert_eq!(Value::Array(got), case["directives"], "{text:?}");
        }
    }

    #[test]
    fn every_theme_value_of_the_oracle_picks_the_side_the_ts_picked() {
        for case in oracle("ghostty.json")["themeFor"].as_array().unwrap() {
            let value = case["value"].as_str().unwrap();
            assert_eq!(theme_for(value, scheme(&case["scheme"])), case["theme"].as_str().unwrap(), "{case}");
        }
    }

    /// The TS writes the config's keys in the order they were first set; the wire struct fixes one order
    /// (spec/ghostty_config.md, Decisions 1), so the answer is compared as a map.
    #[test]
    fn every_config_of_the_oracle_reads_as_the_ts_read_it() {
        for case in oracle("ghostty-configs.json").as_array().unwrap() {
            let mut laptop = laptop_of(&case["files"]);
            if case["platform"] == "linux" {
                laptop = laptop.platform(Platform::Linux);
            }
            let host = fake_host(laptop);
            let got = block_on(read_ghostty_config(&host, scheme(&case["scheme"])));
            assert_eq!(serde_json::to_value(got).unwrap(), case["config"], "{}", case["name"]);
        }
    }
}

mod seed {
    use super::*;

    #[test]
    fn every_menu_of_the_oracle_is_the_plan_the_ts_wrote_key_for_key_with_the_same_runs() {
        for case in oracle("seed.json").as_array().unwrap() {
            let mut laptop = FakeLaptop::new();
            for (key, out) in case["exec"].as_object().unwrap() {
                laptop = laptop.exec(key, out.as_str().unwrap());
            }
            let host = fake_host(laptop);
            let plan = block_on(seed_menu(
                &host,
                case["folder"].as_str().unwrap(),
                &SeedOptions { claude_state_home: "/Users/dev/.claude".to_string() },
            ));
            assert_eq!(serde_json::to_string(&plan).unwrap(), case["text"].as_str().unwrap(), "{}", case["name"]);
            let calls: Vec<String> = case["calls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
            assert_eq!(host.calls(), calls, "{}", case["name"]);
        }
    }
}

mod project {
    use super::*;

    #[test]
    fn every_scan_of_the_oracle_names_the_rows_and_candidates_the_ts_named_in_its_order() {
        for case in oracle("project.json").as_array().unwrap() {
            let mut laptop = FakeLaptop::new();
            for (path, text) in case["files"].as_object().unwrap() {
                laptop = laptop.file(&format!("/Users/dev/proj/{path}"), text.as_str().unwrap());
            }
            let scan = block_on(scan_project(&fake_host(laptop), "/Users/dev/proj"));
            let need = |n: &wsp_collect::project::ProjectNeed| json!({"id": n.id, "name": n.name, "why": n.why});
            let got = json!({"dir": scan.dir, "rows": scan.rows.iter().map(need).collect::<Vec<_>>(), "candidates": scan.candidates.iter().map(need).collect::<Vec<_>>()});
            // Arrays compare in order, so the first-found order of rows and candidates is pinned.
            assert_eq!(got, case["scan"], "{}", case["files"]);
        }
    }
}
