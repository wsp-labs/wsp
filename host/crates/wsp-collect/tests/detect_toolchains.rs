// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/toolchains.test.ts.
mod common;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::detect::toolchains::detect_toolchains;
use wsp_collect::manifest::ManifestEntry;

fn values(rows: &[ManifestEntry]) -> Vec<Value> {
    rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect()
}

mod toolchains {
    use super::*;

    #[test]
    fn mise_brings_its_config_and_the_global_tool_versions() {
        let host = fake_host(FakeLaptop::new().file("~/.config/mise/config.toml", 300).file("~/.tool-versions", 40).which(&["mise"]));
        let rows = block_on(detect_toolchains(&host));
        assert_eq!(
            values(&rows),
            vec![json!({
                "rung": "toolchains", "id": "toolchains/mise", "label": "mise pins",
                "paths": ["~/.config/mise/config.toml", "~/.tool-versions"], "bytes": 340, "default": "bring",
            })]
        );
    }

    #[test]
    fn asdf_is_recognised_by_its_home_dir_even_when_not_on_path() {
        let host = fake_host(FakeLaptop::new().file("~/.asdf/plugins/nodejs/x", 1).file("~/.tool-versions", 40));
        let rows = block_on(detect_toolchains(&host));
        assert_eq!(
            values(&rows),
            vec![
                json!({ "rung": "toolchains", "id": "toolchains/asdf", "label": "asdf pins", "paths": ["~/.tool-versions"], "bytes": 40, "default": "bring" })
            ]
        );
    }

    #[test]
    fn fnm_names_the_current_node_nvm_reads_its_default_alias() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.nvm/nvm.sh", 1)
                .file("~/.nvm/alias/default", "lts/*\n")
                .file("~/.node-version", 8)
                .which(&["fnm"])
                .exec("fnm current", "v22.12.0\n"),
        );
        let rows = block_on(detect_toolchains(&host));
        assert_eq!(
            values(&rows),
            vec![
                json!({ "rung": "toolchains", "id": "toolchains/fnm", "label": "fnm (node v22.12.0)", "paths": ["~/.node-version"], "bytes": 8, "default": "bring" }),
                json!({ "rung": "toolchains", "id": "toolchains/nvm", "label": "nvm (default lts/*)", "paths": [], "bytes": 0, "default": "bring" }),
            ]
        );
    }

    #[test]
    fn each_toolchain_manager_row() {
        let cases: Vec<(&str, Vec<(&str, u64)>, Vec<&str>, &str, &str, Vec<&str>)> = vec![
            ("pyenv", vec![("~/.pyenv/version", 6)], vec![], "toolchains/pyenv", "pyenv (global python)", vec!["~/.pyenv/version"]),
            (
                "uv",
                vec![("~/.config/uv/uv.toml", 20), ("~/.python-version", 5)],
                vec!["uv"],
                "toolchains/uv",
                "uv (python versions)",
                vec!["~/.config/uv/uv.toml", "~/.python-version"],
            ),
            (
                "rustup",
                vec![("~/.rustup/settings.toml", 50), ("~/.cargo/config.toml", 30)],
                vec![],
                "toolchains/rustup",
                "rustup default toolchain",
                vec!["~/.rustup/settings.toml", "~/.cargo/config.toml"],
            ),
        ];
        for (name, files, which, id, label, paths) in cases {
            let laptop = files.iter().fold(FakeLaptop::new(), |l, (p, n)| l.file(p, *n)).which(&which);
            let rows = block_on(detect_toolchains(&fake_host(laptop)));
            // bytes is `expect.any(Number)` in the TS.
            let got: Vec<Value> = rows
                .iter()
                .map(|r| {
                    let mut v = serde_json::to_value(r).unwrap();
                    assert!(v["bytes"].is_number(), "{name}: bytes is a number in {v}");
                    v["bytes"] = json!("<any number>");
                    v
                })
                .collect();
            assert_eq!(
                got,
                vec![
                    json!({ "rung": "toolchains", "id": id, "label": label, "paths": paths, "bytes": "<any number>", "default": "bring" })
                ],
                "{name}"
            );
        }
    }

    #[test]
    fn go_carries_its_env_file_and_names_the_version() {
        let host = fake_host(
            FakeLaptop::new().file("~/.config/go/env", 12).which(&["go"]).exec("go version", "go version go1.23.1 darwin/arm64\n"),
        );
        let rows = block_on(detect_toolchains(&host));
        assert_eq!(
            values(&rows),
            vec![
                json!({ "rung": "toolchains", "id": "toolchains/go", "label": "go 1.23.1", "paths": ["~/.config/go/env"], "bytes": 12, "default": "bring" })
            ]
        );
    }

    #[test]
    fn nothing_installed_gives_no_rows() {
        assert_eq!(block_on(detect_toolchains(&empty_host())), Vec::<ManifestEntry>::new());
    }
}
