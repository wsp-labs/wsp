// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/first-run.test.ts.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use futures::executor::block_on;
use wsp_catalog::first_run::{installs_on_first_run, FirstRunInstaller, RunScript, FIRST_RUN_INSTALLERS};

fn installer(id: &str) -> FirstRunInstaller {
    FIRST_RUN_INSTALLERS.iter().copied().find(|i| i.id() == id).unwrap()
}

/// ASCII's shim as its cloud image writes it at /usr/local/bin/<name>, with that folder and its fallback given.
fn ascii_shim(name: &str, folder: &str, fallback: &str) -> String {
    format!("#!/bin/sh\n# ascii-harness-shim\nu=$(PATH=$(printf %s \"$PATH\" | tr : \"\\n\" | grep -vx {folder} | paste -sd:) command -v {name} 2>/dev/null)\n[ -n \"$u\" ] && exec \"$u\" \"$@\"\nexec {fallback} \"$@\"\n")
}

fn default_shim(name: &str) -> String {
    ascii_shim(name, "/usr/local/bin", &format!("/usr/local/lib/ascii-harnesses/bin/{name}"))
}

fn passes(script: &str, vars: &[(&str, &str)]) -> bool {
    Command::new("/bin/sh").arg("-c").arg(script).env_clear().envs(vars.iter().copied()).status().unwrap().success()
}

fn write_executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

mod a_mise_wrapper {
    use super::*;

    #[test]
    fn names_the_tool_its_script_installs_past_flags_quotes_and_a_version_and_nothing_f() {
        let mise = installer("mise");
        assert_eq!(mise.tool("#!/bin/bash\nexport MISE_MINIMUM_RELEASE_AGE=0\nmise use -g --quiet \"claude\" || exit 1\nexec mise x \"claude\" -- \"claude\" \"$@\"\n"), Some("claude".to_string()));
        assert_eq!(
            mise.tool("#!/bin/sh\nmise install 'npm:@anthropic-ai/claude-code@2.1.289' && exec claude \"$@\"\n"),
            Some("npm:@anthropic-ai/claude-code@2.1.289".to_string())
        );
        assert_eq!(mise.tool("#!/bin/sh\n# mise use -g claude\nexec \"$HOME/.local/share/claude/bin/claude\" \"$@\"\n"), None);
        assert_eq!(mise.tool("#!/usr/bin/env node\nrequire(\"./cli.js\")\n"), None);
    }

    #[test]
    fn is_installed_once_mises_own_installs_folder_for_the_tool_holds_a_version_its_ver() {
        let mise = installer("mise");
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let home_s = home.to_str().unwrap();
        let data = home.join("data");
        let data_s = data.to_str().unwrap();
        let ok = |tool: &str, extra: &[(&str, &str)]| -> bool {
            let mut all = vec![("HOME", home_s), ("PATH", "/usr/bin:/bin")];
            all.extend_from_slice(extra);
            passes(&mise.installed(tool), &all)
        };
        assert!(!ok("claude", &[]));
        fs::create_dir_all(home.join(".local/share/mise/installs/claude")).unwrap();
        assert!(!ok("claude", &[]));
        fs::create_dir(home.join(".local/share/mise/installs/claude/2.1.289")).unwrap();
        assert!(ok("claude", &[]));
        fs::create_dir_all(data.join("installs/npm-anthropic-ai-claude-code/2.1.289")).unwrap();
        assert!(!ok("npm:@anthropic-ai/claude-code@2.1.289", &[]));
        assert!(ok("npm:@anthropic-ai/claude-code@2.1.289", &[("MISE_DATA_DIR", data_s)]));
        fs::create_dir_all(data.join("installs/github-burnt-sushi-ripgrep/14.1.1")).unwrap();
        fs::create_dir_all(data.join("installs/aqua-my-tool-http-server/1.0.0")).unwrap();
        assert!(ok("github:BurntSushi/ripgrep", &[("MISE_DATA_DIR", data_s)]));
        assert!(ok("aqua:my_tool/HTTPServer@1.0.0", &[("MISE_DATA_DIR", data_s)]));
    }
}

mod an_ascii_lazy_run_shim {
    use super::*;

    #[test]
    fn names_the_launcher_it_falls_back_to_and_nothing_for_a_script_that_is_not_the_shi() {
        let mise = installer("mise");
        let lazy_run = installer("lazy-run");
        assert_eq!(lazy_run.tool(&default_shim("cursor-agent")), Some("/usr/local/lib/ascii-harnesses/bin/cursor-agent".to_string()));
        assert_eq!(lazy_run.tool(&default_shim("hermes")), Some("/usr/local/lib/ascii-harnesses/bin/hermes".to_string()));
        assert_eq!(
            lazy_run
                .tool("#!/bin/sh\n# ascii-lazy-harness cursor\nexec /usr/local/lib/ascii-harnesses/lazy-run cursor cursor-agent \"$@\"\n"),
            None
        );
        assert_eq!(lazy_run.tool("#!/bin/sh\nexec /opt/cursor-agent/cursor-agent \"$@\"\n"), None);
        assert_eq!(mise.tool(&default_shim("cursor-agent")), None);
    }

    #[test]
    fn is_installed_once_the_launcher_is_no_longer_lazy_runs_or_once_another_command_of() {
        let lazy_run = installer("lazy-run");
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let shims = dir.join("shims");
        let other = dir.join("other");
        let launcher = dir.join("lib").join("cursor-agent");
        for d in [&shims, &other, &dir.join("lib")] {
            fs::create_dir_all(d).unwrap();
        }
        let dir_s = dir.to_str().unwrap();
        let shim = ascii_shim("cursor-agent", shims.to_str().unwrap(), launcher.to_str().unwrap());
        write_executable(&shims.join("cursor-agent"), &shim);
        fs::write(&launcher, format!("#!/bin/sh\n# ascii-lazy-harness cursor\nexec {dir_s}/lazy-run cursor cursor-agent \"$@\"\n"))
            .unwrap();
        let path = format!("{}:{}:/usr/bin:/bin", shims.display(), other.display());
        let check = || -> bool {
            let tool = lazy_run.tool(&shim).unwrap();
            passes(&lazy_run.installed(&tool), &[("HOME", dir_s), ("PATH", &path)])
        };
        assert!(!check());
        write_executable(&other.join("cursor-agent"), "#!/bin/sh\necho 1.0.0\n");
        assert!(check());
        fs::remove_file(other.join("cursor-agent")).unwrap();
        fs::write(&launcher, "#!/bin/sh\necho 2026.10.01\n").unwrap();
        assert!(check());
    }
}

struct Script {
    asked: Mutex<Vec<String>>,
    answer: Box<dyn Fn(&str) -> Option<String>>,
}

impl RunScript for Script {
    async fn run(&self, script: &str) -> Option<String> {
        self.asked.lock().unwrap().push(script.to_string());
        (self.answer)(script)
    }
}

fn script(answer: impl Fn(&str) -> Option<String> + 'static) -> Script {
    Script { asked: Mutex::new(vec![]), answer: Box::new(answer) }
}

fn bins(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

mod installs_on_first_run {
    use super::*;

    #[test]
    fn asks_nothing_more_where_no_command_is_a_script_and_reads_a_script_the_second_run() {
        let run = script(|_| Some("\u{1e}\u{1e}\u{1e}".to_string()));
        let none = block_on(installs_on_first_run(&run, &bins(&["claude", "codex"])));
        assert_eq!(none, vec![false, false]);
        assert_eq!(run.asked.lock().unwrap().len(), 1);
        let heads = "\u{1e}#!/bin/bash\nmise use -g claude\n\u{1e}\u{7f}ELF\u{1e}".to_string();
        let run = script(move |script| Some(if script.starts_with("for b in") { heads.clone() } else { "1\n0\n".to_string() }));
        let said = block_on(installs_on_first_run(&run, &bins(&["claude", "codex"])));
        assert_eq!(said, vec![true, false]);
    }

    #[test]
    fn answers_false_for_every_bin_when_the_read_could_not_run() {
        let run = script(|_| None);
        assert_eq!(block_on(installs_on_first_run(&run, &bins(&["claude"]))), vec![false]);
    }
}
