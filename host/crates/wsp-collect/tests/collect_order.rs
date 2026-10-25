// SPDX-License-Identifier: AGPL-3.0-only
//! collect()'s answer as the TS wrote it, byte for byte: rows in ladder order, each row's keys in the schema's order
//! (tests/fixtures/oracle/collect.json, written by oracle/rest.oracle.ts over the TS collect test's laptop).
//! spec/manifest.md rule 9.
mod common;

use common::fake_host::*;
use wsp_collect::collect::{collect, CollectOptions};

#[test]
fn the_fixture_laptops_manifest_writes_exactly_the_json_the_ts_wrote() {
    let o: serde_json::Value = serde_json::from_str(include_str!("fixtures/oracle/collect.json")).unwrap();
    let laptop = FakeLaptop::new()
        .file("~/.gitconfig", 512)
        .file("~/.ssh/config", 1200)
        .file("~/.ssh/id_ed25519", 400)
        .file("~/.ssh/id_ed25519.pub", 100)
        .file("~/.ssh/known_hosts", 3000)
        .file("~/.zshrc", 3000)
        .file("~/.zshenv", 200)
        .file("~/.config/starship.toml", 900)
        .file("~/.tmux.conf", 600)
        .file("~/.zsh_history", 800_000)
        .file("~/.config/nvim/init.lua", 120_000)
        .file("~/Library/Application Support/Code/User/settings.json", 2000)
        .file("~/.config/mise/config.toml", 300)
        .file("~/.claude/settings.json", 400)
        .file("~/.claude/CLAUDE.md", 900)
        .file("~/.claude/.credentials.json", 800)
        .file("~/.codex/config.toml", 50)
        .file("~/.codex/auth.json", 900)
        .file("~/.config/gh/hosts.yml", 200)
        .file("~/.aws/config", 300)
        .file("~/go/bin/gopls", 1)
        .which(&["git", "brew", "mise", "npm", "claude", "codex", "go"])
        .exec("git config --global --get user.name", "Dev Person\n")
        .exec("git config --global --get user.email", "dev@example.com\n")
        .exec("git config --global --get gpg.format", "ssh\n")
        .exec(
            "HOMEBREW_NO_AUTO_UPDATE=1 brew bundle dump --file=- --formula --taps --no-restart",
            "tap \"homebrew/bundle\"\nbrew \"gh\"\nbrew \"jq\"\n",
        )
        .exec("npm ls -g --depth=0 --json", r#"{"dependencies":{"npm":{"version":"10"},"pnpm":{"version":"9.12.0"}}}"#)
        .exec("go version", "go version go1.23.1 darwin/arm64\n")
        .exec(
            "go version -m /Users/dev/go/bin/gopls",
            "x\n\tpath\tgolang.org/x/tools/gopls\n\tmod\tgolang.org/x/tools/gopls\tv0.16.2\th1:abc=\n",
        )
        .exec("security find-generic-password -s Claude Code-credentials", "found\n");
    let manifest = block_on(collect(&fake_host(laptop), CollectOptions::default())).unwrap();
    assert_eq!(serde_json::to_string(&manifest).unwrap(), o["text"].as_str().unwrap());
}
