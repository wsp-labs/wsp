// SPDX-License-Identifier: AGPL-3.0-only
//! Review of join A (#2476), must-fix 1 ("no `unsafe` outside FFI anywhere under host/crates/"): every `unsafe` block,
//! in a crate's src or its tests, is a call into the operating system or the allocator. The join's own test reads
//! src alone, and two test supports in wsp-machines dereference raw pointers.
use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a folder") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

const FFI: [&str; 8] = ["libc::", "CStr::", "mem::zeroed", "assume_init", "pre_exec", "System.", "umask(", "environ"];

#[test]
fn every_unsafe_block_under_host_crates_is_a_call_into_the_os() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for krate in std::fs::read_dir(&crates).expect("the crates folder") {
        let krate = krate.expect("a crate").path();
        for part in ["src", "tests"] {
            if krate.join(part).is_dir() {
                sources(&krate.join(part), &mut files);
            }
        }
    }
    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("a source");
        let lines: Vec<&str> = text.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            let opens = code.contains("unsafe {") || code.contains("|| unsafe") || code.contains("unsafe fn");
            if !opens || code.contains("unsafe impl") || file.ends_with("review_rjoin_unsafe.rs") {
                continue;
            }
            let block = lines[at..(at + 5).min(lines.len())].join("\n");
            if !FFI.iter().any(|call| block.contains(call)) {
                found.push(format!("{}:{}", file.strip_prefix(&crates).unwrap_or(file).display(), at + 1));
            }
        }
    }
    assert!(found.is_empty(), "unsafe that calls no OS function: {found:?}");
}
