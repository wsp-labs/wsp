// SPDX-License-Identifier: AGPL-3.0-only
//! `cargo xtask parity [record | collect | held]` and `cargo xtask unit <unit>`, from the repository root or from daemon/.

use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use xtask::check::{self, Options};
use xtask::host::ProcessHost;
use xtask::seed::fresh_seed;

#[derive(Parser)]
#[command(name = "cargo xtask", about = "the repository's own tasks")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Hold one unit of the port to its packet, then run its tests and the tapes it may see.
    Unit {
        /// The unit, as parity/units.tsv names it.
        name: String,
        /// What the unit's work is measured from; the merge base with origin/main when left out.
        #[arg(long)]
        base: Option<String>,
        #[command(flatten)]
        run: Run,
    },
    /// Replay every tape against a host and diff its answers with the TS host's goldens.
    Parity {
        #[command(subcommand)]
        step: Option<Step>,
        #[command(flatten)]
        run: Run,
    },
}

#[derive(Subcommand)]
enum Step {
    /// Replay every tape against the TS host and write its answers as the goldens, keyed to the TS tree's hash.
    Record {
        /// Rewrite the goldens of tapes recorded on another computer whose answers read it, and mark them as recorded
        /// on this one.
        #[arg(long)]
        move_here: bool,
        #[command(flatten)]
        run: Run,
    },
    /// Run the suites that drive the host from outside with the tap on, and keep each new sequence of frames as a tape.
    Collect {
        /// Run the suites on this computer as it is, with no sandbox.
        #[arg(long)]
        no_sandbox: bool,
    },
    /// The tapes held back from implementers, which live in a folder outside the repository.
    Held {
        /// The held-back folder; with none, nothing is held back in this checkout.
        #[arg(long)]
        held: Option<PathBuf>,
        /// Move the tapes the folder's seed draws, and their goldens, from the repository into the folder.
        #[arg(long)]
        draw: bool,
    },
}

#[derive(Args, Clone)]
struct Run {
    /// The host to replay against, as the command line that runs wsp; the TS host's built bin.js when left out.
    #[arg(long)]
    host: Option<String>,
    /// How many tapes replay at once, each against a host of its own.
    #[arg(long, default_value_t = 3)]
    jobs: usize,
    /// Send frames in windows drawn from this seed rather than one reply at a time; 0 draws a seed and prints it.
    #[arg(long)]
    seed: Option<u64>,
    /// Only the tapes whose id holds this text.
    #[arg(long)]
    only: Option<String>,
    /// The held-back folder, whose tapes run beside the repository's; the reviewer's, never an implementer's.
    #[arg(long)]
    held: Option<PathBuf>,
    /// Run the tapes on this computer as it is, with no sandbox: a tape's command line can be `wsp leave`, which
    /// takes what wsp installed off the computer it runs on.
    #[arg(long)]
    no_sandbox: bool,
}

/// Set on the copy of this command that runs inside the sandbox, which trusts it only as the first process of its own
/// process namespace: a caller that sets it gets the sandbox all the same.
const SANDBOX_ENV: &str = "WSP_PARITY_SANDBOX";
/// The folders the copy inside the sandbox may write, split by colons: the goldens for record, none for a check.
const WRITABLE_ENV: &str = "WSP_PARITY_WRITABLE";
/// The checkout, which the sandbox leaves in reach even when it sits inside a folder the live host keeps.
const ROOT_ENV: &str = "WSP_PARITY_ROOT";
/// The caller's folder, entered again inside once the mounts are made, so a path given relative to it still holds.
const CWD_ENV: &str = "WSP_PARITY_CWD";
/// What else the run reads in a folder the sandbox covers, split by colons: git's own folder, the held-back folder, the
/// host it replays against.
const KEEP_ENV: &str = "WSP_PARITY_KEEP";

/// Whether this is the copy inside the sandbox.
fn inside() -> bool {
    // SAFETY: getpid reads the process's own id and cannot fail.
    std::env::var_os(SANDBOX_ENV).is_some() && unsafe { libc::getpid() } == 1
}

/// The copy inside the sandbox is the first process of its PID namespace, which the kernel hands no signal it has no
/// handler for: a Ctrl-C would leave it running. Ending it ends the namespace and every host in it.
extern "C" fn end_the_run(_: libc::c_int) {
    // SAFETY: _exit is async-signal-safe.
    unsafe { libc::_exit(130) }
}

fn main() -> ExitCode {
    if inside() {
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            // SAFETY: the handler only calls _exit.
            unsafe { libc::signal(signal, end_the_run as extern "C" fn(libc::c_int) as libc::sighandler_t) };
        }
    }
    let cli = Cli::parse();
    let root = repo_root();
    let outcome = match cli.task {
        Task::Parity { step: None, run } => parity(&root, &run),
        Task::Parity { step: Some(Step::Record { move_here, run }), .. } => record(&root, &run, move_here),
        Task::Parity { step: Some(Step::Collect { no_sandbox }), .. } => collect(&root, no_sandbox),
        Task::Parity { step: Some(Step::Held { held: dir, draw }), .. } => held(&root, dir, draw),
        Task::Unit { name, base, run } => unit(&root, &name, base, &run),
    };
    match outcome {
        Ok(code) => code,
        Err(e) => {
            eprintln!("parity: {e}");
            ExitCode::from(2)
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize().expect("the repository holds the xtask crate")
}

fn options(run: &Run) -> Result<Options, String> {
    let seed = run.seed.map(|s| if s == 0 { fresh_seed() } else { s });
    let held = run.held.as_ref().map(|d| d.canonicalize().map_err(|e| format!("--held {}: {e}", d.display()))).transpose()?;
    Ok(Options { jobs: run.jobs, seed, only: run.only.clone(), ids: None, held, move_here: false })
}

fn host(root: &Path, run: &Run) -> Result<ProcessHost, String> {
    let program: Vec<String> = match &run.host {
        Some(line) => line.split_whitespace().map(str::to_string).collect(),
        None => {
            let bin = root.join("packages/host/dist/bin.js");
            if !bin.exists() {
                return Err(format!("{} is missing: pnpm build first", bin.display()));
            }
            vec![node()?, bin.display().to_string()]
        }
    };
    if program.is_empty() {
        return Err("--host names no program".into());
    }
    let mut host = ProcessHost::new(program);
    host.known.add(root.display().to_string(), "<repo>");
    Ok(host)
}

fn node() -> Result<String, String> {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .map(|d| Path::new(d).join("node"))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
        .ok_or_else(|| "node is not on the PATH".into())
}

/// Every tape runs in a copy of this command inside its own network, mount, process, host name and IPC namespaces
/// (Linux's unshare), since a tape is what a test or a person once sent a host and its commands act on the computer they
/// run on: a replayed `wsp leave` as root took /usr/local/bin/claude and /opt/wsp/landed off a box on 2026-10-25. Inside,
/// the whole disk is read-only but `writable`, no socket of the computer answers, a frame that would reach a provider, a
/// box or the relay is refused the same way every time, no process outside the run can be signalled, and everything the
/// run started ends with it. Where there are no namespaces the run is refused unless --no-sandbox says to run here as is.
fn sandboxed(root: &Path, no_sandbox: bool, writable: &[PathBuf], keep: &[PathBuf]) -> Result<Option<ExitCode>, String> {
    if inside() {
        return Ok(None);
    }
    if no_sandbox {
        unsandboxed();
        return Ok(None);
    }
    let refused = "no sandbox to run the tapes in here (Linux's unshare with network, mount and process namespaces); a tape's command line can be wsp leave, which takes what wsp installed off this computer. Run it on Linux, or pass --no-sandbox to run the tapes here as they are";
    if !cfg!(target_os = "linux") {
        return Err(refused.into());
    }
    // SAFETY: geteuid reads the process's own id and cannot fail.
    let root_user = unsafe { libc::geteuid() } == 0;
    let flags = xtask::sandbox::flags(root_user);
    if !Command::new("unshare").args(&flags).arg("true").status().is_ok_and(|s| s.success()) {
        return Err(refused.into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let writable: Vec<String> = writable.iter().map(|p| p.display().to_string()).collect();
    let mut cmd = Command::new("unshare");
    cmd.args(&flags).args(["--", "/bin/sh", "-c", xtask::sandbox::SCRIPT]).arg(exe).args(std::env::args_os().skip(1));
    cmd.env(SANDBOX_ENV, "1").env(ROOT_ENV, root).env(CWD_ENV, std::env::current_dir().map_err(|e| e.to_string())?);
    let git = Command::new("git").args(["rev-parse", "--path-format=absolute", "--git-common-dir"]).current_dir(root).output();
    let git = git.ok().filter(|o| o.status.success()).map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    let keep: Vec<String> = keep.iter().chain(&git).map(|p| p.display().to_string()).collect();
    cmd.env(KEEP_ENV, keep.join(":"));
    if writable.is_empty() {
        cmd.env_remove(WRITABLE_ENV);
    } else {
        cmd.env(WRITABLE_ENV, writable.join(":"));
    }
    let status = cmd.status().map_err(|e| format!("unshare: {e}"))?;
    Ok(Some(ExitCode::from(status.code().unwrap_or(2) as u8)))
}

/// The held-back folder and each path the --host line names, which the run reads wherever they sit.
fn kept(run: &Run, opts: &Options) -> Vec<PathBuf> {
    let host = run.host.iter().flat_map(|h| h.split_whitespace()).map(PathBuf::from).filter(|p| p.is_absolute());
    opts.held.iter().cloned().chain(host).collect()
}

/// Said before a run with no sandbox and again after it, so it cannot scroll away.
fn unsandboxed() {
    eprintln!("parity: NO SANDBOX. Every tape's commands run on this computer as this user, wsp leave among them, which takes what wsp installed off it.");
}

fn parity(root: &Path, run: &Run) -> Result<ExitCode, String> {
    let opts = options(run)?;
    if let Some(code) = sandboxed(root, run.no_sandbox, &[], &kept(run, &opts))? {
        return Ok(code);
    }
    let host = host(root, run)?;
    let started = std::time::Instant::now();
    let report = check::check(root, &host, &opts)?;
    print!("{}", report.render());
    println!("time: {:.1} s", started.elapsed().as_secs_f64());
    if run.no_sandbox {
        unsandboxed();
    }
    Ok(if report.ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn record(root: &Path, run: &Run, move_here: bool) -> Result<ExitCode, String> {
    if run.host.is_some() {
        return Err("goldens are the TS host's answers; record takes no --host".into());
    }
    let opts = Options { move_here, ..options(run)? };
    let mut writable = vec![root.join("parity")];
    writable.extend(opts.held.clone());
    if let Some(code) = sandboxed(root, run.no_sandbox, &writable, &kept(run, &opts))? {
        return Ok(code);
    }
    let host = host(root, run)?;
    let started = std::time::Instant::now();
    let ops = ts_ops(root)?;
    let done = check::record(root, &host, &opts, &ops)?;
    xtask::golden::write_tools(root, &ts_tools(&host)?)?;
    for (tape, e) in &done.failed {
        println!("FAIL {tape}: {e}");
    }
    let mut elsewhere: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for (_, computer) in &done.kept {
        *elsewhere.entry(computer.as_str()).or_default() += 1;
    }
    for (computer, n) in elsewhere {
        println!("kept {n} goldens recorded on {computer}, whose answers read that computer; --move-here records them here");
    }
    println!("{} tapes read the computer they ran on, marked as recorded on {}", done.marked.len(), xtask::computer::name());
    if let Some(old) = &done.key_kept {
        println!("KEY stays at {old}: the goldens this run did not record are from that tree; record them all to move it");
    }
    println!(
        "recorded {} goldens, took out {}, TS tree {} ({} host ops), {:.1} s",
        done.written,
        done.removed,
        done.key,
        ops.len(),
        started.elapsed().as_secs_f64()
    );
    if run.no_sandbox {
        unsandboxed();
    }
    Ok(if done.failed.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

/// The tool server's tools, as the host's own `tools/list` names them.
fn ts_tools(host: &ProcessHost) -> Result<Vec<String>, String> {
    let list = r#"{"meta":{"door":"mcp"}}
{"in":{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"parity","version":"1"}}}}
{"in":{"jsonrpc":"2.0","method":"notifications/initialized"}}
{"in":{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#;
    let tape = xtask::tape::Tape::parse("tools", list, "the tool list", 1)?;
    let replay = xtask::host::Host::replay(host, &tape, &mut xtask::seed::Order::for_tape(None, "tools"))?;
    let tools = replay.replies.get(1).and_then(|r| r.pointer("/result/tools")).and_then(serde_json::Value::as_array);
    let names: Vec<String> = tools.into_iter().flatten().filter_map(|t| t["name"].as_str().map(str::to_string)).collect();
    if names.is_empty() {
        return Err("the tool server listed no tools".into());
    }
    Ok(names)
}

/// The host's ops as the TS protocol package lists them.
fn ts_ops(root: &Path) -> Result<Vec<String>, String> {
    let dist = root.join("packages/protocol/dist/index.js");
    let script = format!("import({:?}).then(p => console.log(JSON.stringify(p.RUNTIME_OPS)))", format!("file://{}", dist.display()));
    let out = Command::new(node()?).args(["-e", &script]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("reading the TS host's ops: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("reading the TS host's ops: {e}"))
}

fn collect(root: &Path, no_sandbox: bool) -> Result<ExitCode, String> {
    // The suites write inside their checkout (vite bundles its config beside it); outside it all stays read-only.
    if let Some(code) = sandboxed(root, no_sandbox, &[root.to_path_buf()], &[])? {
        return Ok(code);
    }
    let status = Command::new(node()?).arg(root.join("parity/record/collect.mjs")).current_dir(root).status().map_err(|e| e.to_string())?;
    if no_sandbox {
        unsandboxed();
    }
    Ok(if status.success() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn held(root: &Path, dir: Option<PathBuf>, draw: bool) -> Result<ExitCode, String> {
    let Some(dir) = dir else {
        println!("nothing is held back in this checkout: the held-back tapes live outside the repository, and the reviewer runs them with --held <dir>");
        return Ok(ExitCode::SUCCESS);
    };
    let mut tapes = xtask::tape::load(root)?;
    let held = if dir.join("tapes").is_dir() { xtask::tape::load_held(&dir)? } else { Vec::new() };
    tapes.extend(held);
    if !draw {
        let held = xtask::held::held(&tapes);
        for id in &held {
            println!("{id}");
        }
        eprintln!("{} of {} tapes held back in {}", held.len(), tapes.len(), dir.display());
        return Ok(ExitCode::SUCCESS);
    }
    let drawn = xtask::held::draw(&tapes, xtask::held::seed(&dir)?, &xtask::golden::read_tools(root)?);
    let mut moved = 0;
    for tape in tapes.iter().filter(|t| !t.held && drawn.contains(&t.id)) {
        let from = root.join(xtask::tape::TAPES_DIR).join(format!("{}.tape", tape.id));
        if !from.is_file() {
            continue;
        }
        let golden = xtask::golden::path_of(&xtask::golden::dir(root), &tape.id);
        for (src, dst) in
            [(from, dir.join("tapes").join(format!("{}.tape", tape.id))), (golden, xtask::golden::path_of(&dir.join("goldens"), &tape.id))]
        {
            if !src.is_file() {
                continue;
            }
            std::fs::create_dir_all(dst.parent().expect("a folder")).map_err(|e| e.to_string())?;
            std::fs::rename(&src, &dst)
                .or_else(|_| std::fs::copy(&src, &dst).and_then(|_| std::fs::remove_file(&src)))
                .map_err(|e| format!("{}: {e}", src.display()))?;
        }
        moved += 1;
    }
    if let Some(key) = xtask::golden::read_key(&xtask::golden::dir(root)) {
        if xtask::golden::read_key(&dir.join("goldens")).is_none() {
            xtask::golden::write_key(&dir.join("goldens"), &key)?;
        }
    }
    println!("moved {moved} tapes and their goldens into {}; {} held back there in all", dir.display(), drawn.len());
    Ok(ExitCode::SUCCESS)
}

fn unit(root: &Path, name: &str, base: Option<String>, run: &Run) -> Result<ExitCode, String> {
    let units = xtask::unit::read_units(root)?;
    let unit = units.iter().find(|u| u.name == name).ok_or_else(|| format!("{} names no unit {name}", xtask::unit::UNITS_FILE))?;
    if run.held.is_some() {
        return Err("a unit never runs the held-back tapes; the reviewer runs them with cargo xtask parity --held".into());
    }
    // The unit's own tests run inside the sandbox too: a port of place.leave brings tests that sweep real paths.
    let writable = if inside() { Vec::new() } else { vec![xtask::unit::target_dir(root, unit)?] };
    if let Some(code) = sandboxed(root, run.no_sandbox, &writable, &kept(run, &options(run)?))? {
        return Ok(code);
    }
    let base = match base {
        Some(b) => b,
        None => xtask::unit::merge_base(root)?,
    };
    let refused = xtask::unit::refusals(root, unit, &xtask::unit::changed(root, &base)?);
    for line in &refused {
        println!("REFUSED {line}");
    }
    if !refused.is_empty() {
        return Ok(ExitCode::FAILURE);
    }
    if !xtask::unit::run_tests(root, unit)? {
        println!("FAIL the unit's tests");
        return Ok(ExitCode::FAILURE);
    }
    let tapes = xtask::tape::load(root)?;
    let ids = xtask::unit::visible_tapes(unit, &tapes, &xtask::held::held(&tapes));
    if ids.is_empty() {
        println!("no tape sends an op of {name}");
        return Ok(ExitCode::SUCCESS);
    }
    let host = host(root, run)?;
    let report = check::check(root, &host, &Options { ids: Some(ids), ..options(run)? })?;
    print!("{}", report.render());
    if run.no_sandbox {
        unsandboxed();
    }
    Ok(if report.failed() == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
