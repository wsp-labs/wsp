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
    /// Let the host reach the network; without it a replay on Linux runs in a network namespace of its own.
    #[arg(long)]
    online: bool,
}

/// Set on the copy of this command that runs inside its own network namespace.
const NETNS_ENV: &str = "WSP_PARITY_NETNS";

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

/// On Linux the run goes on in a copy of this command inside a network namespace whose only interface is loopback, so
/// no tape's frame reaches a provider, a box or the relay, and every answer that would have is the same refusal.
fn offline(run: &Run) -> Option<ExitCode> {
    if run.online || !cfg!(target_os = "linux") || std::env::var_os(NETNS_ENV).is_some() {
        return None;
    }
    let ns = |args: &[&str]| {
        Command::new("unshare")
            .args(args)
            .arg("true")
            .status()
            .is_ok_and(|s| s.success())
            .then(|| args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    };
    let Some(flags) = ns(&["--net"]).or_else(|| ns(&["--net", "--map-root-user"])) else {
        eprintln!("parity: no network namespace here (unshare refused); the hosts run online");
        return None;
    };
    let exe = std::env::current_exe().ok()?;
    let mut cmd = Command::new("unshare");
    cmd.args(&flags)
        .args(["--", "/bin/sh", "-c", r#"PATH="$PATH:/usr/sbin:/sbin" ip link set lo up && exec "$0" "$@""#])
        .arg(exe)
        .args(std::env::args_os().skip(1))
        .env(NETNS_ENV, "1");
    let status = cmd.status().ok()?;
    Some(ExitCode::from(status.code().unwrap_or(2) as u8))
}

fn parity(root: &Path, run: &Run) -> Result<ExitCode, String> {
    if let Some(code) = offline(run) {
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
    if let Some(code) = offline(run) {
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
    if std::env::var_os(NETNS_ENV).is_none() {
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
    if let Some(code) = offline(run) {
        return Ok(code);
    }
    let host = host(root, run)?;
    let report = check::check(root, &host, &Options { ids: Some(ids), ..options(run) })?;
    print!("{}", report.render());
    Ok(if report.failed() == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
