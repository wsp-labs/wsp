// SPDX-License-Identifier: AGPL-3.0-only
//! machine_exec_kept: a process kept between a thread's turns on a joined computer's road, from
//! packages/runtime/test/machine-exec-kept.test.ts. Spec: spec/machine_exec_stream.md.
// This computer's bash stands in for that computer, and an agent that answers each line of its input serves one turn
// after another from one process.
mod support;

use std::sync::Arc;

use futures::future::BoxFuture;
use futures::stream::BoxStream;
use futures::{FutureExt, StreamExt};
use wsp_adapters::port::{keep_run, AttachRunOptions, Attached, ExecFailure, ExecOptions as Launch, ExecStream, ExecStreamFactory, WriteOutcome};
use wsp_core::machine_exec::{machine_exec_stream, MachineExecOptions, SleepFn};
use wsp_engine::Machine;

use support::guest::LocalGuest;
use support::strays;

type Lines = BoxStream<'static, Result<String, ExecFailure>>;

/// Ends every run a case started, closed or not, with its whole group: a case that failed or timed out before its
/// close leaves its agent and input pump waiting on a message that never comes.
struct Ends;
impl Drop for Ends {
    fn drop(&mut self) {
        strays::sweep();
    }
}

/// This computer's bash as the computer the run is on; the folder under which the run folder and the cases' own files
/// live is kept while the guard lives.
fn guest() -> (Arc<dyn Machine>, String, String, tempfile::TempDir) {
    let (guest, run_dir, dir) = LocalGuest::new(false, "");
    let root = dir.path().to_string_lossy().into_owned();
    let machine: Arc<dyn Machine> = guest;
    (machine, run_dir, root, dir)
}

fn options(poll_ms: u64, run_dir: &str) -> MachineExecOptions {
    MachineExecOptions { poll_ms: Some(poll_ms), run_dir: Some(run_dir.to_string()), ..Default::default() }
}

fn sleep_fn(f: impl Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static) -> SleepFn {
    Arc::new(f)
}

fn seeded(lines: &[&str]) -> Launch {
    Launch { input: Some(lines.iter().map(|l| l.to_string()).collect()), ..Default::default() }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64
}

/// An agent that answers every line it reads with `got <line>` and its own pid, and on "leave" first starts a sleep
/// whose parent exits, so it stays behind in the run's group as a command a turn left going does.
fn agent(left: &str) -> String {
    format!(r#"while IFS= read -r l; do if [ "$l" = leave ]; then ( sleep 300 & echo $! > {} ); fi; echo "got $l $$"; done"#, support::sq(left))
}

/// Lines off a turn's view until one matches.
async fn until(lines: &mut Lines, matches: impl Fn(&str) -> bool) -> Vec<String> {
    let mut seen = Vec::new();
    loop {
        match lines.next().await {
            None => panic!("the turn ended before a match, after {seen:?}"),
            Some(next) => {
                let line = next.expect("a line, not a failure");
                let hit = matches(&line);
                seen.push(line);
                if hit {
                    return seen;
                }
            }
        }
    }
}

/// Records the run's leader as a pid to end with the case.
async fn record_leader(stream: &Arc<dyn ExecStream>) -> i32 {
    strays::grandchild(&format!("{}.pid", stream.run().expect("a handle"))).await
}

/// Ends every run on the machine with the sweep, as the TS teardown does.
async fn end_guests(machine: &Arc<dyn Machine>, run_dir: &str) {
    machine_exec_stream(machine.clone(), MachineExecOptions { run_dir: Some(run_dir.to_string()), ..Default::default() }, None).sweep(&[]).expect("a sweep").await.expect("the sweep answered");
}

mod a_process_kept_between_turns_on_a_joined_computer_s_road {
    use super::*;

    own_group_test!(rests_past_the_idle_limit_between_turns_and_answers_the_next_turn_s_message_from, 20_000, {
        let _ends = Ends;
        let (machine, run_dir, root, _dir) = guest();
        // Every wait short, the resting one too, so the idle limit is read many times while no turn is open.
        let opts = MachineExecOptions { idle_ms: Some(400.0), sleep: Some(sleep_fn(|ms| support::sleep_ms(ms.min(50)).boxed())), ..options(20, &run_dir) };
        let stream = machine_exec_stream(machine, opts, None).launch(&agent(&format!("{root}/left")), seeded(&["one"]));
        let keeper = keep_run(stream.clone(), None);
        let (first, first_from) = keeper.turn();
        assert_eq!(first_from, None);
        let mut first_lines = first.lines();
        let one = until(&mut first_lines, |line| line.starts_with("got one")).await;
        record_leader(&stream).await;
        first.close_input();
        // Twice the idle limit with no turn open: a turn this quiet would be cut, a resting process is not.
        support::sleep_ms(900).await;
        assert!(keeper.up());

        let (second, second_from) = keeper.turn();
        assert!(second_from.expect("where the second turn starts") > 0);
        assert_eq!(second.write("two".to_string()).await.unwrap(), WriteOutcome::Written);
        let mut second_lines = second.lines();
        let two = until(&mut second_lines, |line| line.starts_with("got two")).await;
        assert_eq!(two, vec![format!("got two {}", one.last().unwrap().split(' ').last().unwrap())]);

        keeper.close(1_000, 500).await;
        support::wait_for("the run folder to empty", 5_000, || support::names(&run_dir).is_empty()).await;
    });

    own_group_test!(wakes_the_resting_reader_for_the_next_turn_s_message_rather_than_waiting_out_its, 20_000, {
        let _ends = Ends;
        let (machine, run_dir, root, _dir) = guest();
        // The resting poll's wait never ends, so the next turn's lines come only if its start wakes the reader.
        let sleep = sleep_fn(|ms| if ms >= 10_000 { futures::future::pending::<()>().boxed() } else { support::sleep_ms(ms).boxed() });
        let opts = MachineExecOptions { sleep: Some(sleep), ..options(20, &run_dir) };
        let stream = machine_exec_stream(machine, opts, None).launch(&agent(&format!("{root}/left")), seeded(&["one"]));
        let keeper = keep_run(stream.clone(), None);
        let (first, _) = keeper.turn();
        let mut first_lines = first.lines();
        until(&mut first_lines, |line| line.starts_with("got one")).await;
        record_leader(&stream).await;
        first.close_input();
        support::sleep_ms(200).await;
        let (second, _) = keeper.turn();
        second.write("two".to_string()).await.unwrap();
        let mut second_lines = second.lines();
        assert_eq!(until(&mut second_lines, |line| line.starts_with("got two")).await.len(), 1);
        keeper.close(1_000, 500).await;
    });

    // What a turn left is read off /proc, which a joined computer has and a Mac standing in for one does not.
    own_group_test!(ends_what_a_turn_left_running_beside_the_agent_once_that_turn_is_over_and_keeps, 20_000, {
        if !std::path::Path::new("/proc/1/stat").exists() {
            return;
        }
        let _ends = Ends;
        let (machine, run_dir, root, _dir) = guest();
        let left = format!("{root}/left");
        let stream = machine_exec_stream(machine, options(20, &run_dir), None).launch(&agent(&left), seeded(&["leave"]));
        let keeper = keep_run(stream.clone(), None);
        let (first, _) = keeper.turn();
        let mut first_lines = first.lines();
        until(&mut first_lines, |line| line.starts_with("got leave")).await;
        record_leader(&stream).await;
        let stray = strays::grandchild(&left).await;
        assert!(strays::alive(stray));
        first.close_input();
        strays::gone(stray).await;
        assert!(keeper.up());

        let (second, _) = keeper.turn();
        second.write("still".to_string()).await.unwrap();
        let mut second_lines = second.lines();
        assert_eq!(until(&mut second_lines, |line| line.starts_with("got still")).await.len(), 1);
        keeper.close(1_000, 500).await;
        support::wait_for("the run folder to empty", 5_000, || support::names(&run_dir).is_empty()).await;
    });

    own_group_test!(re_opens_a_turn_on_a_kept_process_from_where_that_turn_began_past_the_turns_befo, 20_000, {
        let _ends = Ends;
        let (machine, run_dir, root, _dir) = guest();
        let factory = machine_exec_stream(machine, options(20, &run_dir), None);
        let stream = factory.launch(&agent(&format!("{root}/left")), seeded(&["one"]));
        let keeper = keep_run(stream.clone(), None);
        let (first, _) = keeper.turn();
        let mut first_lines = first.lines();
        until(&mut first_lines, |line| line.starts_with("got one")).await;
        record_leader(&stream).await;
        first.close_input();
        let (second, second_from) = keeper.turn();
        second.write("two".to_string()).await.unwrap();
        let mut second_lines = second.lines();
        until(&mut second_lines, |line| line.starts_with("got two")).await;

        // A host that restarts in the second turn re-opens it from its own first byte.
        let attached = factory.attach(&stream.run().expect("a handle"), AttachRunOptions { input: true, started_at: now_ms(), from: Some(second_from.expect("where the second turn starts")) }).expect("an attach").await;
        let Attached::Stream(reopened) = attached.expect("an attach that answers") else { panic!("the run was gone") };
        let mut read = reopened.lines();
        let line = read.next().await.expect("a line").expect("a line, not a failure");
        assert!(line.starts_with("got two "), "{line}");
        keeper.close(1_000, 500).await;
        reopened.kill();
        support::wait_for("the run folder to empty", 5_000, || support::names(&run_dir).is_empty()).await;
    });

    own_group_test!(a_run_a_case_never_closed_is_ended_with_its_input_pump_by_the_teardown_every_cas, 20_000, {
        let _ends = Ends;
        let (machine, run_dir, root, _dir) = guest();
        let stream = machine_exec_stream(machine.clone(), options(20, &run_dir), None).launch(&agent(&format!("{root}/left")), seeded(&["one"]));
        let keeper = keep_run(stream.clone(), None);
        let (turn, _) = keeper.turn();
        let mut lines = turn.lines();
        until(&mut lines, |line| line.starts_with("got one")).await;
        let leader = strays::grandchild(&format!("{}.pid", stream.run().expect("a handle"))).await;
        let pump = strays::grandchild(&format!("{}.tail", stream.run().expect("a handle"))).await;
        assert_eq!([strays::alive(leader), strays::alive(pump)], [true, true]);
        end_guests(&machine, &run_dir).await;
        support::wait_for("the leader and the pump to go", 5_000, || [strays::alive(leader), strays::alive(pump)] == [false, false]).await;
        stream.kill();
    });
}
