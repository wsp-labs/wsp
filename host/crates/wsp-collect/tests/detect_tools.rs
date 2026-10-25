// SPDX-License-Identifier: AGPL-3.0-only
//! The tools rung. Port of packages/collect/test/tools.test.ts at a3b870a6e.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use common::fake_host::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::json;
use wsp_collect::detect::tools::{
    detect_tools, parse_brewfile, parse_bun_globals, parse_cargo_installs, parse_go_version_m, parse_npm_globals, parse_pipx_list,
    parse_pnpm_globals, parse_uv_tool_list, BrewKind, BrewLine, GoModule, Pkg,
};
use wsp_collect::host::{Host, HostExec, Platform, RunOptions};

const BREWFILE: &str = "tap \"homebrew/bundle\"\nbrew \"gh\"\nbrew \"jq\", link: true\nbrew \"oven-sh/bun/bun\"\nbrew \"mas\"\ncask \"rectangle\"\nmas \"Xcode\", id: 497799835\nvscode \"ms-python.python\"\n";

const BREW_DUMP: &str = "HOMEBREW_NO_AUTO_UPDATE=1 brew bundle dump --file=- --formula --taps --no-restart";

fn pkg(name: &str, version: Option<&str>, path: Option<&str>) -> Pkg {
    Pkg { name: name.to_string(), version: version.map(str::to_string), path: path.map(str::to_string) }
}

fn brew_line(kind: BrewKind, name: &str) -> BrewLine {
    BrewLine { kind, name: name.to_string() }
}

fn apt_laptop() -> FakeLaptop {
    FakeLaptop::new()
        .platform(Platform::Linux)
        .which(&["apt-mark", "dpkg-query"])
        .file("/var/lib/dpkg/info/direnv:amd64.list", "/usr/bin/direnv\n")
        .file("/var/lib/dpkg/info/libpq5.list", "/usr/lib/libpq.so.5\n")
        .exec("apt-mark showmanual", "direnv\nlibpq5\n")
        .exec("dpkg-query -Wf ${Package} ${Priority}\n", "direnv optional\nlibpq5 optional\n")
}

/// Answers brew only after cargo has been asked, and says whether it was.
struct RaceExec {
    cargo_asked: RefCell<Option<oneshot::Sender<()>>>,
    asked: RefCell<Option<oneshot::Receiver<()>>>,
    together: Rc<Cell<bool>>,
}

impl HostExec for RaceExec {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        async move { bin == "brew" || bin == "cargo" }.boxed_local()
    }

    fn run<'a>(&'a self, cmd: &'a str, _args: &'a [String], _opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        async move {
            if cmd == "cargo" {
                if let Some(tx) = self.cargo_asked.borrow_mut().take() {
                    let _ = tx.send(());
                }
                return Some("bacon v3.1.0:\n    bacon\n".to_string());
            }
            let rx = self.asked.borrow_mut().take();
            if let Some(rx) = rx {
                let came = matches!(tokio::time::timeout(Duration::from_secs(2), rx).await, Ok(Ok(())));
                self.together.set(came);
            }
            Some("brew \"gh\"\n".to_string())
        }
        .boxed_local()
    }
}

mod tools {
    use super::*;

    #[test]
    fn parses_a_brewfile_into_taps_and_formulae_casks_mas_apps_and_vscode_lines_have_no_linux_build() {
        assert_eq!(
            parse_brewfile(BREWFILE),
            vec![
                brew_line(BrewKind::Tap, "homebrew/bundle"),
                brew_line(BrewKind::Brew, "gh"),
                brew_line(BrewKind::Brew, "jq"),
                brew_line(BrewKind::Brew, "oven-sh/bun/bun"),
                brew_line(BrewKind::Brew, "mas"),
            ]
        );
    }

    #[test]
    fn homebrew_rows_taps_and_formulae_marked_by_the_linux_bottle_snapshot() {
        block_on(async {
            let host = fake_host(FakeLaptop::new().which(&["brew"]).exec(BREW_DUMP, BREWFILE));
            let rows = detect_tools(&host).await.unwrap();
            assert_eq!(
                serde_json::to_value(&rows).unwrap(),
                json!([
                    { "rung": "tools", "id": "tools/brew-tap/homebrew/bundle", "label": "homebrew/bundle", "group": "Homebrew taps", "paths": [], "bytes": 0, "default": "bring", "linux": "yes" },
                    { "rung": "tools", "id": "tools/brew/gh", "label": "gh", "group": "Homebrew", "paths": [], "bytes": 0, "default": "bring", "linux": "yes" },
                    { "rung": "tools", "id": "tools/brew/jq", "label": "jq", "group": "Homebrew", "paths": [], "bytes": 0, "default": "bring", "linux": "yes" },
                    { "rung": "tools", "id": "tools/brew/oven-sh/bun/bun", "label": "oven-sh/bun/bun", "group": "Homebrew", "paths": [], "bytes": 0, "default": "skip", "linux": "unknown" },
                    { "rung": "tools", "id": "tools/brew/mas", "label": "mas", "group": "Homebrew", "paths": [], "bytes": 0, "default": "skip", "reason": "no Linux bottle", "linux": "no" },
                ])
            );
        });
    }

    #[test]
    fn asks_homebrew_for_its_taps_and_formulae_alone_with_its_auto_update_off() {
        // The dump runs no other kind's lister and no brew update.
        block_on(async {
            let host = fake_host(FakeLaptop::new().which(&["brew"]).exec(BREW_DUMP, BREWFILE));
            detect_tools(&host).await.unwrap();
            let brew_calls: Vec<String> = host.calls().into_iter().filter(|c| c.contains("brew")).collect();
            assert_eq!(brew_calls, vec![format!("run {BREW_DUMP}")]);
        });
    }

    #[tokio::test]
    async fn asks_every_manager_at_once_and_its_rows_keep_their_order_whichever_answers_first() {
        let base = fake_host(FakeLaptop::new().which(&["brew", "cargo"]));
        let (tx, rx) = oneshot::channel::<()>();
        let together = Rc::new(Cell::new(false));
        let exec = RaceExec { cargo_asked: RefCell::new(Some(tx)), asked: RefCell::new(Some(rx)), together: together.clone() };
        let host = Host { exec: Rc::new(exec), ..base.host.clone() };
        let rows = detect_tools(&host).await.unwrap();
        assert!(together.get());
        let ids: Vec<String> = rows.into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["tools/brew/gh", "tools/cargo/bacon"]);
    }

    #[test]
    fn default_ticks_no_linux_bottle_locks_the_row_off_with_its_reason() {
        // An unknown Linux build starts unticked but stays tickable, a bottled formula starts ticked.
        block_on(async {
            let host =
                fake_host(FakeLaptop::new().which(&["brew"]).exec(BREW_DUMP, "brew \"mas\"\nbrew \"zingzy/tap/diskbloom\"\nbrew \"gh\"\n"));
            let rows = detect_tools(&host).await.unwrap();
            let got: Vec<serde_json::Value> = rows.iter().map(|r| json!([r.id, r.default, r.reason, r.linux])).collect();
            assert_eq!(
                got,
                vec![
                    json!(["tools/brew/mas", "skip", "no Linux bottle", "no"]),
                    json!(["tools/brew/zingzy/tap/diskbloom", "skip", null, "unknown"]),
                    json!(["tools/brew/gh", "bring", null, "yes"]),
                ]
            );
        });
    }

    #[test]
    fn a_formula_already_installed_on_a_linux_laptop_runs_on_linux() {
        // Whatever the snapshot says.
        block_on(async {
            let host = fake_host(
                FakeLaptop::new().platform(Platform::Linux).which(&["brew"]).exec(BREW_DUMP, "brew \"mas\"\nbrew \"oven-sh/bun/bun\"\n"),
            );
            let rows = detect_tools(&host).await.unwrap();
            let got: Vec<serde_json::Value> = rows.iter().map(|r| json!([r.id, r.default, r.linux])).collect();
            assert_eq!(got, vec![json!(["tools/brew/mas", "bring", "yes"]), json!(["tools/brew/oven-sh/bun/bun", "bring", "yes"])]);
        });
    }

    #[test]
    fn no_brew_means_no_dump_is_attempted() {
        block_on(async {
            let host = fake_host(FakeLaptop::new());
            assert!(detect_tools(&host).await.unwrap().is_empty());
            assert_eq!(host.calls(), Vec::<String>::new());
        });
    }

    #[test]
    fn nix_home_manager_config_is_brought_wholesale() {
        block_on(async {
            let host =
                fake_host(FakeLaptop::new().file("~/.config/home-manager/home.nix", 700).file("~/.config/home-manager/flake.nix", 300));
            let rows = detect_tools(&host).await.unwrap();
            assert_eq!(
                serde_json::to_value(&rows).unwrap(),
                json!([{ "rung": "tools", "id": "tools/nix-home-manager", "label": "Nix home-manager config", "paths": ["~/.config/home-manager"], "bytes": 1000, "default": "bring", "linux": "yes" }])
            );
        });
    }

    #[test]
    fn parses_npm_globals_dropping_npm_and_corepack() {
        let out = json!({ "dependencies": { "npm": { "version": "10.0.0" }, "corepack": { "version": "0.29.0" }, "pnpm": { "version": "9.12.0" }, "@anthropic-ai/claude-code": { "version": "1.0.0" } } }).to_string();
        assert_eq!(parse_npm_globals(&out), vec![pkg("pnpm", Some("9.12.0"), None), pkg("@anthropic-ai/claude-code", Some("1.0.0"), None)]);
        assert_eq!(parse_npm_globals("not json"), Vec::<Pkg>::new());
    }

    #[test]
    fn parses_pnpm_globals_one_project_object_per_global_dir() {
        // Its dependencies map, each with the folder it names.
        let out = json!([{ "path": "/Users/dev/Library/pnpm/global/5", "private": true, "dependencies": { "typescript": { "from": "typescript", "version": "5.6.2", "resolved": "https://x", "path": "/y" }, "@biomejs/biome": { "from": "@biomejs/biome", "version": "1.9.4" } } }]).to_string();
        assert_eq!(
            parse_pnpm_globals(&out),
            vec![pkg("typescript", Some("5.6.2"), Some("/y")), pkg("@biomejs/biome", Some("1.9.4"), None)]
        );
        assert_eq!(parse_pnpm_globals(&json!([{ "path": "/x", "private": true, "dependencies": {} }]).to_string()), Vec::<Pkg>::new());
        assert_eq!(parse_pnpm_globals("not json"), Vec::<Pkg>::new());
    }

    #[test]
    fn parses_bun_globals_a_header_line_then_a_tree_of_name_at_version() {
        // Scoped names included.
        let out = "/Users/dev/.bun/install/global node_modules (4)\n├── @types/node@26.4.1\n└── is-odd@3.0.1\n";
        assert_eq!(parse_bun_globals(out), vec![pkg("@types/node", Some("26.4.1"), None), pkg("is-odd", Some("3.0.1"), None)]);
        assert_eq!(parse_bun_globals(""), Vec::<Pkg>::new());
    }

    #[test]
    fn parses_pipx_uv_tool_and_cargo_listings() {
        let pipx = json!({ "venvs": { "httpie": { "metadata": { "main_package": { "package_version": "3.2.4" } } }, "black": { "metadata": {} } } }).to_string();
        assert_eq!(parse_pipx_list(&pipx), vec![pkg("httpie", Some("3.2.4"), None), pkg("black", None, None)]);
        assert_eq!(
            parse_uv_tool_list("ruff v0.6.3\n- ruff\nhttpx v0.27.0\n- httpx\n"),
            vec![pkg("ruff", Some("0.6.3"), None), pkg("httpx", Some("0.27.0"), None)]
        );
        assert_eq!(
            parse_cargo_installs("ripgrep v14.1.0:\n    rg\nbat v0.24.0 (/src/bat):\n    bat\n"),
            vec![pkg("ripgrep", Some("14.1.0"), None), pkg("bat", Some("0.24.0"), None)]
        );
    }

    #[test]
    fn reads_module_path_and_version_out_of_go_version_m() {
        let out =
            "/Users/dev/go/bin/gopls: go1.23.1\n\tpath\tgolang.org/x/tools/gopls\n\tmod\tgolang.org/x/tools/gopls\tv0.16.2\th1:abc=\n";
        assert_eq!(
            parse_go_version_m(out),
            Some(GoModule { path: "golang.org/x/tools/gopls".to_string(), version: "v0.16.2".to_string() })
        );
        assert_eq!(parse_go_version_m("garbage"), None);
    }

    #[test]
    fn global_installs_become_one_row_per_package_under_their_managers_group() {
        block_on(async {
            let host = fake_host(
                FakeLaptop::new()
                    .file("~/go/bin/gopls", 1)
                    .which(&["npm", "pnpm", "bun", "pipx", "uv", "cargo", "go"])
                    .exec("npm ls -g --depth=0 --json", &json!({ "dependencies": { "pnpm": { "version": "9.12.0" } } }).to_string())
                    .exec("pnpm ls -g --depth=0 --json", &json!([{ "path": "/g", "private": true, "dependencies": { "typescript": { "version": "5.6.2" } } }]).to_string())
                    .exec("bun pm ls -g", "/Users/dev/.bun/install/global node_modules (1)\n└── is-odd@3.0.1\n")
                    .exec("pipx list --json", &json!({ "venvs": { "httpie": { "metadata": { "main_package": { "package_version": "3.2.4" } } } } }).to_string())
                    .exec("uv tool list", "ruff v0.6.3\n- ruff\n")
                    .exec("cargo install --list", "ripgrep v14.1.0:\n    rg\n")
                    .exec(
                        "go version -m /Users/dev/go/bin/gopls",
                        "/Users/dev/go/bin/gopls: go1.23.1\n\tpath\tgolang.org/x/tools/gopls\n\tmod\tgolang.org/x/tools/gopls\tv0.16.2\th1:abc=\n",
                    ),
            );
            let rows = detect_tools(&host).await.unwrap();
            assert_eq!(
                serde_json::to_value(&rows).unwrap(),
                json!([
                    { "rung": "tools", "id": "tools/npm/pnpm", "label": "pnpm@9.12.0", "group": "npm globals", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "9.12.0" },
                    { "rung": "tools", "id": "tools/pnpm/typescript", "label": "typescript@5.6.2", "group": "pnpm globals", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "5.6.2" },
                    { "rung": "tools", "id": "tools/bun/is-odd", "label": "is-odd@3.0.1", "group": "bun globals", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "3.0.1" },
                    { "rung": "tools", "id": "tools/pipx/httpie", "label": "httpie 3.2.4", "group": "pipx", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "3.2.4" },
                    { "rung": "tools", "id": "tools/uv/ruff", "label": "ruff 0.6.3", "group": "uv tools", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "0.6.3" },
                    { "rung": "tools", "id": "tools/cargo/ripgrep", "label": "ripgrep 14.1.0", "group": "cargo installs", "paths": [], "bytes": 0, "default": "bring", "linux": "yes", "version": "14.1.0" },
                    { "rung": "tools", "id": "tools/go/gopls", "label": "gopls", "group": "Go binaries", "paths": ["golang.org/x/tools/gopls@v0.16.2"], "bytes": 0, "default": "bring", "linux": "yes", "version": "v0.16.2" },
                ])
            );
        });
    }

    #[test]
    fn npm_globals_under_a_node_prefix_npm_no_longer_uses_are_listed_too() {
        // A name under both is the current prefix's.
        block_on(async {
            let host = fake_host(
                FakeLaptop::new()
                    .file("/opt/homebrew/lib/node_modules/wrangler/package.json", 1)
                    .which(&["npm"])
                    .exec("npm prefix -g", "/Users/dev/.local\n")
                    .exec("npm ls -g --depth=0 --json", &json!({ "dependencies": { "bun": { "version": "1.4.0" } } }).to_string())
                    .exec(
                        "npm ls -g --depth=0 --json --prefix /opt/homebrew",
                        &json!({ "dependencies": { "npm": { "version": "11.17.0" }, "wrangler": { "version": "4.106.0" }, "bun": { "version": "1.3.0" } } }).to_string(),
                    ),
            );
            let rows = detect_tools(&host).await.unwrap();
            let got: Vec<[String; 2]> = rows.into_iter().map(|r| [r.id, r.label]).collect();
            assert_eq!(
                got,
                vec![
                    ["tools/npm/bun".to_string(), "bun@1.4.0".to_string()],
                    ["tools/npm/wrangler".to_string(), "wrangler@4.106.0".to_string()],
                ]
            );
            assert!(host.calls().contains(&"run npm ls -g --depth=0 --json --prefix /opt/homebrew".to_string()));
        });
    }

    #[test]
    fn the_prefix_npm_itself_uses_is_listed_once() {
        // A prefix with no globals is not asked.
        block_on(async {
            let host = fake_host(
                FakeLaptop::new()
                    .file("/opt/homebrew/lib/node_modules/wrangler/package.json", 1)
                    .which(&["npm"])
                    .exec("npm prefix -g", "/opt/homebrew\n")
                    .exec("npm ls -g --depth=0 --json", &json!({ "dependencies": { "wrangler": { "version": "4.106.0" } } }).to_string()),
            );
            let rows = detect_tools(&host).await.unwrap();
            let ids: Vec<String> = rows.into_iter().map(|r| r.id).collect();
            assert_eq!(ids, vec!["tools/npm/wrangler"]);
            let prefixed: Vec<String> = host.calls().into_iter().filter(|c| c.contains("--prefix")).collect();
            assert_eq!(prefixed, Vec::<String>::new());
        });
    }

    #[test]
    fn bun_with_an_empty_global_dir_lists_nothing() {
        // `bun pm ls -g` exits non-zero there.
        block_on(async {
            let host = fake_host(FakeLaptop::new().which(&["bun"]));
            assert!(detect_tools(&host).await.unwrap().is_empty());
            assert_eq!(host.calls(), vec!["run bun pm ls -g".to_string()]);
        });
    }

    const LONG_MODULE: &str = "github.com/some-organisation/some-very-long-repository-name/cmd/tooling/wsp-go";

    #[test]
    fn a_go_binary_is_labelled_by_name_the_module_path_is_its_first_detail_line() {
        block_on(async {
            for (name, path, version) in [("gopls", "golang.org/x/tools/gopls", "v0.16.2"), ("wsp-go", LONG_MODULE, "v1.4.0")] {
                assert_eq!(LONG_MODULE.len(), 78);
                let host = fake_host(FakeLaptop::new().file(&format!("~/go/bin/{name}"), 1).which(&["go"]).exec(
                    &format!("go version -m /Users/dev/go/bin/{name}"),
                    &format!("x\n\tpath\t{path}\n\tmod\t{path}\t{version}\th1:abc=\n"),
                ));
                let rows = detect_tools(&host).await.unwrap();
                assert_eq!(
                    serde_json::to_value(&rows).unwrap(),
                    json!([{ "rung": "tools", "id": format!("tools/go/{name}"), "label": name, "group": "Go binaries", "paths": [format!("{path}@{version}")], "bytes": 0, "default": "bring", "linux": "yes", "version": version }])
                );
            }
        });
    }

    #[test]
    fn what_apt_has_that_a_person_chose_is_one_row_per_package() {
        // Under its own group, installable on the image.
        block_on(async {
            let rows = detect_tools(&fake_host(apt_laptop())).await.unwrap();
            assert_eq!(
                serde_json::to_value(&rows).unwrap(),
                json!([{ "rung": "tools", "id": "tools/apt/direnv", "label": "direnv", "group": "apt packages", "paths": [], "bytes": 0, "default": "bring", "linux": "yes" }])
            );
        });
    }

    #[test]
    fn files_no_apt_row_for_a_tool_the_catalog_carries() {
        // The build reads no road off an apt row and that row would stand in for the catalog's; a manager whose road
        // it can read keeps such a row.
        block_on(async {
            let both = apt_laptop()
                .which(&["npm"])
                .file("/var/lib/dpkg/info/tmux.list", "/usr/bin/tmux\n")
                .exec("apt-mark showmanual", "direnv\ntmux\n")
                .exec("dpkg-query -Wf ${Package} ${Priority}\n", "direnv optional\ntmux optional\n")
                .exec("npm ls -g --depth=0 --json", &json!({ "dependencies": { "tmux": { "version": "1.0.0" } } }).to_string());
            let ids: Vec<String> = detect_tools(&fake_host(both)).await.unwrap().into_iter().map(|r| r.id).collect();
            assert!(ids.contains(&"tools/apt/direnv".to_string()));
            assert!(!ids.contains(&"tools/apt/tmux".to_string()));
            // npm's road is read off its row, so a catalog tool it has keeps one and the build installs it by npm.
            assert!(ids.contains(&"tools/npm/tmux".to_string()));
        });
    }

    #[test]
    fn a_computer_with_no_apt_is_asked_nothing_about_it() {
        // So a Mac runs no dpkg.
        block_on(async {
            let mut laptop = apt_laptop().exec(BREW_DUMP, "brew \"gh\"\n");
            laptop.which = vec!["brew".to_string()];
            let host = fake_host(laptop);
            let ids: Vec<String> = detect_tools(&host).await.unwrap().into_iter().map(|r| r.id).collect();
            assert_eq!(ids, vec!["tools/brew/gh"]);
            let asked: Vec<String> = host.calls().into_iter().filter(|c| c.contains("apt-mark") || c.contains("dpkg")).collect();
            assert_eq!(asked, Vec::<String>::new());
        });
    }

    #[test]
    fn a_go_binary_without_module_info_is_offered_unticked() {
        block_on(async {
            let host = fake_host(FakeLaptop::new().file("~/go/bin/mystery", 1).which(&["go"]));
            let rows = detect_tools(&host).await.unwrap();
            assert_eq!(
                serde_json::to_value(&rows).unwrap(),
                json!([{ "rung": "tools", "id": "tools/go/mystery", "label": "mystery (no module info)", "group": "Go binaries", "paths": [], "bytes": 0, "default": "skip", "linux": "yes" }])
            );
        });
    }
}

/// Rules of spec/apt_tools.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use super::*;
    use wsp_collect::detect::tools::{js_global_folders, package_commands, Pkg};

    fn pkg(name: &str, version: Option<&str>, path: Option<&str>) -> Pkg {
        Pkg { name: name.into(), version: version.map(Into::into), path: path.map(Into::into) }
    }

    #[test]
    fn json_listings_keep_javascripts_key_order() {
        // JavaScript lists an integer-like key first, whatever the text's order (rule 4.26).
        let npm = r#"{"dependencies":{"b":{"version":"1"},"2":{"version":"2"},"a":{}}}"#;
        assert_eq!(parse_npm_globals(npm), vec![pkg("2", Some("2"), None), pkg("b", Some("1"), None), pkg("a", None, None)]);
        let pipx = r#"{"venvs":{"z":{},"10":{"metadata":{"main_package":{"package_version":"3"}}}}}"#;
        assert_eq!(parse_pipx_list(pipx), vec![pkg("10", Some("3"), None), pkg("z", None, None)]);
    }

    #[test]
    fn a_listing_that_is_not_the_right_json_shape_is_empty() {
        assert_eq!(parse_npm_globals("null"), vec![]);
        assert_eq!(parse_npm_globals("[]"), vec![]);
        assert_eq!(parse_npm_globals(r#"{"dependencies":[]}"#), vec![]);
        assert_eq!(parse_pnpm_globals(r#"{"dependencies":{}}"#), vec![]);
        // A version that is not a string is left off; an element that is not an object is skipped.
        assert_eq!(parse_pnpm_globals(r#"[1,{"dependencies":{"x":{"version":2,"path":"/p"}}}]"#), vec![pkg("x", None, Some("/p"))]);
    }

    #[test]
    fn js_global_folders_lists_each_root_then_pnpms_own_paths() {
        let pnpm = r#"[{"dependencies":{"x":{"path":"/p1"}}},{"dependencies":{"x":{"path":"/p2"},"y":{}}}]"#;
        let host =
            fake_host(FakeLaptop::new().exec("npm root -g", "/usr/local/lib/node_modules\n").exec("pnpm ls -g --depth=0 --json", pnpm));
        let folders = block_on(js_global_folders(&host));
        // npm's own root first, the fixed prefixes after it with the repeat dropped, bun's last, then pnpm's paths.
        assert_eq!(
            folders("x"),
            vec![
                "/usr/local/lib/node_modules/x",
                "/opt/homebrew/lib/node_modules/x",
                "/Users/dev/.bun/install/global/node_modules/x",
                "/p1",
                "/p2"
            ]
        );
        assert_eq!(
            folders("y"),
            vec!["/usr/local/lib/node_modules/y", "/opt/homebrew/lib/node_modules/y", "/Users/dev/.bun/install/global/node_modules/y"]
        );
    }

    #[test]
    fn package_commands_reads_bin_off_every_folder_once_each() {
        let host = fake_host(
            FakeLaptop::new()
                .file("/a/package.json", r#"{"bin":"cli.js"}"#)
                .file("/b/package.json", r#"{"bin":{"two":"t","one":"o"}}"#)
                .file("/c/package.json", "nope"),
        );
        let folders: Vec<String> = ["/a", "/b", "/c", "/d", "/a"].iter().map(|s| s.to_string()).collect();
        assert_eq!(block_on(package_commands(&host, "@scope/tool", &folders)), vec!["tool", "two", "one"]);
        let reads: Vec<String> = host.calls().into_iter().filter(|c| c.starts_with("read ")).collect();
        assert_eq!(
            reads,
            vec!["read /a/package.json", "read /b/package.json", "read /c/package.json", "read /d/package.json", "read /a/package.json"]
        );
    }
}

/// Every case of tests/fixtures/oracle/w2/apt_tools.json (oracle/w2-apt_tools.oracle.ts): the TS's own answers on the
/// edges spec apt_tools's wave-2 rules name (W1 to W16). Each case names what it pins.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, strs, w2};
    use serde_json::Value;
    use wsp_collect::detect::apt::{apt_packages, apt_sizes, parse_apt_manual, parse_dpkg_fields};
    use wsp_collect::detect::tools::{js_global_folders, package_commands};

    fn fixture() -> Value {
        w2("apt_tools")
    }

    fn arg(case: &Value, i: usize) -> &str {
        case["args"][i].as_str().unwrap()
    }

    fn pkg_json(p: &Pkg) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("name".into(), json!(p.name));
        if let Some(v) = &p.version {
            o.insert("version".into(), json!(v));
        }
        if let Some(v) = &p.path {
            o.insert("path".into(), json!(v));
        }
        Value::Object(o)
    }

    fn pkgs_json(p: &[Pkg]) -> Value {
        Value::Array(p.iter().map(pkg_json).collect())
    }

    fn pairs(m: &indexmap::IndexMap<String, String>) -> Value {
        Value::Array(m.iter().map(|(k, v)| json!([k, v])).collect())
    }

    #[test]
    fn dpkg_fields_and_the_manual_list_read_as_the_ts_read_them() {
        let f = fixture();
        for case in cases(&f, "parse_dpkg_fields") {
            assert_eq!(pairs(&parse_dpkg_fields(arg(case, 0))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "parse_apt_manual") {
            assert_eq!(json!(parse_apt_manual(arg(case, 0))), case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn apt_packages_keeps_runs_and_reads_what_the_ts_did() {
        for case in cases(&fixture(), "apt_packages") {
            let h = host(&case["laptop"]);
            match block_on(apt_packages(&h)) {
                Ok(got) => assert_eq!(pkgs_json(&got), case["out"], "{}", case["name"]),
                Err(e) => assert_eq!(json!(e.message), case["error"], "{}", case["name"]),
            }
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    #[test]
    fn apt_sizes_reads_each_field_as_javascript_s_number_in_the_order_asked() {
        for case in cases(&fixture(), "apt_sizes") {
            let h = host(&case["laptop"]);
            let names = strs(&case["args"][0]);
            let got: Vec<(String, f64)> = block_on(apt_sizes(&h, &names)).into_iter().collect();
            let want: Vec<(String, f64)> =
                case["out"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_f64().unwrap())).collect();
            assert_eq!(got, want, "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    #[test]
    fn every_listing_parser_answers_what_the_ts_answered() {
        let f = fixture();
        for case in cases(&f, "parse_brewfile") {
            let got: Vec<Value> = parse_brewfile(arg(case, 0))
                .iter()
                .map(|l| json!({ "kind": if l.kind == BrewKind::Tap { "tap" } else { "brew" }, "name": l.name }))
                .collect();
            assert_eq!(Value::Array(got), case["out"], "{}", case["name"]);
        }
        let parsers: [(&str, fn(&str) -> Vec<Pkg>); 6] = [
            ("parse_npm_globals", parse_npm_globals),
            ("parse_pnpm_globals", parse_pnpm_globals),
            ("parse_pipx_list", parse_pipx_list),
            ("parse_uv_tool_list", parse_uv_tool_list),
            ("parse_cargo_installs", parse_cargo_installs),
            ("parse_bun_globals", parse_bun_globals),
        ];
        for (key, parse) in parsers {
            for case in cases(&f, key) {
                assert_eq!(pkgs_json(&parse(arg(case, 0))), case["out"], "{key}: {}", case["name"]);
            }
        }
        for case in cases(&f, "parse_go_version_m") {
            let got = parse_go_version_m(arg(case, 0)).map(|m| json!({ "path": m.path, "version": m.version })).unwrap_or(Value::Null);
            assert_eq!(got, case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn global_folders_and_package_commands_answer_and_read_what_the_ts_did() {
        let f = fixture();
        for case in cases(&f, "js_global_folders") {
            let h = host(&case["laptop"]);
            let folders = block_on(js_global_folders(&h));
            assert_eq!(json!(folders(arg(case, 0))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
        for case in cases(&f, "package_commands") {
            let h = host(&case["laptop"]);
            let got = block_on(package_commands(&h, arg(case, 0), &strs(&case["args"][1])));
            assert_eq!(json!(got), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    /// Rows compare whole; calls compare in order only where one manager ran (the TS interleaves managers, so a case
    /// with several has no `calls`).
    #[test]
    fn detect_tools_writes_the_rows_the_ts_wrote() {
        for case in cases(&fixture(), "detect_tools") {
            let h = host(&case["laptop"]);
            match block_on(detect_tools(&h)) {
                Ok(rows) => {
                    let got: Vec<Value> = rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect();
                    assert_eq!(Value::Array(got), case["out"], "{}", case["name"]);
                }
                Err(e) => assert_eq!(json!(e.message), case["error"], "{}", case["name"]),
            }
            if !case["calls"].is_null() {
                assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
            }
        }
    }
}
