// SPDX-License-Identifier: AGPL-3.0-only
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
    assert_eq!(
        block_on(seed.links(&reader)),
        vec![("cache/claude-plugins-official/vercel/882e66c26986".to_string(), "0.50.0".to_string())]
    );
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
    let travelling: Vec<&String> =
        claude.config_paths.iter().chain(claude.volatile.iter().flatten()).filter(|p| p.contains("/plugins/")).collect();
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
            InstallRoad::Pnpm(package)
            | InstallRoad::Bun(package)
            | InstallRoad::Uv(package)
            | InstallRoad::Pipx(package)
            | InstallRoad::Cargo(package) => {
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
    assert!(install(&release_road("cli/cli", None, Some(pin.clone())), "gh")
        .unwrap()
        .contains(&format!("  {}\n", pin_check_line("$asset", "$tag", &sum))));
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
        assert!(
            road_module_named(RoadName::Vendor).install(&asked, &cask.bin, None).unwrap().contains(&format!("ver='{}'", cask.version)),
            "{}",
            cask.bin
        );
        let pinned = InstallRoad::Vendor { cask: cask.clone(), version: None, pin: Some(pin_of("1.0.0", &"a".repeat(64))) };
        assert!(
            road_module_named(RoadName::Vendor).install(&pinned, &cask.bin, None).unwrap().contains(&format!("ver='{}'", cask.version)),
            "{}",
            cask.bin
        );
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
    assert_eq!(
        install(&InstallRoad::Pnpm(package_road("wrangler", None, Some(pin.clone()))), "wrangler"),
        ok("pnpm add -g wrangler@4.1.0")
    );
    assert_eq!(install(&InstallRoad::Uv(package_road("ruff", None, Some(tag_pin("0.4.4")))), "ruff"), ok("uv tool install ruff==0.4.4"));
    assert_eq!(
        install(&InstallRoad::Cargo(package_road("bat", None, Some(tag_pin("0.24.0")))), "bat"),
        ok("cargo install bat --version 0.24.0 --locked")
    );
    assert_eq!(
        install(&go_road(Some("golang.org/x/tools/gopls"), None, Some(tag_pin("v0.16.2"))), "gopls"),
        ok("go install golang.org/x/tools/gopls@v0.16.2")
    );
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
    assert_eq!(
        installed(&go_road(Some("golang.org/x/tools/gopls"), None, None), "gopls"),
        r#"go version -m /root/go/bin/'gopls' 2>/dev/null | awk '$1=="mod"{print $3}'"#
    );
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
    let gemini: [(&str, Option<AgentSignInKind>); 3] =
        [("oauth_creds.json", Some(AgentSignInKind::Oauth)), ("GEMINI_API_KEY", Some(AgentSignInKind::ApiKey)), ("", None)];
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
        Some(&SignIn::None(NoSignIn {
            kind: SignInKind::None,
            note: s(why),
            sources: vec![LoginSource::File],
            status: login.status.clone(),
            state_on_machine: None
        }))
    );
    assert_eq!(
        keys_row_of(&KeyFiles { paths: vec!["~/.x/keys".to_string()], note: "why".to_string() }, None),
        SignIn::None(NoSignIn {
            kind: SignInKind::None,
            note: s("why"),
            sources: vec![LoginSource::File],
            status: None,
            state_on_machine: None
        })
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
        agents
            .iter()
            .filter(|a| a.guest_state_home.is_some())
            .map(|a| (a.id.as_str(), a.guest_state_home.clone().unwrap()))
            .collect::<Vec<_>>(),
        vec![("claude", "/root/.claude-cfg".to_string())]
    );
    // The variable a person's config folder for that agent is laid in as; an agent with none takes no config folder.
    assert_eq!(
        agents
            .iter()
            .filter(|a| a.state_home_env.is_some())
            .map(|a| (a.id.as_str(), a.state_home_env.clone().unwrap()))
            .collect::<Vec<_>>(),
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
        vec![
            ("claude", "claude update".to_string()),
            ("codex", "npm i -g @openai/codex@latest".to_string()),
            ("opencode", "opencode upgrade".to_string())
        ]
    );
    assert_eq!(line_of(entry("codex")), "npm install -g @openai/codex@0.155.1");
    assert_eq!(line_of(entry("pi")), "npm install -g --ignore-scripts @earendil-works/pi-coding-agent@0.84.4");
    assert_eq!(line_of(entry("claude")), CLAUDE_INSTALL.as_str());
    assert!(re(r"git clone -q --depth 1 --branch v[\d.]+ https://github\.com/NousResearch/hermes-agent\.git")
        .is_match(&line_of(entry("hermes"))));
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
    assert_eq!(
        install_shown(entry("qwen")),
        Ok(InstallShown::Line("npm install -g --ignore-scripts @qwen-code/qwen-code@0.24.5".to_string()))
    );
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
    let listing = ["amd64", "arm64"]
        .iter()
        .map(|a| format!("\"browser_download_url\": \"https://example.com/spoo_0.4.1_linux_{a}\""))
        .collect::<Vec<_>>()
        .join(",");
    write_stub(
        &bin.join("curl"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = -o ]; then cp \"{}\" \"$2\"; else printf '%s\\n' '{}'; fi\n",
            dir.join("served").display(),
            listing
        ),
    );
    // A Mac has shasum and no sha256sum; the Linux machines this script targets have the coreutils one.
    write_stub(&bin.join("sha256sum"), SHA256SUM_STUB);
    let run = |arch: &str, pin: ToolPin| -> Output {
        let road = release_road("spoo-me/spoo-cli", Some("v0.4.1"), Some(pin));
        let script = install(&road, "spoo")
            .unwrap()
            .replacen(r#"arch="$(uname -m)""#, &format!("arch={arch}"), 1)
            .replace("/usr/local/bin", bin.to_str().unwrap());
        Command::new("bash")
            .args(["--noprofile", "--norc", "-c", &script])
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HOME", dir)
            .output()
            .unwrap()
    };
    let recorded =
        |arch: Option<Arch>, sha256: &str| ToolPin { tag: "v0.4.1".to_string(), sha256: Some(sha256.to_string()), arch, latest: None };
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
                let InstallRoad::Release { repo: road, .. } = &a.install_road else {
                    panic!("{}: latest is github, the road is not a release", a.id)
                };
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
    assert_eq!(
        catalog_agents().iter().filter(|a| a.about.repo.is_none()).map(|a| a.id.as_str()).collect::<Vec<_>>(),
        vec!["amp", "cursor"]
    );
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
    assert!(re(r#"^su -s /bin/bash linuxbrew -c '\[ -d "\$PWD" \][\s\S]*HOMEBREW_NO_AUTO_UPDATE=1[\s\S]*brew install go'$"#)
        .is_match(&line_of(entry("go"))));
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
    let x86_only =
        ReleaseAssets { x86_64: Some(ReleaseAsset { name: "gh_linux_amd64.tar.gz".to_string(), sha256: "e".repeat(64) }), aarch64: None };
    let armless = install(
        &InstallRoad::Release {
            repo: s("cli/cli"),
            version: s("v2.86.0"),
            assets: Some(x86_only.clone()),
            pin: None,
            go: s("github.com/cli/cli/v2/cmd/gh"),
        },
        "gh",
    )
    .unwrap();
    assert!(armless.contains("aarch64) asset= sha= ;;"));
    assert!(armless.contains("go install 'github.com/cli/cli/v2/cmd/gh@v2.86.0'"));
    let no_go =
        install(&InstallRoad::Release { repo: s("cli/cli"), version: s("v2.86.0"), assets: Some(x86_only), pin: None, go: None }, "gh")
            .unwrap();
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
