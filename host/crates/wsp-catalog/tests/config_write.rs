// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/config-write.test.ts.
use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use wsp_catalog::config_write::{
    config_landing, config_refusal, config_sum, config_write_line, under_base, ConfigWrite, CONFIG_CHANGED_EXIT, CONFIG_LINKS_EXIT,
    CONFIG_LINK_EXIT,
};
use wsp_words::agents_report::config_hard_link_refusal;

fn home() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new().prefix("wsp-config-write-").tempdir().unwrap();
    let real = fs::canonicalize(dir.path()).unwrap();
    (dir, real)
}

fn sum_of(file: &Path) -> String {
    config_sum(&fs::read(file).unwrap())
}

fn s(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

struct Written {
    status: Option<i32>,
    stdout: String,
}

fn write(file: &Path, base: &Path, sum: Option<String>, from: Option<&Path>, text: &str) -> Written {
    let line = config_write_line(&ConfigWrite { file: s(file), base: s(base), sum, bytes: text.len() as u64, from: from.map(s) });
    let mut child = Command::new("/bin/sh").args(["-c", &line]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    if from.is_none() {
        stdin.write_all(text.as_bytes()).unwrap();
    }
    drop(stdin);
    let res = child.wait_with_output().unwrap();
    Written { status: res.status.code(), stdout: String::from_utf8_lossy(&res.stdout).to_string() }
}

fn temps(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).filter(|n| n.starts_with(".wsp-config-")).collect();
    names.sort();
    names
}

fn age(file: &Path, by: Duration) {
    let handle = fs::OpenOptions::new().write(true).open(file).unwrap();
    handle.set_modified(SystemTime::now() - by).unwrap();
}

fn mode(file: &Path) -> u32 {
    fs::metadata(file).unwrap().permissions().mode() & 0o777
}

mod config_sum {
    use super::*;

    #[test]
    fn is_what_cksum_prints_for_the_same_bytes() {
        for text in ["", "a", "{\n  \"mcpServers\": {}\n}\n", &"\u{e9}\u{0}".repeat(3000)] {
            let bytes = text.as_bytes();
            let mut child = Command::new("cksum").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
            let mut stdin = child.stdin.take().unwrap();
            let fed = bytes.to_vec();
            let feeder = std::thread::spawn(move || stdin.write_all(&fed).unwrap());
            let out = child.wait_with_output().unwrap();
            feeder.join().unwrap();
            let said = String::from_utf8(out.stdout).unwrap().trim().split_whitespace().collect::<Vec<_>>().join(" ");
            let shown: String = text.chars().take(10).collect();
            assert_eq!(config_sum(bytes), said, "{shown:?}");
        }
    }
}

mod under_base {
    use super::*;

    #[test]
    fn holds_every_path_inside_a_base_of_and_a_sibling_that_shares_a_bases_name_out_of() {
        let at = |r: &str, b: &str| -> String {
            let script = format!("r={r}; b={b}; {}", under_base("echo in", "echo out"));
            let out = Command::new("/bin/sh").args(["-c", &script]).output().unwrap();
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        assert_eq!(
            vec![at("/.mcp.json", "/"), at("/root/a.json", "/"), at("/root/a.json", "/root"), at("/rootx/a.json", "/root"), at("/root", "/root")],
            vec!["in", "in", "in", "out", "in"]
        );
    }
}

mod config_write_line {
    use super::*;

    #[test]
    fn replaces_the_file_whole_by_a_rename_from_beside_it_keeping_its_mode_and_makes_a() {
        let (_guard, h) = home();
        let file = h.join(".claude.json");
        fs::write(&file, "old\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        assert_eq!(write(&file, &h, Some(sum_of(&file)), None, "new\n").status, Some(0));
        assert_eq!(fs::read_to_string(&file).unwrap(), "new\n");
        assert_eq!(mode(&file), 0o640);
        assert_eq!(temps(&h), Vec::<String>::new());

        let made = h.join(".codex").join("config.toml");
        assert_eq!(write(&made, &h, None, None, "model = \"x\"\n").status, Some(0));
        assert_eq!(fs::read_to_string(&made).unwrap(), "model = \"x\"\n");
        assert_eq!(mode(&made), 0o600);
    }

    #[test]
    fn writes_through_a_link_inside_the_base_and_leaves_it_a_link_and_refuses_one_out_o() {
        let (_guard, h) = home();
        fs::create_dir(h.join("dotfiles")).unwrap();
        fs::write(h.join("dotfiles").join("settings.json"), "{}\n").unwrap();
        let file = h.join("settings.json");
        symlink(h.join("dotfiles").join("settings.json"), &file).unwrap();
        assert_eq!(write(&file, &h, Some(sum_of(&file)), None, "{ \"a\": 1 }\n").status, Some(0));
        assert!(fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(h.join("dotfiles").join("settings.json")).unwrap(), "{ \"a\": 1 }\n");

        let (_out_guard, out) = home();
        fs::write(out.join("x.json"), "{}\n").unwrap();
        let linked = h.join("out.json");
        symlink(out.join("x.json"), &linked).unwrap();
        let res = write(&linked, &h, Some(sum_of(&linked)), None, "[]\n");
        assert_eq!(res.status, Some(CONFIG_LINK_EXIT));
        assert_eq!(
            config_refusal(res.status.unwrap(), &res.stdout, &s(&linked), &|p| p.to_string()),
            Some(format!("{} is a link to {}, outside the folder it belongs to, so wsp does not write through it.", s(&linked), s(&out.join("x.json"))))
        );
        assert_eq!(fs::read_to_string(out.join("x.json")).unwrap(), "{}\n");
    }

    #[test]
    fn refuses_a_file_with_a_second_hard_link_which_a_rename_would_split_from_it_and_wr() {
        let (_guard, h) = home();
        let file = h.join("opencode.json");
        fs::write(&file, "{}\n").unwrap();
        fs::hard_link(&file, h.join("twin.json")).unwrap();
        let res = write(&file, &h, Some(sum_of(&file)), None, "{ \"mcp\": {} }\n");
        assert_eq!(res.status, Some(CONFIG_LINKS_EXIT));
        assert_eq!(config_refusal(res.status.unwrap(), &res.stdout, &s(&file), &|_| "~/opencode.json".to_string()), Some(config_hard_link_refusal("~/opencode.json")));
        assert!(config_hard_link_refusal("~/opencode.json").contains("find / -xdev -samefile ~/opencode.json"));
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}\n");
        assert_eq!(fs::read_to_string(h.join("twin.json")).unwrap(), "{}\n");
        assert_eq!(temps(&h), Vec::<String>::new());
    }

    #[test]
    fn writes_nothing_over_a_file_that_changed_since_the_read_or_one_that_appeared_wher() {
        let (_guard, h) = home();
        let file = h.join("config.toml");
        fs::write(&file, "a\n").unwrap();
        let sum = sum_of(&file);
        fs::write(&file, "b\n").unwrap();
        assert_eq!(write(&file, &h, Some(sum), None, "c\n").status, Some(CONFIG_CHANGED_EXIT));
        assert_eq!(write(&file, &h, None, None, "c\n").status, Some(CONFIG_CHANGED_EXIT));
        assert_eq!(fs::read_to_string(&file).unwrap(), "b\n");
        assert_eq!(
            config_refusal(CONFIG_CHANGED_EXIT, "", &s(&file), &|_| "~/config.toml".to_string()),
            Some("~/config.toml changed while wsp was writing it, so nothing was written; try again.".to_string())
        );
        assert_eq!(temps(&h), Vec::<String>::new());
    }

    #[test]
    fn takes_the_bytes_from_a_landing_beside_the_file_and_takes_the_landing_with_it() {
        let (_guard, h) = home();
        let file = h.join("settings.json");
        let landing = config_landing(&s(&file), "0123456789ab").unwrap();
        assert_eq!(landing, s(&h.join(".wsp-config-tmp.0123456789ab")));
        fs::write(&landing, "landed\n").unwrap();
        assert_eq!(write(&file, &h, None, Some(Path::new(&landing)), "landed\n").status, Some(0));
        assert_eq!(fs::read_to_string(&file).unwrap(), "landed\n");
        assert!(!Path::new(&landing).exists());
    }

    #[test]
    fn leaves_the_old_file_whole_when_the_write_is_killed_partway_and_a_later_write_swe() {
        let (_guard, h) = home();
        let file = h.join(".claude.json");
        let old = format!("{}\n", serde_json::json!({ "keep": "x".repeat(1000) }));
        fs::write(&file, &old).unwrap();
        let next = format!("{}\n", serde_json::json!({ "next": "y".repeat(200_000) })).into_bytes();
        let line = config_write_line(&ConfigWrite { file: s(&file), base: s(&h), sum: Some(sum_of(&file)), bytes: next.len() as u64, from: None });
        let mut child = Command::new("/bin/sh").args(["-c", &line]).process_group(0).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&next[..next.len() / 2]).unwrap();
        for _ in 0..200 {
            if temps(&h).iter().any(|n| fs::metadata(h.join(n)).map(|m| m.len() as usize > old.len()).unwrap_or(false)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status().unwrap();
        child.wait().unwrap();
        drop(stdin);
        assert_eq!(fs::read_to_string(&file).unwrap(), old);
        let left = temps(&h);
        assert_eq!(left.len(), 1);

        // A temp file of a write still running is young, and stays; the killed one's is aged past the sweep's age.
        fs::write(h.join(".wsp-config-tmp.Young1"), "").unwrap();
        age(&h.join(&left[0]), Duration::from_secs(11 * 60));
        assert_eq!(write(&file, &h, Some(sum_of(&file)), None, "whole\n").status, Some(0));
        assert_eq!(fs::read_to_string(&file).unwrap(), "whole\n");
        assert_eq!(temps(&h), vec![".wsp-config-tmp.Young1".to_string()]);
    }

    #[test]
    fn sweeps_only_the_temp_shapes_it_mints_a_landings_parts_among_them_so_a_persons_fi() {
        let (_guard, h) = home();
        let file = h.join(".claude.json");
        fs::write(&file, "{}\n").unwrap();
        let theirs = [
            ".wsp-config-notes.md",
            ".wsp-config-tmp.notes.md",
            ".wsp-config-tmp.abc12",
            ".wsp-config-tmp.0123456789abc",
            ".wsp-config-tmp.0123456789aZ",
            ".wsp-config-tmp.0123456789ab.parts",
        ];
        let ours = [".wsp-config-tmp.aB3xY9", ".wsp-config-tmp.0123456789ab", ".wsp-config-tmp.0123456789ab.part0", ".wsp-config-tmp.0123456789ab.part12"];
        for n in theirs.iter().chain(ours.iter()) {
            fs::write(h.join(n), "kept").unwrap();
            age(&h.join(n), Duration::from_secs(60 * 60));
        }
        assert_eq!(write(&file, &h, Some(sum_of(&file)), None, "[]\n").status, Some(0));
        let mut want: Vec<String> = theirs.iter().map(|n| n.to_string()).collect();
        want.sort();
        assert_eq!(temps(&h), want);
    }

    #[test]
    fn names_a_link_whose_path_holds_a_tab_or_a_newline_whole_in_its_refusal() {
        let (_guard, h) = home();
        let (_out_guard, out) = home();
        let target = out.join("a\tb\nc.json");
        fs::write(&target, "{}\n").unwrap();
        let linked = h.join("x\ty\nz.json");
        symlink(&target, &linked).unwrap();
        let res = write(&linked, &h, Some(sum_of(&linked)), None, "[]\n");
        assert_eq!(res.status, Some(CONFIG_LINK_EXIT));
        assert_eq!(
            config_refusal(res.status.unwrap(), &res.stdout, &s(&linked), &|p| p.to_string()),
            Some(format!("{} is a link to {}, outside the folder it belongs to, so wsp does not write through it.", s(&linked), s(&target)))
        );
    }

    #[test]
    fn writes_nothing_when_fewer_bytes_came_than_were_promised() {
        let (_guard, h) = home();
        let file = h.join("settings.json");
        fs::write(&file, "old\n").unwrap();
        let line = config_write_line(&ConfigWrite { file: s(&file), base: s(&h), sum: Some(sum_of(&file)), bytes: 100, from: None });
        let mut child = Command::new("/bin/sh").args(["-c", &line]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"cut\n").unwrap();
        drop(stdin);
        assert_eq!(child.wait().unwrap().code(), Some(1));
        assert_eq!(fs::read_to_string(&file).unwrap(), "old\n");
        assert_eq!(temps(&h), Vec::<String>::new());
    }
}
