# detect_small

Source: `packages/collect/src/detect/presence.ts` (16), `toolchains.ts` (56), `shell.ts` (59), `identity.ts` (70) at `a3b870a6e`. Tests: `toolchains.test.ts` (6), `shell.test.ts` (8), `identity.test.ts` (9). Rust: `src/detect/{presence,toolchains,shell,identity}.rs`; tests `tests/detect_toolchains.rs` (6), `tests/detect_shell.rs` (8), `tests/detect_identity.rs` (9), `tests/detect_turn_servers.rs` `presence_is_*` (1).

## What it does

Four of the ladder's detectors: whether a catalog entry is on the computer, the toolchain managers and their pins, the shell rung's rows, and the identity rung's rows. Each builds rows with `detect/common.rs` (`found`, `exists`, `row`, `entry`, `item`, `present`, `first_line`), which the detect writer owns; this page pins only what these files decide.

## Behaviour

1. `presence_of` (presence.ts:12-16): `found(config_paths)` (each `~` path that stats, in order, bytes summed), then `which(bin)`; no path and no bin is `None`; else `{paths, bytes, bin}`. Both asks happen whatever the first answered.
2. Toolchains (toolchains.ts:18-56), managers in this order: mise (bin `mise`, marker `~/.config/mise/config.toml`, pins `~/.config/mise/config.toml ~/.tool-versions`, label `mise pins`); asdf (`asdf`, `~/.asdf`, `~/.tool-versions ~/.asdfrc`, `asdf pins`); fnm (`fnm`, pins `~/.node-version ~/.nvmrc`, label `fnm (node <first line of fnm current>)` or `fnm`); nvm (marker `~/.nvm`, pin `~/.nvmrc`, label `nvm (default <first line of ~/.nvm/alias/default>)` or `nvm`); pyenv (`pyenv`, `~/.pyenv`, `~/.pyenv/version`, `pyenv (global python)`); uv (`uv`, `~/.config/uv/uv.toml ~/.python-version`, `uv (python versions)`); rustup (marker `~/.rustup/settings.toml`, pins it and `~/.cargo/config.toml`, `rustup default toolchain`); go (`go`, `~/.config/go/env`, label `go <digits of /go(\d+(?:\.\d+)*)/ in the first line of go version>` or `go`). A manager is present when its bin is on PATH (asked first) or any marker exists (markers asked in order until one hits); its row is `entry(toolchains, toolchains/<id>, label, found(pins))`. The label is read only for a present manager.
3. Shell (shell.ts:37-59): rows in this order: one `row` per `RC_FILES` name (`shell/<name>`, label `~/.<name>`, path `~/.<name>`); the eight ROWS of :8-17; `shell/oh-my-zsh` (path `~/.oh-my-zsh/custom`, always) when `~/.oh-my-zsh` exists; each plugin manager of :20-25 whose marker exists, as an `item` (zinit, zplug) or an always `row` of its list file (antidote, fisher); `shell/history` over the four history paths, default skip. Rows that are `None` drop out. When `login_shell` answers, every row gets `login`.
4. `login_shell` (:31-35): `None` without SHELL; else read `/etc/shells` (missing reads as empty), split on `\n`, JS-trim each; SHELL listed gives its text after the last `/`.
5. Identity (identity.ts:20-70), in order: `which git`; `found(~/.gitconfig, ~/.config/git/config)`; with git, `git config --global --get user.name` and `user.email` (first lines). A user or a config gives `identity/git-user` over the configs, `required: true` only with a user. With git: `gpg.format`, then `commit.gpgsign`, and only when that is absent `user.signingkey`; a format or a signing answer gives `identity/git-signing`, label `git signing (<format or gpg>); the signing key itself stays on this computer`, over `found([gpg.ssh.allowedSignersFile])` asked only for format `ssh`. Then the `identity/ssh-config` row (path and label `~/.ssh/config`, detail `SSH_IGNORE_UNKNOWN_DETAIL`). Then `list(~/.ssh)`: names ending `.pub` give one required `identity/ssh-public-keys` row labelled `SSH public keys (<n>)`; every other name that is not `config authorized_keys environment rc allowed_signers`, holds no `known_hosts`, does not end `.sock` and stats as a file gives `identity/ssh-key/<name>` (default skip, reason `private key, never copied; the machine gets its own key`, bytes its size); then `identity/ssh-known-hosts` over the names holding `known_hosts` (skip, `host entries are rebuilt on first connect`); then `identity/gpg` when `~/.gnupg` exists (skip, `GPG keys are never copied`). No file under `~/.ssh` is read.

## Edge cases

- An empty laptop gives no rows in all three detectors.
- A SHELL that `/etc/shells` does not list leaves every row without `login`.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `presence_is_the_config_paths_that_exist_and_whether_the_command_is_on_path` |
| 2 | the six `tests/detect_toolchains.rs` cases |
| 3, 4 | the eight `tests/detect_shell.rs` cases |
| 5 | the nine `tests/detect_identity.rs` cases, `never_reads_a_file_under_ssh` among them |
