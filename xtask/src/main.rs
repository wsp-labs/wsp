// SPDX-License-Identifier: AGPL-3.0-only
//! `cargo xtask parity [record | collect]`, from the repository root or from daemon/.

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
        #[command(flatten)]
        run: Run,
    },
    /// Run the suites that drive the host from outside with the tap on, and keep each new sequence of frames as a tape.
    Collect,
    /// List the tapes held back from implementers, which only the reviewer and this command run.
    Held,
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
    /// Run the tapes on this computer as it is, with no sandbox: a tape's command line can be `wsp leave`, which
    /// takes what wsp installed off the computer it runs on.
    #[arg(long)]
    no_sandbox: bool,
}

/// Set on the copy of this command that runs inside the sandbox.
const SANDBOX_ENV: &str = "WSP_PARITY_SANDBOX";
/// The folder the copy inside the sandbox may write, which is where the goldens go.
const WRITABLE_ENV: &str = "WSP_PARITY_WRITABLE";

/// What the copy inside the namespaces does before it runs: loopback up; every top-level folder but the kernel's, the
/// devices and /tmp read-only, so nothing a replayed command does reaches what is installed on this computer; an empty
/// /run, so no service manager or bus socket answers it; and the parity folder writable again for the goldens.
const SANDBOX_SH: &str = r#"set -e
PATH="$PATH:/usr/sbin:/sbin"
ip link set lo up
mount --make-rprivate /
for d in /*; do
  case "$d" in /proc|/sys|/dev|/tmp|/run) continue ;; esac
  if [ -d "$d" ] && [ ! -L "$d" ]; then mount --bind -o ro "$d" "$d"; fi
done
mount -t tmpfs -o mode=755 tmpfs /run
mount --bind "$WSP_PARITY_WRITABLE" "$WSP_PARITY_WRITABLE"
exec "$0" "$@""#;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = repo_root();
    let outcome = match cli.task {
        Task::Parity { step: None, run } => parity(&root, &run),
        Task::Parity { step: Some(Step::Record { run }), .. } => record(&root, &run),
        Task::Parity { step: Some(Step::Collect), .. } => collect(&root),
        Task::Parity { step: Some(Step::Held), .. } => held(&root),
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

fn options(run: &Run) -> Options {
    let seed = run.seed.map(|s| if s == 0 { fresh_seed() } else { s });
    Options { jobs: run.jobs, seed, only: run.only.clone(), ids: None }
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

/// Every tape runs in a copy of this command inside its own network, mount and process namespaces (Linux's unshare),
/// since a tape is what a test or a person once sent a host and its commands act on the computer they run on: a
/// replayed `wsp leave` as root took /usr/local/bin/claude and /opt/wsp/landed off a box on 2026-10-25. A frame that
/// would reach a provider, a box or the relay is refused the same way every time, no process outside the run can be
/// signalled, and everything the run started ends with it. Where there are no namespaces the run is refused unless
/// --no-sandbox says to run the tapes here as they are.
fn sandboxed(root: &Path, run: &Run) -> Result<Option<ExitCode>, String> {
    if std::env::var_os(SANDBOX_ENV).is_some() {
        return Ok(None);
    }
    if run.no_sandbox {
        eprintln!("parity: no sandbox; every tape's commands run on this computer as this user");
        return Ok(None);
    }
    let refused = "no sandbox to run the tapes in here (Linux's unshare with network, mount and process namespaces); a tape's command line can be wsp leave, which takes what wsp installed off this computer. Run it on Linux, or pass --no-sandbox to run the tapes here as they are";
    if !cfg!(target_os = "linux") {
        return Err(refused.into());
    }
    // SAFETY: geteuid reads the process's own id and cannot fail.
    let root_user = unsafe { libc::geteuid() } == 0;
    let mut flags = vec!["--net", "--mount", "--pid", "--fork", "--mount-proc"];
    if !root_user {
        flags.push("--map-root-user");
    }
    if !Command::new("unshare").args(&flags).arg("true").status().is_ok_and(|s| s.success()) {
        return Err(refused.into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = Command::new("unshare")
        .args(&flags)
        .args(["--", "/bin/sh", "-c", SANDBOX_SH])
        .arg(exe)
        .args(std::env::args_os().skip(1))
        .env(SANDBOX_ENV, "1")
        .env(WRITABLE_ENV, root.join("parity"))
        .status()
        .map_err(|e| format!("unshare: {e}"))?;
    Ok(Some(ExitCode::from(status.code().unwrap_or(2) as u8)))
}

fn parity(root: &Path, run: &Run) -> Result<ExitCode, String> {
    if let Some(code) = sandboxed(root, run)? {
        return Ok(code);
    }
    let host = host(root, run)?;
    let started = std::time::Instant::now();
    let report = check::check(root, &host, &options(run))?;
    print!("{}", report.render());
    println!("time: {:.1} s", started.elapsed().as_secs_f64());
    Ok(if report.ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn record(root: &Path, run: &Run) -> Result<ExitCode, String> {
    if let Some(code) = sandboxed(root, run)? {
        return Ok(code);
    }
    if run.host.is_some() {
        return Err("goldens are the TS host's answers; record takes no --host".into());
    }
    let host = host(root, run)?;
    let started = std::time::Instant::now();
    let ops = ts_ops(root)?;
    let done = check::record(root, &host, &options(run), &ops)?;
    for (tape, e) in &done.failed {
        println!("FAIL {tape}: {e}");
    }
    println!(
        "recorded {} goldens, took out {}, TS tree {} ({} host ops), {:.1} s",
        done.written,
        done.removed,
        done.key,
        ops.len(),
        started.elapsed().as_secs_f64()
    );
    Ok(if done.failed.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
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

fn collect(root: &Path) -> Result<ExitCode, String> {
    let status = Command::new(node()?).arg(root.join("parity/record/collect.mjs")).current_dir(root).status().map_err(|e| e.to_string())?;
    Ok(if status.success() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn held(root: &Path) -> Result<ExitCode, String> {
    let tapes = xtask::tape::load(root)?;
    let held = xtask::held::held(&tapes, xtask::held::seed(root)?);
    for id in &held {
        println!("{id}");
    }
    eprintln!("{} of {} tapes held back", held.len(), tapes.len());
    Ok(ExitCode::SUCCESS)
}

fn unit(root: &Path, name: &str, base: Option<String>, run: &Run) -> Result<ExitCode, String> {
    let units = xtask::unit::read_units(root)?;
    let unit = units.iter().find(|u| u.name == name).ok_or_else(|| format!("{} names no unit {name}", xtask::unit::UNITS_FILE))?;
    // The copy inside the network namespace replays the tapes alone: the refusals and the tests ran before it started.
    if std::env::var_os(SANDBOX_ENV).is_none() {
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
    }
    let tapes = xtask::tape::load(root)?;
    let held = xtask::held::held(&tapes, xtask::held::seed(root)?);
    let ids = xtask::unit::visible_tapes(unit, &tapes, &held);
    if ids.is_empty() {
        println!("no tape sends an op of {name}");
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(code) = sandboxed(root, run)? {
        return Ok(code);
    }
    let host = host(root, run)?;
    let report = check::check(root, &host, &Options { ids: Some(ids), ..options(run) })?;
    print!("{}", report.render());
    Ok(if report.failed() == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
