// SPDX-License-Identifier: AGPL-3.0-only
//! Command names that a JavaScript object treats specially: the TS keeps a bucket's counts in a plain `{}`, so a name that
//! reads as an array index is listed first in ascending order and `__proto__` is never kept. Expected values are what node
//! 22.23 printed for `b.commands[name] = (b.commands[name] ?? 0) + 1` over the same names, in this order.
#![allow(clippy::result_large_err)]
mod common;

use common::fake_host::block_on;
use futures::StreamExt;
use wsp_collect::history::reader::{Call, CallKind};
use wsp_collect::history::tally::reduce_calls;

fn shell(line: &str) -> Call {
    Call { session: "s".to_string(), folder: None, at: None, kind: CallKind::Shell { line: line.to_string() } }
}

fn commands_of(lines: &[&str]) -> Vec<(String, u64)> {
    let calls: Vec<_> = lines.iter().map(|l| Ok(shell(l))).collect();
    let buckets = block_on(reduce_calls(futures::stream::iter(calls).boxed_local())).unwrap();
    buckets[0].commands.iter().map(|(k, v)| (k.clone(), *v)).collect()
}

fn named(rows: &[(&str, u64)]) -> Vec<(String, u64)> {
    rows.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// node: `{"7":1,"42":1,"4294967294":1,"ls":2,"007":1,"4294967295":1}` for 42, ls, 7, ls, 007, 4294967295, 4294967294.
#[test]
fn names_that_read_as_array_indexes_come_first_in_ascending_order() {
    let got = commands_of(&["42 a", "ls", "7 b", "ls", "007 c", "4294967295 d", "4294967294 e"]);
    assert_eq!(got, named(&[("7", 1), ("42", 1), ("4294967294", 1), ("ls", 2), ("007", 1), ("4294967295", 1)]));
}

/// node: assigning text to `__proto__` of a plain object is ignored, so the name is never kept; the call is still counted.
#[test]
fn a_command_named_proto_is_never_kept() {
    let calls = vec![Ok(shell("__proto__ run")), Ok(shell("ls"))];
    let buckets = block_on(reduce_calls(futures::stream::iter(calls).boxed_local())).unwrap();
    assert_eq!((buckets[0].calls, buckets[0].commands.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>()), (2, vec![("ls", 1)]));
}
