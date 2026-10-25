// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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
        install(
            &InstallRoad::Npm {
                package: package_road("@earendil-works/pi-coding-agent", Some("0.84.4"), None),
                ignore_scripts: Some(true)
            },
            "x"
        ),
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
    assert!(
        tagged.contains(r#"echo "Error: release "'v0.4.1'" of "'spoo-me/spoo-cli'" has no Linux build, and go is not on the machine" >&2"#)
    );
    assert!(install(&spoo("v0.4.1", Some("github.com/spoo-me/spoo-cli@v0.4.0"), None), "spoo")
        .unwrap()
        .contains("go install 'github.com/spoo-me/spoo-cli@v0.4.0'"));
    assert!(!install(&spoo("v0.4.1", None, None), "spoo").unwrap().contains("command -v go"));
    // A road that pins a version takes a row's through at(); Homebrew, apt and a script install what their source serves and have none.
    assert_eq!(
        road_module_named(RoadName::Npm).at(&npm_road("wrangler", None, None), "4.1.0"),
        Some(npm_road("wrangler", Some("4.1.0"), None))
    );
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
    assert!(install(&spoo("v0.4.1", None, Some(pin_of("v0.4.1", &"c".repeat(64)))), "spoo")
        .unwrap()
        .contains(&format!("[ \"$sum\" = '{}' ]", "c".repeat(64))));
    assert!(!install(&spoo("v0.4.1", None, Some(pin_of("v0.4.0", &"c".repeat(64)))), "spoo").unwrap().contains(r#"[ "$sum" ="#));
    // No version: a pin fixes the tag and is checked, as a vendor install does; with neither there is no release to read and the row is noted.
    assert!(install(&release_road("cli/cli", None, Some(pin_of("v2.86.0", &"d".repeat(64)))), "gh")
        .unwrap()
        .contains("releases/tags/v2.86.0"));
    let aarch64_pin = ToolPin { arch: Some(Arch::Aarch64), ..pin_of("v2.86.0", &"d".repeat(64)) };
    assert!(install(&release_road("cli/cli", None, Some(aarch64_pin)), "gh")
        .unwrap()
        .contains(&format!("[ \"$sum\" = '{}' ]", "d".repeat(64))));
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
    assert_eq!(
        shown(&InstallRoad::Apt { packages: vec!["neovim".to_string(), "fd-find".to_string()], pin: None }, "x"),
        s("apt-get install neovim fd-find")
    );
    assert_eq!(shown(&release_road("cli/cli", None, None), "gh"), s("names no release tag; name the version"));
    assert_eq!(shown(&release_road("cli/cli", Some("v2.86.0"), None), "gh"), s("the v2.86.0 release of github.com/cli/cli"));
    assert_eq!(
        shown(&release_road("cli/cli", None, Some(pin_of("v2.86.0", &"d".repeat(64)))), "gh"),
        s("the v2.86.0 release of github.com/cli/cli")
    );
    assert_eq!(shown(&InstallRoad::Vendor { cask: GCLOUD.clone(), version: None, pin: None }, "x"), Some(GCLOUD.from.clone()));
    for road in
        [RoadName::Npm, RoadName::Pnpm, RoadName::Bun, RoadName::Uv, RoadName::Pipx, RoadName::Cargo, RoadName::Go, RoadName::Script]
    {
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
    assert_eq!(
        env(RoadName::Npm),
        "export npm_config_fetch_timeout=60000 npm_config_fetch_retries=1 npm_config_fetch_retry_maxtimeout=10000"
    );
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
    let out = Command::new("bash")
        .args(["-c", &format!("{}\ncurl -o /tmp/x 'https://example.test/a b'", *CURL_NET)])
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(out.status.success());
    // --silent drops the progress meter from stderr; --show-error keeps the one error line the reason rule reads.
    assert_eq!(
        stdout_of(&out).split('\n').filter(|l| !l.is_empty()).collect::<Vec<_>>(),
        [
            "--connect-timeout",
            "15",
            "--speed-limit",
            "1",
            "--speed-time",
            "60",
            "--retry",
            "1",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "-o",
            "/tmp/x",
            "https://example.test/a b"
        ]
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
        &InstallRoad::Script {
            script: CLAUDE_INSTALL.clone(),
            version: s(CLAUDE_CODE.version),
            pin: None,
            bins: Some(vec![LOCAL_BIN.to_string()])
        }
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
        install(
            &InstallRoad::Release { repo: s("cli/cli"), version: None, assets: None, pin: None, go: s("github.com/cli/cli/v2/cmd/gh") },
            "gh",
        ),
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
        vec![
            "curl",
            "node",
            "pnpm",
            "uv",
            "python",
            "git",
            "jq",
            "ripgrep",
            "build-essential",
            "fd",
            "sqlite3",
            "wget",
            "zip",
            "xz",
            "rsync",
            "gh",
            "agent-browser"
        ]
    );
    assert!(re("one Mac").is_match(source_of(entry("agent-browser")).note.as_deref().unwrap_or("")));
}

#[test]
fn seeds_every_golden_with_the_base_floor_the_entries_flagged_for_it_in_catalog_ord() {
    let floor = base_floor();
    // curl leads: a base image need not ship one, and every road below that fetches a release types it.
    assert_eq!(
        floor.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        vec!["curl", "uv", "python", "git", "jq", "ripgrep", "build-essential", "fd", "sqlite3", "wget", "zip", "xz", "rsync"]
    );
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
    let want: Vec<Option<String>> = [
        "apt-index",
        "curl",
        "uv",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
        "apt-index",
    ]
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
            "crush",
            "qwen",
            "goose",
            "amp",
            "cursor",
            "curl",
            "pnpm",
            "uv",
            "python",
            "git",
            "jq",
            "ripgrep",
            "build-essential",
            "fd",
            "sqlite3",
            "wget",
            "zip",
            "xz",
            "rsync",
            "gh",
            "agent-browser",
            "docker",
            "rust",
            "maven",
            "bun",
            "yarn",
            "ruff",
            "black",
            "mypy",
            "pyright",
            "pytest",
            "prettier",
            "eslint",
            "typescript",
            "wrangler",
            "cloudflared",
            "kubectl",
            "aws",
            "vercel",
            "netlify",
            "fly",
            "supabase",
            "railway",
            "doppler",
            "op",
            "ffmpeg",
            "yq",
            "git-lfs",
            "tmux",
            "ruby",
            "php",
            "postgresql-client",
            "redis-tools",
            "golangci-lint",
            "mise",
            "git-delta",
            "shellcheck",
            "swift",
            "elixir",
            "bazel",
            "llvm",
            "playwright",
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
    assert_eq!(
        catalog().iter().filter(|e| matches!(e.size(), Size::Unmeasured { .. })).map(|e| e.id()).collect::<Vec<_>>(),
        vec!["qwen", "amp"]
    );
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
    assert_eq!(
        line_of(entry("build-essential")),
        "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq build-essential cmake ninja-build"
    );
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
    assert_eq!(
        line_of(entry("fd")),
        "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq fd-find\nln -sfn /usr/bin/fdfind /usr/local/bin/fd"
    );
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
    assert_eq!(
        line_of(entry("yarn")),
        format!("corepack enable yarn\nCOREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack install -g yarn@{YARN_VERSION}")
    );
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
    let rows = [
        "ruby",
        "php",
        "postgresql-client",
        "redis-tools",
        "golangci-lint",
        "mise",
        "git-delta",
        "shellcheck",
        "swift",
        "elixir",
        "bazel",
        "llvm",
        "playwright",
        "op",
    ];
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
    assert_eq!(
        line_of(entry("php")),
        "export DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq php-cli composer php-mbstring php-xml php-curl php-zip"
    );
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
    assert!(
        line_of(entry("golangci-lint")).contains(r#"url='https://github.com/golangci/golangci-lint/releases/download/v2.13.2/'"$asset""#)
    );
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
