// SPDX-License-Identifier: AGPL-3.0-only
//! packages/engine/test/machine-context.test.ts: "the probe", "the facts" and "the guest scripts on a local bash".
//! The last group runs the real probe and bounded command under this box's shells against a temp root and a fake home.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::Engine as _;
use wsp_engine::golden_seam::{df_kb_cmd, DfColumn, GoldenBaseTool, ImportAgentRow, ImportFiles, ImportResult, ImportRow, RetiredRow, SkippedPath, TOOLS_PATH};
use wsp_engine::machine::ExecResult;
use wsp_engine::machine_context::{
    bounded_command, context_agents, merge_facts, overlay_probe, parse_probe, probe_command, render_machine_context, Alias,
    AliasesUnread, BuildFacts, ContextInput, Disk, FileNote, GuestRoots, MissingItem, ProbeShells, WorkspaceName,
    ALIAS_PROBE_BASH, ALIAS_PROBE_FISH, ALIAS_PROBE_ZSH, BROWSER_SHIM_PATH, CONTEXT_MARKER,
};

fn facts() -> BuildFacts {
    BuildFacts {
        tools: vec![MissingItem { id: "tools/brew-cask/raycast".into(), label: "raycast".into(), note: "macOS app, no Linux build".into() }],
        agents: vec![MissingItem { id: "agents/zed".into(), label: "Zed".into(), note: "no installer known".into() }],
        files: vec![FileNote { path: "~/.ssh/id_ed25519".into(), note: "private key, never copied".into() }],
    }
}

fn probe_out() -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&facts()).unwrap());
    [
        "some rc noise", "WSP_CTX", "KERNEL 6.6.30", "DISK 20466256 11720704", "OVERLAY no", "HAS tmux", "HAS fish", "HAS brew",
        "HAS golden-path", "HAS wsp-open", "VERSION python3: Python 3.12.13", "VERSION uv: uv 0.12.9", "VERSION cc: ",
        "AGENT claude", "AGENT codex", "AGENT gemini", "AGENT opencode", "AGENT pi", "AGENT hermes", "SECRET OPENAI_API_KEY",
        "SECRET GH_TOKEN", "SHELL zsh", "ALIAS o open", "ALIAS rc code", &format!("FACTS {b64}"), "WSP_CTX_END", "",
    ]
    .join("\n")
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn alias(name: &str, word: &str) -> Alias {
    Alias { name: name.into(), word: word.into() }
}

fn item(id: &str, label: &str, note: &str) -> MissingItem {
    MissingItem { id: id.into(), label: label.into(), note: note.into() }
}

fn note(path: &str, note: &str) -> FileNote {
    FileNote { path: path.into(), note: note.into() }
}

/// machine-context.test.ts:110
#[test]
fn parses_every_line_kind_between_the_markers_and_ignores_the_rest() {
    let probe = parse_probe(&probe_out()).expect("a probe");
    assert_eq!(probe.kernel.as_deref(), Some("6.6.30"));
    assert_eq!(probe.disk, Some(Disk { size_bytes: 20466256.0 * 1024.0, free_bytes: 11720704.0 * 1024.0 }));
    assert!(!probe.overlay);
    assert_eq!(probe.has.iter().map(String::as_str).collect::<Vec<_>>(), ["tmux", "fish", "brew", "golden-path", "wsp-open"]);
    assert_eq!(
        probe.versions,
        vec![GoldenBaseTool { name: "python3".into(), version: "3.12.13".into() }, GoldenBaseTool { name: "uv".into(), version: "0.12.9".into() }]
    );
    assert_eq!(probe.agents, context_agents().iter().map(|a| a.id.clone()).collect::<Vec<_>>());
    assert_eq!(probe.secrets, ids(&["OPENAI_API_KEY", "GH_TOKEN"]));
    assert_eq!(probe.shell, "zsh");
    assert_eq!(probe.aliases, vec![alias("o", "open"), alias("rc", "code")]);
    assert_eq!(probe.conflicts.len(), 0);
    assert_eq!(probe.facts, Some(facts()));
}

/// machine-context.test.ts:129
#[test]
fn is_nothing_without_both_markers_and_tolerates_empty_reads() {
    assert_eq!(parse_probe(""), None);
    assert_eq!(parse_probe("WSP_CTX\nKERNEL 1\n"), None);
    let probe = parse_probe("WSP_CTX\nKERNEL \nDISK \nOVERLAY yes\nSHELL \nCONFLICT gemini\nCONFLICT nobody\nSECRET not a name\nWSP_CTX_END\n").expect("a probe");
    assert_eq!(probe.kernel, None);
    assert_eq!(probe.disk, None);
    assert!(probe.overlay);
    assert_eq!(probe.shell, "bash");
    assert_eq!(probe.conflicts.iter().map(String::as_str).collect::<Vec<_>>(), ["gemini"]);
    assert_eq!(probe.secrets, Vec::<String>::new());
    assert_eq!(probe.versions, vec![]);
    assert_eq!(probe.facts, None);
}

/// machine-context.test.ts:143
#[test]
fn asks_the_base_floors_commands_for_their_versions_on_the_tools_path_keeping_the_first_line_that_carries_one() {
    let cmd = probe_command(None, None, None);
    let first = r"grep -m1 -E '[0-9]+\.[0-9]+'";
    assert!(cmd.contains(&format!(r#"echo "VERSION uv: $(uv --version 2>/dev/null | {first})""#)), "{cmd}");
    assert!(cmd.contains(&format!(r#"echo "VERSION python3: $(python3 --version 2>/dev/null | {first})""#)), "{cmd}");
    assert!(cmd.contains(&format!(r#"echo "VERSION unzip: $(unzip -v 2>/dev/null | {first})""#)), "{cmd}");
    assert_eq!(cmd.split('\n').filter(|l| l.starts_with("export PATH=")).count(), 1, "{cmd}");
}

fn result(over: impl FnOnce(&mut ImportResult)) -> ImportResult {
    let mut r = ImportResult { recipe_hash: "h".into(), tools: vec![], agents: vec![], ..Default::default() };
    over(&mut r);
    r
}

fn row(id: &str, label: &str, outcome: &str, note: Option<&str>) -> ImportRow {
    ImportRow { id: id.into(), label: label.into(), outcome: outcome.into(), note: note.map(String::from) }
}

/// machine-context.test.ts:156
#[test]
fn adds_what_did_not_install_drops_what_did_and_keeps_the_rest() {
    let next = merge_facts(
        Some(&facts()),
        Some(&result(|r| {
            r.tools = vec![
                row("tools/brew/jq", "jq", "installed", None),
                row("tools/brew/x", "x", "failed", Some("Error: no bottle")),
                row("tools/brew-cask/raycast", "Raycast", "skipped", Some("macOS app, no Linux build")),
            ];
            r.agents = vec![ImportAgentRow { id: "agents/zed".into(), name: "Zed".into(), outcome: "installed".into(), note: None }];
            r.files = Some(ImportFiles { bytes: 1, skipped: vec![SkippedPath { path: "~/.netrc".into(), note: ".netrc is never copied; sign in on the machine".into() }] });
        })),
    );
    assert_eq!(
        next.tools,
        vec![item("tools/brew-cask/raycast", "Raycast", "macOS app, no Linux build"), item("tools/brew/x", "x", "Error: no bottle")]
    );
    assert_eq!(next.agents, vec![]);
    assert_eq!(
        next.files,
        vec![note("~/.ssh/id_ed25519", "private key, never copied"), note("~/.netrc", ".netrc is never copied; sign in on the machine")]
    );
}

/// machine-context.test.ts:177
#[test]
fn a_base_floor_row_that_did_not_land_is_a_tool_that_did_not_install_beside_the_persons() {
    let next = merge_facts(
        None,
        Some(&result(|r| {
            r.base = Some(vec![
                row("base/node", "Node 22 with npm", "installed", None),
                row("base/fd", "fd", "failed", Some("E: Unable to locate package fd-find")),
            ]);
            r.tools = vec![row("tools/brew/x", "x", "failed", Some("Error: no bottle"))];
        })),
    );
    assert_eq!(next.tools, vec![item("base/fd", "fd", "E: Unable to locate package fd-find"), item("tools/brew/x", "x", "Error: no bottle")]);
}

/// machine-context.test.ts:191
#[test]
fn a_retired_row_takes_its_own_row_out_of_the_facts_and_no_result_leaves_the_facts_as_they_were() {
    let next = merge_facts(Some(&facts()), Some(&result(|r| r.retired = Some(vec![RetiredRow { id: "tools/brew-cask/raycast".into() }]))));
    assert_eq!(next.tools, vec![]);
    assert_eq!(next.agents, facts().agents);
    let no_zed = merge_facts(Some(&facts()), Some(&result(|r| r.retired = Some(vec![RetiredRow { id: "agents/zed".into() }]))));
    assert_eq!(no_zed.agents, vec![]);
    assert_eq!(no_zed.tools, facts().tools);
    assert_eq!(merge_facts(Some(&facts()), None), facts());
    assert_eq!(merge_facts(None, None), BuildFacts { tools: vec![], agents: vec![], files: vec![] });
}

struct Guest {
    _dir: tempfile::TempDir,
    roots: GuestRoots,
    pid_files: Vec<PathBuf>,
}

impl Guest {
    fn new() -> Guest {
        let dir = tempfile::Builder::new().prefix("wsp-ctx-").tempdir().unwrap();
        let roots = GuestRoots { etc: dir.path().join("etc").to_string_lossy().into_owned(), home: dir.path().join("home").to_string_lossy().into_owned() };
        std::fs::create_dir_all(&roots.etc).unwrap();
        std::fs::create_dir_all(&roots.home).unwrap();
        Guest { _dir: dir, roots, pid_files: vec![] }
    }

    fn home(&self) -> &Path {
        Path::new(&self.roots.home)
    }

    fn etc(&self) -> &Path {
        Path::new(&self.roots.etc)
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        for file in &self.pid_files {
            for pid in pids_in(file) {
                if alive(pid) {
                    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
                }
            }
        }
    }
}

fn pids_in(file: &Path) -> Vec<u32> {
    match std::fs::read_to_string(file) {
        Ok(s) => s.lines().filter(|l| !l.is_empty()).map(|l| l.parse().unwrap()).collect(),
        Err(_) => vec![],
    }
}

/// A killed job whose parent is gone sits as a zombie until init reaps it, and kill(pid, 0) still answers for it.
fn alive(pid: u32) -> bool {
    if unsafe { libc::kill(pid as i32, 0) } != 0 {
        return false;
    }
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat.rsplit_once(')').map(|(_, rest)| rest.trim_start().as_bytes().first() != Some(&b'Z')).unwrap_or(true),
        Err(_) => false,
    }
}

async fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_millis(1000);
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// A login shell reads its startup files from HOME, and zsh from ZDOTDIR first, so both name the fake home: the shell
/// of whoever runs the suite never runs, since what their rc file does is theirs and not the case's.
async fn run(script: &str, home: &Path) -> ExecResult {
    let out = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(script)
        .env("HOME", home)
        .env("ZDOTDIR", home)
        .output()
        .await
        .expect("bash runs");
    ExecResult {
        exit_code: out.status.code().unwrap_or(1) as i64,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn write_stub(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::remove_file(path);
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn write(path: impl AsRef<Path>, body: &str) {
    if let Some(dir) = path.as_ref().parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

/// machine-context.test.ts:480
#[tokio::test]
async fn the_probe_prints_its_markers_the_kernel_and_the_disk_and_names_each_hook_the_persons_file_claims() {
    let g = Guest::new();
    let roots = &g.roots;
    let df = df_kb_cmd(&[DfColumn::Size, DfColumn::Free], Some(&roots.home));
    assert!(probe_command(Some(roots), None, None).contains(&df), "{df}");
    let mut probe = parse_probe(&run(&probe_command(Some(roots), None, None), g.home()).await.stdout).expect("a probe");
    assert!(probe.kernel.as_deref().is_some_and(|k| !k.is_empty()));
    assert!(probe.disk.expect("a disk").size_bytes > 0.0);
    assert_eq!(probe.has.contains("wsp-open"), Path::new(BROWSER_SHIM_PATH).exists());
    assert_eq!(probe.conflicts.len(), 0);
    assert_eq!(probe.facts, None);

    write(g.home().join(".codex/config.toml"), "model = \"x\"\n# developer_instructions = \"commented\"\n[mcp_servers.a]\ndeveloper_instructions = \"in a table\"\n");
    write(g.home().join(".gemini/settings.json"), "{\n  // a comment\n  \"context\": { \"fileName\": \"CUSTOM.md\" }\n}\n");
    write(g.home().join(".pi/agent/APPEND_SYSTEM.md"), &format!("{CONTEXT_MARKER}\nours\n"));
    write(g.home().join(".hermes/config.yaml"), "model:\n  environment_hint: \"not the agent block\"\nagent:\n  environment_hint: \"\"\n");
    write(g.etc().join("wsp/machine-context.json"), &serde_json::to_string(&facts()).unwrap());
    write(g.etc().join("profile.d/wsp-secrets.sh"), "export GH_TOKEN='sk-ant-x'\nexport BAD-NAME='x'\n");
    probe = parse_probe(&run(&probe_command(Some(roots), None, None), g.home()).await.stdout).expect("a probe");
    assert_eq!(probe.conflicts.iter().map(String::as_str).collect::<Vec<_>>(), ["gemini"]);
    assert_eq!(probe.facts, Some(facts()));
    assert_eq!(probe.secrets, ids(&["GH_TOKEN"]));

    write(g.home().join(".codex/config.toml"), "developer_instructions = \"\"\"\ntheirs\n\"\"\"\n");
    write(g.home().join(".pi/agent/APPEND_SYSTEM.md"), "the person's own\n");
    write(g.home().join(".hermes/config.yaml"), "agent:\n  model: x\n  environment_hint: 'set by them'\n");
    write(g.home().join(".gemini/settings.json"), "{ \"theme\": \"dark\" }\n");
    probe = parse_probe(&run(&probe_command(Some(roots), None, None), g.home()).await.stdout).expect("a probe");
    let mut conflicts: Vec<&str> = probe.conflicts.iter().map(String::as_str).collect();
    conflicts.sort();
    assert_eq!(conflicts, ["hermes", "pi"]);
}

/// machine-context.test.ts:516
#[tokio::test]
async fn asks_modprobe_for_an_overlay_the_kernel_does_not_list_yet_and_says_no_where_it_will_not_load() {
    let g = Guest::new();
    let filesystems = g.etc().join("filesystems");
    let bin = g.etc().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let overlay_on = |modprobe: String, listed: &'static str| {
        let (filesystems, bin, home) = (filesystems.clone(), bin.clone(), g.home().to_path_buf());
        async move {
            std::fs::write(&filesystems, listed).unwrap();
            write_stub(&bin.join("modprobe"), &format!("#!/bin/sh\necho \"$*\" >> {}/asked\n{modprobe}\n", bin.display()));
            let _ = std::fs::remove_file(bin.join("asked"));
            let script = format!("PATH={}:$PATH; {}", bin.display(), overlay_probe(Some(&filesystems.to_string_lossy())));
            run(&script, &home).await.stdout
        }
    };
    let asked = || std::fs::read_to_string(bin.join("asked")).unwrap_or_default();
    assert_eq!(overlay_on("exit 1".into(), "nodev\tsysfs\nnodev\toverlay\n").await, "OVERLAY yes\n");
    assert_eq!(asked(), "");
    assert_eq!(overlay_on(format!("printf 'nodev\\toverlay\\n' >> {}", filesystems.display()), "nodev\tsysfs\n").await, "OVERLAY yes\n");
    assert_eq!(asked(), "overlay\n");
    assert_eq!(
        overlay_on("echo \"modprobe: ERROR: could not insert 'overlay': Operation not permitted\" >&2; exit 1".into(), "nodev\tsysfs\n").await,
        "OVERLAY no\n"
    );
    assert_eq!(asked(), "overlay\n");
    assert_eq!(overlay_on("exit 0".into(), "nodev\tsysfs\n").await, "OVERLAY no\n");
}

/// machine-context.test.ts:537
#[tokio::test]
async fn opens_no_shell_of_the_machines_own_where_it_may_open_none_and_reads_every_other_fact_just_the_same() {
    let g = Guest::new();
    let marker = g.home().join("sourced");
    // Every startup file a login shell of any family reads under the home it is pointed at, each writing its own
    // name: a box's root home is the one every workspace on it writes, so a line in any of them is a workspace's.
    for rc in [".profile", ".bash_profile", ".bashrc", ".zshenv", ".zprofile", ".zshrc"] {
        write(g.home().join(rc), &format!("echo {rc} >> {}\n", marker.display()));
    }

    let none = probe_command(Some(&g.roots), Some(TOOLS_PATH), Some(ProbeShells::None));
    assert!(!none.contains("-lic"), "{none}");
    assert!(!none.contains("bash -l"), "{none}");
    let quiet = parse_probe(&run(&none, g.home()).await.stdout).expect("a probe");
    assert!(!marker.exists());
    assert!(quiet.kernel.as_deref().is_some_and(|k| !k.is_empty()));
    assert!(quiet.disk.expect("a disk").size_bytes > 0.0);
    assert!(!quiet.shell.is_empty());
    assert_eq!(quiet.aliases_unread, Some(AliasesUnread::Shared));
    assert_eq!(quiet.aliases, vec![]);

    // And the machine wsp forked, whose home is its own root's: the shell is opened there and the files are read.
    let login = probe_command(Some(&g.roots), Some(TOOLS_PATH), Some(ProbeShells::Login));
    assert!(login.contains("-lic"), "{login}");
    let read = parse_probe(&run(&login, g.home()).await.stdout).expect("a probe");
    assert_eq!(read.aliases_unread, None);
    // Which shell was opened is this computer's own, so what it reads first is read off the word the probe named
    // rather than off one written here: the file differs by family and the platform decides which family runs.
    let sourced = std::fs::read_to_string(&marker).unwrap_or_default();
    assert!(!sourced.trim().is_empty(), "no startup file ran");
    let first = match read.shell.as_str() {
        "bash" => Some(".bash_profile"),
        "sh" => Some(".profile"),
        "zsh" => Some(".zshenv"),
        _ => None,
    };
    if let Some(first) = first {
        assert!(sourced.contains(first), "{} read {sourced:?}", read.shell);
    }
}

fn shell_of(name: &str) -> Option<PathBuf> {
    ["/bin", "/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"].iter().map(|d| Path::new(d).join(name)).find(|p| p.exists())
}

const ALIASES: [(&str, &str); 9] = [
    ("g", "git"),
    ("ll", "ls -la"),
    ("o", "open-not-here"),
    ("s", "sudo not-here-either"),
    ("e", "FOO=1 not-here-third"),
    ("q", "\\not-here-fourth --flag"),
    ("kca", "_kca(){ kubectl \"$@\" --all-namespaces; unset -f _kca; }; _kca"),
    ("grp", "{ grep -rn . ; }"),
    ("abs", "/Applications/Nowhere.app/Contents/MacOS/nowhere"),
];

fn missing() -> Vec<Alias> {
    vec![alias("e", "not-here-third"), alias("o", "open-not-here"), alias("q", "not-here-fourth"), alias("s", "not-here-either")]
}

/// The aliases as a POSIX rc file, plus one whose body starts on a new line, the shape of oh-my-zsh's deprecation wrappers.
fn sh_rc() -> String {
    let lines: Vec<String> = ALIASES.iter().map(|(k, v)| format!("alias {k}='{}'", v.replace('\'', "'\\''"))).collect();
    format!("{}\nalias cb=$'\\n  echo deprecated >&2\\n  git_current_branch'\n", lines.join("\n"))
}

fn alias_lines(stdout: &str) -> Vec<Alias> {
    parse_probe(&format!("WSP_CTX\n{stdout}WSP_CTX_END\n")).expect("a probe").aliases
}

async fn run_in(shell: &Path, args: &[&str], env: &[(&str, &Path)]) -> String {
    let mut cmd = tokio::process::Command::new(shell);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().await.expect("the shell runs");
    assert!(out.status.success(), "{} {args:?}: {}", shell.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// machine-context.test.ts:593
#[tokio::test]
async fn the_zsh_alias_probe_names_only_aliases_whose_first_word_is_a_missing_command_on_this_zsh() {
    let zsh = shell_of("zsh").expect("zsh is not installed on this box, so the zsh alias probe has no shell to run under");
    let g = Guest::new();
    write(g.home().join(".zshrc"), &sh_rc());
    write(g.home().join(".zshenv"), "");
    let out = run_in(&zsh, &["-lic", ALIAS_PROBE_ZSH], &[("ZDOTDIR", g.home()), ("HOME", g.home())]).await;
    assert_eq!(alias_lines(&out), missing());
}

/// machine-context.test.ts:601
#[tokio::test]
async fn the_bash_alias_probe_does_the_same_on_this_bash_from_a_fake_homes_profile() {
    let bash = shell_of("bash").expect("bash is not installed on this box, so the bash alias probe has no shell to run under");
    let g = Guest::new();
    write(g.home().join(".bash_profile"), &sh_rc());
    let out = run_in(&bash, &["-lic", ALIAS_PROBE_BASH], &[("HOME", g.home()), ("ZDOTDIR", g.home())]).await;
    assert_eq!(alias_lines(&out), missing());
}

/// What fish prints for the same aliases, each body escaped as its own `alias` listing escapes it: quoted when it holds
/// a space, backslashed when it holds a backslash. A fixture, so the probe is read against the rule and not against
/// whichever fish and whichever config this machine happens to carry.
const FISH_LISTING: &str = "alias g git
alias ll 'ls -la'
alias o open-not-here
alias s 'sudo not-here-either'
alias e 'FOO=1 not-here-third'
alias q \\\\not-here-fourth\\ --flag
alias abs /Applications/Nowhere.app/Contents/MacOS/nowhere
alias grp '{ grep -rn . ; }'";

/// machine-context.test.ts:622
#[tokio::test]
async fn the_fish_alias_probe_names_only_aliases_whose_first_word_is_a_missing_command_over_a_fixed_listing() {
    let fish = shell_of("fish").expect("fish is not installed on this box, so the fish alias probe has no shell to run under");
    let g = Guest::new();
    let bin = g.home().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for cmd in ["git", "ls"] {
        write_stub(&bin.join(cmd), "#!/bin/sh\nexit 0\n");
    }
    // The listing arrives as a variable and PATH holds only the two commands the fixture calls present, so neither
    // the answer nor fish's startup reads anything of this machine's.
    let script = format!("function alias; string split \\n -- $WSP_FISH_ALIASES; end\n{ALIAS_PROBE_FISH}");
    let out = tokio::process::Command::new(&fish)
        .args(["--no-config", "-c", &script])
        .env_clear()
        .env("PATH", &bin)
        .env("HOME", g.home())
        .env("WSP_FISH_ALIASES", FISH_LISTING)
        .output()
        .await
        .expect("fish runs");
    assert_eq!(alias_lines(&String::from_utf8_lossy(&out.stdout)), missing(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// machine-context.test.ts:634
#[tokio::test]
async fn a_bounded_command_is_killed_at_its_bound_fails_there_so_the_script_can_say_so_and_the_script_goes_on() {
    let g = Guest::new();
    let started = Instant::now();
    let res = run(&format!("{} || echo late\necho after", bounded_command(1, "sleep 20")), g.home()).await;
    assert_eq!(res.stdout, "late\nafter\n");
    assert!(started.elapsed() < Duration::from_millis(6_000), "{:?}", started.elapsed());
    assert_eq!(run(&format!("{} || echo late\necho after", bounded_command(5, "echo quick")), g.home()).await.stdout, "quick\nafter\n");
}

/// A login file of either family that leaves a job in the background holding the shell's output open, the way a
/// completion script run with & does, then does `rest`; every job writes its pid to the file returned.
fn holding_output(g: &mut Guest, rest: &str) -> PathBuf {
    let pids = g.home().join("pids");
    g.pid_files.push(pids.clone());
    for rc in [".zshrc", ".bash_profile"] {
        write(g.home().join(rc), &format!("sleep 300 & echo $! >> {}\n{rest}", pids.display()));
    }
    pids
}

/// machine-context.test.ts:652
#[tokio::test]
async fn a_login_shell_whose_rc_file_leaves_a_job_holding_the_output_open_ends_the_probe_once_it_has_answered_and_the_job_ends_with_it() {
    let mut g = Guest::new();
    let pids = holding_output(&mut g, "");
    let started = Instant::now();
    let probe = parse_probe(&run(&probe_command(Some(&g.roots), None, None), g.home()).await.stdout).expect("a probe");
    assert!(started.elapsed() < Duration::from_millis(8_000), "{:?}", started.elapsed());
    assert_eq!(probe.aliases_unread, None);
    assert!(!pids_in(&pids).is_empty());
    wait_for("every job the rc file started to end", || pids_in(&pids).into_iter().filter(|p| alive(*p)).next().is_none()).await;
}

/// machine-context.test.ts:663
#[tokio::test]
async fn a_login_shell_that_has_not_answered_by_its_bound_is_ended_with_every_job_and_the_probe_says_the_aliases_went_unread() {
    let mut g = Guest::new();
    let rest = format!("sleep 299 & echo $! >> {}; wait $!\n", g.home().join("pids").display());
    let pids = holding_output(&mut g, &rest);
    let probe = parse_probe(&run(&probe_command(Some(&g.roots), None, None), g.home()).await.stdout).expect("a probe");
    assert_eq!(probe.aliases_unread, Some(AliasesUnread::Late));
    let doc = render_machine_context(&ContextInput {
        workspace: Some(WorkspaceName { name: "box-1".into() }),
        golden: None,
        probe,
        facts: facts(),
        agent: None,
        at_provider: false,
    });
    assert!(doc.contains("- Aliases: not read. The login shell had not finished starting after 8 seconds, so wsp stopped it."), "{doc}");
    assert_eq!(pids_in(&pids).len(), 2);
    wait_for("every job the rc file started to end", || pids_in(&pids).into_iter().filter(|p| alive(*p)).next().is_none()).await;
}
