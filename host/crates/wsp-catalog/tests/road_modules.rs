// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/road-modules.test.ts.
// What a road says about reading a row back off a machine: whether the road has a presence test of its own, and
// which directories it links the commands it installs into. Both are read by the recipe job and by the doctor
// through one planned step, so a computers row and a doctor line cannot disagree about a tool. And where a job's
// managers install: under the machine's home for an image, under a folder of wsp's own for a computer somebody owns.
use indexmap::IndexMap;

use wsp_catalog::catalog::catalog_tool_for;
use wsp_catalog::linux_casks::{CaskSums, LinuxCask};
use wsp_wire::types::daemon_contract::PNPM_HOME;
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_catalog::road_modules::{as_linuxbrew, install_env, install_homes, road_module, road_module_named, InstallHomes, APT_BIN, BREW_PREFIX, CARGO_BIN, FROM_A_READABLE_DIR, INSTALL_HOMES};
use wsp_catalog::roads::{InstallRoad, PackageRoad, RoadName, GUEST_HOME, HOME_BIN, LOCAL_BIN, ROADS};

fn pkg(name: &str) -> PackageRoad {
    PackageRoad { package: name.to_string(), version: None, pin: None }
}

fn brew(formula: &str) -> InstallRoad {
    InstallRoad::Brew { formula: formula.to_string(), pin: None }
}

fn go(module: &str, version: &str) -> InstallRoad {
    InstallRoad::Go { module: Some(module.to_string()), version: Some(version.to_string()), pin: None }
}

fn script(text: &str, bins: Option<&[&str]>) -> InstallRoad {
    InstallRoad::Script { script: text.to_string(), version: None, pin: None, bins: bins.map(|b| b.iter().map(|s| s.to_string()).collect()) }
}

/// A road of each kind whose row names nothing beyond its kind, the TS `{ road } as never`.
fn bare(road: RoadName) -> InstallRoad {
    match road {
        RoadName::Brew => brew("x"),
        RoadName::Npm => InstallRoad::Npm { package: pkg("x"), ignore_scripts: None },
        RoadName::Pnpm => InstallRoad::Pnpm(pkg("x")),
        RoadName::Bun => InstallRoad::Bun(pkg("x")),
        RoadName::Uv => InstallRoad::Uv(pkg("x")),
        RoadName::Pipx => InstallRoad::Pipx(pkg("x")),
        RoadName::Cargo => InstallRoad::Cargo(pkg("x")),
        RoadName::Go => InstallRoad::Go { module: None, version: None, pin: None },
        RoadName::Release => InstallRoad::Release { repo: None, version: None, assets: None, pin: None, go: None },
        RoadName::Vendor => InstallRoad::Vendor {
            cask: LinuxCask {
                bin: "x".into(),
                from: "x".into(),
                detail: "x".into(),
                version: "x".into(),
                sha256: CaskSums { x86_64: "x".into(), aarch64: "x".into() },
                install: "x".into(),
                uninstall: "x".into(),
            },
            version: None,
            pin: None,
        },
        RoadName::Apt => InstallRoad::Apt { packages: vec![], pin: None },
        RoadName::Script => script("x", None),
    }
}

fn present(road: &InstallRoad, bin: &str) -> Option<String> {
    road_module(road).present(road, bin)
}

fn bins(road: &InstallRoad, homes: Option<&InstallHomes>) -> Vec<String> {
    road_module(road).bins(road, homes)
}

fn check(road: &InstallRoad, bin: &str) -> Option<String> {
    road_module(road).check(road, bin)
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn env_of(rows: &[(&str, String)]) -> IndexMap<String, String> {
    rows.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

mod a_roads_own_presence_read {
    use super::*;

    #[test]
    fn is_the_prefixs_link_for_a_core_formula_and_the_link_or_the_command_for_a_tap_for() {
        // A core formula is never read by its name: node comes from the base stage's own installer and gh from the
        // release road, both under the name a formula of the same name would carry, so a command read would call a box
        // green for a Homebrew row Homebrew never installed. That is the row this rule was filed on.
        assert_eq!(present(&brew("go"), "x"), Some(format!("test -e {BREW_PREFIX}/opt/go")));
        assert_eq!(
            present(&brew("felixkratz/formulae/sketchybar"), "x"),
            Some(format!("test -e {BREW_PREFIX}/opt/sketchybar || command -v 'sketchybar' >/dev/null 2>&1"))
        );
        // Every other road puts its own command on PATH under the name the row carries, so the command is the read and
        // the step's own check is what runs after an install.
        for road in ROADS.iter().filter(|r| **r != RoadName::Brew) {
            assert_eq!(road_module_named(*road).present(&bare(*road), "x"), None, "{road:?}");
        }
    }
}

mod a_roads_own_check {
    use super::*;

    #[test]
    fn is_homebrews_own_list_for_a_formula_on_the_one_linuxbrew_line_and_no_other_road() {
        // The read after an install, which runs brew and so cannot be answered inside a workspace; a row read by a
        // command written beside it reads whatever that command said on the day it was written.
        assert_eq!(check(&brew("bat"), "x"), Some(as_linuxbrew("list --versions bat")));
        assert!(check(&brew("bat"), "x").unwrap().contains(FROM_A_READABLE_DIR.as_str()));
        for road in ROADS.iter().filter(|r| **r != RoadName::Brew) {
            assert_eq!(road_module_named(*road).check(&bare(*road), "x"), None, "{road:?}");
        }
    }
}

mod the_directories_a_road_links_its_commands_into {
    use super::*;

    #[test]
    fn is_answered_by_every_road_as_absolute_paths_and_is_not_the_visibility_list() {
        let mut named: Vec<&str> = ROADS.iter().map(|r| road_module_named(*r).road.as_str()).collect();
        named.sort();
        let mut roads: Vec<&str> = ROADS.iter().map(|r| r.as_str()).collect();
        roads.sort();
        assert_eq!(named, roads);
        for road in ROADS {
            // The script road alone answers off its own row, since no two of its installers link into one directory.
            if road != RoadName::Script {
                assert!(!bins(&bare(road), None).is_empty(), "{road:?}");
            }
            let with_script = if road == RoadName::Script { script("x", None) } else { bare(road) };
            for dir in bins(&with_script, None) {
                assert!(dir.starts_with('/'), "{road:?} links into {dir}, which is no absolute path");
            }
        }
        // Homebrew's prefix is where a formula lands, and the road to /usr/local/bin is what a tap formula with no
        // Linux bottle takes; its roots are the trees a workspace has to see, which is the coarser question.
        assert_eq!(bins(&brew("go"), None), vec![format!("{BREW_PREFIX}/bin"), format!("{BREW_PREFIX}/sbin"), LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Apt { packages: strs(&["ffmpeg"]), pin: None }, None), vec![APT_BIN.to_string(), "/usr/sbin".into(), "/bin".into(), "/sbin".into()]);
        assert_eq!(bins(&InstallRoad::Npm { package: pkg("pnpm"), ignore_scripts: None }, None), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Release { repo: Some("cli/cli".into()), version: None, assets: None, pin: None, go: None }, None), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Pnpm(pkg("wrangler")), None), vec![PNPM_HOME.to_string()]);
        assert_eq!(bins(&InstallRoad::Uv(pkg("ruff")), None), vec![HOME_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Cargo(pkg("ripgrep")), None), vec![CARGO_BIN.to_string()]);
    }

    #[test]
    fn is_the_script_roads_own_row_and_nothing_for_a_script_whose_row_names_none() {
        assert_eq!(bins(&script("curl -o node.tar.gz ...", Some(&[LOCAL_BIN])), None), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&script("apt-get install -y docker-ce", None), None), Vec::<String>::new());
    }
}

mod where_a_jobs_managers_install {
    use super::*;

    fn line(road: &InstallRoad, bin: &str, at: &InstallHomes) -> String {
        match road_module(road).install(road, bin, Some(at)) {
            Ok(install) => install,
            Err(note) => panic!("{}", note.0),
        }
    }

    fn rust_road() -> InstallRoad {
        catalog_tool_for("rust").unwrap().install_road.clone()
    }

    #[test]
    fn is_a_folder_of_wsps_own_under_the_prefix_with_every_command_in_the_one_folder_th() {
        let homes = install_homes(Some(TOOL_PREFIX));
        // Under /opt, which the protocol brings into every workspace on that computer through an overlay of its own,
        // so a tool installed here answers inside one while no process in one can write it on the computer itself.
        assert_eq!(TOOL_PREFIX, "/opt/wsp");
        for name in INSTALL_HOMES {
            let home = homes.get(name);
            assert!(home.home.starts_with(&format!("{TOOL_PREFIX}/")), "{name:?}");
            assert_eq!(home.bin, LOCAL_BIN, "{name:?}");
            // Nothing a manager is told names a directory under the machine's home, which every workspace there writes.
            for (knob, value) in &home.env {
                assert!(value.starts_with(&format!("{TOOL_PREFIX}/")) || value == LOCAL_BIN, "{knob}={value}");
            }
        }
        // What a step reads its row back by: that one folder for a manager a knob moves, and the manager's own folder
        // ahead of it for one with no such knob, whose commands are linked from there.
        assert_eq!(bins(&InstallRoad::Uv(pkg("ruff")), Some(&homes)), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Pipx(pkg("black")), Some(&homes)), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Bun(pkg("wrangler")), Some(&homes)), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&go("github.com/x/y", "v1"), Some(&homes)), vec![LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Cargo(pkg("ripgrep")), Some(&homes)), vec![format!("{TOOL_PREFIX}/cargo/bin"), LOCAL_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Pnpm(pkg("wrangler")), Some(&homes)), vec![format!("{TOOL_PREFIX}/pnpm/bin"), LOCAL_BIN.to_string()]);
    }

    #[test]
    fn is_each_managers_own_folder_under_the_machines_home_for_a_job_with_no_prefix_whi() {
        let homes = install_homes(Some(TOOL_PREFIX));
        let own = install_homes(None);
        let rust = rust_road();
        assert_eq!(bins(&InstallRoad::Uv(pkg("ruff")), Some(&own)), vec![HOME_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Pipx(pkg("black")), Some(&own)), vec![HOME_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Bun(pkg("wrangler")), Some(&own)), vec![format!("{GUEST_HOME}/.bun/bin")]);
        assert_eq!(bins(&go("github.com/x/y", "v1"), Some(&own)), vec![format!("{GUEST_HOME}/go/bin")]);
        assert_eq!(bins(&InstallRoad::Cargo(pkg("ripgrep")), Some(&own)), vec![CARGO_BIN.to_string()]);
        assert_eq!(bins(&InstallRoad::Pnpm(pkg("wrangler")), Some(&own)), vec![PNPM_HOME.to_string()]);
        // The one knob an image's job exports, the one it has always exported: pnpm has no folder of its own by default.
        assert_eq!(install_env(&own), env_of(&[("PNPM_HOME", PNPM_HOME.to_string())]));
        // Nothing is linked and nothing is added to the PATH of a line: an image's scripts read as they did.
        assert_eq!(line(&InstallRoad::Cargo(pkg("ripgrep")), "rg", &own), "cargo install ripgrep --locked");
        assert_eq!(line(&InstallRoad::Pnpm(pkg("wrangler")), "wrangler", &own), "pnpm add -g wrangler");
        assert_eq!(line(&rust, "cargo", &own), line(&rust, "cargo", &homes).split("\nfind ").next().unwrap());
    }

    #[test]
    fn tells_each_manager_its_home_in_the_knobs_the_job_exports_every_one_the_manager_r() {
        let homes = install_homes(Some(TOOL_PREFIX));
        assert_eq!(
            install_env(&homes),
            env_of(&[
                ("PNPM_HOME", format!("{TOOL_PREFIX}/pnpm")),
                ("BUN_INSTALL", format!("{TOOL_PREFIX}/bun")),
                ("BUN_INSTALL_BIN", LOCAL_BIN.to_string()),
                ("UV_TOOL_DIR", format!("{TOOL_PREFIX}/uv/tools")),
                ("UV_TOOL_BIN_DIR", LOCAL_BIN.to_string()),
                ("UV_PYTHON_INSTALL_DIR", format!("{TOOL_PREFIX}/uv/python")),
                ("PIPX_HOME", format!("{TOOL_PREFIX}/pipx")),
                ("PIPX_BIN_DIR", LOCAL_BIN.to_string()),
                ("CARGO_HOME", format!("{TOOL_PREFIX}/cargo")),
                ("RUSTUP_HOME", format!("{TOOL_PREFIX}/rustup")),
                ("GOPATH", format!("{TOOL_PREFIX}/go")),
                ("GOBIN", LOCAL_BIN.to_string()),
            ])
        );
    }

    #[test]
    fn links_what_a_manager_with_no_folder_knob_left_in_its_own_folder_and_takes_the_li() {
        let homes = install_homes(Some(TOOL_PREFIX));
        let link = format!("find {TOOL_PREFIX}/cargo/bin -maxdepth 1 -type f -perm -u+x -exec ln -sfn {{}} {LOCAL_BIN}/ ';'");
        let ripgrep = InstallRoad::Cargo(pkg("ripgrep"));
        let cargo = line(&ripgrep, "rg", &homes);
        assert!(cargo.contains("cargo install ripgrep --locked"));
        assert!(cargo.ends_with(&link));
        assert_eq!(
            road_module(&ripgrep).uninstall(&ripgrep, "rg", Some(&homes)),
            Ok(format!("export PATH={TOOL_PREFIX}/cargo/bin:$PATH; cargo uninstall ripgrep\nrm -f {LOCAL_BIN}/'rg'"))
        );
        // pnpm refuses to install a global while the folder it links into is off PATH, so its own lines carry it.
        let wrangler = InstallRoad::Pnpm(pkg("wrangler"));
        let pnpm = line(&wrangler, "wrangler", &homes);
        assert!(pnpm.contains(&format!("export PATH={TOOL_PREFIX}/pnpm/bin:$PATH; pnpm add -g wrangler")));
        assert!(pnpm.ends_with(&format!("find {TOOL_PREFIX}/pnpm/bin -maxdepth 1 -type f -perm -u+x -exec ln -sfn {{}} {LOCAL_BIN}/ ';'")));
        // And the version read runs under the same folder, since pnpm refuses that line too.
        assert!(road_module(&wrangler).installed(&wrangler, "wrangler", Some(&homes)).unwrap().contains(&format!("export PATH={TOOL_PREFIX}/pnpm/bin:$PATH; ")));
    }

    #[test]
    fn maps_the_rust_rows_own_command_folder_to_the_prefixs_and_links_every_command_rus() {
        let homes = install_homes(Some(TOOL_PREFIX));
        let own = install_homes(None);
        let rust = rust_road();
        // cargo itself stands in the cargo home's own folder, which the fixed PATH does not hold, so no cargo row
        // could run on a computer somebody owns until the toolchain moved under the prefix with its commands linked.
        assert_eq!(bins(&rust, None), vec![CARGO_BIN.to_string()]);
        assert_eq!(bins(&rust, Some(&homes)), vec![format!("{TOOL_PREFIX}/cargo/bin"), LOCAL_BIN.to_string()]);
        let text = line(&rust, "cargo", &homes);
        assert!(text.contains("rustup-init"));
        assert!(text.ends_with(&format!("find {TOOL_PREFIX}/cargo/bin -maxdepth 1 -type f -perm -u+x -exec ln -sfn {{}} {LOCAL_BIN}/ ';'")));
        // A script row whose own folder no manager keeps is the row's own and stands, with nothing linked.
        let node = catalog_tool_for("node").unwrap().install_road.clone();
        assert_eq!(bins(&node, Some(&homes)), vec![LOCAL_BIN.to_string()]);
        assert_eq!(line(&node, "node", &homes), line(&node, "node", &own));
    }
}
