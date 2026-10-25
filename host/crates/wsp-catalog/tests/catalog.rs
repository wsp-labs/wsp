// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/catalog.test.ts.
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use futures::executor::block_on;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_catalog::catalog::{
    agent_name, base_entry_for, base_floor, base_note, catalog, catalog_agents, catalog_entry, catalog_id_of_row, catalog_tool_for, catalog_tool_for_dependency, default_agent,
    guest_env, install_after, install_line, install_shown, login_row, login_rows, size_bytes, size_method_words, smoke_of, AgentEntry, Brings, CatalogEntry, EntryKind, Evidence,
    InstallShown, LatestSource, RoadEvidence, Size, SizeMethod, ToolEntry, UsageLog, HISTORY_FORMATS,
};
use wsp_catalog::catalog_data::agent_modules;
use wsp_catalog::git_hosts::{runs_threads, THREAD_AGENTS};
use wsp_catalog::linux_casks::{linux_casks, CURSOR, GCLOUD, KUBECTL};
use wsp_words::words::image::pin_mismatch_line;
use wsp_wire::types::agents_report::AgentSignInKind;
use wsp_wire::views::golden_image::ToolPinArch as Arch;
use wsp_words::words::computer::Platform;
use wsp_wire::views::golden_image::ToolPin;
use wsp_wire::types::daemon_contract::HOMEBREW_HOME;
use wsp_wire::types::daemon_contract::HOMEBREW_PREFIX;
use wsp_wire::types::daemon_contract::PNPM_HOME;
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_catalog::release_pins::RELEASE_PINS;
use wsp_catalog::road_modules::{
    fixes_version, install_homes, reads_row_road, road_module, road_module_named, road_step, RoadNote, ToolRow, APT_INDEX, APT_UPDATE, BREW_ENV, BREW_PREFIX, CURL_NET, HOMEBREW_STEP,
    NET_READ_S, NET_RETRIES,
};
use wsp_catalog::roads::{
    node_install_script, node_release, pin_check_line, standing_pin, unpinned, version_of, InstallRoad, PackageRoad, ReleaseAsset, ReleaseAssets, RoadName, CLAUDE_CODE, CLAUDE_CONFIG_DIR,
    CLAUDE_INSTALL, DOCKER_INSTALL, GOLDEN_SETUP, GUEST_HOME, LOCAL_BIN, PLAYWRIGHT_VERSION, ROADS, UV_INSTALL, YARN_VERSION,
};
use wsp_catalog::signin::{has_login, keys_id_of, keys_row_of, login_id_of, mints_token, no_sign_in, KeyFiles, LoginSource, NoSignIn, SignIn, SignInKind};
use wsp_catalog::signin_rows::SIGN_IN_ROWS;
use wsp_catalog::skills::{PluginRoad, SeedReader};

const WORKSPACE_OVERLAID: [&str; 5] = ["/usr", "/etc", "/opt", "/var", "/srv"];
const SHARED_TOOL_ROOTS: [&str; 1] = [HOMEBREW_HOME];
const AGENT_STORE_ENVS: [&str; 2] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME"];
const AGENT_BINS: [&str; 11] = ["amp", "claude", "codex", "crush", "cursor-agent", "gemini", "goose", "hermes", "opencode", "pi", "qwen"];

fn tools_path() -> String {
    format!(
        "/root/.local/bin:/usr/local/sbin:/usr/local/bin:{HOMEBREW_PREFIX}/bin:{HOMEBREW_PREFIX}/sbin:/root/go/bin:/root/.cargo/bin:{PNPM_HOME}:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    )
}

const STUB_RUNNER_SH: &str = r##"#!/bin/sh
body="${0%/*}/.${0##*/}.stub"
[ -f "$body" ] || { echo "stub-runner: no script at $body" >&2; exit 127; }
IFS= read -r first < "$body"
case $first in "#!"*) exec ${first#??} "$body" "$@" ;; esac
exec /bin/sh "$body" "$@"
"##;

const SHA256SUM_STUB: &str = r#"#!/bin/sh
hash() {
  if [ -x /usr/bin/sha256sum ]; then /usr/bin/sha256sum "$1"; else /usr/bin/shasum -a 256 "$1"; fi | cut -d' ' -f1
}
if [ "$1" = -c ]; then
  bad=0
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    want="${line%%  *}"
    f="${line#*  }"
    if [ "$(hash "$f")" = "$want" ]; then echo "$f: OK"; else echo "$f: FAILED"; bad=$((bad + 1)); fi
  done
  [ "$bad" -eq 0 ] || { echo "sha256sum: WARNING: $bad computed checksum did NOT match" >&2; exit 1; }
  exit 0
fi
echo "$(hash "$1")  $1"
"#;

fn stub_runner() -> &'static Path {
    static RUNNER: OnceLock<PathBuf> = OnceLock::new();
    RUNNER.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("wsp-catalog-stub-runner-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stub-runner.sh");
        fs::write(&path, STUB_RUNNER_SH).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
        loop {
            match Command::new(&path).output() {
                Err(e) if e.raw_os_error() == Some(26) => std::thread::sleep(std::time::Duration::from_millis(20)),
                _ => break,
            }
        }
        path
    })
}

fn write_stub(path: &Path, script: &str) {
    let name = path.file_name().unwrap().to_str().unwrap();
    fs::write(path.parent().unwrap().join(format!(".{name}.stub")), script).unwrap();
    let _ = fs::remove_file(path);
    std::os::unix::fs::symlink(stub_runner(), path).unwrap();
}

fn sha256_hex(path: &Path) -> String {
    let out = if Path::new("/usr/bin/sha256sum").exists() {
        Command::new("/usr/bin/sha256sum").arg(path).output().unwrap()
    } else {
        Command::new("/usr/bin/shasum").args(["-a", "256"]).arg(path).output().unwrap()
    };
    String::from_utf8(out.stdout).unwrap().split_whitespace().next().unwrap().to_string()
}

fn stdout_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn run_install(dir: &Path, script: &str) -> Output {
    let tools = dir.join("tools");
    fs::create_dir(&tools).unwrap();
    write_stub(&tools.join("sha256sum"), SHA256SUM_STUB);
    Command::new("bash")
        .args(["--noprofile", "--norc", "-c", script])
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
        .env("HOME", dir)
        .output()
        .unwrap()
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

fn entry(id: &str) -> &'static CatalogEntry {
    catalog_entry(id).unwrap_or_else(|| panic!("no catalog entry {id}"))
}

fn agent(id: &str) -> &'static AgentEntry {
    entry(id).as_agent().unwrap_or_else(|| panic!("{id} is no agent"))
}

fn tool(id: &str) -> &'static ToolEntry {
    entry(id).as_tool().unwrap_or_else(|| panic!("{id} is no tool"))
}

fn line_of(e: &CatalogEntry) -> String {
    install_line(e).unwrap()
}

fn source_of(e: &CatalogEntry) -> &Evidence {
    match e {
        CatalogEntry::Agent(a) => &a.source,
        CatalogEntry::Tool(t) => &t.source,
    }
}

fn tool_id(pkg: &str) -> Option<&'static str> {
    catalog_tool_for(pkg).map(|t| t.id.as_str())
}

fn base_id(pkg: &str) -> Option<&'static str> {
    base_entry_for(pkg).map(|t| t.id.as_str())
}

fn s(text: &str) -> Option<String> {
    Some(text.to_string())
}

fn pin_of(tag: &str, sha256: &str) -> ToolPin {
    ToolPin { tag: tag.to_string(), sha256: Some(sha256.to_string()), arch: None, latest: None }
}

fn tag_pin(tag: &str) -> ToolPin {
    ToolPin { tag: tag.to_string(), sha256: None, arch: None, latest: None }
}

fn package_road(package: &str, version: Option<&str>, pin: Option<ToolPin>) -> PackageRoad {
    PackageRoad { package: package.to_string(), version: version.map(str::to_string), pin }
}

fn npm_road(package: &str, version: Option<&str>, pin: Option<ToolPin>) -> InstallRoad {
    InstallRoad::Npm { package: package_road(package, version, pin), ignore_scripts: None }
}

fn release_road(repo: &str, version: Option<&str>, pin: Option<ToolPin>) -> InstallRoad {
    InstallRoad::Release { repo: Some(repo.to_string()), version: version.map(str::to_string), assets: None, pin, go: None }
}

fn go_road(module: Option<&str>, version: Option<&str>, pin: Option<ToolPin>) -> InstallRoad {
    InstallRoad::Go { module: module.map(str::to_string), version: version.map(str::to_string), pin }
}

fn script_road(script: &str, version: Option<&str>) -> InstallRoad {
    InstallRoad::Script { script: script.to_string(), version: version.map(str::to_string), pin: None, bins: None }
}

fn sample_road(name: RoadName) -> InstallRoad {
    match name {
        RoadName::Brew => InstallRoad::Brew { formula: "go".to_string(), pin: None },
        RoadName::Npm => npm_road("wrangler", None, None),
        RoadName::Pnpm => InstallRoad::Pnpm(package_road("wrangler", None, None)),
        RoadName::Bun => InstallRoad::Bun(package_road("wrangler", None, None)),
        RoadName::Uv => InstallRoad::Uv(package_road("ruff", None, None)),
        RoadName::Pipx => InstallRoad::Pipx(package_road("black", None, None)),
        RoadName::Cargo => InstallRoad::Cargo(package_road("bat", None, None)),
        RoadName::Go => go_road(Some("golang.org/x/tools/gopls"), None, None),
        RoadName::Release => release_road("cli/cli", Some("v2.86.0"), None),
        RoadName::Vendor => InstallRoad::Vendor { cask: GCLOUD.clone(), version: None, pin: None },
        RoadName::Apt => InstallRoad::Apt { packages: vec!["tmux".to_string()], pin: None },
        RoadName::Script => script_road("x", None),
    }
}

fn install(road: &InstallRoad, bin: &str) -> Result<String, RoadNote> {
    road_module(road).install(road, bin, None)
}

fn uninstall(road: &InstallRoad, bin: &str) -> Result<String, RoadNote> {
    road_module(road).uninstall(road, bin, None)
}

fn ok(text: &str) -> Result<String, RoadNote> {
    Ok(text.to_string())
}

fn note(text: &str) -> Result<String, RoadNote> {
    Err(RoadNote(text.to_string()))
}

fn tool_row(name: &str, version: Option<&str>) -> ToolRow {
    ToolRow { name: name.to_string(), version: version.map(str::to_string), paths: vec![], label: name.to_string() }
}

fn sentences(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut current = String::new();
    for word in text.split(' ') {
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        if word.ends_with('.') {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

struct SeedFiles {
    files: HashMap<String, String>,
    reads: RefCell<Vec<String>>,
}

impl SeedReader for SeedFiles {
    async fn read(&self, rel: &str) -> Option<String> {
        self.reads.borrow_mut().push(rel.to_string());
        self.files.get(rel).cloned()
    }
}

struct SeedText(&'static str);

impl SeedReader for SeedText {
    async fn read(&self, _rel: &str) -> Option<String> {
        Some(self.0.to_string())
    }
}

mod catalog {
    use super::*;

    #[test]
    fn the_default_agent_is_the_first_entry_and_it_is_an_agent_with_a_context_module() {
        assert!(std::ptr::eq(default_agent(), catalog_agents()[0]));
        assert_eq!(default_agent().id, "claude");
        assert!(default_agent().context.is_some());
    }

    #[test]
    fn marks_the_agents_that_print_their_plan_s_limits_in_a_turn_and_names_opencode_s_u() {
        let agents = catalog_agents();
        assert_eq!(agents.iter().filter(|a| a.prints_limits == Some(true)).map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["claude", "codex"]);
        assert_eq!(
            agents.iter().filter(|a| a.usage_log.is_some()).map(|a| (a.id.as_str(), a.usage_log.clone().unwrap())).collect::<Vec<_>>(),
            vec![("opencode", UsageLog { format: "opencode-sqlite".to_string(), root: "~/.local/share/opencode/opencode.db".to_string() })]
        );
    }

    #[test]
    fn names_a_session_history_for_the_agents_with_a_reader_in_a_known_format_under_a_h() {
        let agents = catalog_agents();
        assert_eq!(agents.iter().filter(|a| a.history.is_some()).map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["claude", "codex", "hermes"]);
        for a in &agents {
            let Some(history) = &a.history else { continue };
            assert!(HISTORY_FORMATS.contains(&history.format), "{}", a.id);
            assert!(re(r"^~/\.").is_match(&history.root), "{}", a.id);
        }
    }

    #[test]
    fn finds_the_tool_a_package_name_stands_for_by_id_command_road_name_cover_or_brough() {
        assert_eq!(tool_id("rg"), Some("ripgrep"));
        assert_eq!(tool_id("cli/cli"), None);
        assert_eq!(tool_id("awscli"), Some("aws"));
        assert_eq!(tool_id("npm"), Some("node"));
        assert_eq!(tool_id("openjdk@21"), Some("java"));
        assert_eq!(tool_id("cargo"), Some("rust"));
        // Homebrew's own name for the same toolchain, and the installer's: both are the rust row, so neither plans a second install.
        assert_eq!(tool_id("rustup"), Some("rust"));
        assert_eq!(tool_id("rustup-init"), Some("rust"));
        assert_eq!(base_id("cargo"), None);
        assert_eq!(base_id("npm"), None);
        assert_eq!(tool_id("claude"), None);
    }

    #[test]
    fn gives_every_entry_its_own_id() {
        let ids: Vec<&str> = catalog().iter().map(|e| e.id()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len());
        assert_eq!(catalog_entry("gh").map(|e| e.name()), Some("GitHub CLI"));
        assert!(catalog_entry("nothing").is_none());
    }

    #[test]
    fn names_an_entry_as_the_catalog_does_and_an_id_it_does_not_know_as_itself() {
        assert_eq!(agent_name("claude"), "Claude Code");
        assert_eq!(agent_name("gemini"), "Gemini CLI");
        assert_eq!(agent_name("gh"), "GitHub CLI");
        assert_eq!(agent_name("zed"), "zed");
    }

    #[test]
    fn a_golden_s_env_for_an_agent_points_it_at_its_guest_state_home_and_its_image_s_pl() {
        let want: IndexMap<String, String> = IndexMap::from([
            ("CLAUDE_CONFIG_DIR".to_string(), CLAUDE_CONFIG_DIR.to_string()),
            ("CLAUDE_CODE_PLUGIN_SEED_DIR".to_string(), "/root/.claude-plugin-seed".to_string()),
        ]);
        assert_eq!(guest_env(agent("claude")), want);
        for a in catalog_agents().into_iter().skip(1) {
            assert_eq!(guest_env(a), IndexMap::<String, String>::new(), "{}", a.id);
        }
    }

    #[test]
    fn claude_code_s_plugin_seed_only_a_plugin_its_marketplace_pins_to_a_commit_of_its() {
        let seed = PluginRoad::Claude.seed().unwrap();
        let record = |install_path: String| json!([{ "scope": "user", "installPath": install_path, "gitCommitSha": "0123456789abcdef0123456789abcdef01234567" }]);
        let dir = seed.dir();
        let files: HashMap<String, String> = HashMap::from([
            (
                "installed_plugins.json".to_string(),
                json!({
                    "version": 2,
                    "plugins": {
                        "vercel@claude-plugins-official": record(format!("{dir}/cache/claude-plugins-official/vercel/0.50.0")),
                        "code-simplifier@claude-plugins-official": record(format!("{dir}/cache/claude-plugins-official/code-simplifier/1.0.0")),
                        "pinned@claude-plugins-official": record(format!("{dir}/cache/claude-plugins-official/pinned/882e66c26986")),
                        "x@gone": record(format!("{dir}/cache/gone/x/1.0.0")),
                    },
                })
                .to_string(),
            ),
            (
                "marketplaces/claude-plugins-official/.claude-plugin/marketplace.json".to_string(),
                json!({
                    "plugins": [
                        { "name": "vercel", "source": { "source": "url", "url": "https://github.com/acme/vercel-plugin.git", "sha": "882e66c2698689324c341b7bb8fd8beb92c27bf6" } },
                        { "name": "code-simplifier", "source": "./plugins/code-simplifier" },
                        { "name": "pinned", "source": { "source": "url", "url": "https://github.com/acme/pinned.git", "sha": "882e66c2698689324c341b7bb8fd8beb92c27bf6" } },
                    ],
                })
                .to_string(),
            ),
        ]);
        let reader = SeedFiles { files, reads: RefCell::new(vec![]) };
        assert_eq!(block_on(seed.links(&reader)), vec![("cache/claude-plugins-official/vercel/882e66c26986".to_string(), "0.50.0".to_string())]);
        assert_eq!(reader.reads.borrow().iter().filter(|r| r.contains("claude-plugins-official")).count(), 1);
        assert_eq!(block_on(seed.links(&SeedText("not json"))), Vec::<(String, String)>::new());
        // The rule keys on the install layout of the Claude Code it was measured on; a pin bump measures it again.
        assert_eq!(CLAUDE_CODE.version, "2.1.296", "re-measure the seed link rule");
        // An index an older image copied names another computer's folders, and every build takes it off, seed or not.
        assert_eq!(
            PluginRoad::Claude.copied_before(),
            vec![format!("{CLAUDE_CONFIG_DIR}/plugins/known_marketplaces.json"), format!("{CLAUDE_CONFIG_DIR}/plugins/installed_plugins.json")]
        );
        assert!(!seed.build("x@acme", "acme/plugins").contains("rm -f"));
        let settings = r#"{
  "model": "opus",
  "enabledPlugins": { "a@used": true, "b@off": false },
  "extraKnownMarketplaces": {
    "used": { "source": { "source": "github", "repo": "acme/used" } },
    "off": { "source": { "source": "github", "repo": "acme/off" } }
  }
}
"#;
        let used: Value = serde_json::from_str(&PluginRoad::Claude.used_only(settings)).unwrap();
        assert_eq!(
            used,
            json!({ "model": "opus", "enabledPlugins": { "a@used": true, "b@off": false }, "extraKnownMarketplaces": { "used": { "source": { "source": "github", "repo": "acme/used" } } } })
        );
        for same in [r#"{"model":"opus"}"#, "{ not json", r#"{"extraKnownMarketplaces":{"used":{}},"enabledPlugins":{"a@used":true}}"#] {
            assert_eq!(PluginRoad::Claude.used_only(same), same);
        }
    }

    #[test]
    fn names_the_github_repos_the_marketplace_index_fetches_from_and_nothing_a_git_conf() {
        let known = json!({
            "a": { "source": { "source": "github", "repo": "acme/plugins" } },
            "b": { "source": { "source": "git", "url": "https://lab.example/mem.git" } },
            "c": { "source": { "source": "directory", "path": "/home/x/plugins" } },
            "d": { "source": { "source": "github", "repo": "acme/x y\n" } },
            "e": {},
        });
        assert_eq!(PluginRoad::Claude.github_repos(&known.to_string()), vec!["acme/plugins".to_string()]);
        for none in ["{ not json", "[]", "null"] {
            assert_eq!(PluginRoad::Claude.github_repos(none), Vec::<String>::new(), "{none}");
        }
    }

    #[test]
    fn claude_code_s_plugin_indexes_never_travel_they_name_this_computer_s_plugin_folde() {
        let claude = agent("claude");
        let travelling: Vec<&String> = claude.config_paths.iter().chain(claude.volatile.iter().flatten()).filter(|p| p.contains("/plugins/")).collect();
        assert!(travelling.is_empty(), "{travelling:?}");
    }

    #[test]
    fn sends_every_entry_down_a_known_road_with_its_argument() {
        for e in catalog() {
            let road = e.install_road();
            let id = e.id();
            assert!(ROADS.contains(&road.name()), "{id}");
            match road {
                InstallRoad::Brew { formula, .. } => assert!(re(r"^[\w@.+-]+$").is_match(formula), "{id}"),
                InstallRoad::Npm { package, .. } => assert!(re(r"^(@[\w.-]+/)?[\w.-]+$").is_match(&package.package), "{id}"),
                InstallRoad::Pnpm(package) | InstallRoad::Bun(package) | InstallRoad::Uv(package) | InstallRoad::Pipx(package) | InstallRoad::Cargo(package) => {
                    assert!(re(r"^(@[\w.-]+/)?[\w.-]+$").is_match(&package.package), "{id}")
                }
                InstallRoad::Go { module, .. } => assert!(re(r"^[\w.-]+\.[a-z]+/").is_match(module.as_deref().unwrap()), "{id}"),
                InstallRoad::Release { repo, .. } => assert!(re(r"^[\w.-]+/[\w.-]+$").is_match(repo.as_deref().unwrap()), "{id}"),
                InstallRoad::Vendor { cask, .. } => assert!(linux_casks().contains(&cask), "{id}"),
                InstallRoad::Apt { packages, .. } => assert!(!packages.is_empty(), "{id}"),
                InstallRoad::Script { script, .. } => assert_ne!(script, "", "{id}"),
            }
        }
    }

    #[test]
    fn names_the_artifact_and_the_sum_before_anything_runs_every_release_row_a_pinned_t() {
        let releases: Vec<&CatalogEntry> = catalog().iter().filter(|e| e.install_road().name() == RoadName::Release).collect();
        assert_eq!(
            releases.iter().map(|e| e.id()).collect::<Vec<_>>(),
            vec!["crush", "goose", "gh", "cloudflared", "fly", "supabase", "doppler", "yq", "golangci-lint", "mise", "git-delta", "bazel"]
        );
        let mut repos = vec![];
        for e in &releases {
            let id = e.id();
            let InstallRoad::Release { repo, version, assets, .. } = e.install_road() else { panic!("{id} is no release row") };
            let repo = repo.clone().unwrap();
            let pin = RELEASE_PINS.get(&repo).unwrap_or_else(|| panic!("{id} has no pin"));
            assert_eq!(version.as_deref(), Some(pin.tag.as_str()), "{id}");
            assert_eq!(assets.as_ref(), Some(&pin.assets), "{id}");
            let assets = assets.as_ref().unwrap();
            for (arch, asset) in [("x86_64", &assets.x86_64), ("aarch64", &assets.aarch64)] {
                let asset = asset.as_ref().unwrap_or_else(|| panic!("{id} {arch}"));
                assert!(re(r"^[\w.+-]+$").is_match(&asset.name), "{id} {arch}");
                assert!(re(r"^[0-9a-f]{64}$").is_match(&asset.sha256), "{id} {arch}");
            }
            repos.push(repo);
        }
        let mut pinned: Vec<String> = RELEASE_PINS.keys().cloned().collect();
        pinned.sort();
        repos.sort();
        assert_eq!(pinned, repos);
        for cask in linux_casks() {
            assert!(re(r"^v?\d").is_match(&cask.version), "{}", cask.bin);
            assert!(re(r"^[0-9a-f]{64}$").is_match(&cask.sha256.x86_64), "{}", cask.bin);
            assert!(re(r"^[0-9a-f]{64}$").is_match(&cask.sha256.aarch64), "{}", cask.bin);
        }
        for e in catalog().iter().filter(|e| e.install_road().name() == RoadName::Vendor) {
            let InstallRoad::Vendor { cask, version, .. } = e.install_road() else { unreachable!() };
            assert_eq!(version.as_deref(), Some(cask.version.as_str()), "{}", e.id());
        }
        // Nothing any row runs asks a source what it is serving today.
        for e in catalog() {
            let line = line_of(e);
            for moving in ["releases/latest", "stable.txt", "components-2.json", "@latest"] {
                assert!(!line.contains(moving), "{} reads {moving}", e.id());
            }
        }
    }

    #[test]
    fn one_rule_says_which_pin_stands_and_one_line_checks_it_the_sum_the_tag_and_both_s() {
        let pin = pin_of("v2.86.0", &"d".repeat(64));
        // A pin stands while the road names its tag or none; a version past it, or no pin, leaves nothing to check.
        assert_eq!(standing_pin(&release_road("cli/cli", None, Some(pin.clone()))), Some(pin.clone()));
        assert_eq!(standing_pin(&release_road("cli/cli", Some("v2.86.0"), Some(pin.clone()))), Some(pin.clone()));
        assert_eq!(standing_pin(&release_road("cli/cli", Some("v2.87.0"), Some(pin.clone()))), None);
        assert_eq!(standing_pin(&release_road("cli/cli", None, None)), None);
        assert_eq!(standing_pin(&InstallRoad::Vendor { cask: KUBECTL.clone(), version: None, pin: Some(pin.clone()) }), Some(pin.clone()));
        assert_eq!(standing_pin(&npm_road("bun", Some("1.4.0"), None)), None);
        // The road without its record is what a first run installs: the pin goes, the version and assets the row names stay.
        assert_eq!(unpinned(&release_road("cli/cli", None, Some(pin.clone()))), release_road("cli/cli", None, None));
        assert_eq!(unpinned(&release_road("cli/cli", Some("v2.86.0"), Some(pin.clone()))), release_road("cli/cli", Some("v2.86.0"), None));
        assert_eq!(unpinned(&npm_road("bun", None, None)), npm_road("bun", None, None));
        // The failure names what came down, at which tag, then the recorded sum and the served one, each cut to 12 characters so the reason line keeps both.
        let sum = pin.sha256.clone().unwrap();
        assert_eq!(
            pin_check_line("$asset", "$tag", &sum),
            format!(
                "[ \"$sum\" = '{}' ] || {{ echo \"Error: $asset at $tag does not match the checksum recorded on its first install: recorded dddddddddddd, served ${{sum:0:12}}\" >&2; exit 1; }}",
                "d".repeat(64)
            )
        );
        let armed = ToolPin { arch: Some(Arch::X8664), ..pin.clone() };
        let release = install(&release_road("cli/cli", None, Some(armed.clone())), "gh").unwrap();
        assert!(release.contains("tag='v2.86.0'"));
        assert!(release.contains(&format!("if [ \"$arch\" = 'x86_64' ]; then {}; fi", pin_check_line("$asset", "$tag", &sum))));
        // A sum recorded before pins named their arch is checked on every arch, as it always was: every seal before then was built on x86_64.
        assert!(install(&release_road("cli/cli", None, Some(pin.clone())), "gh").unwrap().contains(&format!("  {}\n", pin_check_line("$asset", "$tag", &sum))));
        // A cask checks the sum it pins for the arch, whatever a recipe recorded, so its first install is checked too.
        for cask in linux_casks() {
            let line = &cask.install;
            assert!(line.contains("x86_64) a="), "{}", cask.bin);
            assert!(line.contains(&cask.sha256.x86_64), "{}", cask.bin);
            assert!(line.contains(&cask.sha256.aarch64), "{}", cask.bin);
            assert!(line.contains("| sha256sum -c - >/dev/null"), "{}", cask.bin);
            assert!(!line.contains("[ \"$sum\" ="), "{}", cask.bin);
            // The version line reads the cask's own, the one its two sums belong to, so a row asking for another version moves nothing.
            assert!(line.contains(&format!("ver='{}'", cask.version)), "{}", cask.bin);
            let asked = InstallRoad::Vendor { cask: cask.clone(), version: Some("1.0.0".to_string()), pin: None };
            assert!(road_module_named(RoadName::Vendor).install(&asked, &cask.bin, None).unwrap().contains(&format!("ver='{}'", cask.version)), "{}", cask.bin);
            let pinned = InstallRoad::Vendor { cask: cask.clone(), version: None, pin: Some(pin_of("1.0.0", &"a".repeat(64))) };
            assert!(road_module_named(RoadName::Vendor).install(&pinned, &cask.bin, None).unwrap().contains(&format!("ver='{}'", cask.version)), "{}", cask.bin);
        }
        // The vendor's current version is asked of nobody: no cask reads a version off the network.
        for cask in linux_casks() {
            assert!(!cask.install.contains("ver=\"$("), "{}", cask.bin);
        }
        // The words are the protocol's, so the reason line a person reads is the one the format test pins.
        assert!(pin_check_line("x", "y", "z").contains(&pin_mismatch_line("x", "y", "z", "${sum:0:12}")));
    }

    #[test]
    fn one_rule_says_the_version_a_road_installs_at_one_says_whether_a_copy_gets_it_and() {
        let pin = tag_pin("4.1.0");
        // The row's version first, else the recorded pin while it stands, else the source's current one.
        assert_eq!(version_of(&npm_road("wrangler", Some("4.2.0"), Some(pin.clone()))), s("4.2.0"));
        assert_eq!(version_of(&npm_road("wrangler", None, Some(pin.clone()))), s("4.1.0"));
        assert_eq!(version_of(&npm_road("wrangler", Some("4.1.0"), Some(pin.clone()))), s("4.1.0"));
        assert_eq!(version_of(&npm_road("wrangler", None, None)), None);
        assert_eq!(version_of(&release_road("cli/cli", None, Some(pin_of("v2.86.0", &"d".repeat(64))))), s("v2.86.0"));
        // A package road installs at the pin the way it installs at a row's version.
        assert_eq!(install(&npm_road("wrangler", None, Some(pin.clone())), "wrangler"), ok("npm install -g wrangler@4.1.0"));
        assert_eq!(install(&InstallRoad::Pnpm(package_road("wrangler", None, Some(pin.clone()))), "wrangler"), ok("pnpm add -g wrangler@4.1.0"));
        assert_eq!(install(&InstallRoad::Uv(package_road("ruff", None, Some(tag_pin("0.4.4")))), "ruff"), ok("uv tool install ruff==0.4.4"));
        assert_eq!(install(&InstallRoad::Cargo(package_road("bat", None, Some(tag_pin("0.24.0")))), "bat"), ok("cargo install bat --version 0.24.0 --locked"));
        assert_eq!(install(&go_road(Some("golang.org/x/tools/gopls"), None, Some(tag_pin("v0.16.2"))), "gopls"), ok("go install golang.org/x/tools/gopls@v0.16.2"));
        // A pin the row's version moved past does not stand: the row installs at its version, a first install again.
        assert_eq!(install(&npm_road("wrangler", Some("4.2.0"), Some(pin.clone())), "wrangler"), ok("npm install -g wrangler@4.2.0"));
        // Which roads fix a version on a copy: the ones that install at one, and any road carrying a version of its own,
        // which after the pins table is every release and vendor row the catalog carries.
        assert!(fixes_version(&npm_road("wrangler", None, None)));
        assert!(!fixes_version(&release_road("cli/cli", None, None)));
        assert!(fixes_version(&release_road("cli/cli", Some("v2.86.0"), None)));
        assert!(!fixes_version(&InstallRoad::Vendor { cask: KUBECTL.clone(), version: None, pin: None }));
        assert!(fixes_version(&InstallRoad::Vendor { cask: KUBECTL.clone(), version: Some(KUBECTL.version.clone()), pin: None }));
        assert!(fixes_version(&go_road(Some("x"), None, None)));
        assert!(!fixes_version(&InstallRoad::Brew { formula: "go".to_string(), pin: None }));
        assert!(!fixes_version(&InstallRoad::Apt { packages: vec!["tmux".to_string()], pin: None }));
        assert!(!fixes_version(&script_road("x", None)));
        assert!(fixes_version(&script_road("x", Some("6.3.3"))));
        for id in ["swift", "playwright", "hermes", "gh", "bazel", "gcloud", "kubectl"] {
            assert!(fixes_version(entry(id).install_road()), "{id}");
        }
        for id in ["rust", "yarn", "op"] {
            assert!(!fixes_version(entry(id).install_road()), "{id}");
        }
        assert!(fixes_version(entry("claude").install_road()));
        // A catalog row installs at the catalog's tag whatever version a recipe row asks for: the engine's plan test
        // pins the note the row gets, and the road itself never moves.
        assert_eq!(version_of(entry("gh").install_road()), s("v2.101.0"));
        assert_eq!(version_of(entry("kubectl").install_road()), Some(KUBECTL.version.clone()));
        assert!(road_module_named(RoadName::Release).at(&sample_road(RoadName::Release), "1.0.0").is_none());
        assert!(road_module_named(RoadName::Vendor).at(&sample_road(RoadName::Vendor), "1.0.0").is_none());
        // How each road reads the installed version back, each in the form its install takes: no v where the install takes none, the v where go wants it.
        let installed = |road: &InstallRoad, bin: &str| road_module(road).installed(road, bin, None).unwrap();
        assert_eq!(
            installed(&npm_road("@openai/codex", None, None), "codex"),
            r#"node -p 'require(process.argv[1] + "/package.json").version' "$(npm root -g)/"'@openai/codex'"#
        );
        assert!(installed(&InstallRoad::Pnpm(package_road("wrangler", None, None)), "wrangler").contains("$(pnpm root -g)"));
        assert_eq!(
            installed(&InstallRoad::Bun(package_road("wrangler", None, None)), "wrangler"),
            r#"bun pm ls -g 2>/dev/null | grep -oE "(^| )wrangler@[^[:space:]]+" | head -n 1 | sed 's/.*@//'"#
        );
        assert_eq!(
            installed(&InstallRoad::Uv(package_road("ruff", None, None)), "ruff"),
            r#"uv tool list 2>/dev/null | awk -v p='ruff' '$1==p{sub(/^v/,"",$2); sub(/:$/,"",$2); print $2}'"#
        );
        assert!(installed(&InstallRoad::Pipx(package_road("black", None, None)), "black").contains("pipx list --short"));
        assert!(installed(&InstallRoad::Cargo(package_road("bat", None, None)), "bat").contains("cargo install --list"));
        assert_eq!(installed(&go_road(Some("golang.org/x/tools/gopls"), None, None), "gopls"), r#"go version -m /root/go/bin/'gopls' 2>/dev/null | awk '$1=="mod"{print $3}'"#);
        assert!(installed(&InstallRoad::Brew { formula: "go".to_string(), pin: None }, "go").contains("list --versions go"));
        assert_eq!(
            installed(&InstallRoad::Apt { packages: vec!["clang".to_string(), "clang-format".to_string()], pin: None }, "clang"),
            r#"dpkg-query -W -f='${Version}\n' 'clang' 2>/dev/null"#
        );
        assert_eq!(
            installed(&script_road("x", None), "claude"),
            r#"'claude' --version 2>/dev/null | head -n 1 | grep -oE '[0-9][^ ,()]*\.[0-9][^ ,()]*' | head -n 1"#
        );
        // A release's and a vendor's own install line says the tag and sum; neither reads again.
        let release = sample_road(RoadName::Release);
        let vendor = sample_road(RoadName::Vendor);
        assert!(road_module(&release).installed(&release, "gh", None).is_none());
        assert!(road_module(&vendor).installed(&vendor, "gcloud", None).is_none());
    }

    #[test]
    fn one_rule_names_the_id_a_recipe_row_is_known_by_on_every_computer_the_catalog_id() {
        assert_eq!(catalog_id_of_row("tools/npm/wrangler"), s("wrangler"));
        assert_eq!(catalog_id_of_row("tools/catalog/wrangler"), s("wrangler"));
        assert_eq!(catalog_id_of_row("tools/brew/gh"), s("gh"));
        assert_eq!(catalog_id_of_row("agents/codex"), s("codex"));
        assert_eq!(catalog_id_of_row("agents/mcp/claude/wsp"), None);
        assert_eq!(catalog_id_of_row("tools/brew/zingzy/tap/diskbloom"), None);
        assert_eq!(catalog_id_of_row("shell/zshrc"), None);
        assert_eq!(catalog_id_of_row("logins/gh"), None);
    }

    #[test]
    fn proves_every_browser_or_device_sign_in_with_a_status_check() {
        for e in catalog() {
            if matches!(e.sign_in().kind(), SignInKind::Oauth | SignInKind::Device) {
                assert!(e.sign_in().status().is_some(), "{}", e.id());
            }
        }
        for (name, row) in SIGN_IN_ROWS.iter() {
            assert_eq!(catalog_entry(name).map(|e| e.sign_in()), Some(row), "{name}");
        }
    }

    #[test]
    fn answers_each_agent_s_sign_in_kind_off_its_own_status_where_the_status_names_one() {
        let claude: [(&str, Option<AgentSignInKind>); 5] = [
            (r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max"}"#, Some(AgentSignInKind::Subscription)),
            (r#"{"loggedIn":true,"authMethod":"claude.ai"}"#, Some(AgentSignInKind::Oauth)),
            (r#"{"loggedIn":true,"authMethod":"oauth_token"}"#, Some(AgentSignInKind::Subscription)),
            (r#"{"loggedIn":true,"authMethod":"none","apiKeySource":"ANTHROPIC_API_KEY"}"#, Some(AgentSignInKind::ApiKey)),
            (r#"{"loggedIn":false,"authMethod":"none"}"#, None),
        ];
        let codex: [(&str, Option<AgentSignInKind>); 3] = [
            ("Logged in using ChatGPT", Some(AgentSignInKind::Subscription)),
            ("Logged in using an API key - sk-proj-***abcd", Some(AgentSignInKind::ApiKey)),
            ("Not logged in", None),
        ];
        let gemini: [(&str, Option<AgentSignInKind>); 3] = [("oauth_creds.json", Some(AgentSignInKind::Oauth)), ("GEMINI_API_KEY", Some(AgentSignInKind::ApiKey)), ("", None)];
        // Their status says signed in or not and never which kind of login it is.
        let silent = ["opencode", "cursor", "pi", "hermes"];
        let every_output: Vec<&str> = claude.iter().chain(codex.iter()).chain(gemini.iter()).map(|(output, _)| *output).collect();
        for a in catalog_agents() {
            let Some(status) = a.sign_in.status() else { continue };
            if silent.contains(&a.id.as_str()) {
                for output in &every_output {
                    assert_eq!(status.kind(output), None, "{}: {output}", a.id);
                }
                continue;
            }
            let kinds: &[(&str, Option<AgentSignInKind>)] = match a.id.as_str() {
                "claude" => &claude,
                "codex" => &codex,
                "gemini" => &gemini,
                other => panic!("{other} has no kinds recorded"),
            };
            for (output, said) in kinds {
                assert_eq!(status.kind(output), *said, "{}: {output}", a.id);
            }
        }
    }

    #[test]
    fn names_claude_code_s_plan_off_its_own_status_where_the_account_names_one_and_none() {
        let status = entry("claude").sign_in().status().unwrap();
        assert_eq!(status.plan(r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max"}"#), s("max"));
        assert_eq!(status.plan(r#"{"loggedIn":true,"authMethod":"claude.ai"}"#), None);
        assert_eq!(status.plan(r#"{"loggedIn":true,"apiKeySource":"ANTHROPIC_API_KEY","subscriptionType":"max"}"#), None);
    }

    #[test]
    fn files_one_sign_in_row_per_login_id_and_a_keys_row_beside_a_login_whose_key_files() {
        // Every entry with a login or a note about having none has a row under its login id; a plain tool has none.
        for e in catalog() {
            let own = login_row(&login_id_of(e.id()));
            if has_login(e.sign_in()).is_some() || mints_token(e.sign_in()).is_some() || e.sign_in().note().is_some() {
                assert_eq!(own.map(|r| &r.sign_in), Some(e.sign_in()), "{}", e.id());
            } else {
                assert!(own.is_none(), "{}", e.id());
            }
        }
        assert_eq!(login_row("kube").map(|r| r.entry.id()), Some("kubectl"));
        assert!(login_row("kubectl").is_none());
        // Hermes keeps provider keys in a file beside its device login, so it alone has a keys row: nothing to run on the
        // machine, the login's own status proves the copy, and the row says why only a copy brings the keys.
        let keyed: Vec<_> = login_rows().iter().filter(|r| r.keys.is_some()).collect();
        assert_eq!(keyed.iter().map(|r| (r.id.as_str(), r.entry.id())).collect::<Vec<_>>(), vec![("hermes-keys", "hermes")]);
        assert_eq!(keys_id_of("hermes"), "hermes-keys");
        let hermes = entry("hermes").sign_in();
        let why = "the keys in ~/.hermes/.env travel only by copy; no sign-in produces them";
        let login = has_login(hermes).unwrap();
        assert_eq!(login.keys, Some(KeyFiles { paths: vec!["~/.hermes/.env".to_string()], note: why.to_string() }));
        assert_eq!(
            login_row("hermes-keys").map(|r| &r.sign_in),
            Some(&SignIn::None(NoSignIn { kind: SignInKind::None, note: s(why), sources: vec![LoginSource::File], status: login.status.clone(), state_on_machine: None }))
        );
        assert_eq!(
            keys_row_of(&KeyFiles { paths: vec!["~/.x/keys".to_string()], note: "why".to_string() }, None),
            SignIn::None(NoSignIn { kind: SignInKind::None, note: s("why"), sources: vec![LoginSource::File], status: None, state_on_machine: None })
        );
        // A keys row's note fits the detail pane beside its path line.
        for r in &keyed {
            assert!(r.keys.as_ref().unwrap().note.chars().count() <= 76, "{}", r.id);
        }
        let ids: std::collections::HashSet<&str> = login_rows().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids.len(), login_rows().len());
    }

    #[test]
    fn ships_the_six_agents_whose_project_state_a_move_can_follow_each_with_a_measured() {
        let agents = catalog_agents();
        let shipped: Vec<&&AgentEntry> = agents.iter().filter(|a| a.source.road == RoadEvidence::Measured).collect();
        assert_eq!(shipped.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["claude", "codex", "gemini", "opencode", "pi", "hermes"]);
        for a in &shipped {
            assert!(!a.project_state.is_empty(), "{}", a.id);
            assert!(has_login(&a.sign_in).is_some() || mints_token(&a.sign_in).is_some(), "{}", a.id);
            assert!(size_bytes(&a.size).unwrap_or(0) > 0, "{}", a.id);
        }
        for e in catalog().iter().filter(|e| e.kind() == EntryKind::Agent) {
            let a = e.as_agent().unwrap();
            assert!(re(r"^\.[\w./-]*[\w-]$").is_match(&a.state_home), "{}", a.id);
            if let Some(guest) = &a.guest_state_home {
                assert!(guest.starts_with("/root/"), "{}", a.id);
            }
            assert_eq!(smoke_of(e), format!("{} --version", a.bin));
        }
        assert_eq!(
            agents.iter().filter(|a| a.guest_state_home.is_some()).map(|a| (a.id.as_str(), a.guest_state_home.clone().unwrap())).collect::<Vec<_>>(),
            vec![("claude", "/root/.claude-cfg".to_string())]
        );
        // The variable a person's config folder for that agent is laid in as; an agent with none takes no config folder.
        assert_eq!(
            agents.iter().filter(|a| a.state_home_env.is_some()).map(|a| (a.id.as_str(), a.state_home_env.clone().unwrap())).collect::<Vec<_>>(),
            vec![("claude", "CLAUDE_CONFIG_DIR".to_string()), ("codex", "CODEX_HOME".to_string())]
        );
        // The test run reads these off the modules' text, since its config loads before any package is built.
        let mut envs: Vec<String> = agents.iter().filter_map(|a| a.state_home_env.clone()).collect();
        envs.sort();
        assert_eq!(AGENT_STORE_ENVS.to_vec(), envs);
        // And the commands the run stands in for, so no agent's own is reached off the suite's PATH.
        let mut bins: Vec<String> = agents.iter().map(|a| a.bin.clone()).collect();
        bins.sort();
        assert_eq!(AGENT_BINS.to_vec(), bins);
        // The update an agent's row answers is the vendor's own command, shown to the person and never run by wsp.
        assert_eq!(
            agents.iter().filter(|a| a.update_line.is_some()).map(|a| (a.id.as_str(), a.update_line.clone().unwrap())).collect::<Vec<_>>(),
            vec![("claude", "claude update".to_string()), ("codex", "npm i -g @openai/codex@latest".to_string()), ("opencode", "opencode upgrade".to_string())]
        );
        assert_eq!(line_of(entry("codex")), "npm install -g @openai/codex@0.155.1");
        assert_eq!(line_of(entry("pi")), "npm install -g --ignore-scripts @earendil-works/pi-coding-agent@0.84.4");
        assert_eq!(line_of(entry("claude")), CLAUDE_INSTALL.as_str());
        assert!(re(r"git clone -q --depth 1 --branch v[\d.]+ https://github\.com/NousResearch/hermes-agent\.git").is_match(&line_of(entry("hermes"))));
        // An agent's npm road is pinned in the data; an unpinned one would install whatever the registry serves that day.
        for a in &agents {
            if let InstallRoad::Npm { package, .. } = &a.install_road {
                assert!(re(r"^\d").is_match(package.version.as_deref().unwrap_or("")), "{}", a.id);
            }
        }
    }

    #[test]
    fn keeps_one_module_per_agent_under_src_agents_each_registered_once_the_agents_firs() {
        let agents = catalog_agents();
        let mut modules: Vec<String> = agent_modules().into_iter().map(|a| a.id).collect();
        modules.sort();
        let mut ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
        ids.sort();
        assert_eq!(ids, modules);
        assert_eq!(
            agents.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            vec!["claude", "codex", "gemini", "opencode", "pi", "hermes", "crush", "qwen", "goose", "amp", "cursor"]
        );
        let all = catalog();
        for (i, a) in agents.iter().enumerate() {
            assert_eq!(all[i].as_agent(), Some(*a));
        }
        assert!(all[agents.len()..].iter().all(|e| e.kind() == EntryKind::Tool));
    }

    #[test]
    fn manages_crush_qwen_code_goose_and_amp_by_a_pinned_road_with_nothing_of_theirs_me() {
        let added: Vec<&AgentEntry> = ["crush", "qwen", "goose", "amp"].into_iter().map(agent).collect();
        for a in &added {
            assert_eq!(a.source, Evidence { sessions: 0, images: 0, road: RoadEvidence::Unmeasured, note: None }, "{}", a.id);
            assert!(a.project_state.is_empty(), "{}", a.id);
            assert!(a.config_paths.is_empty(), "{}", a.id);
            assert_eq!(a.sign_in, no_sign_in(), "{}", a.id);
            assert!(a.mcp.is_none(), "{}", a.id);
            assert!(!runs_threads(&a.id), "{}", a.id);
        }
        // A release road carries the tag and both arches' assets with their sums, off the pins table.
        for (id, repo) in [("crush", "charmbracelet/crush"), ("goose", "aaif-goose/goose")] {
            let pin = RELEASE_PINS.get(repo).unwrap();
            assert_eq!(
                entry(id).install_road(),
                &InstallRoad::Release { repo: s(repo), version: Some(pin.tag.clone()), assets: Some(pin.assets.clone()), pin: None, go: None },
                "{id}"
            );
            assert!(re(r"^[0-9a-f]{64}$").is_match(&pin.assets.x86_64.as_ref().unwrap().sha256), "{id}");
            assert!(re(r"^[0-9a-f]{64}$").is_match(&pin.assets.aarch64.as_ref().unwrap().sha256), "{id}");
            assert!(line_of(entry(id)).contains("sha256sum -c -"), "{id}");
        }
        assert_eq!(install_shown(entry("crush")), Ok(InstallShown::Words("the v0.96.1 release of github.com/charmbracelet/crush".to_string())));
        assert_eq!(install_shown(entry("goose")), Ok(InstallShown::Words("the v1.52.0 release of github.com/aaif-goose/goose".to_string())));
        // Goose moved to the AAIF at the Linux Foundation: github.com/block/goose only redirects there, and the assets are the same bytes.
        assert!(RELEASE_PINS.get("block/goose").is_none());
        assert!(line_of(entry("goose")).contains("https://github.com/aaif-goose/goose/releases/download/v1.52.0/"));
        assert_eq!(agent("goose").about.repo, s("https://github.com/aaif-goose/goose"));
        assert_eq!(install_shown(entry("qwen")), Ok(InstallShown::Line("npm install -g --ignore-scripts @qwen-code/qwen-code@0.24.5".to_string())));
        assert_eq!(install_shown(entry("amp")), Ok(InstallShown::Line("npm install -g @ampcode/cli@0.0.1790352060-g26b83c".to_string())));
        assert_eq!(agent("qwen").node, Some(22));
        assert_eq!(size_bytes(entry("amp").size()), None);
        // Every folder an install writes a skill into is one of the agent's own, under the home.
        for a in &added {
            for r in &a.skill_roots.user {
                assert!(r.dir.starts_with("~/"), "{}", a.id);
            }
        }
    }

    #[test]
    fn runs_cursor_s_agent_from_its_vendor_s_linux_download_at_a_pinned_version_checked() {
        let cursor = agent("cursor");
        assert_eq!(cursor.bin, "cursor-agent");
        assert_eq!(cursor.install_road, InstallRoad::Vendor { cask: CURSOR.clone(), version: Some(CURSOR.version.clone()), pin: None });
        assert_eq!(line_of(entry("cursor")), CURSOR.install);
        assert!(line_of(entry("cursor")).contains(r#"curl -o "$tmp/$pkg" "https://downloads.cursor.com/lab/$ver/linux/$a/$pkg""#));
        assert_eq!(install_shown(entry("cursor")), Ok(InstallShown::Words("Cursor's Linux release".to_string())));
        assert_eq!(&cursor.sign_in, SIGN_IN_ROWS.get("cursor").unwrap());
        let SignIn::Login(login) = SIGN_IN_ROWS.get("cursor").unwrap() else { panic!("cursor's row is no login") };
        assert_eq!(login.kind, SignInKind::Key);
        assert_eq!(login.login, "cursor-agent login");
        assert_eq!(login.key_env, s("CURSOR_API_KEY"));
        assert_eq!(login.state_on_machine, vec![".config/cursor/auth.json".to_string()]);
        assert!(runs_threads("cursor"));
        let mark = cursor.mark.as_ref().unwrap();
        assert_eq!(mark.license, "MIT");
        assert!(re(r"^https://github\.com/lobehub/lobe-icons/blob/[0-9a-f]{40}/src/Cursor/").is_match(&mark.source));
    }

    #[test]
    fn cursor_s_install_unpacks_the_checked_download_under_opt_and_links_both_its_names() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("src").join("dist-package")).unwrap();
        fs::write(dir.join("src").join("dist-package").join("cursor-agent"), "the launcher\n").unwrap();
        let served = dir.join("served.tar.gz");
        assert!(Command::new("tar").arg("-czf").arg(&served).arg("-C").arg(dir.join("src")).arg("dist-package").status().unwrap().success());
        let sum = sha256_hex(&served);
        let home = dir.join("opt").join("cursor-agent");
        let bin = dir.join("bin");
        fs::create_dir(&bin).unwrap();
        // The arch is forced and the paths moved so the case reads the same on any machine that runs the suite.
        let script = CURSOR
            .install
            .replacen(r#"arch="$(uname -m)""#, "arch=x86_64", 1)
            .replacen(&CURSOR.sha256.x86_64, &sum, 1)
            .replacen(r#""https://downloads.cursor.com/lab/$ver/linux/$a/$pkg""#, &format!("\"file://{}\"", served.display()), 1)
            .replace("/opt/cursor-agent", home.to_str().unwrap())
            .replace("/usr/local/bin", bin.to_str().unwrap());
        let run = run_install(dir, &format!("{}\n{}", *CURL_NET, script));
        assert_eq!(stderr_of(&run), "");
        assert_eq!(run.status.code(), Some(0));
        assert!(stdout_of(&run).contains(&format!("WSP_ROAD release cursor-agent-2026.09.26-dd393fe-linux-x64 {sum} 2026.09.26-dd393fe")));
        // The one name every road gives it; the generic "agent" its installer also links is left off the PATH.
        assert_eq!(fs::read_link(bin.join("cursor-agent")).unwrap(), home.join("cursor-agent"));
        assert_eq!(fs::read_to_string(bin.join("cursor-agent")).unwrap(), "the launcher\n");
        assert!(!bin.join("agent").exists());
    }

    #[test]
    fn a_release_pin_s_sum_is_checked_only_against_the_file_of_the_arch_it_was_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let bin = dir.join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(dir.join("served"), "the arm build\n").unwrap();
        let sum = sha256_hex(&dir.join("served"));
        let listing = ["amd64", "arm64"].iter().map(|a| format!("\"browser_download_url\": \"https://example.com/spoo_0.4.1_linux_{a}\"")).collect::<Vec<_>>().join(",");
        write_stub(
            &bin.join("curl"),
            &format!("#!/bin/sh\nif [ \"$1\" = -o ]; then cp \"{}\" \"$2\"; else printf '%s\\n' '{}'; fi\n", dir.join("served").display(), listing),
        );
        // A Mac has shasum and no sha256sum; the Linux machines this script targets have the coreutils one.
        write_stub(&bin.join("sha256sum"), SHA256SUM_STUB);
        let run = |arch: &str, pin: ToolPin| -> Output {
            let road = release_road("spoo-me/spoo-cli", Some("v0.4.1"), Some(pin));
            let script = install(&road, "spoo").unwrap().replacen(r#"arch="$(uname -m)""#, &format!("arch={arch}"), 1).replace("/usr/local/bin", bin.to_str().unwrap());
            Command::new("bash")
                .args(["--noprofile", "--norc", "-c", &script])
                .env_clear()
                .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
                .env("HOME", dir)
                .output()
                .unwrap()
        };
        let recorded = |arch: Option<Arch>, sha256: &str| ToolPin { tag: "v0.4.1".to_string(), sha256: Some(sha256.to_string()), arch, latest: None };
        // An x86_64 builder's sum on an arm machine: the arm file is never held to it, and its own sum and arch are printed for the record.
        let other = run("aarch64", recorded(Some(Arch::X8664), &"c".repeat(64)));
        assert_eq!(stderr_of(&other), "");
        assert_eq!(other.status.code(), Some(0));
        assert!(stdout_of(&other).contains(&format!("WSP_ROAD release spoo_0.4.1_linux_arm64 {sum} v0.4.1 aarch64")));
        // The same arch is held to its sum, and a file that does not match it is refused by name.
        let same = run("aarch64", recorded(Some(Arch::Aarch64), &"c".repeat(64)));
        assert_eq!(same.status.code(), Some(1));
        assert!(stderr_of(&same).contains(&pin_mismatch_line("spoo_0.4.1_linux_arm64", "v0.4.1", &"c".repeat(12), &sum[..12])));
        assert_eq!(run("aarch64", recorded(Some(Arch::Aarch64), &sum)).status.code(), Some(0));
        // A pin that names no arch holds the file to its sum on either arch, so no seal made before pins named one loses its check.
        for arch in ["x86_64", "aarch64"] {
            let old = run(arch, recorded(None, &"c".repeat(64)));
            let served = if arch == "x86_64" { "spoo_0.4.1_linux_amd64" } else { "spoo_0.4.1_linux_arm64" };
            assert_eq!(old.status.code(), Some(1), "{arch}");
            assert!(stderr_of(&old).contains(&pin_mismatch_line(served, "v0.4.1", &"c".repeat(12), &sum[..12])), "{arch}");
        }
    }

    #[test]
    fn says_which_agents_wsp_opens_threads_on() {
        let threaded: Vec<&str> = catalog_agents().iter().filter(|a| runs_threads(&a.id)).map(|a| a.id.as_str()).collect();
        assert_eq!(threaded, THREAD_AGENTS.to_vec());
        assert!(!runs_threads("nope"));
    }

    #[test]
    fn says_where_each_agent_s_newest_version_is_published_its_own_npm_package_or_relea() {
        for a in catalog_agents() {
            match &a.latest {
                Some(LatestSource::Npm { package }) => {
                    let InstallRoad::Npm { package: road, .. } = &a.install_road else { panic!("{}: latest is npm, the road is not", a.id) };
                    assert_eq!(&road.package, package, "{}", a.id);
                }
                Some(LatestSource::Github { repo }) => {
                    let InstallRoad::Release { repo: road, .. } = &a.install_road else { panic!("{}: latest is github, the road is not a release", a.id) };
                    assert_eq!(road.as_ref(), Some(repo), "{}", a.id);
                }
                Some(LatestSource::Text { url }) => assert!(re(r"^https://").is_match(url), "{}", a.id),
                None => {}
            }
        }
    }

    #[test]
    fn says_who_makes_each_agent_what_it_is_in_a_sentence_or_two_its_license_where_it_l() {
        for a in catalog_agents() {
            let parts = sentences(&a.about.description);
            assert!(parts.len() <= 2, "{}", a.id);
            for part in &parts {
                assert!(re(r"^[A-Z].*\.$").is_match(part), "{}: {part}", a.id);
            }
            assert!(!re(r"\bAI\b").is_match(&a.about.description), "{}", a.id);
            if let Some(repo) = &a.about.repo {
                assert!(re(r"^https://github\.com/[\w.-]+/[\w.-]+$").is_match(repo), "{}", a.id);
            }
            if let Some(homepage) = &a.about.homepage {
                assert!(re(r"^https://").is_match(homepage), "{}", a.id);
            }
        }
        // Neither Amp's source nor Cursor's is public.
        assert_eq!(catalog_agents().iter().filter(|a| a.about.repo.is_none()).map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["amp", "cursor"]);
        assert_eq!(install_shown(entry("codex")), Ok(InstallShown::Line("npm install -g @openai/codex@0.155.1".to_string())));
        // A vendor's script is many lines no person pastes, and a release is a download checked against its sum.
        assert_eq!(install_shown(entry("claude")), Ok(InstallShown::Words("by its own installer".to_string())));
        assert_eq!(install_shown(entry("hermes")), Ok(InstallShown::Words("by its own installer".to_string())));
        assert_eq!(install_shown(entry("gh")), Ok(InstallShown::Words("the v2.101.0 release of github.com/cli/cli".to_string())));
        assert_eq!(install_shown(entry("gcloud")), Ok(InstallShown::Words("Google's Linux release".to_string())));
    }

    #[test]
    fn gives_every_entry_one_install_line_from_its_road_s_module_apt_npm_homebrew_as_li() {
        assert_eq!(line_of(entry("git")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq git");
        // Bookworm has docker.io but no compose v2 package, so compose comes as the cli plugin from its release, checksummed.
        assert_eq!(
            line_of(entry("docker")),
            [
                "export DEBIAN_FRONTEND=noninteractive",
                "apt-get install -y -qq docker.io",
                r#"arch="$(uname -m)""#,
                r#"case "$arch" in"#,
                "  x86_64) sha=db1889184726840f75c4f9c001048430d4f25b3be3cb084d3ddd762bc0aed576 ;;",
                "  aarch64) sha=732e3a84c1a0f67256ce80bc2598a24546b10ca05f9faa97efceb1171ece2ef7 ;;",
                r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
                "esac",
                r#"curl -o /tmp/docker-compose "https://github.com/docker/compose/releases/download/v5.5.1/docker-compose-linux-$arch""#,
                r#"echo "$sha  /tmp/docker-compose" | sha256sum -c - >/dev/null"#,
                "install -D -m 0755 /tmp/docker-compose /usr/libexec/docker/cli-plugins/docker-compose",
                "rm -f /tmp/docker-compose",
            ]
            .join("\n")
        );
        // Rust comes by rustup, whose installer is pinned to the sha256 rust-lang publishes beside the archived release.
        // The script road runs every one of these under set -e, so no script carries the line itself.
        assert_eq!(
            line_of(entry("rust")),
            [
                r#"arch="$(uname -m)""#,
                r#"case "$arch" in"#,
                "  x86_64) sha=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71 ;;",
                "  aarch64) sha=15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433 ;;",
                r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
                "esac",
                r#"curl -o /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/1.29.1/$arch-unknown-linux-gnu/rustup-init""#,
                r#"echo "$sha  /tmp/rustup-init" | sha256sum -c - >/dev/null"#,
                "chmod +x /tmp/rustup-init",
                "/tmp/rustup-init -y --no-modify-path --profile default --default-toolchain stable",
                "rm -f /tmp/rustup-init",
            ]
            .join("\n")
        );
        assert_eq!(line_of(entry("pnpm")), "npm install -g pnpm@11.9.0");
        assert_eq!(line_of(entry("wrangler")), "npm install -g wrangler");
        assert!(re(r#"^su -s /bin/bash linuxbrew -c '\[ -d "\$PWD" \][\s\S]*HOMEBREW_NO_AUTO_UPDATE=1[\s\S]*brew install go'$"#).is_match(&line_of(entry("go"))));
        // A catalog row on the release road downloads the asset the pins table names, at the tag it names, and checks
        // the sum beside it before anything is unpacked: no API read, no listing grepped, no release fetched by date.
        let gh = line_of(entry("gh"));
        let pin = RELEASE_PINS.get("cli/cli").unwrap();
        let x86 = pin.assets.x86_64.as_ref().unwrap();
        let arm = pin.assets.aarch64.as_ref().unwrap();
        assert!(gh.contains("name='gh'"));
        assert!(gh.contains(&format!("tag='{}'", pin.tag)));
        assert!(gh.contains(&format!(
            "case \"$arch\" in x86_64) asset='{}' sha={} ;; aarch64) asset='{}' sha={} ;;",
            x86.name, x86.sha256, arm.name, arm.sha256
        )));
        assert!(gh.contains(&format!("url='https://github.com/cli/cli/releases/download/{}/'\"$asset\"", pin.tag)));
        let checked = gh.find(r#"echo "$sha  $tmp/$asset" | sha256sum -c - >/dev/null"#).unwrap();
        let unpacked = gh.find(r#"tar -xzf "$tmp/$asset""#).unwrap();
        assert!(checked < unpacked);
        assert!(gh.contains(r#"echo "WSP_ROAD release $asset $sha $tag""#));
        assert!(!gh.contains("api.github.com"));
        assert!(!gh.contains("browser_download_url"));
        // Every arch the table names an asset for leaves no fall-through to render: go is the road for an arch with none.
        assert!(!gh.contains("go install"));
        assert!(!gh.contains("command -v go"));
        // An arch the release has no asset for takes the row's main package at the tag, and is refused where it names none.
        let x86_only = ReleaseAssets { x86_64: Some(ReleaseAsset { name: "gh_linux_amd64.tar.gz".to_string(), sha256: "e".repeat(64) }), aarch64: None };
        let armless = install(
            &InstallRoad::Release { repo: s("cli/cli"), version: s("v2.86.0"), assets: Some(x86_only.clone()), pin: None, go: s("github.com/cli/cli/v2/cmd/gh") },
            "gh",
        )
        .unwrap();
        assert!(armless.contains("aarch64) asset= sha= ;;"));
        assert!(armless.contains("go install 'github.com/cli/cli/v2/cmd/gh@v2.86.0'"));
        let no_go = install(&InstallRoad::Release { repo: s("cli/cli"), version: s("v2.86.0"), assets: Some(x86_only), pin: None, go: None }, "gh").unwrap();
        assert!(!no_go.contains("go install"));
        assert!(no_go.contains(r#"echo "Error: release "'v2.86.0'" of "'cli/cli'" has no Linux build for $arch" >&2"#));
        // A release road with no tag installs nothing: nothing in the catalog fetches the current release any more.
        assert_eq!(install(&release_road("cli/cli", None, None), "gh"), note("names no release tag; name the version"));
        assert!(!gh.contains(r#"[ "$sum" ="#));
        assert_eq!(line_of(entry("gcloud")), GCLOUD.install);
        assert_eq!(line_of(entry("kubectl")), KUBECTL.install);
        // The floor runs before Homebrew or the release machinery exist on the machine.
        for e in base_floor() {
            assert!(![RoadName::Brew, RoadName::Release, RoadName::Vendor].contains(&e.install_road.name()), "{}", e.id);
        }
    }

    #[test]
    fn says_which_managers_the_build_reads_a_row_s_road_off_every_package_manager_a_col() {
        // The collector files a tools row under a manager's name, and the plan reads that row's road off the module.
        for road in ["brew", "npm", "pnpm", "bun", "uv", "pipx", "cargo", "go"] {
            assert!(reads_row_road(road), "{road}");
        }
        // apt is on every machine and brings no row of its own, so a row filed under it installs nothing: the catalog
        // row of a tool apt carries is what installs it, and the collector and the recipe verb read this to know.
        for road in ["apt", "release", "vendor", "script"] {
            assert!(!reads_row_road(road), "{road}");
        }
        assert!(!reads_row_road("snap"));
    }

    #[test]
    fn has_one_module_per_road_with_its_words_and_each_module_writes_the_install_and_it() {
        for road in ROADS {
            assert_eq!(road_module_named(road).road, road);
            assert!(re(r"^(by|with|as|from) ").is_match(&road_module_named(road).words()), "{}", road.as_str());
        }
        let npm = road_module_named(RoadName::Npm).from_row(&tool_row("bun", Some("1.4.0"))).unwrap();
        assert_eq!((install(&npm, "x"), uninstall(&npm, "x")), (ok("npm install -g bun@1.4.0"), ok("npm uninstall -g bun")));
        assert_eq!(
            install(&InstallRoad::Npm { package: package_road("@earendil-works/pi-coding-agent", Some("0.84.4"), None), ignore_scripts: Some(true) }, "x"),
            ok("npm install -g --ignore-scripts @earendil-works/pi-coding-agent@0.84.4")
        );
        let pnpm = road_module_named(RoadName::Pnpm).from_row(&tool_row("turbo", Some("2.5.0"))).unwrap();
        assert_eq!((install(&pnpm, "x"), uninstall(&pnpm, "x")), (ok("pnpm add -g turbo@2.5.0"), ok("pnpm remove -g turbo")));
        let bun = road_module_named(RoadName::Bun).from_row(&tool_row("eslint", None)).unwrap();
        assert_eq!((install(&bun, "x"), uninstall(&bun, "x")), (ok("bun add -g eslint"), ok("bun remove -g eslint")));
        let uv = road_module_named(RoadName::Uv).from_row(&tool_row("ty", Some("0.0.56"))).unwrap();
        assert_eq!((install(&uv, "x"), uninstall(&uv, "x")), (ok("uv tool install ty==0.0.56"), ok("uv tool uninstall ty")));
        let pipx = road_module_named(RoadName::Pipx).from_row(&tool_row("black", Some("24.1.0"))).unwrap();
        assert_eq!((install(&pipx, "x"), uninstall(&pipx, "x")), (ok("pipx install black==24.1.0"), ok("pipx uninstall black")));
        // --locked takes the dependency set the crate's own lockfile names, so two builders a month apart build the same tool.
        let cargo = road_module_named(RoadName::Cargo);
        assert_eq!(install(&cargo.from_row(&tool_row("bat", Some("0.24.0"))).unwrap(), "x"), ok("cargo install bat --version 0.24.0 --locked"));
        let bat = cargo.from_row(&tool_row("bat", None)).unwrap();
        assert_eq!((install(&bat, "x"), uninstall(&bat, "x")), (ok("cargo install bat --locked"), ok("cargo uninstall bat")));
        // A Go row carries its module in its first path (or an older label); the row's version wins over the module's; no module, nothing to run.
        let go = road_module_named(RoadName::Go);
        let go_row = |name: &str, version: Option<&str>, paths: &[&str], label: &str| ToolRow {
            name: name.to_string(),
            version: version.map(str::to_string),
            paths: paths.iter().map(|p| p.to_string()).collect(),
            label: label.to_string(),
        };
        let gopls = go.from_row(&go_row("gopls", None, &["golang.org/x/tools/gopls@v0.16.2"], "gopls")).unwrap();
        assert_eq!(install(&gopls, "x"), ok("go install golang.org/x/tools/gopls@v0.16.2"));
        assert_eq!(go.bin(&gopls), s("gopls"));
        assert_eq!(uninstall(&gopls, "x"), note("go has no uninstall; the binary stays in /root/go/bin"));
        assert_eq!(
            install(&go.from_row(&go_row("gopls", Some("v0.17.0"), &["golang.org/x/tools/gopls@v0.16.2"], "gopls")).unwrap(), "x"),
            ok("go install golang.org/x/tools/gopls@v0.17.0")
        );
        assert_eq!(
            install(&go.from_row(&go_row("gopls", None, &[], "gopls (golang.org/x/tools/gopls@v0.16.2)")).unwrap(), "x"),
            ok("go install golang.org/x/tools/gopls@v0.16.2")
        );
        assert_eq!(go.bin(&go.from_row(&go_row("spoo", None, &["github.com/spoo-me/spoo/v2@v2.0.0"], "spoo")).unwrap()), s("spoo"));
        let junk = go.from_row(&go_row("junk", None, &[], "junk (no module info)")).unwrap();
        assert_eq!(junk, go_road(None, None, None));
        assert_eq!(install(&junk, "x"), note("no module to install from"));
        assert_eq!(go.bin(&junk), None);
        // Homebrew runs as its own user; a tap formula that took the road comes off from /usr/local/bin when the cellar never had it.
        let gh = road_module_named(RoadName::Brew).from_row(&tool_row("gh", None)).unwrap();
        // su keeps the caller's folder, and root's home is one linuxbrew cannot read on some images: every brew moves
        // off such a folder before it runs, and stays where it was called when that folder can be read.
        assert!(re(r#"^su -s /bin/bash linuxbrew -c '\[ -d "\$PWD" \] && \[ -r "\$PWD" \] \|\| cd /home/linuxbrew\nHOMEBREW_NO_AUTO_UPDATE=1 .*brew install gh'$"#).is_match(&install(&gh, "x").unwrap()));
        assert!(re(r"brew uninstall gh'$").is_match(&uninstall(&gh, "x").unwrap()));
        let diskbloom = InstallRoad::Brew { formula: "zingzy/tap/diskbloom".to_string(), pin: None };
        assert!(re(
            r"^if \[ -x /home/linuxbrew/.linuxbrew/bin/brew \] && su [\s\S]*brew list --formula zingzy/tap/diskbloom[\s\S]* >/dev/null 2>&1; then su [\s\S]*brew uninstall zingzy/tap/diskbloom[\s\S]*; else rm -f /usr/local/bin/'diskbloom'; fi$"
        )
        .is_match(&uninstall(&diskbloom, "x").unwrap()));
        // A release at a tag fetches that tag and prints it; with a pin for the same tag the sum is checked; a row that names no repository only comes off.
        let spoo = |version: &str, go: Option<&str>, pin: Option<ToolPin>| InstallRoad::Release {
            repo: s("spoo-me/spoo-cli"),
            version: Some(version.to_string()),
            assets: None,
            pin,
            go: go.map(str::to_string),
        };
        let tagged = install(&spoo("v0.4.1", Some("github.com/spoo-me/spoo-cli"), None), "spoo").unwrap();
        assert!(tagged.contains(r#"release="$(curl 'https://api.github.com/repos/spoo-me/spoo-cli/releases/tags/v0.4.1' || true)""#));
        assert!(!tagged.contains("tag=\"$("));
        assert!(tagged.contains("tag='v0.4.1'"));
        assert!(tagged.contains(r#"echo "WSP_ROAD release $asset $sum $tag $arch""#));
        // A bare module goes in at the tag; one that carries its own version keeps it; a road with none has no go branch.
        assert!(tagged.contains("go install 'github.com/spoo-me/spoo-cli@v0.4.1'"));
        assert!(tagged.contains(r#"echo "Error: release "'v0.4.1'" of "'spoo-me/spoo-cli'" has no Linux build, and go is not on the machine" >&2"#));
        assert!(install(&spoo("v0.4.1", Some("github.com/spoo-me/spoo-cli@v0.4.0"), None), "spoo").unwrap().contains("go install 'github.com/spoo-me/spoo-cli@v0.4.0'"));
        assert!(!install(&spoo("v0.4.1", None, None), "spoo").unwrap().contains("command -v go"));
        // A road that pins a version takes a row's through at(); Homebrew, apt and a script install what their source serves and have none.
        assert_eq!(road_module_named(RoadName::Npm).at(&npm_road("wrangler", None, None), "4.1.0"), Some(npm_road("wrangler", Some("4.1.0"), None)));
        assert_eq!(
            road_module_named(RoadName::Cargo).at(&InstallRoad::Cargo(package_road("bat", Some("0.23.0"), None)), "0.24.0"),
            Some(InstallRoad::Cargo(package_road("bat", Some("0.24.0"), None)))
        );
        // The release and vendor roads take no version from a row: each installs the artifact the catalog pinned, so a
        // recipe's own version moves nothing and the row is noted instead.
        for road in [RoadName::Npm, RoadName::Pnpm, RoadName::Bun, RoadName::Uv, RoadName::Pipx, RoadName::Cargo, RoadName::Go] {
            assert!(road_module_named(road).at(&sample_road(road), "1.0.0").is_some(), "{}", road.as_str());
        }
        for road in [RoadName::Brew, RoadName::Apt, RoadName::Script, RoadName::Release, RoadName::Vendor] {
            assert!(road_module_named(road).at(&sample_road(road), "1.0.0").is_none(), "{}", road.as_str());
        }
        assert!(install(&spoo("v0.4.1", None, Some(pin_of("v0.4.1", &"c".repeat(64)))), "spoo").unwrap().contains(&format!("[ \"$sum\" = '{}' ]", "c".repeat(64))));
        assert!(!install(&spoo("v0.4.1", None, Some(pin_of("v0.4.0", &"c".repeat(64)))), "spoo").unwrap().contains(r#"[ "$sum" ="#));
        // No version: a pin fixes the tag and is checked, as a vendor install does; with neither there is no release to read and the row is noted.
        assert!(install(&release_road("cli/cli", None, Some(pin_of("v2.86.0", &"d".repeat(64)))), "gh").unwrap().contains("releases/tags/v2.86.0"));
        let aarch64_pin = ToolPin { arch: Some(Arch::Aarch64), ..pin_of("v2.86.0", &"d".repeat(64)) };
        assert!(install(&release_road("cli/cli", None, Some(aarch64_pin)), "gh").unwrap().contains(&format!("[ \"$sum\" = '{}' ]", "d".repeat(64))));
        let bare_release = InstallRoad::Release { repo: None, version: None, assets: None, pin: None, go: None };
        assert_eq!(install(&bare_release, "spoo"), note("no GitHub release to install from"));
        assert_eq!(uninstall(&bare_release, "spoo"), ok("rm -f /usr/local/bin/'spoo'"));
        // A vendor's download is the cask's own script, whatever version the row carries.
        let gcloud = InstallRoad::Vendor { cask: GCLOUD.clone(), version: s("575.0.0"), pin: None };
        assert_eq!(
            (install(&gcloud, "x"), uninstall(&gcloud, "x"), road_module_named(RoadName::Vendor).bin(&gcloud)),
            (ok(&GCLOUD.install), ok(&GCLOUD.uninstall), s("gcloud"))
        );
        let kubectl = InstallRoad::Vendor { cask: KUBECTL.clone(), version: None, pin: Some(pin_of("v1.37.0", &"c".repeat(64))) };
        assert_eq!(install(&kubectl, "x"), ok(&KUBECTL.install));
        // A cask's own sum rides every install of it, so the bytes are checked whether or not a recipe recorded a pin.
        assert!(install(&kubectl, "x").unwrap().contains(&KUBECTL.sha256.x86_64));
        // apt rows wait on the one index read; purge takes what the package alone pulled in.
        let apt = InstallRoad::Apt { packages: vec!["neovim".to_string()], pin: None };
        assert_eq!(
            (install(&apt, "x"), uninstall(&apt, "x"), road_module(&apt).after()),
            (
                ok("export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq neovim"),
                ok("export DEBIAN_FRONTEND=noninteractive\napt-get purge -y -qq neovim && apt-get autoremove -y -qq --purge"),
                Some(APT_INDEX)
            )
        );
        assert_eq!(APT_UPDATE.as_str(), "export DEBIAN_FRONTEND=noninteractive\napt-get update -qq");
        assert_eq!((road_module_named(RoadName::Brew).after(), road_module_named(RoadName::Npm).after()), (Some(HOMEBREW_STEP), Some("node")));
        assert_eq!(
            (install(&script_road("echo hi", None), "x"), uninstall(&script_road("echo hi", None), "hi")),
            (ok("echo hi"), note("hi has no uninstaller; left on the machine"))
        );
    }

    #[test]
    fn each_module_says_the_one_line_a_person_reads_for_an_install_whose_script_is_not() {
        let shown = |road: &InstallRoad, bin: &str| road_module(road).shown(road, bin);
        assert_eq!(shown(&InstallRoad::Brew { formula: "gh".to_string(), pin: None }, "x"), s("brew install gh"));
        assert_eq!(shown(&InstallRoad::Apt { packages: vec!["neovim".to_string(), "fd-find".to_string()], pin: None }, "x"), s("apt-get install neovim fd-find"));
        assert_eq!(shown(&release_road("cli/cli", None, None), "gh"), s("names no release tag; name the version"));
        assert_eq!(shown(&release_road("cli/cli", Some("v2.86.0"), None), "gh"), s("the v2.86.0 release of github.com/cli/cli"));
        assert_eq!(shown(&release_road("cli/cli", None, Some(pin_of("v2.86.0", &"d".repeat(64)))), "gh"), s("the v2.86.0 release of github.com/cli/cli"));
        assert_eq!(shown(&InstallRoad::Vendor { cask: GCLOUD.clone(), version: None, pin: None }, "x"), Some(GCLOUD.from.clone()));
        for road in [RoadName::Npm, RoadName::Pnpm, RoadName::Bun, RoadName::Uv, RoadName::Pipx, RoadName::Cargo, RoadName::Go, RoadName::Script] {
            assert!(shown(&sample_road(road), "x").is_none(), "{}", road.as_str());
        }
    }

    #[test]
    fn installs_every_road_inside_a_tree_a_workspace_on_a_computer_somebody_owns_can_se() {
        // Found on spoo, 2026-09-18: gh and every other Homebrew row was on the box and in no workspace of it, because
        // /home/linuxbrew is outside the trees a workspace overlays and outside the /root it binds, while the PATH
        // inside named the prefix all the same. A road that installs somewhere else is the same bug, so every road
        // says where it installs and this holds the lot of them to what a workspace can see.
        let mut visible: Vec<&str> = WORKSPACE_OVERLAID.to_vec();
        visible.push(GUEST_HOME);
        visible.extend(SHARED_TOOL_ROOTS);
        for road in ROADS {
            let roots = road_module_named(road).roots();
            assert!(!roots.is_empty(), "{}", road.as_str());
            for root in roots {
                assert!(root.starts_with('/'), "{} installs into {root}, which is no absolute path", road.as_str());
                let under: Vec<&&str> = visible.iter().filter(|tree| root == **tree || root.starts_with(&format!("{tree}/"))).collect();
                assert!(
                    !under.is_empty(),
                    "{} installs into {root}, which no workspace on a computer somebody owns can see: it belongs under {}, or the protocol's SHARED_TOOL_ROOTS gains its root and the daemon binds it in",
                    road.as_str(),
                    visible.join(", ")
                );
            }
        }
        // And every shared tool root is on the one PATH a machine's tools sit on: a root brought into a workspace whose
        // bin directories nothing names is a prefix no command is found in.
        for root in SHARED_TOOL_ROOTS {
            assert!(tools_path().split(':').any(|dir| dir.starts_with(&format!("{root}/"))), "{root}");
        }
        // The formula road's own prefix is that root, read from the protocol by both sides.
        assert!(BREW_PREFIX.starts_with(&format!("{}/", SHARED_TOOL_ROOTS[0])));
    }

    #[test]
    fn names_the_directories_each_script_links_its_commands_into_on_the_script_row_that() {
        // Six of the catalogue's scripts link into /usr/local/bin and two do not, so one directory on the road module
        // would put a false note on every box's rust and docker rows. The directories sit on each script's own row and
        // the module reads them off it.
        let script_bins = |id: &str| -> Option<Vec<String>> {
            match catalog_entry(id).map(|e| e.install_road()) {
                Some(InstallRoad::Script { bins, .. }) => bins.clone(),
                _ => None,
            }
        };
        for id in ["node", "uv", "python", "fd", "yarn", "claude"] {
            assert_eq!(script_bins(id), Some(vec!["/usr/local/bin".to_string()]), "{id}");
        }
        assert_eq!(script_bins("docker"), Some(vec!["/usr/bin".to_string()]));
        assert_eq!(script_bins("rust"), Some(vec![format!("{GUEST_HOME}/.cargo/bin")]));
        // A script whose row names none says nothing, which is a row that gets no note rather than a wrong one.
        assert_eq!(script_bins("hermes"), None);
    }

    #[test]
    fn gives_every_road_a_step_how_long_it_may_run_whether_a_run_that_hit_the_limit_is() {
        // A dead read fails in about a minute and is tried once more, on every road whose tool takes the knobs from its environment.
        assert_eq!((NET_READ_S, NET_RETRIES), (60, 1));
        let env = |road: RoadName| road_step(road).env.join("\n");
        assert_eq!(env(RoadName::Npm), "export npm_config_fetch_timeout=60000 npm_config_fetch_retries=1 npm_config_fetch_retry_maxtimeout=10000");
        assert_eq!(env(RoadName::Pnpm), env(RoadName::Npm));
        assert_eq!(env(RoadName::Pipx), "export PIP_TIMEOUT=60 PIP_RETRIES=1");
        assert_eq!(env(RoadName::Uv), "export UV_HTTP_TIMEOUT=60 UV_HTTP_RETRIES=1");
        assert_eq!(env(RoadName::Cargo), "export CARGO_HTTP_TIMEOUT=60 CARGO_NET_RETRY=1");
        assert_eq!(env(RoadName::Release), *CURL_NET);
        assert_eq!(env(RoadName::Vendor), *CURL_NET);
        // A script may run any of them, so it gets every line; Homebrew takes its retry count from its own environment.
        // A script road's step is a whole script whose last line may be a cleanup, so the road runs it under set -e: a
        // failed download must not read as an install on a machine that already carries the tool.
        let mut script = vec!["set -euo pipefail".to_string()];
        for road in [RoadName::Npm, RoadName::Pipx, RoadName::Uv, RoadName::Cargo] {
            script.extend(road_step(road).env);
        }
        script.push(CURL_NET.clone());
        assert_eq!(road_step(RoadName::Script).env, script);
        for road in ROADS {
            assert_eq!(road_step(road).env.contains(&"set -euo pipefail".to_string()), road == RoadName::Script, "{}", road.as_str());
        }
        assert!(BREW_ENV.contains("HOMEBREW_CURL_RETRIES=1"));
        for road in [RoadName::Brew, RoadName::Bun, RoadName::Go, RoadName::Apt] {
            assert_eq!(road_step(road).env, Vec::<String>::new(), "{}", road.as_str());
        }
        // Package managers and downloads get a shorter step than anything that may compile; a download that ran the clock out is tried once more, a compile is not.
        for road in [RoadName::Npm, RoadName::Pnpm, RoadName::Bun, RoadName::Uv, RoadName::Pipx, RoadName::Release, RoadName::Vendor] {
            let step = road_step(road);
            assert_eq!((step.limit_s, step.retry), (300, true), "{}", road.as_str());
        }
        for road in [RoadName::Brew, RoadName::Go, RoadName::Apt, RoadName::Script] {
            let step = road_step(road);
            assert_eq!((step.limit_s, step.retry), (600, false), "{}", road.as_str());
        }
        let cargo = road_step(RoadName::Cargo);
        assert_eq!((cargo.limit_s, cargo.retry), (1200, false));
    }

    #[test]
    fn the_curl_every_road_script_types_goes_through_one_function_that_clocks_the_conne() {
        // The function runs under this machine's bash, ahead of a curl stand-in on PATH that prints what reached it.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_stub(&dir.join("curl"), "#!/bin/sh\nprintf \"%s\\n\" \"$@\"\n");
        let path = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default());
        let out = Command::new("bash").args(["-c", &format!("{}\ncurl -o /tmp/x 'https://example.test/a b'", *CURL_NET)]).env("PATH", path).output().unwrap();
        assert!(out.status.success());
        // --silent drops the progress meter from stderr; --show-error keeps the one error line the reason rule reads.
        assert_eq!(
            stdout_of(&out).split('\n').filter(|l| !l.is_empty()).collect::<Vec<_>>(),
            ["--connect-timeout", "15", "--speed-limit", "1", "--speed-time", "60", "--retry", "1", "--fail", "--silent", "--show-error", "--location", "-o", "/tmp/x", "https://example.test/a b"]
        );
    }

    #[test]
    fn the_default_agent_is_the_vendor_s_own_binary_at_a_pinned_version_checked_against() {
        // The image's setup takes off the file an image sealed before held under the home, first on its PATH; the row's
        // script names no folder under the home, since a box shares that home with every workspace on it.
        assert_eq!(GOLDEN_SETUP.as_str(), format!("rm -f /root/.local/bin/claude\n{}", *CLAUDE_INSTALL));
        assert!(re(r"^\d+\.\d+\.\d+$").is_match(CLAUDE_CODE.version));
        assert_eq!(
            CLAUDE_INSTALL.as_str(),
            [
                r#"arch="$(uname -m)""#.to_string(),
                r#"case "$arch" in"#.to_string(),
                format!("  x86_64) plat=linux-x64 sha={} ;;", CLAUDE_CODE.sha256.x86_64),
                format!("  aarch64) plat=linux-arm64 sha={} ;;", CLAUDE_CODE.sha256.aarch64),
                r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#.to_string(),
                "esac".to_string(),
                "trap 'rm -f /tmp/claude' EXIT".to_string(),
                format!(r#"curl -o /tmp/claude "https://downloads.claude.ai/claude-code-releases/{}/$plat/claude""#, CLAUDE_CODE.version),
                r#"echo "$sha  /tmp/claude" | sha256sum -c - >/dev/null"#.to_string(),
                format!("install -D -m 0755 /tmp/claude {LOCAL_BIN}/claude"),
            ]
            .join("\n")
        );
        for (arch, sum) in [("x86_64", CLAUDE_CODE.sha256.x86_64), ("aarch64", CLAUDE_CODE.sha256.aarch64)] {
            assert!(re(r"^[0-9a-f]{64}$").is_match(sum), "{arch}");
        }
        // The row installs into the one directory it names, at the version its own text fixes, so a copy gets that version.
        let claude = entry("claude");
        let road = claude.install_road();
        assert_eq!(
            road,
            &InstallRoad::Script { script: CLAUDE_INSTALL.clone(), version: s(CLAUDE_CODE.version), pin: None, bins: Some(vec![LOCAL_BIN.to_string()]) }
        );
        assert!(fixes_version(road));
        assert_eq!(road_module(road).bins(road, None), vec![LOCAL_BIN.to_string()]);
        // A box's job reads the row under its prefix and finds the harness in the same folder, which its PATH holds.
        assert_eq!(road_module(road).bins(road, Some(&install_homes(Some(TOOL_PREFIX)))), vec![LOCAL_BIN.to_string()]);
        // Nothing downloads a script and runs it any more: the module that did is gone with the road.
        for e in catalog() {
            assert!(!re(r#"\bbash "\$f""#).is_match(&line_of(e)), "{}", e.id());
        }
    }

    #[test]
    fn the_agent_s_install_leaves_no_download_behind_on_a_refused_sum_it_runs_here_with() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // A local file stands in for the vendor's 215 MB download, so the refusal is read without touching the network.
        fs::write(dir.join("served"), "not the agent's binary\n").unwrap();
        let flip = |sha: &str| format!("{}{}", if sha.starts_with('0') { "1" } else { "0" }, &sha[1..]);
        let download = dir.join("download");
        let installed = dir.join("bin").join("claude");
        // The script road's step carries `set -euo pipefail`, pinned above, so a refused sum ends the script where it stands.
        // The arch is forced so the case reads the checksum road on any machine that runs the suite, not the arch word of the machine's own uname.
        let script = format!("set -euo pipefail\n{}", *CLAUDE_INSTALL)
            .replacen(r#"arch="$(uname -m)""#, "arch=x86_64", 1)
            .replacen(&format!("{LOCAL_BIN}/claude"), installed.to_str().unwrap(), 1)
            .replace("/tmp/claude", download.to_str().unwrap())
            .replacen(
                &format!(r#""https://downloads.claude.ai/claude-code-releases/{}/$plat/claude""#, CLAUDE_CODE.version),
                &format!("\"file://{}\"", dir.join("served").display()),
                1,
            )
            .replace(CLAUDE_CODE.sha256.x86_64, &flip(CLAUDE_CODE.sha256.x86_64))
            .replace(CLAUDE_CODE.sha256.aarch64, &flip(CLAUDE_CODE.sha256.aarch64));
        let run = run_install(dir, &script);
        assert_eq!(run.status.code(), Some(1));
        assert!(stderr_of(&run).contains("sha256sum"));
        assert!(!download.exists());
        assert!(!installed.exists());
    }

    #[test]
    fn no_road_line_spells_curl_s_flags_itself_the_function_is_their_one_home() {
        let own = re(r"\bcurl +-[A-Za-z]*[fsSL]\b");
        let mut lines: Vec<(String, bool)> = catalog().iter().map(|e| (line_of(e), true)).collect();
        lines.extend(linux_casks().iter().map(|c| (c.install.clone(), true)));
        let release_lines = [
            install(&InstallRoad::Release { repo: s("cli/cli"), version: None, assets: None, pin: None, go: s("github.com/cli/cli/v2/cmd/gh") }, "gh"),
            install(&release_road("spoo-me/spoo-cli", Some("v0.4.1"), Some(pin_of("v0.4.1", &"b".repeat(64)))), "spoo"),
        ];
        for line in release_lines {
            match line {
                Ok(text) => lines.push((text, true)),
                Err(RoadNote(text)) => lines.push((text, false)),
            }
        }
        lines.push((UV_INSTALL.clone(), true));
        lines.push((node_install_script(22, &node_release(22).unwrap()), true));
        lines.push((DOCKER_INSTALL.clone(), true));
        for (text, _) in &lines {
            assert!(!own.is_match(text), "{text}");
        }
        assert!(lines.iter().filter(|(text, is_script)| *is_script && re(r"\bcurl\b").is_match(text)).count() >= 8);
    }

    #[test]
    fn names_the_evidence_behind_every_default_sessions_on_this_mac_and_lab_images_that() {
        for e in catalog() {
            assert!(source_of(e).images <= 5, "{}", e.id());
        }
        assert_eq!(
            catalog().iter().filter(|e| e.kind() == EntryKind::Tool && e.as_tool().unwrap().default_on).map(|e| e.id()).collect::<Vec<_>>(),
            vec!["curl", "node", "pnpm", "uv", "python", "git", "jq", "ripgrep", "build-essential", "fd", "sqlite3", "wget", "zip", "xz", "rsync", "gh", "agent-browser"]
        );
        assert!(re("one Mac").is_match(source_of(entry("agent-browser")).note.as_deref().unwrap_or("")));
    }

    #[test]
    fn seeds_every_golden_with_the_base_floor_the_entries_flagged_for_it_in_catalog_ord() {
        let floor = base_floor();
        // curl leads: a base image need not ship one, and every road below that fetches a release types it.
        assert_eq!(floor.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["curl", "uv", "python", "git", "jq", "ripgrep", "build-essential", "fd", "sqlite3", "wget", "zip", "xz", "rsync"]);
        let flagged: Vec<&ToolEntry> = catalog().iter().filter_map(|e| e.as_tool()).filter(|t| t.floor).collect();
        assert_eq!(floor, flagged);
        for (i, e) in floor.iter().enumerate() {
            assert!(e.default_on, "{}", e.id);
            assert_ne!(e.install_road.name(), RoadName::Release, "{}", e.id);
            if let InstallRoad::Npm { package, .. } = &e.install_road {
                assert!(package.version.is_some(), "{}", e.id);
            }
            // A row that waits on another floor row waits on one the catalog lists before it.
            if let Some(dep) = install_after(e) {
                if dep != "apt-index" {
                    let at = floor.iter().position(|x| x.id == dep).unwrap_or_else(|| panic!("{}: {dep} is not on the floor", e.id));
                    assert!(at < i, "{}", e.id);
                }
            }
        }
        assert_eq!(
            catalog().iter().filter_map(|e| e.as_tool()).filter(|t| t.default_on && !t.floor).map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["node", "pnpm", "gh", "agent-browser"]
        );
        // An apt package runs on the index read once, a script on what its entry names.
        let afters: Vec<Option<String>> = floor.iter().map(|e| install_after(e)).collect();
        let want: Vec<Option<String>> = ["apt-index", "curl", "uv", "apt-index", "apt-index", "apt-index", "apt-index", "apt-index", "apt-index", "apt-index", "apt-index", "apt-index", "apt-index"]
            .iter()
            .map(|a| s(a))
            .collect();
        assert_eq!(afters, want);
        // Node and pnpm are rows a recipe ticks: an image whose recipe asks for neither carries no node at all, and the
        // npm road brings node itself where a ticked row walks it.
        let node = tool("node");
        assert_eq!((node.floor, node.default_on), (false, true));
        assert_eq!(node.brings, Some(vec![Brings { bin: "npm".to_string(), version: "npm --version".to_string() }]));
        assert_eq!(install_after(node), s("curl"));
        let pnpm = tool("pnpm");
        assert_eq!((pnpm.floor, pnpm.default_on), (false, true));
        assert_eq!(install_after(pnpm), s("node"));
        // Docker is a row a person ticks and never the floor's: half a gigabyte on every image that carries it, and a
        // guest whose kernel has no overlayfs cannot start it. Its engine is by apt, so its script waits on the index
        // read as the apt rows do.
        let docker = tool("docker");
        assert_eq!((docker.floor, docker.default_on), (false, false));
        assert_eq!(docker.brings, Some(vec![Brings { bin: "docker compose".to_string(), version: "docker compose version".to_string() }]));
        assert_eq!(docker.install_road.name(), RoadName::Script);
        assert_eq!(install_after(docker), s(APT_INDEX));
        // Python comes as uv's managed 3.12, pinned by uv's own release, and python3 on PATH is that interpreter.
        let python = tool("python");
        let InstallRoad::Script { script, .. } = &python.install_road else { panic!("python is no script road") };
        assert!(script.contains("uv python install 3.12"));
        assert!(script.contains(r#"ln -sfn "$(uv python find --managed-python 3.12)" /usr/local/bin/python3"#));
        assert!(script.contains("sha256sum -c"));
        assert_eq!(python.size, Size::Measured { bytes: 108105728, on: "2026-09-07".to_string(), method: SizeMethod::Du });
    }

    #[test]
    fn names_the_base_row_a_recipe_s_tools_row_stands_for_by_id_command_road_argument_o() {
        assert_eq!(base_id("jq"), Some("jq"));
        assert_eq!(base_id("rg"), Some("ripgrep"));
        assert_eq!(base_id("python@3.12"), Some("python"));
        assert_eq!(base_id("python3"), Some("python"));
        // A name a row covers whose row is not on the floor stands for no base row: the tick installs it in the tools
        // stage like any other.
        assert_eq!(tool_id("docker-compose"), Some("docker"));
        assert_eq!(base_id("docker-compose"), None);
        assert_eq!(tool_id("pnpm"), Some("pnpm"));
        assert_eq!(base_id("pnpm"), None);
        assert_eq!(tool_id("node@22"), Some("node"));
        assert_eq!(base_id("node"), None);
        assert_eq!(tool_id("node@24"), None);
        assert_eq!(base_id("python@3.14"), None);
        assert_eq!(base_id("git"), Some("git"));
        assert_eq!(tool_id("npm"), Some("node"));
        assert_eq!(base_id("npm"), None);
        assert_eq!(base_id("gh"), None);
        assert_eq!(base_id("agent-browser"), None);
    }

    #[test]
    fn a_covered_row_s_note_names_the_computer_that_was_read_and_both_majors_when_it_ru() {
        let python = base_entry_for("python").unwrap();
        assert_eq!(base_note(python, Some("3.14.0"), Platform::Darwin), "Python 3.12 is part of the base; this Mac runs Python 3.14");
        assert_eq!(base_note(python, Some("v3.12.7"), Platform::Darwin), "Python 3.12 is part of the base");
        assert_eq!(base_note(python, None, Platform::Darwin), "Python 3.12 is part of the base");
        // A note built on a Linux computer names that computer: the platform read, never the one the sentence was written for.
        assert_eq!(base_note(python, Some("3.14.0"), Platform::Linux), "Python 3.12 is part of the base; this computer runs Python 3.14");
        assert_eq!(base_note(python, Some("3.12.7"), Platform::Linux), "Python 3.12 is part of the base");
        // Only a row that pins a major has one to compare; the rest name the base row whatever version was read here.
        assert_eq!(base_note(base_entry_for("uv").unwrap(), Some("10.0.0"), Platform::Darwin), "uv is part of the base");
        assert_eq!(base_note(base_entry_for("jq").unwrap(), Some("1.6"), Platform::Linux), "jq is part of the base");
    }

    #[test]
    fn says_which_roads_no_guest_has_run_yet() {
        let unmeasured: Vec<&str> = catalog().iter().filter(|e| source_of(e).road == RoadEvidence::Unmeasured).map(|e| e.id()).collect();
        assert_eq!(
            unmeasured,
            vec![
                "crush", "qwen", "goose", "amp", "cursor", "curl", "pnpm", "uv", "python", "git", "jq", "ripgrep", "build-essential", "fd", "sqlite3", "wget", "zip", "xz", "rsync", "gh",
                "agent-browser", "docker", "rust", "maven", "bun", "yarn", "ruff", "black", "mypy", "pyright", "pytest", "prettier", "eslint", "typescript", "wrangler", "cloudflared",
                "kubectl", "aws", "vercel", "netlify", "fly", "supabase", "railway", "doppler", "op", "ffmpeg", "yq", "git-lfs", "tmux", "ruby", "php", "postgresql-client", "redis-tools",
                "golangci-lint", "mise", "git-delta", "shellcheck", "swift", "elixir", "bazel", "llvm", "playwright",
            ]
        );
        assert_eq!(
            catalog_agents().iter().filter(|a| a.source.road != RoadEvidence::Measured).map(|a| a.id.as_str()).collect::<Vec<_>>(),
            vec!["crush", "qwen", "goose", "amp", "cursor"]
        );
    }

    #[test]
    fn gives_every_row_a_size_in_bytes_with_the_day_and_the_way_it_was_measured_or_the() {
        for e in catalog() {
            match e.size() {
                Size::Measured { bytes, on, .. } => {
                    assert!(*bytes > 0, "{}", e.id());
                    assert!(re(r"^2026-\d\d-\d\d$").is_match(on), "{}", e.id());
                    assert_eq!(size_bytes(e.size()), Some(*bytes), "{}", e.id());
                }
                Size::Unmeasured { unmeasured } => {
                    assert!(unmeasured.chars().count() > 20, "{}", e.id());
                    assert_eq!(size_bytes(e.size()), None, "{}", e.id());
                }
            }
        }
        let methods = [SizeMethod::Df, SizeMethod::Apt, SizeMethod::Brew, SizeMethod::Du, SizeMethod::Unpacked];
        for e in catalog() {
            if let Size::Measured { method, .. } = e.size() {
                assert!(methods.contains(method), "{}", e.id());
            }
        }
        // Qwen Code and Amp install by npm, whose bytes only an install could read.
        assert_eq!(catalog().iter().filter(|e| matches!(e.size(), Size::Unmeasured { .. })).map(|e| e.id()).collect::<Vec<_>>(), vec!["qwen", "amp"]);
        for method in methods {
            assert!(!size_method_words(method).contains('\u{2014}'), "{method:?}");
        }
        // The du rows were read on a Debian bookworm host, the node:22-bookworm image among them; the label says the host, not one image.
        assert_eq!(size_method_words(SizeMethod::Du), "du over what the install wrote, before and after, on a Debian bookworm host");
        for e in catalog() {
            let text = format!("{} {}", e.name(), source_of(e).note.as_deref().unwrap_or(""));
            assert!(!text.contains('\u{2014}'), "{}", e.id());
        }
    }

    #[test]
    fn carries_the_tier_1_rows_the_lab_sandboxes_ship_the_cheap_universal_ones_on_the_f() {
        // curl leads: a base image need not ship one, and every road below that fetches a release types it.
        assert_eq!(
            base_floor().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["curl", "uv", "python", "git", "jq", "ripgrep", "build-essential", "fd", "sqlite3", "wget", "zip", "xz", "rsync"]
        );
        assert_eq!(line_of(entry("build-essential")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq build-essential cmake ninja-build");
        assert_eq!(
            tool("build-essential").brings,
            Some(vec![
                Brings { bin: "cmake".to_string(), version: "cmake --version".to_string() },
                Brings { bin: "ninja".to_string(), version: "ninja --version".to_string() }
            ])
        );
        assert_eq!(tool_id("make"), Some("build-essential"));
        assert_eq!(tool_id("gcc"), Some("build-essential"));
        assert_eq!(tool_id("cmake"), Some("build-essential"));
        // Debian ships fd as fdfind; the row puts the name agents type on PATH.
        assert_eq!(line_of(entry("fd")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq fd-find\nln -sfn /usr/bin/fdfind /usr/local/bin/fd");
        assert_eq!(install_after(tool("fd")), s(APT_INDEX));
        assert_eq!(tool_id("fdfind"), Some("fd"));
        assert_eq!(tool_id("unzip"), Some("zip"));
        assert_eq!(tool_id("xz-utils"), Some("xz"));
        assert_eq!(tool("zip").brings, Some(vec![Brings { bin: "unzip".to_string(), version: "unzip -v".to_string() }]));
        let optional = ["bun", "yarn", "ruff", "black", "mypy", "pyright", "pytest", "prettier", "eslint", "typescript"];
        for id in optional {
            let e = entry(id);
            assert_eq!(e.kind(), EntryKind::Tool, "{id}");
            let t = e.as_tool().unwrap();
            assert!(!t.default_on, "{id}");
            assert!(!t.floor, "{id}");
        }
        assert_eq!(line_of(entry("bun")), "npm install -g bun");
        // An exact version, not corepack's moving stable alias: an image built a month later takes the same yarn.
        assert_eq!(line_of(entry("yarn")), format!("corepack enable yarn\nCOREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack install -g yarn@{YARN_VERSION}"));
        assert!(re(r"^\d+\.\d+\.\d+$").is_match(YARN_VERSION));
        assert_eq!(install_after(tool("yarn")), s("node"));
        assert_eq!(line_of(entry("ruff")), "uv tool install ruff");
        assert_eq!(line_of(entry("typescript")), "npm install -g typescript");
        assert_eq!(entry("typescript").bin(), "tsc");
        assert_eq!(tool_id("tsc"), Some("typescript"));
        assert_eq!(tool_id("bunx"), Some("bun"));
    }

    #[test]
    fn carries_the_tier_2_and_tier_3_rows_the_lab_sandboxes_ship_and_a_browser_for_the() {
        let rows = ["ruby", "php", "postgresql-client", "redis-tools", "golangci-lint", "mise", "git-delta", "shellcheck", "swift", "elixir", "bazel", "llvm", "playwright", "op"];
        for id in rows {
            let e = entry(id);
            assert_eq!(e.kind(), EntryKind::Tool, "{id}");
            let t = e.as_tool().unwrap();
            assert_eq!((t.default_on, t.floor), (false, false), "{id}");
            assert!(size_bytes(&t.size).unwrap_or(0) > 0, "{id}");
        }
        // The apt rows, with what each brings along under its own name.
        assert_eq!(line_of(entry("ruby")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq ruby ruby-dev ruby-bundler");
        assert_eq!(tool_id("bundle"), Some("ruby"));
        assert_eq!(tool_id("bundler"), Some("ruby"));
        assert_eq!(tool_id("gem"), Some("ruby"));
        assert_eq!(line_of(entry("php")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq php-cli composer php-mbstring php-xml php-curl php-zip");
        assert_eq!(tool_id("composer"), Some("php"));
        assert_eq!(tool_id("psql"), Some("postgresql-client"));
        assert_eq!(tool_id("pg_dump"), Some("postgresql-client"));
        assert_eq!(tool_id("redis-cli"), Some("redis-tools"));
        assert_eq!(tool_id("redis"), Some("redis-tools"));
        assert_eq!(line_of(entry("shellcheck")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq shellcheck");
        assert_eq!(line_of(entry("elixir")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq elixir");
        assert_eq!(tool_id("erlang"), Some("elixir"));
        assert_eq!(tool_id("mix"), Some("elixir"));
        assert_eq!(line_of(entry("llvm")), "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq clang clang-format clang-tidy");
        assert_eq!(tool_id("clang-tidy"), Some("llvm"));
        for id in ["ruby", "php", "postgresql-client", "redis-tools", "shellcheck", "elixir", "llvm"] {
            assert_eq!(install_after(tool(id)), s(APT_INDEX), "{id}");
        }
        // The release rows: each repository's pinned Linux asset, downloaded from the tag's own address and checked.
        assert!(line_of(entry("golangci-lint")).contains(r#"url='https://github.com/golangci/golangci-lint/releases/download/v2.13.2/'"$asset""#));
        assert!(line_of(entry("mise")).contains("asset='mise-v2026.9.12-linux-x64'"));
        assert!(line_of(entry("git-delta")).contains("name='delta'"));
        assert!(line_of(entry("git-delta")).contains(r#"url='https://github.com/dandavison/delta/releases/download/0.19.2/'"$asset""#));
        // bazelisk goes on PATH under the name agents type, and covers the name it is published under.
        assert!(line_of(entry("bazel")).contains("name='bazel'"));
        assert!(line_of(entry("bazel")).contains("asset='bazelisk-linux-amd64'"));
        assert_eq!(tool_id("bazelisk"), Some("bazel"));
        // Swift from swift.org's Debian 12 tarball, checksummed, under its own prefix, after the apt index its dependencies need.
        let swift = line_of(entry("swift"));
        assert_eq!(
            swift.split('\n').take(2).collect::<Vec<_>>(),
            [
                "export DEBIAN_FRONTEND=noninteractive",
                "apt-get install -y -qq binutils libicu-dev libcurl4-openssl-dev libedit-dev libsqlite3-dev libncurses-dev libpython3-dev libxml2-dev pkg-config uuid-dev tzdata git gcc libstdc++-12-dev"
            ]
        );
        assert!(swift.contains(r#"curl -o "/tmp/$pkg" "https://download.swift.org/swift-6.3.3-release/$dir/swift-6.3.3-RELEASE/$pkg""#));
        assert!(swift.contains("  x86_64) dir=debian12 sha=19e0c78cad5418ad48bfa87aa20c53ac9ac9996d1695d04dd94f7c7ea4eb133f ;;"));
        assert!(swift.contains("  aarch64) dir=debian12-aarch64 sha=ecba8ef87b54a5048d466af500f3169c939a6b8a2cb7c600f76b5184457f293a ;;"));
        assert!(swift.contains(r#"echo "$sha  /tmp/$pkg" | sha256sum -c - >/dev/null"#));
        assert!(swift.contains(r#"tar -xzf "/tmp/$pkg" -C /opt/swift --strip-components=1"#));
        assert!(swift.contains("ln -sfn /opt/swift/usr/bin/swift /usr/local/bin/swift"));
        assert_eq!(install_after(tool("swift")), s(APT_INDEX));
        assert_eq!(tool_id("swiftc"), Some("swift"));
        // The browser: Playwright's own Chromium at the Playwright the render tests import, on the node its npm road
        // brings; the row answers to the package names a project depends on.
        assert_eq!(
            line_of(entry("playwright")),
            [
                "export DEBIAN_FRONTEND=noninteractive",
                "npm install -g playwright@1.62.1",
                r#"command -v playwright >/dev/null || { b="$(npm prefix -g)/bin"; case "$b" in /root/*) ;; *) PATH="$b:$PATH" ;; esac; }"#,
                "playwright install --with-deps chromium",
            ]
            .join("\n")
        );
        assert_eq!(install_after(tool("playwright")), s("node"));
        assert_eq!(tool_id("chromium"), Some("playwright"));
        // A project's npm dependency lands on a row only through the names the row lists for it: the browser's packages
        // do, a client library named like a server's tools does not, and neither does a command or cover word.
        for pkg in ["playwright", "@playwright/test", "playwright-core"] {
            assert_eq!(catalog_tool_for_dependency(RoadName::Npm, pkg).map(|t| t.id.as_str()), Some("playwright"), "{pkg}");
        }
        for pkg in ["redis", "sqlite", "sqlite3", "chromium", "bun", "eslint", "typescript", "pg"] {
            assert_eq!(catalog_tool_for_dependency(RoadName::Npm, pkg).map(|t| t.id.as_str()), None, "{pkg}");
        }
        assert_eq!(catalog_tool_for_dependency(RoadName::Uv, "playwright").map(|t| t.id.as_str()), None);
        // The pinned Playwright is the one the app manifests pin, so the golden's Chromium is the revision the render tests look for.
        for app in ["web", "desktop"] {
            let text = fs::read_to_string(format!("{}/../../../apps/{app}/package.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let manifest: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(manifest["devDependencies"]["playwright"].as_str(), Some(PLAYWRIGHT_VERSION), "{app}");
        }
        assert_eq!(entry("playwright").size(), &Size::Measured { bytes: 1015808000, on: "2026-09-07".to_string(), method: SizeMethod::Du });
        // 1Password: its repository under its pinned key, that one index read, then the package; the sum is checked before the key is trusted.
        let op = line_of(entry("op"));
        assert_eq!(
            op.split('\n').collect::<Vec<_>>(),
            [
                "export DEBIAN_FRONTEND=noninteractive",
                "curl -o /usr/share/keyrings/1password-archive-keyring.asc https://downloads.1password.com/linux/keys/1password.asc",
                r#"echo "f39e7dd9dedc581ced85732832f217e0de5860a3b80279b5af4bc7c6d8157bae  /usr/share/keyrings/1password-archive-keyring.asc" | sha256sum -c - >/dev/null"#,
                r#"arch="$(dpkg --print-architecture)""#,
                r#"echo "deb [arch=$arch signed-by=/usr/share/keyrings/1password-archive-keyring.asc] https://downloads.1password.com/linux/debian/$arch stable main" > /etc/apt/sources.list.d/1password.list"#,
                "apt-get update -qq -o Dir::Etc::sourcelist=/etc/apt/sources.list.d/1password.list -o Dir::Etc::sourceparts=- -o APT::Get::List-Cleanup=0",
                "apt-get install -y -qq 1password-cli",
            ]
        );
        assert_eq!(tool_id("1password-cli"), Some("op"));
    }

    #[test]
    fn pipes_a_download_into_a_shell_on_no_road_the_harness_vendor_s_own_installer_is_s() {
        let piped = re(r"curl[^\n]*\|\s*(ba)?sh");
        assert_eq!(catalog().iter().filter(|e| piped.is_match(&line_of(e))).map(|e| e.id()).collect::<Vec<_>>(), Vec::<&str>::new());
    }

    #[test]
    fn carries_no_token_looking_value() {
        let text = serde_json::to_string(catalog()).unwrap().replace(r#""token":"sk-ant-oat01-[A-Za-z0-9_-]{20,}""#, r#""token":{}"#);
        assert!(!re(r"gho_|sk-ant|ya29\.|AKIA").is_match(&text));
    }
}
