// SPDX-License-Identifier: AGPL-3.0-only
//! Round 2 of wsp-labs/wsp-map#2448 (wsp-store): a row read back at the host's cap (B3, P1), against serde_json's own
//! reading of the same text, and the row cloned and dropped on a 1 MiB stack (ruling 3).
mod common;

use common::{run, writer, Home};
use serde::Deserialize;
use serde_json::Value;
use wsp_store::row_json::{parse_row, ROW_DEPTH_MAX};
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::Store;

fn in_child(name: &str, body: fn()) {
    if std::env::var("ROUND2_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("ROUND2_2448_CHILD", name)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let why: Vec<&str> =
        stderr.lines().filter(|l| l.contains("overflow") || l.contains("left:") || l.contains("panicked")).take(2).collect();
    assert!(out.status.success(), "the child failed ({}): {}", out.status, why.join(" | "));
}

/// `levels` objects, each holding the next under "a", the innermost holding 1.
fn objects(levels: usize) -> Value {
    let mut v = Value::from(1);
    for _ in 0..levels {
        v = Value::Object([("a".to_string(), v)].into_iter().collect());
    }
    v
}

fn stored_and_read(levels: usize) -> Result<Value, String> {
    let home = Home::new();
    let store = sqlite_store(&home.state(), writer());
    run(store.put("things", "a", objects(levels))).expect("stored");
    let read = run(store.get("things", "a")).map_err(|e| e.message)?.expect("a row");
    let listed = run(store.list("things")).map_err(|e| e.message)?;
    assert_eq!(listed.len(), 1);
    Ok(read)
}

// P1 of the review: sqlite_store.rs parse_row read a row with serde_json's 128-level limit, so `get` of a row past it failed
// and so did the `list` holding it. node's JSON.parse reads the row.
#[test]
fn a_record_200_deep_reads_back_by_get_and_list() {
    let read = stored_and_read(200).expect("a record 200 deep reads back");
    assert_eq!(read, objects(200));
}

// A slate's record wraps a value of 500 levels in up to 6 more (record, kept, snapshot, document, values, declaration), and
// the store reads a row of 500 + 6 levels back. The first row past that fails the call with a line, not the process.
#[test]
fn a_row_reads_back_to_the_cap_and_not_past_it() {
    let at_cap = stored_and_read(ROW_DEPTH_MAX).expect("a row at the cap reads back");
    assert_eq!(at_cap, objects(ROW_DEPTH_MAX));
    let past = stored_and_read(ROW_DEPTH_MAX + 1).expect_err("a row past the cap is refused");
    assert_eq!(past, format!("a stored row nests deeper than {ROW_DEPTH_MAX} levels"));
}

// A row is cloned (the slates' records are copied out of the store) and dropped: a Value of objects dies in clone at 628 levels
// on a 1 MiB debug stack, and the cap is 506.
fn clone_body() {
    let read = stored_and_read(ROW_DEPTH_MAX).expect("a row at the cap");
    let copy = read.clone();
    assert_eq!(copy, read);
    drop(copy);
    drop(read);
}

#[test]
fn a_row_at_the_cap_is_cloned_and_dropped_on_1_mib() {
    in_child("a_row_at_the_cap_is_cloned_and_dropped_on_1_mib", clone_body);
}

// The loop that reads past serde_json's 128 levels against serde_json itself with its limit off, on a stack of its own:
// random documents (whitespace, escapes, numbers, empty containers, duplicate keys) carrying a spine of 129 to 506 levels.
struct Rng(u64);

impl Rng {
    fn next(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

fn gap(rng: &mut Rng) -> &'static str {
    ["", "", "", " ", "\n", "\t ", "\r\n  "][rng.next(7) as usize]
}

fn scalar(rng: &mut Rng) -> String {
    const NUMBERS: [&str; 9] = ["0", "-0", "17", "-3.25", "1e5", "1.5E+300", "123456789012345678901234567890", "0.1", "5e-324"];
    match rng.next(8) {
        0 => "null".into(),
        1 => "true".into(),
        2 => "false".into(),
        3 => format!(
            "\"{}\"",
            ["", "plain", "a\\\"b", "\\n\\t\\\\", "\\u00e9\\ud83d\\ude00", "[{]},:", "\u{e9}\u{1f600}"][rng.next(7) as usize]
        ),
        _ => NUMBERS[rng.next(9) as usize].into(),
    }
}

fn container(rng: &mut Rng, levels: usize, spine: bool) -> String {
    let object = rng.next(2) == 0;
    let members = if levels == 0 { rng.next(3) } else { rng.next(3) } as usize;
    let mut items: Vec<String> =
        (0..members).map(|_| if levels > 0 && rng.next(4) == 0 { container(rng, 0, false) } else { scalar(rng) }).collect();
    if spine {
        items.insert(rng.next(items.len() as u64 + 1) as usize, container(rng, levels - 1, levels > 1));
    }
    let key = |rng: &mut Rng| ["a", "b", "a", "k\\u00e9", ""][rng.next(5) as usize];
    let body: Vec<String> = items
        .into_iter()
        .map(|item| {
            if object {
                format!("{}\"{}\"{}:{}{}", gap(rng), key(rng), gap(rng), gap(rng), item)
            } else {
                format!("{}{}", gap(rng), item)
            }
        })
        .collect();
    let (open, close) = if object { ('{', '}') } else { ('[', ']') };
    format!("{open}{}{}{close}", body.join(&format!("{},", gap(rng))), gap(rng))
}

#[test]
fn the_deep_reader_reads_what_serde_json_reads() {
    let big = std::thread::Builder::new().stack_size(256 * 1024 * 1024);
    big.spawn(|| {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let (mut compared, mut refused) = (0, 0);
        for round in 0..600 {
            let levels = 128 + rng.next((ROW_DEPTH_MAX - 128) as u64) as usize;
            let text = format!("{}{}{}", gap(&mut rng), container(&mut rng, levels, true), gap(&mut rng));
            let mut de = serde_json::Deserializer::from_str(&text);
            de.disable_recursion_limit();
            let oracle = Value::deserialize(&mut de).and_then(|v| de.end().map(|()| v)).expect("the oracle reads the text");
            let read = parse_row(&text).unwrap_or_else(|e| panic!("round {round}, {levels} levels: {}", e.message));
            assert_eq!(read, oracle, "round {round}");
            assert_eq!(serde_json::to_string(&read).unwrap(), serde_json::to_string(&oracle).unwrap(), "round {round}: member order");
            compared += 1;
            let broken = format!("{}x", text.trim_end());
            assert!(parse_row(&broken).is_err(), "round {round}: trailing text");
            let cut = &text.trim_end()[..text.trim_end().len() - 1];
            assert!(parse_row(cut).is_err(), "round {round}: cut short");
            refused += 2;
            std::mem::forget(oracle);
            std::mem::forget(read);
        }
        eprintln!("compared {compared} documents and refused {refused} broken ones");
    })
    .unwrap()
    .join()
    .unwrap();
}
