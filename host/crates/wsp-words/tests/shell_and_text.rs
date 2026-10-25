// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/tool-result.test.ts:5-26, oom.test.ts:5-26 (whole files), docker-shim.test.ts:5-240
//! (whole file, the shim run through /bin/sh with a fake docker after it on PATH) and format.test.ts:328-340
//! (psCpuSeconds). Skipped: none. One test reads constants only and so passes against the stubs:
//! `the_daemon_is_killed_last_but_never_exempt_runs_ahead_of_default_priority_and_th`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;
use wsp_words::docker_shim::docker_shim;
use wsp_words::oom::{work_argv, work_score_line, Argv};
use wsp_wire::types::docker_shim::DOCKER_NOTES_CAP_BYTES;
use wsp_wire::types::oom::{DAEMON_NICE, DAEMON_OOM_SCORE_ADJ, WORK_OOM_SCORE_ADJ};
use wsp_wire::views::session_events::{FilePatch, PatchHunk};
use wsp_words::ps_time::ps_cpu_seconds;
use wsp_words::tool_result::{kept_patch, whole_file_hunk, KeptPatch};

fn wire<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap()
}

mod an_edit_s_hunks {
    use super::*;

    fn hunk(at: i64, lines: &[&str]) -> PatchHunk {
        wire(json!({ "oldStart": at, "oldLines": lines.len(), "newStart": at, "newLines": lines.len(), "lines": lines }))
    }

    fn file(path: &str, hunks: Vec<PatchHunk>) -> FilePatch {
        wire(json!({ "path": path, "hunks": hunks }))
    }

    #[test]
    fn write_a_whole_file_as_one_hunk_of_every_line_its_last_newline_ending_a_line_rath() {
        assert_eq!(whole_file_hunk("hi\nthere\n", true), wire(json!({ "oldStart": 0, "oldLines": 0, "newStart": 1, "newLines": 2, "lines": ["+hi", "+there"] })));
        assert_eq!(whole_file_hunk("gone", false), wire(json!({ "oldStart": 1, "oldLines": 1, "newStart": 0, "newLines": 0, "lines": ["-gone"] })));
        assert_eq!(whole_file_hunk("", true), wire(json!({ "oldStart": 0, "oldLines": 0, "newStart": 0, "newLines": 0, "lines": [] })));
    }

    #[test]
    fn keep_the_hunks_that_fit_in_order_across_files_the_one_that_crosses_the_cap_cut_t() {
        let patch: Vec<FilePatch> = vec![
            file("a", vec![hunk(1, &["-aaaa", "+bbbb"]), hunk(9, &["-cccc", "+dddd"])]),
            file("b", vec![hunk(1, &["-eeee", "+ffff"])]),
        ];
        assert_eq!(kept_patch(&patch, 100), KeptPatch { patch: patch.clone(), patch_cut: false });
        assert_eq!(kept_patch(&patch, 20), KeptPatch { patch: vec![patch[0].clone()], patch_cut: true });
        let cut: PatchHunk = wire(json!({ "oldStart": 9, "oldLines": 1, "newStart": 9, "newLines": 0, "lines": ["-cccc"] }));
        assert_eq!(
            kept_patch(&patch, 15),
            KeptPatch { patch: vec![file("a", vec![patch[0].hunks[0].clone(), cut])], patch_cut: true }
        );
        assert_eq!(kept_patch(&patch, 4), KeptPatch { patch: vec![], patch_cut: true });
        let created = file("n", vec![whole_file_hunk("one\ntwo\nthree\n", true)]);
        assert_eq!(
            kept_patch(&[created], 9),
            KeptPatch {
                patch: vec![file("n", vec![wire(json!({ "oldStart": 0, "oldLines": 0, "newStart": 1, "newLines": 2, "lines": ["+one", "+two"] }))])],
                patch_cut: true
            }
        );
    }
}

mod who_the_kernel_takes_first {
    use super::*;

    #[test]

    #[ignore = "checks constants alone, which the interface copied by script; no stub to fail on"]
    fn the_daemon_is_killed_last_but_never_exempt_runs_ahead_of_default_priority_and_th() {
        assert_eq!(DAEMON_OOM_SCORE_ADJ, -999);
        assert!(WORK_OOM_SCORE_ADJ > 0);
        assert!(WORK_OOM_SCORE_ADJ <= 1000);
        assert!(DAEMON_NICE < 0);
        assert!(DAEMON_NICE >= -20);
    }

    #[test]
    fn the_sh_line_writes_the_work_score_to_the_running_shell_s_own_entry_and_puts_it_b() {
        assert_eq!(work_score_line(None), "{ echo 500 > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1");
        assert_eq!(work_score_line(Some(0.0)), "{ echo 0 > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1");
    }

    #[test]
    fn a_command_the_daemon_launches_runs_behind_that_line_as_argv_exec_d_into_never_in() {
        assert_eq!(
            work_argv("git", &["status", "a b", "c'd"]),
            Argv {
                file: "/bin/sh".to_string(),
                args: ["-c", &format!("{}; exec \"$0\" \"$@\"", work_score_line(None)), "git", "status", "a b", "c'd"].map(String::from).to_vec(),
            }
        );
        let zsh = work_argv("/usr/bin/zsh", &["-l"]);
        assert_eq!(zsh.args[zsh.args.len() - 2..], ["/usr/bin/zsh".to_string(), "-l".to_string()]);
    }
}

mod the_docker_a_thread_finds_first_on_its_path {
    use super::*;

    /// What the docker after the shim was handed: every word, and the variables compose and the shim read.
    #[derive(Clone, Debug, PartialEq)]
    struct Seen {
        argv: Vec<String>,
        project: Option<String>,
        passed: Option<String>,
    }

    struct Ran {
        seen: Seen,
        status: Option<i32>,
        stderr: String,
    }

    struct Raw {
        stdout: String,
        stderr: String,
        status: Option<i32>,
        timed_out: bool,
    }

    // The fake docker after the shim: the TS one is a node script printing JSON; this one is sh, and prints the count
    // and every word NUL-terminated, then whether COMPOSE_PROJECT_NAME and WSP_DOCKER_PASSED are set and their values.
    const REAL: &str = r#"#!/bin/sh
printf '%s\0' "$#" "$@"
printf '%s\0' "${COMPOSE_PROJECT_NAME+set}" "${COMPOSE_PROJECT_NAME-}" "${WSP_DOCKER_PASSED+set}" "${WSP_DOCKER_PASSED-}"
exit 7
"#;

    const THREAD: &str = "t_1f0c-ab";

    fn write_script(path: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn spawn(command: &mut Command) -> std::process::Child {
        // A script written a moment ago can be refused with ETXTBSY while another test's fork still holds its write fd.
        for _ in 0..100 {
            match command.spawn() {
                Err(e) if e.raw_os_error() == Some(26) => std::thread::sleep(Duration::from_millis(20)),
                other => return other.unwrap(),
            }
        }
        command.spawn().unwrap()
    }

    fn exec(command: &mut Command, limit: Duration) -> Raw {
        let mut child = spawn(command.stdout(Stdio::piped()).stderr(Stdio::piped()));
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let out_reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            out.read_to_end(&mut bytes).ok();
            bytes
        });
        let err_reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            err.read_to_end(&mut bytes).ok();
            bytes
        });
        let deadline = Instant::now() + limit;
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status.code();
            }
            if Instant::now() > deadline {
                timed_out = true;
                child.kill().ok();
                child.wait().ok();
                break None;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        Raw {
            stdout: String::from_utf8_lossy(&out_reader.join().unwrap()).into_owned(),
            stderr: String::from_utf8_lossy(&err_reader.join().unwrap()).into_owned(),
            status,
            timed_out,
        }
    }

    fn parse(stdout: &str) -> Seen {
        let mut fields: Vec<&str> = stdout.split('\0').collect();
        fields.pop();
        let count: usize = fields[0].parse().unwrap();
        let argv = fields[1..=count].iter().map(|s| s.to_string()).collect();
        let tail = &fields[count + 1..];
        Seen {
            argv,
            project: (tail[0] == "set").then(|| tail[1].to_string()),
            passed: (tail[2] == "set").then(|| tail[3].to_string()),
        }
    }

    struct Fixture {
        dir: PathBuf,
        bin: String,
        real: String,
        notes: PathBuf,
        folder: PathBuf,
        label: [&'static str; 2],
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn unique(prefix: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("{prefix}{}-{}-{nanos}", std::process::id(), COUNT.fetch_add(1, Ordering::SeqCst)))
    }

    fn s(path: &Path) -> &str {
        path.to_str().unwrap()
    }

    impl Fixture {
        fn new() -> Fixture {
            let dir = unique("wsp-docker-shim-");
            let bin = dir.join("thread-bin");
            let real = dir.join("usr bin");
            let notes = dir.join("docker-notes");
            let folder = dir.join("acme").join("lab");
            // The shim's text first: a stub that panics then leaves no folder behind.
            let shim = docker_shim(s(&bin), s(&notes));
            for d in [&bin, &real, &folder] {
                fs::create_dir_all(d).unwrap();
            }
            // A shell started in a folder with no PWD reads the folder's real path: /private/var on a Mac.
            let folder = fs::canonicalize(&folder).unwrap();
            write_script(&bin.join("docker"), &shim);
            write_script(&real.join("docker"), REAL);
            Fixture { bin: s(&bin).to_string(), real: s(&real).to_string(), dir, notes, folder, label: ["--label", "wsp.thread=t_1f0c-ab"] }
        }

        fn default_path(&self) -> String {
            format!("{}:{}:/usr/bin:/bin", self.bin, self.real)
        }

        fn command(&self, args: &[&str], vars: &[(&str, &str)], path: &str) -> Command {
            let mut command = Command::new(Path::new(&self.bin).join("docker"));
            command.args(args).env_clear().env("PATH", path).envs(vars.iter().copied()).current_dir(&self.folder);
            command
        }

        fn run_on(&self, args: &[&str], vars: &[(&str, &str)], path: &str) -> Ran {
            let raw = exec(&mut self.command(args, vars, path), Duration::from_secs(60));
            if raw.stdout.is_empty() {
                return Ran { seen: Seen { argv: vec![], project: None, passed: None }, status: raw.status, stderr: raw.stderr };
            }
            Ran { seen: parse(&raw.stdout), status: raw.status, stderr: raw.stderr }
        }

        fn run(&self, args: &[&str], vars: &[(&str, &str)]) -> Ran {
            self.run_on(args, vars, &self.default_path())
        }

        fn in_thread(&self, args: &[&str], vars: &[(&str, &str)]) -> Ran {
            let mut all = vec![("WSP_THREAD", THREAD)];
            all.extend_from_slice(vars);
            self.run(args, &all)
        }

        fn lab(&self, before: &[&str], after: &[&str]) -> Vec<String> {
            before.iter().chain(self.label.iter()).chain(after.iter()).map(|w| w.to_string()).collect()
        }

        fn notes_of(&self, thread: &str) -> Vec<Vec<String>> {
            fs::read_to_string(self.notes.join(thread)).unwrap().trim_end().split('\n').map(|l| l.split('\t').map(String::from).collect()).collect()
        }
    }

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn labels_run_create_container_run_and_create_and_compose_run_after_any_global_flag() {
        let f = Fixture::new();
        let cases: Vec<(Vec<&str>, Vec<String>)> = vec![
            (vec!["run", "-d", "--name", "x", "busybox", "sleep", "60"], f.lab(&["run"], &["-d", "--name", "x", "busybox", "sleep", "60"])),
            (vec!["create", "busybox"], f.lab(&["create"], &["busybox"])),
            (vec!["container", "run", "busybox"], f.lab(&["container", "run"], &["busybox"])),
            (vec!["container", "create", "busybox"], f.lab(&["container", "create"], &["busybox"])),
            (vec!["compose", "run", "web"], f.lab(&["compose", "run"], &["web"])),
            (
                vec!["compose", "-f", "a b.yaml", "--project-directory", "run", "run", "web"],
                f.lab(&["compose", "-f", "a b.yaml", "--project-directory", "run", "run"], &["web"]),
            ),
            (vec!["compose", "--workdir", "run", "run", "web"], f.lab(&["compose", "--workdir", "run", "run"], &["web"])),
            (vec!["--log-level", "warn", "run", "busybox"], f.lab(&["--log-level", "warn", "run"], &["busybox"])),
            (
                vec!["-l", "warn", "-H", "unix:///var/run/docker.sock", "run", "busybox"],
                f.lab(&["-l", "warn", "-H", "unix:///var/run/docker.sock", "run"], &["busybox"]),
            ),
            (vec!["--context", "run", "run", "busybox"], f.lab(&["--context", "run", "run"], &["busybox"])),
            (
                vec!["-c", "default", "--config", "/tmp/cfg dir", "create", "busybox"],
                f.lab(&["-c", "default", "--config", "/tmp/cfg dir", "create"], &["busybox"]),
            ),
            (
                vec!["--host=tcp://h:2375", "-Dl", "info", "--tls", "container", "run", "busybox"],
                f.lab(&["--host=tcp://h:2375", "-Dl", "info", "--tls", "container", "run"], &["busybox"]),
            ),
            (
                vec!["--debug", "--tlscert", "c.pem", "compose", "-p", "lab", "run", "--rm", "web", "sh", "-c", "echo 'a b'"],
                f.lab(&["--debug", "--tlscert", "c.pem", "compose", "-p", "lab", "run"], &["--rm", "web", "sh", "-c", "echo 'a b'"]),
            ),
        ];
        for (args, want) in cases {
            let ran = f.in_thread(&args, &[]);
            assert_eq!(ran.seen.argv, want, "{}", args.join(" "));
            assert_eq!(ran.status, Some(7));
        }
    }

    #[test]
    fn hands_every_other_call_to_the_real_docker_with_the_same_words_and_its_exit_code() {
        let f = Fixture::new();
        let all: Vec<Vec<&str>> = vec![
            vec![],
            vec!["ps", "-a", "--filter", "label=wsp.thread"],
            vec!["exec", "-it", "x", "run"],
            vec!["image", "create"],
            vec!["container", "ls", "run"],
            vec!["compose", "ps", "run"],
            vec!["compose", "down"],
            vec!["-H", "run", "ps"],
            vec!["--version"],
            vec!["build", "-t", "run", "."],
            vec!["", "run"],
        ];
        for args in all {
            let ran = f.in_thread(&args, &[]);
            assert_eq!(ran.seen.argv, words(&args), "{}", args.join(" "));
            assert_eq!(ran.status, Some(7));
        }
        assert!(!f.notes.exists());
    }

    #[test]
    fn labels_nothing_outside_a_thread_or_for_a_thread_id_the_daemon_would_not_read() {
        let f = Fixture::new();
        let long = "a".repeat(65);
        let envs: Vec<Vec<(&str, &str)>> = vec![
            vec![],
            vec![("WSP_THREAD", "")],
            vec![("WSP_THREAD", "../../etc")],
            vec![("WSP_THREAD", "a b")],
            vec![("WSP_THREAD", &long)],
        ];
        for vars in &envs {
            assert_eq!(f.run(&["run", "busybox"], vars).seen.argv, words(&["run", "busybox"]));
            assert_eq!(f.run(&["compose", "up", "-d"], vars).seen.argv, words(&["compose", "up", "-d"]));
        }
        let max = "a".repeat(64);
        assert_eq!(f.in_thread(&["run", "x"], &[("WSP_THREAD", &max)]).seen.argv, words(&["run", "--label", &format!("wsp.thread={max}"), "x"]));
        assert!(!f.notes.exists());
    }

    #[test]
    fn runs_compose_up_and_create_with_the_person_s_words_and_environment_unchanged_and() {
        let f = Fixture::new();
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let from_env = [("COMPOSE_PROJECT_NAME", "from-env")];
        let cases: Vec<(Vec<&str>, Vec<(&str, &str)>, &str)> = vec![
            (vec!["compose", "up", "-d"], vec![], ""),
            (vec!["compose", "-p", "lab", "up", "-d", "--build"], vec![], "lab"),
            (vec!["compose", "--project-name=lab-2", "create"], vec![], "lab-2"),
            (vec!["compose", "--project-name", "lab_3", "up"], vec![], "lab_3"),
            (vec!["compose", "-plab4", "up"], vec![], "lab4"),
            (vec!["compose", "-p=lab5", "up"], vec![], "lab5"),
            (vec!["compose", "--workdir", "up", "-p", "lab6", "up"], vec![], "lab6"),
            (vec!["--log-level", "warn", "compose", "-f", "compose.yaml", "up", "-d"], from_env.to_vec(), "from-env"),
            (vec!["compose", "-p", "flag", "up"], from_env.to_vec(), "flag"),
        ];
        for (args, vars, _) in &cases {
            let ran = f.in_thread(args, vars);
            assert_eq!(ran.seen.argv, words(args), "{}", args.join(" "));
            let want = vars.iter().find(|(k, _)| *k == "COMPOSE_PROJECT_NAME").map(|(_, v)| v.to_string());
            assert_eq!(ran.seen.project, want);
            assert_eq!(ran.status, Some(7));
        }
        let lines = f.notes_of(THREAD);
        let got: Vec<Vec<String>> = lines.iter().map(|l| l[..3].to_vec()).collect();
        let want: Vec<Vec<String>> = cases.iter().map(|(_, _, project)| vec![THREAD.to_string(), s(&f.folder).to_string(), project.to_string()]).collect();
        assert_eq!(got, want);
        for line in &lines {
            assert_eq!(line.len(), 4);
            let at: u64 = line[3].parse().unwrap();
            let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
            assert!(at >= before, "{at}");
            assert!(at <= now_ms.div_ceil(1000), "{at}");
        }
    }

    #[test]
    fn notes_the_folder_compose_takes_for_the_project_project_directory_s_else_the_firs() {
        let f = Fixture::new();
        fs::create_dir_all(f.folder.join("sub").join("deeper")).unwrap();
        let elsewhere_path = f.dir.join("elsewhere-1");
        fs::create_dir_all(&elsewhere_path).unwrap();
        let elsewhere = fs::canonicalize(elsewhere_path).unwrap();
        let sub = f.folder.join("sub");
        let deeper = sub.join("deeper");
        let elsewhere_file = format!("{}/compose.yaml", s(&elsewhere));
        let composed = [("COMPOSE_FILE", "sub/compose.yaml:compose.yaml")];
        let cases: Vec<(Vec<&str>, Vec<(&str, &str)>, PathBuf)> = vec![
            (vec!["compose", "-f", "sub/compose.yaml", "up", "-d"], vec![], sub.clone()),
            (vec!["compose", "--file=sub/deeper/compose.yaml", "-f", "compose.yaml", "up"], vec![], deeper.clone()),
            (vec!["compose", "-f", &elsewhere_file, "up"], vec![], elsewhere.clone()),
            (vec!["compose", "--project-directory", "sub", "-f", "sub/deeper/compose.yaml", "up"], vec![], sub.clone()),
            (vec!["compose", "--project-directory=../lab/sub", "up"], vec![], sub.clone()),
            (vec!["compose", "--workdir", "sub/deeper", "create"], vec![], deeper.clone()),
            (vec!["compose", "up"], composed.to_vec(), sub.clone()),
            (vec!["compose", "-f", "-", "up"], vec![], f.folder.clone()),
            (vec!["compose", "-f", "gone/compose.yaml", "up"], vec![], f.folder.clone()),
        ];
        for (args, vars, _) in &cases {
            assert_eq!(f.in_thread(args, vars).seen.argv, words(args), "{}", args.join(" "));
        }
        let got: Vec<String> = f.notes_of(THREAD).iter().map(|l| l[1].clone()).collect();
        let want: Vec<String> = cases.iter().map(|(_, _, at)| s(at).to_string()).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn takes_compose_file_from_the_env_file_compose_reads_when_nothing_else_names_the_f() {
        let f = Fixture::new();
        fs::create_dir_all(f.folder.join("sub").join("deeper")).unwrap();
        let sub = f.folder.join("sub");
        let deeper = sub.join("deeper");
        let up = vec!["compose", "up"];
        let cases: Vec<(&str, Vec<&str>, Vec<(&str, &str)>, PathBuf)> = vec![
            ("COMPOSE_FILE=sub/compose.yaml", vec!["compose", "up", "-d"], vec![], sub.clone()),
            ("A=1\nexport COMPOSE_FILE = \"sub/deeper/compose.yaml:compose.yaml\"\r", up.clone(), vec![], deeper.clone()),
            ("COMPOSE_FILE='sub/compose.yaml'", vec!["compose", "create"], vec![], sub.clone()),
            ("COMPOSE_FILE=sub/compose.yaml", up.clone(), vec![("COMPOSE_FILE", "sub/deeper/compose.yaml")], deeper.clone()),
            ("COMPOSE_FILE=sub/compose.yaml", vec!["compose", "-f", "compose.yaml", "up"], vec![], f.folder.clone()),
            ("COMPOSE_FILE=sub/compose.yaml", vec!["compose", "--env-file", "other.env", "up"], vec![], deeper.clone()),
            ("SECRET=hunter2", up.clone(), vec![], f.folder.clone()),
        ];
        fs::write(f.folder.join("other.env"), "COMPOSE_FILE=sub/deeper/compose.yaml\n").unwrap();
        for (line, args, vars, _) in &cases {
            fs::write(f.folder.join(".env"), format!("SECRET=hunter2\n{line}\n")).unwrap();
            let mut all = vec![("WSP_THREAD", THREAD)];
            all.extend_from_slice(vars);
            let r = exec(&mut f.command(args, &all, &f.default_path()), Duration::from_secs(5));
            assert!(!r.timed_out, "{line}");
            assert_eq!(r.status, Some(7), "{line}");
            assert_eq!(parse(&r.stdout).argv, words(args), "{line}");
            assert!(!format!("{}{}", r.stdout, r.stderr).contains("hunter2"));
        }
        let got: Vec<String> = f.notes_of(THREAD).iter().map(|l| l[1].clone()).collect();
        let want: Vec<String> = cases.iter().map(|(_, _, _, at)| s(at).to_string()).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn never_waits_on_an_env_file_that_is_a_pipe() {
        let f = Fixture::new();
        fs::remove_file(f.folder.join(".env")).ok();
        assert!(Command::new("mkfifo").arg(f.folder.join(".env")).status().unwrap().success());
        let r = exec(&mut f.command(&["compose", "up"], &[("WSP_THREAD", THREAD)], &f.default_path()), Duration::from_secs(5));
        assert_eq!((r.timed_out, r.status), (false, Some(7)));
        let got: Vec<String> = f.notes_of(THREAD).iter().map(|l| l[1].clone()).collect();
        assert_eq!(got, vec![s(&f.folder).to_string()]);
    }

    #[test]
    fn hands_a_second_thread_s_compose_up_the_very_words_and_environment_the_first_one() {
        let f = Fixture::new();
        let args = ["compose", "-p", "lab", "up", "-d"];
        let first = f.in_thread(&args, &[("COMPOSE_PROJECT_NAME", "x")]);
        let second = f.run(&args, &[("WSP_THREAD", "t_other"), ("COMPOSE_PROJECT_NAME", "x")]).seen;
        let person = f.run(&args, &[("COMPOSE_PROJECT_NAME", "x")]).seen;
        assert_eq!((first.seen.argv.clone(), first.seen.project.clone(), first.status), (words(&args), Some("x".to_string()), Some(7)));
        assert_eq!(first.seen, second);
        assert_eq!(Seen { passed: None, ..first.seen.clone() }, Seen { passed: None, ..person });
        assert_eq!(f.notes_of(THREAD).len(), 1);
        assert_eq!(f.notes_of("t_other").len(), 1);
    }

    #[test]
    fn keeps_a_thread_s_notes_file_to_its_newest_lines_once_it_passes_the_cap() {
        let f = Fixture::new();
        fs::create_dir(&f.notes).unwrap();
        let line = format!("t_1f0c-ab\t{}\tlab\t1700000000\n", s(&f.folder));
        let cap = DOCKER_NOTES_CAP_BYTES as usize;
        fs::write(f.notes.join(THREAD), line.repeat(cap.div_ceil(line.len()) + 1)).unwrap();
        f.in_thread(&["compose", "up"], &[]);
        let lines = f.notes_of(THREAD);
        assert_eq!(lines.len(), 101);
        assert_eq!(lines.last().unwrap()[..3], [THREAD.to_string(), s(&f.folder).to_string(), String::new()]);
    }

    #[test]
    fn runs_the_call_when_the_note_cannot_be_written_and_says_nothing_of_it() {
        let f = Fixture::new();
        fs::write(&f.notes, "a file where the folder goes").unwrap();
        let ran = f.in_thread(&["compose", "up"], &[]);
        assert_eq!((ran.seen.argv, ran.status, ran.stderr), (words(&["compose", "up"]), Some(7), String::new()));
    }

    #[test]
    fn says_docker_is_not_installed_and_exits_127_where_no_docker_follows_it_on_path() {
        let f = Fixture::new();
        let empty = f.dir.join("empty");
        fs::create_dir(&empty).unwrap();
        for path in [format!("{}:{}", f.bin, s(&empty)), format!("{}:{}", f.bin, f.bin), f.bin.clone()] {
            let r = f.run_on(&["run", "busybox"], &[("WSP_THREAD", THREAD)], &path);
            assert_eq!((r.status, r.stderr), (Some(127), "docker is not installed on this computer\n".to_string()));
        }
    }

    #[test]
    fn finds_the_real_docker_past_its_own_folder_wherever_path_repeats_it_and_past_a_se() {
        let f = Fixture::new();
        let other = f.dir.join("place-bin");
        fs::create_dir(&other).unwrap();
        write_script(&other.join("docker"), &docker_shim(s(&other), s(&f.dir.join("other-notes"))));
        let path = format!("{}:{}:{}:{}:{}", f.bin, f.bin, s(&other), f.bin, f.real);
        let ran = f.run_on(&["run", "busybox"], &[("WSP_THREAD", THREAD)], &path);
        assert_eq!((ran.seen.argv.clone(), ran.status), (f.lab(&["run"], &["busybox"]), Some(7)));
        assert_eq!(ran.seen.passed, Some(format!("{}:{}", s(&other), f.bin)));
        assert_eq!(f.run_on(&["compose", "up"], &[("WSP_THREAD", THREAD)], &format!("{path}:/usr/bin:/bin")).seen.argv, words(&["compose", "up"]));
        assert_eq!(f.notes_of(THREAD).len(), 1);
        assert!(!f.dir.join("other-notes").exists());
    }

    #[test]
    fn passes_over_its_own_folder_however_path_spells_it() {
        let f = Fixture::new();
        // Node's join folds the `..` of the third spelling away, so it reads as the folder itself.
        for spelt in [format!("{}/", f.bin), format!("{}//", f.bin), f.bin.clone()] {
            let ran = f.run_on(&["run", "busybox"], &[("WSP_THREAD", THREAD)], &format!("{spelt}:{}", f.real));
            assert_eq!((ran.seen.argv, ran.status), (f.lab(&["run"], &["busybox"]), Some(7)), "{spelt}");
        }
        let other = f.dir.join("place-bin");
        fs::create_dir(&other).unwrap();
        write_script(&other.join("docker"), &docker_shim(s(&other), s(&f.dir.join("other-notes"))));
        let o = s(&other);
        let ran = f.run_on(&["run", "busybox"], &[("WSP_THREAD", THREAD)], &format!("{}/:{o}/:{}//:{o}:{}", f.bin, f.bin, f.real));
        assert_eq!((ran.seen.argv, ran.status), (f.lab(&["run"], &["busybox"]), Some(7)));
    }
}

mod the_cumulative_cpu_ps_prints {
    use super::*;

    #[test]
    fn reads_days_hours_minutes_seconds_and_this_mac_s_hundredths_and_nothing_else_as_n() {
        // Fixture provenance: Linux prints whole seconds (00:02:17), macOS hundredths (12:31.07), and either adds a day
        // field past 24 hours. The runtime's turn clock and the daemon's processes module both read this one parser.
        assert_eq!(ps_cpu_seconds("00:00:03"), 3.0);
        assert_eq!(ps_cpu_seconds("00:02:17"), 137.0);
        assert!((ps_cpu_seconds("12:31.07") - 751.07).abs() < 5e-6);
        assert_eq!(ps_cpu_seconds("1-18:19:15"), (86_400 + 18 * 3600 + 19 * 60 + 15) as f64);
        assert_eq!(ps_cpu_seconds("-"), 0.0);
        assert_eq!(ps_cpu_seconds(""), 0.0);
        assert_eq!(ps_cpu_seconds("what"), 0.0);
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use super::*;

    use serde_json::{json, Value};
    use wsp_wire::views::session_events::FilePatch;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    fn fp(v: Value) -> FilePatch {
        serde_json::from_value(v).unwrap()
    }

    fn env(pairs: &[(&str, &str)]) -> indexmap::IndexMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn quoting_and_escaping_text() {
        assert_eq!(wsp_words::quote::one_line(""), ""); // oneLine("")
        assert_eq!(wsp_words::quote::one_line("a  b\n\tc "), "a b c"); // oneLine("a  b\n\tc ")
        assert_eq!(wsp_words::quote::one_line("  lead"), "lead"); // oneLine("  lead")
        assert_eq!(wsp_words::quote::one_line("x\u{a0}y"), "x y"); // oneLine("x y")
        assert_eq!(wsp_words::quote::one_line("\u{feff}z"), "z"); // oneLine("﻿z")
        assert_eq!(wsp_words::quote::fence_for(""), "```"); // fenceFor("")
        assert_eq!(wsp_words::quote::fence_for("no ticks"), "```"); // fenceFor("no ticks")
        assert_eq!(wsp_words::quote::fence_for("a ` b"), "```"); // fenceFor("a ` b")
        assert_eq!(wsp_words::quote::fence_for("``` x ````"), "`````"); // fenceFor("``` x ````")
        assert_eq!(wsp_words::quote::fence_for("`"), "```"); // fenceFor("`")
        assert_eq!(wsp_words::regexp::escape_reg_exp("a.b*c"), "a\\.b\\*c"); // escapeRegExp("a.b*c")
        assert_eq!(wsp_words::regexp::escape_reg_exp("^$[]{}()|\\+?"), "\\^\\$\\[\\]\\{\\}\\(\\)\\|\\\\\\+\\?"); // escapeRegExp("^$[]{}()|\\+?")
        assert_eq!(wsp_words::regexp::escape_reg_exp("plain"), "plain"); // escapeRegExp("plain")
        assert_eq!(wsp_words::regexp::escape_reg_exp("/-"), "/-"); // escapeRegExp("/-")
    }

    #[test]
    fn ps_cpu_reads_every_column_form_and_nothing_else_as_zero() {
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("0:00.00"), 0.0); // psCpuSeconds("0:00.00")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("1:02.50"), 62.5); // psCpuSeconds("1:02.50")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("12:34"), 754.0); // psCpuSeconds("12:34")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("01:00:00"), 3600.0); // psCpuSeconds("01:00:00")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("2-03:04:05"), 183845.0); // psCpuSeconds("2-03:04:05")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("1-00:00:00.5"), 86400.5); // psCpuSeconds("1-00:00:00.5")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds(""), 0.0); // psCpuSeconds("")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("abc"), 0.0); // psCpuSeconds("abc")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("1:2:3:4"), 223384.0); // psCpuSeconds("1:2:3:4")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("99:99"), 6039.0); // psCpuSeconds("99:99")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds(" 1:00"), 60.0); // psCpuSeconds(" 1:00")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("5"), 5.0); // psCpuSeconds("5")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("1.5"), 1.5); // psCpuSeconds("1.5")
        assert_eq!(wsp_words::ps_time::ps_cpu_seconds("-1:00"), 60.0); // psCpuSeconds("-1:00")
    }

    #[test]
    fn hunks_are_built_parsed_and_cut_by_units() {
        assert_eq!(js(&wsp_words::tool_result::whole_file_hunk("", true)), "{\"oldStart\":0,\"oldLines\":0,\"newStart\":0,\"newLines\":0,\"lines\":[]}"); // JSON.stringify(wholeFileHunk("", "+"))
        assert_eq!(js(&wsp_words::tool_result::whole_file_hunk("a\nb\n", true)), "{\"oldStart\":0,\"oldLines\":0,\"newStart\":1,\"newLines\":2,\"lines\":[\"+a\",\"+b\"]}"); // JSON.stringify(wholeFileHunk("a\nb\n", "+"))
        assert_eq!(js(&wsp_words::tool_result::whole_file_hunk("a\nb", false)), "{\"oldStart\":1,\"oldLines\":2,\"newStart\":0,\"newLines\":0,\"lines\":[\"-a\",\"-b\"]}"); // JSON.stringify(wholeFileHunk("a\nb", "-"))
        assert_eq!(js(&wsp_words::tool_result::whole_file_hunk("\n", true)), "{\"oldStart\":0,\"oldLines\":0,\"newStart\":1,\"newLines\":1,\"lines\":[\"+\"]}"); // JSON.stringify(wholeFileHunk("\n", "+"))
        assert_eq!(js(&wsp_words::tool_result::whole_file_hunk("one", false)), "{\"oldStart\":1,\"oldLines\":1,\"newStart\":0,\"newLines\":0,\"lines\":[\"-one\"]}"); // JSON.stringify(wholeFileHunk("one", "-"))
        assert_eq!(js(&wsp_words::tool_result::unified_hunks("")), "[]"); // JSON.stringify(unifiedHunks(""))
        assert_eq!(js(&wsp_words::tool_result::unified_hunks("header\n@@ -1 +1 @@\n-a\n+b\n")), "[{\"oldStart\":1,\"oldLines\":1,\"newStart\":1,\"newLines\":1,\"lines\":[\"-a\",\"+b\"]}]"); // JSON.stringify(unifiedHunks("header\n@@ -1 +1 @@\n-a\n+b\n"))
        assert_eq!(js(&wsp_words::tool_result::unified_hunks("@@ -3,2 +3,0 @@\n-a\n-b\n@@ -10 +8,3 @@ fn x\n+1\n+2\n+3")), "[{\"oldStart\":3,\"oldLines\":2,\"newStart\":3,\"newLines\":0,\"lines\":[\"-a\",\"-b\"]},{\"oldStart\":10,\"oldLines\":1,\"newStart\":8,\"newLines\":3,\"lines\":[\"+1\",\"+2\",\"+3\"]}]"); // JSON.stringify(unifiedHunks("@@ -3,2 +3,0 @@\n-a\n-b\n@@ -10 +8,3 @@ fn x\n+1\n+2\n+3"))
        assert_eq!(js(&wsp_words::tool_result::unified_hunks("@@ bad @@\nx")), "[]"); // JSON.stringify(unifiedHunks("@@ bad @@\nx"))
        assert_eq!(js(&wsp_words::tool_result::unified_hunks("@@ -1,0 +1 @@\n+only\n")), "[{\"oldStart\":1,\"oldLines\":0,\"newStart\":1,\"newLines\":1,\"lines\":[\"+only\"]}]"); // JSON.stringify(unifiedHunks("@@ -1,0 +1 @@\n+only\n"))
        assert_eq!(js(&wsp_words::tool_result::kept_patch(&[], 5)), "{\"patch\":[]}"); // JSON.stringify(keptPatch([], 5))
        assert_eq!(js(&wsp_words::tool_result::kept_patch(&[fp(json!({"path":"a","hunks":[{"oldStart":1,"oldLines":2,"newStart":1,"newLines":2,"lines":["-ab","+cd"," ef","\\ No newline"]}]}))], 4)), "{\"patch\":[{\"path\":\"a\",\"hunks\":[{\"oldStart\":1,\"oldLines\":1,\"newStart\":1,\"newLines\":0,\"lines\":[\"-ab\"]}]}],\"patchCut\":true}"); // JSON.stringify(keptPatch([{"path":"a","hunks":[{"oldStart":1,"oldLines":2,"newStart":1,"newLines":2,"lines":["-ab","+cd"," ef","\\ No newline"]}]}], 4))
        assert_eq!(js(&wsp_words::tool_result::kept_patch(&[fp(json!({"path":"a","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-ab","+cd"]}]})), fp(json!({"path":"b","hunks":[{"oldStart":1,"oldLines":0,"newStart":1,"newLines":1,"lines":["+x"]}]}))], 6)), "{\"patch\":[{\"path\":\"a\",\"hunks\":[{\"oldStart\":1,\"oldLines\":1,\"newStart\":1,\"newLines\":1,\"lines\":[\"-ab\",\"+cd\"]}]}],\"patchCut\":true}"); // JSON.stringify(keptPatch([{"path":"a","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-ab","+cd"]}]},{"path":"b","hunks":[{"oldStart":1,"oldLines":0,"newStart":1,"newLines":1,"lines":["+x"]}]}], 6))
        assert_eq!(js(&wsp_words::tool_result::kept_patch(&[fp(json!({"path":"a","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-ab","+cd"]}]}))], 1)), "{\"patch\":[],\"patchCut\":true}"); // JSON.stringify(keptPatch([{"path":"a","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-ab","+cd"]}]}], 1))
        assert_eq!(js(&wsp_words::tool_result::kept_patch(&[fp(json!({"path":"a","movedTo":"c","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-ab"]}]})), fp(json!({"path":"b","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-zz"]}]}))], 3)), "{\"patch\":[{\"path\":\"a\",\"movedTo\":\"c\",\"hunks\":[{\"oldStart\":1,\"oldLines\":1,\"newStart\":1,\"newLines\":1,\"lines\":[\"-ab\"]}]}],\"patchCut\":true}"); // JSON.stringify(keptPatch([{"path":"a","movedTo":"c","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-ab"]}]},{"path":"b","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-zz"]}]}], 3))
        assert_eq!(js(&wsp_words::tool_result::kept_patch(&[fp(json!({"path":"a","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["+😀"]}]}))], 2)), "{\"patch\":[],\"patchCut\":true}"); // JSON.stringify(keptPatch([{"path":"a","hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["+😀"]}]}], 2))
    }

    #[test]
    fn oom_and_shell_startup_lines() {
        assert_eq!(wsp_words::oom::work_score_line(None), "{ echo 500 > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1"); // workScoreLine()
        assert_eq!(wsp_words::oom::work_score_line(Some(-5.0)), "{ echo -5 > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1"); // workScoreLine(-5)
        assert_eq!({ let a = wsp_words::oom::work_argv("/bin/claude", &["-p", "a b"]); js(&json!({ "file": a.file, "args": a.args })) }, "{\"file\":\"/bin/sh\",\"args\":[\"-c\",\"{ echo 500 > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1; exec \\\"$0\\\" \\\"$@\\\"\",\"/bin/claude\",\"-p\",\"a b\"]}"); // JSON.stringify(workArgv("/bin/claude", ["-p", "a b"]))
        assert_eq!({ let v = wsp_words::thread_shell::thread_shell_vars("/t/bin"); js(&json!({ "ZDOTDIR": v.zdotdir, "BASH_ENV": v.bash_env })) }, "{\"ZDOTDIR\":\"/t/bin\",\"BASH_ENV\":\"/t/bin/.bash_env\"}"); // JSON.stringify(threadShellVars("/t/bin"))
        assert_eq!(js(&wsp_words::thread_shell::thread_shell_files("/t/my bin", &env(&[]))), "[[\".zshenv\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/my bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zshenv\\\" ]] && source \\\"$ZDOTDIR/.zshenv\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/my bin'\\n[[ :$PATH: == ':/t/my bin:'* ]] || PATH='/t/my bin':$PATH\\n\"],[\".zprofile\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/my bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zprofile\\\" ]] && source \\\"$ZDOTDIR/.zprofile\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/my bin'\\n[[ :$PATH: == ':/t/my bin:'* ]] || PATH='/t/my bin':$PATH\\n\"],[\".zshrc\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/my bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zshrc\\\" ]] && source \\\"$ZDOTDIR/.zshrc\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/my bin'\\n[[ :$PATH: == ':/t/my bin:'* ]] || PATH='/t/my bin':$PATH\\n\"],[\".zlogin\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/my bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zlogin\\\" ]] && source \\\"$ZDOTDIR/.zlogin\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/my bin'\\n[[ :$PATH: == ':/t/my bin:'* ]] || PATH='/t/my bin':$PATH\\n\"],[\".bash_env\",\"case :$PATH: in ':/t/my bin:'*) ;; *) PATH='/t/my bin':$PATH ;; esac\\n\"]]"); // JSON.stringify(threadShellFiles("/t/my bin"))
        assert_eq!(js(&wsp_words::thread_shell::thread_shell_files("/t/bin", &env(&[("ZDOTDIR", "/home/u/.z"), ("BASH_ENV", "/home/u/.benv")]))), "[[\".zshenv\",\"ZDOTDIR=${_wsp_zdotdir:-'/home/u/.z'}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zshenv\\\" ]] && source \\\"$ZDOTDIR/.zshenv\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".zprofile\",\"ZDOTDIR=${_wsp_zdotdir:-'/home/u/.z'}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zprofile\\\" ]] && source \\\"$ZDOTDIR/.zprofile\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".zshrc\",\"ZDOTDIR=${_wsp_zdotdir:-'/home/u/.z'}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zshrc\\\" ]] && source \\\"$ZDOTDIR/.zshrc\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".zlogin\",\"ZDOTDIR=${_wsp_zdotdir:-'/home/u/.z'}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zlogin\\\" ]] && source \\\"$ZDOTDIR/.zlogin\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".bash_env\",\"[ -r '/home/u/.benv' ] && ! [ '/home/u/.benv' -ef '/t/bin/.bash_env' ] && . '/home/u/.benv'\\ncase :$PATH: in ':/t/bin:'*) ;; *) PATH='/t/bin':$PATH ;; esac\\n\"]]"); // JSON.stringify(threadShellFiles("/t/bin", {ZDOTDIR:"/home/u/.z", BASH_ENV:"/home/u/.benv"}))
        assert_eq!(js(&wsp_words::thread_shell::thread_shell_files("/t/bin", &env(&[("ZDOTDIR", ""), ("BASH_ENV", "")]))), "[[\".zshenv\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zshenv\\\" ]] && source \\\"$ZDOTDIR/.zshenv\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".zprofile\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zprofile\\\" ]] && source \\\"$ZDOTDIR/.zprofile\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".zshrc\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zshrc\\\" ]] && source \\\"$ZDOTDIR/.zshrc\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".zlogin\",\"ZDOTDIR=${_wsp_zdotdir:-$HOME}\\n[[ $ZDOTDIR -ef '/t/bin' ]] && ZDOTDIR=$HOME\\n_wsp_zdotdir=\\n[[ -r \\\"$ZDOTDIR/.zlogin\\\" ]] && source \\\"$ZDOTDIR/.zlogin\\\"\\n_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}\\nZDOTDIR='/t/bin'\\n[[ :$PATH: == ':/t/bin:'* ]] || PATH='/t/bin':$PATH\\n\"],[\".bash_env\",\"case :$PATH: in ':/t/bin:'*) ;; *) PATH='/t/bin':$PATH ;; esac\\n\"]]"); // JSON.stringify(threadShellFiles("/t/bin", {ZDOTDIR:"", BASH_ENV:""}))
    }

    #[test]
    fn a_threads_cgroup_is_named_per_utf16_unit_and_ended_by_one_function() {
        assert_eq!(wsp_words::thread_cgroup::thread_cgroup("t1"), "/wsp-threads/t1"); // threadCgroup("t1")
        assert_eq!(wsp_words::thread_cgroup::thread_cgroup("01HAB-cd_ef"), "/wsp-threads/01HAB-cd_ef"); // threadCgroup("01HAB-cd_ef")
        assert_eq!(wsp_words::thread_cgroup::thread_cgroup("a/b c.d"), "/wsp-threads/a_b_c_d"); // threadCgroup("a/b c.d")
        assert_eq!(wsp_words::thread_cgroup::thread_cgroup("\u{e9}\u{1f600}"), "/wsp-threads/___"); // threadCgroup("é😀")
        assert_eq!(wsp_words::thread_cgroup::cgroup_join_line("/wsp-threads/t1"), "[ -f /sys/fs/cgroup/cgroup.controllers ] && mkdir -p '/sys/fs/cgroup/wsp-threads/t1' 2>/dev/null && { echo '+memory +pids' > '/sys/fs/cgroup/wsp-threads'/cgroup.subtree_control; echo $$ > '/sys/fs/cgroup/wsp-threads/t1'/cgroup.procs; } 2>/dev/null; true"); // cgroupJoinLine("/wsp-threads/t1")
        assert_eq!(wsp_words::thread_cgroup::cgroup_end_script("/wsp-threads/it's", false), "wsp_end() {\n  C=$1; at=\"/sys/fs/cgroup$1\"\n  [ -d \"$at\" ] || return 0\n  mine() { g=$(sed -n 's/^0:://p' /proc/$1/cgroup 2>/dev/null); case \"$g\" in \"$C\"|\"$C\"/*) ;; *) return 1 ;; esac; s=$(sed 's/.*) //' /proc/$1/stat 2>/dev/null); [ -n \"$s\" ] && [ \"${s%% *}\" != Z ]; }\n  left() { L=; for p in $P; do mine \"$p\" && L=\"$L $p\"; done; P=$L; }\n  P=$(find \"$at\" -name cgroup.procs -exec cat {} + 2>/dev/null)\n  [ -n \"$P\" ] && kill -TERM $P 2>/dev/null\n  i=0; left; while [ $i -lt 30 ] && [ -n \"$P\" ]; do sleep 0.1; i=$((i+1)); left; done\n  [ -n \"$P\" ] && kill -KILL $P 2>/dev/null\n  i=0; left; while [ $i -lt 10 ] && [ -n \"$P\" ]; do sleep 0.1; i=$((i+1)); left; done\n  [ -z \"$P\" ] || { echo \"processes$P of $C did not end\" >&2; return 1; }\n  [ \"$2\" = 1 ] || return 0\n  echo 1 > \"$at/cgroup.kill\" 2>/dev/null || { Q=$(find \"$at\" -name cgroup.procs -exec cat {} + 2>/dev/null); [ -n \"$Q\" ] && kill -KILL $Q 2>/dev/null; }\n  i=0; while [ $i -lt 30 ] && [ -d \"$at\" ]; do find \"$at\" -depth -type d -exec rmdir {} + 2>/dev/null; [ -d \"$at\" ] && sleep 0.1; i=$((i+1)); done\n  [ -d \"$at\" ] && { echo \"$C still holds processes\" >&2; return 1; }\n  return 0\n}\nwsp_end '/wsp-threads/it'\\''s' 0"); // cgroupEndScript("/wsp-threads/it's")
        assert_eq!(wsp_words::thread_cgroup::cgroup_end_script("/wsp-threads/t1", true).lines().last().unwrap().to_string(), "wsp_end '/wsp-threads/t1' 1"); // cgroupEndScript("/wsp-threads/t1", {remove:true}).split("\n").at(-1)
        assert_eq!(wsp_words::thread_cgroup::thread_cgroups_end_script(None), "wsp_end() {\n  C=$1; at=\"/sys/fs/cgroup$1\"\n  [ -d \"$at\" ] || return 0\n  mine() { g=$(sed -n 's/^0:://p' /proc/$1/cgroup 2>/dev/null); case \"$g\" in \"$C\"|\"$C\"/*) ;; *) return 1 ;; esac; s=$(sed 's/.*) //' /proc/$1/stat 2>/dev/null); [ -n \"$s\" ] && [ \"${s%% *}\" != Z ]; }\n  left() { L=; for p in $P; do mine \"$p\" && L=\"$L $p\"; done; P=$L; }\n  P=$(find \"$at\" -name cgroup.procs -exec cat {} + 2>/dev/null)\n  [ -n \"$P\" ] && kill -TERM $P 2>/dev/null\n  i=0; left; while [ $i -lt 30 ] && [ -n \"$P\" ]; do sleep 0.1; i=$((i+1)); left; done\n  [ -n \"$P\" ] && kill -KILL $P 2>/dev/null\n  i=0; left; while [ $i -lt 10 ] && [ -n \"$P\" ]; do sleep 0.1; i=$((i+1)); left; done\n  [ -z \"$P\" ] || { echo \"processes$P of $C did not end\" >&2; return 1; }\n  [ \"$2\" = 1 ] || return 0\n  echo 1 > \"$at/cgroup.kill\" 2>/dev/null || { Q=$(find \"$at\" -name cgroup.procs -exec cat {} + 2>/dev/null); [ -n \"$Q\" ] && kill -KILL $Q 2>/dev/null; }\n  i=0; while [ $i -lt 30 ] && [ -d \"$at\" ]; do find \"$at\" -depth -type d -exec rmdir {} + 2>/dev/null; [ -d \"$at\" ] && sleep 0.1; i=$((i+1)); done\n  [ -d \"$at\" ] && { echo \"$C still holds processes\" >&2; return 1; }\n  return 0\n}\n[ -d '/sys/fs/cgroup/wsp-threads' ] || exit 0\nfor d in '/sys/fs/cgroup/wsp-threads'/*/; do [ -d \"$d\" ] || continue; t=$(basename \"$d\"); wsp_end '/wsp-threads'/\"$t\" 1 && echo '/sys/fs/cgroup/wsp-threads'/\"$t\"; done\nrmdir '/sys/fs/cgroup/wsp-threads' 2>/dev/null\nexit 0"); // threadCgroupsEndScript()
        assert_eq!({ let s = wsp_words::thread_cgroup::thread_cgroups_end_script(Some("/x y")); let l: Vec<&str> = s.lines().collect(); l[l.len() - 4..].join("\n") }, "[ -d '/sys/fs/cgroup/x y' ] || exit 0\nfor d in '/sys/fs/cgroup/x y'/*/; do [ -d \"$d\" ] || continue; t=$(basename \"$d\"); wsp_end '/x y'/\"$t\" 1 && echo '/sys/fs/cgroup/x y'/\"$t\"; done\nrmdir '/sys/fs/cgroup/x y' 2>/dev/null\nexit 0"); // threadCgroupsEndScript("/x y").split("\n").slice(-4).join("\n")
    }

    #[test]
    fn the_docker_shim_quotes_its_two_paths() {
        assert_eq!(wsp_words::docker_shim::docker_shim("/opt/my bin/docker", "/n's").lines().take(3).collect::<Vec<_>>().join("\n"), "#!/bin/sh\nbin='/opt/my bin/docker'\nnotes='/n'\\''s'"); // dockerShim("/opt/my bin/docker", "/n's").split("\n").slice(0, 3).join("\n")
        assert_eq!(wsp_words::js::utf16::len(&wsp_words::docker_shim::docker_shim("/b", "/n")) as f64, 3180.0); // dockerShim("/b", "/n").length
    }
}
