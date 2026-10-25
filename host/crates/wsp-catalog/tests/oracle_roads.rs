// SPDX-License-Identifier: AGPL-3.0-only
// The roads' constants, scripts, casks and pins, and every road module on the oracle's sample roads, against
// tests/fixtures/roads.json and road_modules.json, which the TS catalog wrote (oracle/dump.oracle.ts).
mod oracle_common;

use oracle_common::{fixture, json, opt, s, same};
use serde_json::{json, Map, Value};
use wsp_catalog::base_image::BASE_IMAGE_COMMANDS;
use wsp_catalog::linux_casks::{linux_casks, CURSOR, GCLOUD, KUBECTL};
use wsp_catalog::mac_paths::{MAC_BIN_DIRS, MAC_BREW, MAC_ONLY};
use wsp_wire::views::golden_image::ToolPin;
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_catalog::release_pins::{pinned_release, RELEASE_PINS};
use wsp_catalog::road_modules::*;
use wsp_catalog::road_release::{asset_install, release_install, ASSET_ARCH_AARCH64, ASSET_ARCH_X86_64, ASSET_SKIPPED};
use wsp_catalog::roads::*;

fn constants() -> Map<String, Value> {
    let mut m = Map::new();
    let mut put = |k: &str, v: Value| {
        m.insert(k.to_string(), v);
    };
    put("MIB", json(MIB));
    put("GUEST_HOME", json(GUEST_HOME));
    put("HOME_BIN", json(HOME_BIN));
    put("LOCAL_BIN", json(LOCAL_BIN));
    put("APT_ENV", json(APT_ENV));
    put("CLAUDE_CONFIG_REL", json(CLAUDE_CONFIG_REL));
    put("GUEST_REWRITES", json(GUEST_REWRITES));
    put("CLAUDE_CONFIG_DIR", json(CLAUDE_CONFIG_DIR));
    put("CLAUDE_PLUGIN_SEED", json(CLAUDE_PLUGIN_SEED));
    put("CLAUDE_KEY_FILE", json(CLAUDE_KEY_FILE));
    put("CLAUDE_CODE", json(CLAUDE_CODE));
    put("CLAUDE_LATEST", json(&*CLAUDE_LATEST));
    put("CLAUDE_INSTALL", json(&*CLAUDE_INSTALL));
    put("GOLDEN_SETUP", json(&*GOLDEN_SETUP));
    put("GOLDEN_SMOKE", json(GOLDEN_SMOKE));
    put("UV", json(UV));
    put("UV_INSTALL", json(&*UV_INSTALL));
    put("NODE_RELEASES", Value::Object(NODE_RELEASES.iter().map(|(major, r)| (major.to_string(), json(r))).collect()));
    put("NODE_PATH_LINE", json(NODE_PATH_LINE));
    put("NODE_BIN", json(NODE_BIN));
    put("COMPOSE", json(COMPOSE));
    put("COMPOSE_PLUGIN", json(COMPOSE_PLUGIN));
    put("DOCKER_INSTALL", json(&*DOCKER_INSTALL));
    put("PYTHON_INSTALL", json(&*PYTHON_INSTALL));
    put("RUSTUP", json(RUSTUP));
    put("RUSTUP_INSTALL", json(&*RUSTUP_INSTALL));
    put("FD_INSTALL", json(&*FD_INSTALL));
    put("YARN_VERSION", json(YARN_VERSION));
    put("YARN_INSTALL", json(&*YARN_INSTALL));
    put("HERMES", json(HERMES));
    put("HERMES_INSTALL", json(&*HERMES_INSTALL));
    put("PLAYWRIGHT", json!({ "version": PLAYWRIGHT_VERSION }));
    put("PLAYWRIGHT_INSTALL", json(&*PLAYWRIGHT_INSTALL));
    put("SWIFT", json(SWIFT));
    put("SWIFT_INSTALL", json(&*SWIFT_INSTALL));
    put("OP_INSTALL", json(&*OP_INSTALL));
    put("NET_READ_S", json(NET_READ_S));
    put("NET_RETRIES", json(NET_RETRIES));
    put("CURL_NET", json(&*CURL_NET));
    put("CARGO_BIN", json(CARGO_BIN));
    put("APT_BIN", json(APT_BIN));
    put("APT_INDEX", json(APT_INDEX));
    put("HOMEBREW_STEP", json(HOMEBREW_STEP));
    put("APT_UPDATE", json(&*APT_UPDATE));
    put("HOMEBREW", json!({ "tag": HOMEBREW_TAG, "commit": HOMEBREW_COMMIT }));
    put("LINUXBREW_HOME", json(LINUXBREW_HOME));
    put("BREW_PREFIX", json(BREW_PREFIX));
    put("BREW_REPO", json(&*BREW_REPO));
    put("BREW_ENV", json(&*BREW_ENV));
    put("BREW", json(&*BREW));
    put("BREW_REAL", json(&*BREW_REAL));
    put("FROM_A_READABLE_DIR", json(&*FROM_A_READABLE_DIR));
    put("LINUXBREW_SHIM", json(&*LINUXBREW_SHIM));
    put("BREW_SHIM_PRESENT", json(&*BREW_SHIM_PRESENT));
    put("ASSET_ARCH", json!({ "x86_64": ASSET_ARCH_X86_64, "aarch64": ASSET_ARCH_AARCH64 }));
    put("ASSET_SKIPPED", json(ASSET_SKIPPED));
    put("INSTALL_HOMES", json(INSTALL_HOMES));
    put("ROADS", json(ROADS));
    put("TOOL_PREFIX", json(TOOL_PREFIX));
    m
}

fn installed(r: Result<String, RoadNote>) -> Value {
    match r {
        Ok(line) => json(line),
        Err(RoadNote(note)) => json!({ "note": note }),
    }
}

fn uninstalled(r: Result<String, RoadNote>) -> Value {
    match r {
        Ok(cmd) => json!({ "cmd": cmd }),
        Err(RoadNote(note)) => json!({ "note": note }),
    }
}

mod constants_and_scripts {
    use super::*;

    #[test]
    fn every_constant_and_pinned_installer_is_the_ts_text_byte_for_byte() {
        let want = fixture("roads.json")["constants"].clone();
        let got = constants();
        for (k, w) in want.as_object().unwrap() {
            same(k, got.get(k).cloned().unwrap_or_else(|| panic!("{k} has no Rust twin")), w);
        }
    }

    #[test]
    fn the_node_install_script_keeps_a_node_between_the_floor_and_its_own_major() {
        let w = fixture("roads.json")["nodeInstallScript"].clone();
        let r22 = node_release(22).unwrap();
        let r20 = node_release(20).unwrap();
        same("floor22", json(node_install_script(22, &r22)), &w["floor22"]);
        same("floor18on20", json(node_install_script(18, &r20)), &w["floor18on20"]);
        assert!(node_release(18).is_none());
    }

    #[test]
    fn the_casks_and_the_release_pins_are_the_ts_tables_in_order() {
        let w = fixture("roads.json");
        same("casks", json(linux_casks()), &w["casks"]);
        same("gcloud", json(&*GCLOUD), &w["casks"][0]);
        same("kubectl", json(&*KUBECTL), &w["casks"][1]);
        same("cursor", json(&*CURSOR), &w["cursorCask"]);
        let pins: Value = Value::Object(RELEASE_PINS.iter().map(|(repo, p)| (repo.clone(), json!({ "tag": p.tag, "assets": json(&p.assets) }))).collect());
        same("releasePins", pins.clone(), &w["releasePins"]);
        let order: Vec<&String> = RELEASE_PINS.keys().collect();
        let want: Vec<&String> = w["releasePins"].as_object().unwrap().keys().collect();
        assert_eq!(order, want);
    }

    #[test]
    fn a_pinned_release_carries_the_tag_assets_and_go_and_refuses_a_repo_the_table_lacks() {
        let w = fixture("roads.json")["pinnedRelease"].clone();
        same("gh", json(pinned_release("cli/cli", Some("github.com/cli/cli/v2/cmd/gh")).unwrap()), &w["gh"]);
        same("mise", json(pinned_release("jdx/mise", None).unwrap()), &w["mise"]);
        let err = pinned_release("nobody/nothing", None).unwrap_err();
        same("missing", json!({ "error": err.message }), &w["missing"]);
    }

    #[test]
    fn the_mac_paths_and_the_base_image_commands_are_the_ts_lists() {
        let w = fixture("roads.json");
        same("MAC_BREW", json(MAC_BREW), &w["macPaths"]["MAC_BREW"]);
        same("MAC_ONLY", json(MAC_ONLY), &w["macPaths"]["MAC_ONLY"]);
        same("MAC_BIN_DIRS", json(MAC_BIN_DIRS), &w["macPaths"]["MAC_BIN_DIRS"]);
        same("baseImageCommands", json(BASE_IMAGE_COMMANDS.iter().collect::<Vec<_>>()), &w["baseImageCommands"]);
    }

    #[test]
    fn a_home_relative_path_lands_where_the_guest_rewrites_put_it() {
        for (rel, want) in fixture("roads.json")["rewrittenRel"].as_object().unwrap() {
            same(&format!("rewrittenRel {rel:?}"), json(rewritten_rel(rel, None)), want);
        }
        assert_eq!(rewritten_rel("a/x", Some(&[("a/", "b/")])), "b/x");
        assert_eq!(rewritten_rel("a", Some(&[("a/", "b/")])), "b");
        assert_eq!(rewritten_rel("a", Some(&[("a/", "c")])), "c");
    }

    #[test]
    fn the_pin_check_go_binary_formula_name_and_linuxbrew_lines_are_the_ts_text() {
        let w = fixture("roads.json");
        same("pinCheckLine", json(pin_check_line("$asset", "$tag", "0123456789abcdef0123456789abcdef")), &w["pinCheckLine"]);
        for (m, want) in w["goBinary"].as_object().unwrap() {
            same(&format!("goBinary {m}"), json(go_binary(m)), want);
        }
        for (f, want) in w["formulaShortName"].as_object().unwrap() {
            same(&format!("formulaShortName {f}"), json(formula_short_name(f)), want);
        }
        same("brewHasCheck", json(brew_has_check(&["bat", "go"])), &w["brewHasCheck"]);
        same("asLinuxbrew", json(as_linuxbrew("install bat")), &w["asLinuxbrew"]);
        same("asLinuxbrewScript", json(as_linuxbrew_script("brew update\nbrew upgrade")), &w["asLinuxbrewScript"]);
    }

    #[test]
    fn a_word_names_a_road_only_where_it_is_one_of_the_twelve() {
        assert_eq!(is_road(Some("brew")), Some(RoadName::Brew));
        assert_eq!(is_road(Some("script")), Some(RoadName::Script));
        assert_eq!(is_road(Some("")), None);
        assert_eq!(is_road(None), None);
        assert_eq!(is_road(Some("Brew")), None);
        let names: Vec<&str> = ROADS.iter().map(|r| r.as_str()).collect();
        assert_eq!(names, ["brew", "npm", "pnpm", "bun", "uv", "pipx", "cargo", "go", "release", "vendor", "apt", "script"]);
    }

    #[test]
    fn a_pin_stands_while_the_version_is_its_tag_or_none_and_goes_once_it_moved() {
        let pin = ToolPin { tag: "1.0".into(), sha256: None, arch: None, latest: None };
        assert_eq!(pin_state_of(None, None), PinState::None);
        assert_eq!(pin_state_of(Some("1.0"), None), PinState::None);
        assert_eq!(pin_state_of(None, Some(&pin)), PinState::Same);
        assert_eq!(pin_state_of(Some("1.0"), Some(&pin)), PinState::Same);
        assert_eq!(pin_state_of(Some("2.0"), Some(&pin)), PinState::Moved);
    }
}

mod modules {
    use super::*;

    #[test]
    fn the_homes_for_no_prefix_and_for_wsps_own_prefix_are_the_ts_homes() {
        let w = fixture("road_modules.json");
        same("own", json(install_homes(None)), &w["homes"]["own"]);
        same("prefix", json(install_homes(Some(TOOL_PREFIX))), &w["homes"]["prefix"]);
        same("installEnv own", json(install_env(&install_homes(None))), &w["installEnv"]["own"]);
        same("installEnv prefix", json(install_env(&install_homes(Some(TOOL_PREFIX)))), &w["installEnv"]["prefix"]);
        let own = install_homes(None);
        for name in INSTALL_HOMES {
            assert_eq!(json(own.get(name)), w["homes"]["own"][json(name).as_str().unwrap()]);
        }
    }

    #[test]
    fn every_road_steps_limit_retry_and_lines_are_the_ts_ones() {
        let w = fixture("road_modules.json")["roadSteps"].clone();
        for road in ROADS {
            same(road.as_str(), json(road_step(road)), &w[road.as_str()]);
        }
    }

    #[test]
    fn every_modules_words_roots_paste_after_and_knobs_are_the_ts_ones() {
        let w = fixture("road_modules.json")["modules"].clone();
        let prefix = install_homes(Some(TOOL_PREFIX));
        for road in ROADS {
            let m = road_module_named(road);
            let want = &w[road.as_str()];
            same(&format!("{} words", road.as_str()), json(m.words()), &want["words"]);
            same(&format!("{} roots", road.as_str()), json(m.roots()), &want["roots"]);
            same(&format!("{} pastes", road.as_str()), json(m.pastes()), &want["pastes"]);
            same(&format!("{} after", road.as_str()), opt(m.after()), &want["after"]);
            let env = m.env(&install_homes(None)).map(|own| json!({ "own": json(own), "prefix": json(m.env(&prefix).unwrap()) }));
            same(&format!("{} env", road.as_str()), opt(env), &want["env"]);
        }
    }

    #[test]
    fn every_sample_road_installs_takes_off_reads_back_and_pins_as_the_ts_module_does() {
        let prefix = install_homes(Some(TOOL_PREFIX));
        let w = fixture("road_modules.json");
        for case in w["samples"].as_array().unwrap() {
            let label = s(&case["label"]);
            let road: InstallRoad = serde_json::from_value(case["road"].clone()).unwrap_or_else(|e| panic!("{label}: {e}"));
            let bin = s(&case["bin"]);
            let m = road_module(&road);
            assert_eq!(m.road, road.name(), "{label}");
            same(&format!("{label} bins"), json!({ "own": m.bins(&road, None), "prefix": m.bins(&road, Some(&prefix)) }), &case["bins"]);
            same(&format!("{label} present"), opt(m.present(&road, bin)), &case["present"]);
            same(&format!("{label} check"), opt(m.check(&road, bin)), &case["check"]);
            same(&format!("{label} shown"), opt(m.shown(&road, bin)), &case["shown"]);
            same(&format!("{label} install"), json!({ "own": installed(m.install(&road, bin, None)), "prefix": installed(m.install(&road, bin, Some(&prefix))) }), &case["install"]);
            same(&format!("{label} uninstall"), json!({ "own": uninstalled(m.uninstall(&road, bin, None)), "prefix": uninstalled(m.uninstall(&road, bin, Some(&prefix))) }), &case["uninstall"]);
            same(&format!("{label} names"), json(m.names(&road)), &case["names"]);
            same(&format!("{label} roadBin"), opt(m.bin(&road)), &case["roadBin"]);
            same(&format!("{label} at"), opt(m.at(&road, "9.9.9")), &case["at"]);
            let read = m.installed(&road, bin, None).map(|own| json!({ "own": own, "prefix": m.installed(&road, bin, Some(&prefix)).unwrap() }));
            same(&format!("{label} installed"), opt(read), &case["installed"]);
            same(&format!("{label} fixesVersion"), json(fixes_version(&road)), &case["fixesVersion"]);
            same(&format!("{label} versionOf"), opt(version_of(&road)), &case["versionOf"]);
            same(&format!("{label} standingPin"), opt(standing_pin(&road)), &case["standingPin"]);
            same(&format!("{label} unpinned"), json(unpinned(&road)), &case["unpinned"]);
        }
    }

    #[test]
    fn a_managers_row_reads_the_road_the_ts_reader_reads() {
        let w = fixture("road_modules.json");
        let rows = [
            ToolRow { name: "tool".into(), version: None, paths: vec![], label: "tool".into() },
            ToolRow { name: "tool".into(), version: Some("1.2.3".into()), paths: vec![], label: "tool 1.2.3".into() },
            ToolRow { name: "gopls".into(), version: None, paths: vec!["golang.org/x/tools/gopls@v0.16.1".into()], label: "gopls".into() },
            ToolRow { name: "gopls".into(), version: Some("v0.17.0".into()), paths: vec!["golang.org/x/tools/gopls@v0.16.1".into()], label: "gopls".into() },
            ToolRow { name: "old".into(), version: None, paths: vec![], label: "old (example.com/old@v1.0.0)".into() },
            ToolRow { name: "bad".into(), version: None, paths: vec!["@v1".into()], label: "bad".into() },
            ToolRow { name: "bad".into(), version: None, paths: vec!["example.com/x@".into()], label: "bad".into() },
        ];
        for case in w["fromRow"].as_array().unwrap() {
            let manager = s(&case["manager"]);
            same(&format!("{manager} reads"), json(reads_row_road(manager)), &case["reads"]);
            let got: Vec<Value> = rows.iter().map(|r| opt(row_road(manager, r))).collect();
            same(&format!("{manager} rows"), json(got), &case["rows"]);
        }
    }

    #[test]
    fn a_rows_own_bin_folders_move_with_their_manager_under_a_prefix() {
        let w = fixture("road_modules.json")["homeBins"].clone();
        let bins: Vec<String> = [CARGO_BIN, LOCAL_BIN, "/opt/x/bin", HOME_BIN, "/root/.local/share/pnpm"].iter().map(|x| x.to_string()).collect();
        same("own", json(home_bins(&bins, &install_homes(None))), &w["own"]);
        same("prefix", json(home_bins(&bins, &install_homes(Some(TOOL_PREFIX)))), &w["prefix"]);
    }

    #[test]
    fn the_release_scripts_are_the_ts_text_for_every_shape_of_assets_and_pin() {
        let w = fixture("road_modules.json")["releaseLines"].clone();
        let sha = |c: char| c.to_string().repeat(64);
        let both: wsp_catalog::roads::ReleaseAssets = serde_json::from_value(json!({ "x86_64": { "name": "a b.tar.gz", "sha256": sha('1') }, "aarch64": { "name": "c.zip", "sha256": sha('2') } })).unwrap();
        let half: wsp_catalog::roads::ReleaseAssets = serde_json::from_value(json!({ "aarch64": { "name": "b-arm64", "sha256": sha('2') } })).unwrap();
        let none = wsp_catalog::roads::ReleaseAssets::default();
        same("assetComplete", json(asset_install("gh", "cli/cli", "v2", &both, None)), &w["assetComplete"]);
        same("assetHalfGo", json(asset_install("b", "a/b", "v1", &half, Some("github.com/a/b/cmd/other"))), &w["assetHalfGo"]);
        same("assetNone", json(asset_install("b", "a/b", "v1", &none, None)), &w["assetNone"]);
        let pin = |arch: Option<&str>, sum: bool| -> ToolPin { serde_json::from_value(json!({ "tag": "v1" })).map(|mut p: ToolPin| { if sum { p.sha256 = Some(sha('f')); } p.arch = arch.map(|a| serde_json::from_value(json!(a)).unwrap()); p }).unwrap() };
        same("releaseNoPin", json(release_install("b", "a/b", "v1", None, None)), &w["releaseNoPin"]);
        same("releasePinNoArch", json(release_install("b", "a/b", "v1", Some(&pin(None, true)), Some("github.com/a/b@v0.9.0"))), &w["releasePinNoArch"]);
        same("releasePinArch", json(release_install("b", "a/b", "v1", Some(&pin(Some("x86_64"), true)), Some("github.com/a/b"))), &w["releasePinArch"]);
        same("releasePinNoSum", json(release_install("b", "a/b", "v1", Some(&pin(None, false)), None)), &w["releasePinNoSum"]);
    }
}
