// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/collect.test.ts.
mod common;

use std::cell::RefCell;
use std::collections::HashSet;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::collect::{collect, CollectOptions};
use wsp_collect::manifest::{parse_manifest, Manifest, Rung, RUNGS};

/// A developer's MacBook as the collector sees it.
fn laptop() -> FakeHost {
    let npm = json!({ "dependencies": { "npm": { "version": "10" }, "pnpm": { "version": "9.12.0" } } }).to_string();
    let files: [(&str, u64); 21] = [
        ("~/.gitconfig", 512),
        ("~/.ssh/config", 1200),
        ("~/.ssh/id_ed25519", 400),
        ("~/.ssh/id_ed25519.pub", 100),
        ("~/.ssh/known_hosts", 3000),
        ("~/.zshrc", 3000),
        ("~/.zshenv", 200),
        ("~/.config/starship.toml", 900),
        ("~/.tmux.conf", 600),
        ("~/.zsh_history", 800_000),
        ("~/.config/nvim/init.lua", 120_000),
        ("~/Library/Application Support/Code/User/settings.json", 2000),
        ("~/.config/mise/config.toml", 300),
        ("~/.claude/settings.json", 400),
        ("~/.claude/CLAUDE.md", 900),
        ("~/.claude/.credentials.json", 800),
        ("~/.codex/config.toml", 50),
        ("~/.codex/auth.json", 900),
        ("~/.config/gh/hosts.yml", 200),
        ("~/.aws/config", 300),
        ("~/go/bin/gopls", 1),
    ];
    let laptop = files
        .iter()
        .fold(FakeLaptop::new(), |l, (p, n)| l.file(p, *n))
        .which(&["git", "brew", "mise", "npm", "claude", "codex", "go"])
        .exec("git config --global --get user.name", "Dev Person\n")
        .exec("git config --global --get user.email", "dev@example.com\n")
        .exec("git config --global --get gpg.format", "ssh\n")
        .exec(
            "HOMEBREW_NO_AUTO_UPDATE=1 brew bundle dump --file=- --formula --taps --no-restart",
            "tap \"homebrew/bundle\"\nbrew \"gh\"\nbrew \"jq\"\n",
        )
        .exec("npm ls -g --depth=0 --json", &npm)
        .exec("go version", "go version go1.23.1 darwin/arm64\n")
        .exec(
            "go version -m /Users/dev/go/bin/gopls",
            "x\n\tpath\tgolang.org/x/tools/gopls\n\tmod\tgolang.org/x/tools/gopls\tv0.16.2\th1:abc=\n",
        )
        .exec("security find-generic-password -s Claude Code-credentials", "found\n");
    fake_host(laptop)
}

mod collect {
    use super::*;

    #[test]
    fn composes_every_rung_in_ladder_order_into_a_valid_manifest() {
        let manifest = block_on(collect(&laptop(), CollectOptions::default())).expect("collect");
        let reparsed = parse_manifest(&serde_json::to_value(&manifest).unwrap()).expect("parse_manifest");
        assert_eq!(reparsed, manifest);
        let order: Vec<usize> = manifest.entries.iter().map(|e| RUNGS.iter().position(|r| *r == e.rung).unwrap()).collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted);
        let seen: HashSet<Rung> = manifest.entries.iter().map(|e| e.rung).collect();
        let all: HashSet<Rung> = RUNGS.iter().copied().collect();
        assert_eq!(seen, all);
    }

    #[test]
    fn matches_the_fixture_laptop_snapshot() {
        let manifest = block_on(collect(&laptop(), CollectOptions::default())).expect("collect");
        let got = serde_json::to_value(&manifest).unwrap();
        // The snapshot is vitest's with sorted keys, written once as plain JSON. serde_json's Map under preserve_order
        // is an IndexMap, whose equality ignores key order, so row key order does not matter here; array order does.
        let want: Value = serde_json::from_str(include_str!("fixtures/collect-snapshot.json")).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn reports_each_rung_s_row_count_as_it_finishes_in_ladder_order_before_the_manifest() {
        let seen: RefCell<Vec<(Rung, usize)>> = RefCell::new(Vec::new());
        let on_rung = |rung: Rung, count: usize| seen.borrow_mut().push((rung, count));
        let manifest = block_on(collect(&laptop(), CollectOptions { on_rung: Some(&on_rung) })).expect("collect");
        let seen = seen.into_inner();
        assert_eq!(seen.iter().map(|(r, _)| *r).collect::<Vec<_>>(), RUNGS.to_vec());
        for (rung, count) in &seen {
            assert_eq!(*count, manifest.entries.iter().filter(|e| e.rung == *rung).count(), "{rung:?}");
        }
        assert!(seen.iter().all(|(_, n)| *n > 0));
    }

    #[test]
    fn an_empty_laptop_reports_zero_for_every_rung_the_detectors_cover() {
        let seen: RefCell<Vec<usize>> = RefCell::new(Vec::new());
        let on_rung = |_rung: Rung, count: usize| seen.borrow_mut().push(count);
        let _ = block_on(collect(&empty_host(), CollectOptions { on_rung: Some(&on_rung) }));
        assert_eq!(seen.into_inner(), RUNGS.iter().map(|_| 0).collect::<Vec<usize>>());
    }

    #[test]
    fn an_empty_laptop_is_an_empty_manifest() {
        let manifest = block_on(collect(&empty_host(), CollectOptions::default())).expect("collect");
        assert_eq!(manifest, Manifest { entries: vec![] });
    }
}

/// collect's cases of tests/fixtures/oracle/w2/manifest.json: the manifest text, each rung's count and every call
/// (spec manifest wave-2 rule W6). The calls compare as a multiset: the tools rung asks its managers at once, and the
/// TS interleaves them where the Rust may not.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, w2};

    #[test]
    fn collect_writes_the_ts_manifest_counts_each_rung_and_makes_the_same_calls() {
        for case in cases(&w2("manifest"), "collect") {
            let what = case["name"].as_str().unwrap();
            let h = host(&case["laptop"]);
            let seen = RefCell::new(Vec::new());
            let on_rung = |rung: Rung, count: usize| seen.borrow_mut().push(json!([rung.as_str(), count]));
            match block_on(collect(&h, CollectOptions { on_rung: Some(&on_rung) })) {
                Ok(m) => {
                    assert_eq!(serde_json::to_string(&m).unwrap(), case["out"]["text"].as_str().unwrap(), "{what}");
                }
                Err(e) => assert_eq!(json!(e.message), case["out"]["error"], "{what}"),
            }
            assert_eq!(Value::Array(seen.into_inner()), case["on_rung"], "on_rung of {what}");
            let mut got = h.calls();
            let mut want: Vec<String> = case["calls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
            got.sort();
            want.sort();
            assert_eq!(got, want, "calls of {what}");
        }
    }
}
