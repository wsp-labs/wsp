// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/machine-facts.test.ts: the one reader of what a machine says it is, over fixtures
//! taken off real machines: an Ubuntu box, a Mac, and one that answers with neither.
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::ExecResult;
use wsp_engine::machine_facts::{mem_mb_of, os_name_of, read_os_name, read_values, uptime_ms_of, MEM_READ, OS_READ, UPTIME_READ};

/// What the lines print on an Ubuntu box: no product version, a distribution name, and the seconds /proc keeps.
/// machine-facts.test.ts:9
const UBUNTU: &str = "pretty Ubuntu 24.04.3 LTS\nmac \nkernel Linux 6.8.0-79-generic\nuptime 96521.42\nboot \nhome /home/dev\n";
/// What they print on a Mac: no /etc/os-release, no /proc, and a boot time instead of a length.
/// machine-facts.test.ts:11
const MAC: &str = "pretty \nmac 26.4\nkernel Darwin 25.4.0\nuptime \nboot { sec = 1789041249, usec = 855807 } Thu Sep 10 17:24:09 2026\nhome /Users/zingzy\n";

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

/// machine-facts.test.ts:49 `exec`: answers `out` and records the command it was asked.
fn exec(asked: &Arc<Mutex<Vec<String>>>, out: ExecResult) -> impl FnOnce(String) -> BoxFuture<'static, Result<ExecResult>> {
    let asked = asked.clone();
    move |cmd: String| {
        asked.lock().unwrap().push(cmd);
        async move { Ok(out) }.boxed()
    }
}

fn answer(exit_code: i64, stdout: &str) -> ExecResult {
    ExecResult { exit_code, stdout: stdout.to_string(), stderr: String::new() }
}

/// machine-facts.test.ts:14
#[test]
fn reads_one_answer_per_line_and_a_line_that_printed_nothing_is_an_answer_the_machine_does_not_have() {
    assert_eq!(
        read_values(UBUNTU),
        map(&[
            ("pretty", "Ubuntu 24.04.3 LTS"),
            ("mac", ""),
            ("kernel", "Linux 6.8.0-79-generic"),
            ("uptime", "96521.42"),
            ("boot", ""),
            ("home", "/home/dev")
        ])
    );
    assert_eq!(read_values(""), map(&[]));
    assert_eq!(read_values("kernel Linux 6.8.0\n"), map(&[("kernel", "Linux 6.8.0")]));
}

/// machine-facts.test.ts:20
#[test]
fn names_the_system_the_way_its_maker_does() {
    assert_eq!(os_name_of(&read_values(MAC)), Some("macOS 26.4".to_string()));
    assert_eq!(os_name_of(&read_values(UBUNTU)), Some("Ubuntu 24.04.3 LTS".to_string()));
    assert_eq!(os_name_of(&read_values("pretty \nmac \nkernel FreeBSD 14.1-RELEASE\n")), Some("FreeBSD 14.1-RELEASE".to_string()));
    // A machine that ran none of the lines has no name here: the caller says what it knows instead of guessing.
    assert_eq!(os_name_of(&read_values("pretty \nmac \nkernel \n")), None);
    assert_eq!(os_name_of(&map(&[])), None);
}

/// machine-facts.test.ts:29
#[test]
fn takes_the_length_proc_keeps_and_works_one_out_against_the_clock_from_a_boot_time() {
    assert_eq!(uptime_ms_of(&read_values(UBUNTU), now_ms()), Some(96_521_420));
    assert_eq!(uptime_ms_of(&read_values(MAC), 1_789_041_249_000 + 3_600_000), Some(3_600_000));
    // A clock behind the machine's own boot time reads as just up rather than as a negative length.
    assert_eq!(uptime_ms_of(&read_values(MAC), 1_789_041_249_000 - 5_000), Some(0));
    assert_eq!(uptime_ms_of(&read_values("uptime \nboot \n"), now_ms()), None);
    assert_eq!(uptime_ms_of(&map(&[]), now_ms()), None);
}

/// machine-facts.test.ts:38
#[test]
fn reads_the_memory_line_in_whole_mb_and_nothing_where_the_line_printed_no_figure() {
    assert_eq!(mem_mb_of(&read_values("memkb 4128768\n")), Some(4032));
    assert_eq!(mem_mb_of(&read_values("memkb 8060000\n")), Some(7871));
    assert_eq!(mem_mb_of(&read_values("memkb \n")), None);
    assert_eq!(mem_mb_of(&read_values("memkb 0\n")), None);
    assert_eq!(mem_mb_of(&map(&[])), None);
    assert!(MEM_READ.starts_with(r#"printf "memkb "#));
}

/// machine-facts.test.ts:47
#[tokio::test]
async fn one_command_asks_for_the_name_and_a_machine_that_could_not_run_it_leaves_the_caller_without_one() {
    let asked = Arc::new(Mutex::new(Vec::<String>::new()));
    assert_eq!(read_os_name(exec(&asked, answer(0, UBUNTU))).await, Some("Ubuntu 24.04.3 LTS".to_string()));
    assert_eq!(*asked.lock().unwrap(), vec![OS_READ.join("\n")]);
    assert_eq!(read_os_name(exec(&asked, answer(127, UBUNTU))).await, None);
    assert_eq!(read_os_name(|_cmd: String| async { Err(EngineError::Other { message: "spawn failed".into(), code: None }) }).await, None);
    // Nothing in the lines reads this process: they are what a machine anywhere is asked, over whatever road reaches it.
    for line in OS_READ.iter().chain(UPTIME_READ.iter()) {
        assert!(line.starts_with("printf "), "{line}");
    }
}
