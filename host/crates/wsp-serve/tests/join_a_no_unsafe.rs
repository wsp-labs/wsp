// SPDX-License-Identifier: AGPL-3.0-only
//! create_runtime join A (#2476), must-fix 1: no `unsafe` that is not a call into the operating system. The trait's
//! building callback is higher-ranked, so no lifetime is cast; a cast of one is a `transmute`, and wsp-serve calls
//! nothing outside Rust.
use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a source folder") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
fn no_crate_transmutes() {
    let mut files = Vec::new();
    for krate in std::fs::read_dir(crates()).expect("the crates folder") {
        let src = krate.expect("a crate").path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    assert!(files.len() > 100, "the scan found {} files", files.len());
    let found: Vec<_> = files.iter().filter(|file| std::fs::read_to_string(file).expect("a source").contains("transmute")).collect();
    assert!(found.is_empty(), "a lifetime is cast with transmute in {found:?}");
}

#[test]
fn wsp_serve_has_no_unsafe_block_at_all() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let found: Vec<_> = files
        .iter()
        .filter(|file| {
            std::fs::read_to_string(file)
                .expect("a source")
                .lines()
                .any(|line| !line.trim_start().starts_with("//") && (line.contains("unsafe {") || line.contains("unsafe fn")))
        })
        .collect();
    assert!(found.is_empty(), "unsafe in {found:?}");
}
