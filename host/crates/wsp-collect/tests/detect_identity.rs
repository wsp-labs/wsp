// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/identity.test.ts.
mod common;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::detect::identity::detect_identity;
use wsp_collect::manifest::ManifestEntry;
use wsp_collect::ssh_config::SSH_IGNORE_UNKNOWN_DETAIL;

fn git(l: FakeLaptop) -> FakeLaptop {
    l.exec("git config --global --get user.name", "Dev Person\n").exec("git config --global --get user.email", "dev@example.com\n")
}

fn value(row: &ManifestEntry) -> Value {
    serde_json::to_value(row).unwrap()
}

fn values(rows: &[ManifestEntry]) -> Vec<Value> {
    rows.iter().map(value).collect()
}

/// toMatchObject for the flat fields these rows compare: every key named is equal, others are free.
fn assert_matches(row: &ManifestEntry, want: &Value, ctx: &str) {
    let got = value(row);
    for (k, v) in want.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "{ctx}: field {k} of {got}");
    }
}

mod identity {
    use super::*;

    #[test]
    fn git_identity_is_required_and_points_at_the_config_files_that_exist() {
        let host = fake_host(git(FakeLaptop::new().file("~/.gitconfig", 512).which(&["git"])));
        let rows = block_on(detect_identity(&host));
        let git = rows.first().expect("a git row");
        assert_eq!(
            value(git),
            json!({ "rung": "identity", "id": "identity/git-user", "label": "git name and email", "paths": ["~/.gitconfig"], "bytes": 512, "default": "bring", "required": true })
        );
    }

    #[test]
    fn a_gitconfig_with_no_user_set_is_offered_but_not_required() {
        let host = fake_host(FakeLaptop::new().file("~/.config/git/config", 100).which(&["git"]));
        let rows = block_on(detect_identity(&host));
        let git = rows.first().expect("a git row");
        assert_matches(git, &json!({ "id": "identity/git-user", "paths": ["~/.config/git/config"], "default": "bring" }), "git-user");
        assert_eq!(git.required, None);
    }

    #[test]
    fn no_git_and_no_gitconfig_means_no_identity_row() {
        assert_eq!(block_on(detect_identity(&empty_host())), Vec::<ManifestEntry>::new());
    }

    #[test]
    fn signing_config_travels_the_allowed_signers_file_comes_with_ssh_signing() {
        let host = fake_host(
            git(FakeLaptop::new().file("~/.gitconfig", 512).file("~/.ssh/allowed_signers", 90).which(&["git"]))
                .exec("git config --global --get gpg.format", "ssh\n")
                .exec("git config --global --get gpg.ssh.allowedSignersFile", "~/.ssh/allowed_signers\n"),
        );
        let rows = block_on(detect_identity(&host));
        let signing = rows.iter().find(|r| r.id == "identity/git-signing").expect("a git-signing row");
        assert_eq!(
            value(signing),
            json!({
                "rung": "identity", "id": "identity/git-signing", "label": "git signing (ssh); the signing key itself stays on this computer",
                "paths": ["~/.ssh/allowed_signers"], "bytes": 90, "default": "bring",
            })
        );
    }

    #[test]
    fn ssh_becomes() {
        let rows: Vec<(&str, &str, Value)> = vec![
            (
                "config",
                "identity/ssh-config",
                json!({ "default": "bring", "paths": ["~/.ssh/config"], "detail": SSH_IGNORE_UNKNOWN_DETAIL }),
            ),
            (
                "id_ed25519",
                "identity/ssh-key/id_ed25519",
                json!({ "default": "skip", "reason": "private key, never copied; the machine gets its own key" }),
            ),
            ("work_rsa", "identity/ssh-key/work_rsa", json!({ "default": "skip" })),
            (
                "known_hosts",
                "identity/ssh-known-hosts",
                json!({ "default": "skip", "reason": "host entries are rebuilt on first connect" }),
            ),
            (
                "google_compute_known_hosts",
                "identity/ssh-known-hosts",
                json!({ "default": "skip", "paths": ["~/.ssh/google_compute_known_hosts"] }),
            ),
        ];
        for (name, id, shape) in rows {
            let host = fake_host(FakeLaptop::new().file(&format!("~/.ssh/{name}"), 40));
            let found = block_on(detect_identity(&host));
            let row = found.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("{name}: no row {id}"));
            let mut want = json!({ "rung": "identity", "id": id, "bytes": 40 });
            for (k, v) in shape.as_object().unwrap() {
                want[k] = v.clone();
            }
            assert_matches(row, &want, name);
        }
    }

    #[test]
    fn public_keys_are_one_required_row_authorized_keys_and_sockets_are_not_keys() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.ssh/id_ed25519.pub", 100)
                .file("~/.ssh/work.pub", 120)
                .file("~/.ssh/authorized_keys", 300)
                .file("~/.ssh/agent.sock", 0),
        );
        let rows = block_on(detect_identity(&host));
        assert_eq!(
            values(&rows),
            vec![json!({
                "rung": "identity", "id": "identity/ssh-public-keys", "label": "SSH public keys (2)",
                "paths": ["~/.ssh/id_ed25519.pub", "~/.ssh/work.pub"], "bytes": 220, "default": "bring", "required": true,
            })]
        );
    }

    #[test]
    fn a_gpg_keyring_is_listed_locked_off() {
        let rows = block_on(detect_identity(&fake_host(FakeLaptop::new().file("~/.gnupg/pubring.kbx", 5000))));
        assert_eq!(
            values(&rows),
            vec![json!({
                "rung": "identity", "id": "identity/gpg", "label": "GPG keyring", "paths": ["~/.gnupg"], "bytes": 5000,
                "default": "skip", "reason": "GPG keys are never copied",
            })]
        );
    }

    #[test]
    fn the_ssh_config_row_says_what_the_copy_gains_as_a_detail_leaving_reason_to_mean_l() {
        let rows = block_on(detect_identity(&fake_host(FakeLaptop::new().file("~/.ssh/config", 900))));
        let config = rows.first().expect("an ssh-config row");
        assert_eq!(
            value(config),
            json!({
                "rung": "identity", "id": "identity/ssh-config", "label": "~/.ssh/config", "paths": ["~/.ssh/config"], "bytes": 900, "default": "bring",
                "detail": "copied with IgnoreUnknown at the top, since ssh on the machine does not know every option yours does, and one unknown option would stop it reading the file",
            })
        );
        assert_eq!(config.detail.as_deref(), Some(SSH_IGNORE_UNKNOWN_DETAIL));
        assert_eq!(config.reason, None);
    }

    #[test]
    fn never_reads_a_file_under_ssh() {
        let host = fake_host(FakeLaptop::new().file("~/.ssh/id_ed25519", "PRIVATE").file("~/.ssh/config", "Host x"));
        block_on(detect_identity(&host));
        let reads: Vec<String> = host.calls().into_iter().filter(|c| c.starts_with("read")).collect();
        assert_eq!(reads, Vec::<String>::new());
    }
}

/// Every case of tests/fixtures/oracle/w2/detect_small.json (oracle/w2-detect_small.oracle.ts): spec detect_small's
/// wave-2 rules, for presence, toolchains, shell and identity.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, strs, w2};
    use wsp_collect::detect::presence::presence_of;
    use wsp_collect::detect::shell::{detect_shell, login_shell};
    use wsp_collect::detect::toolchains::detect_toolchains;

    fn rows(r: &[ManifestEntry]) -> Value {
        Value::Array(r.iter().map(|r| serde_json::to_value(r).unwrap()).collect())
    }

    #[test]
    fn presence_answers_and_reads_what_the_ts_did() {
        for case in cases(&w2("detect_small"), "presence_of") {
            let h = host(&case["laptop"]);
            let got = block_on(presence_of(&h, &strs(&case["args"][0]), case["args"][1].as_str().unwrap()))
                .map(|p| json!({ "paths": p.paths, "bytes": p.bytes, "bin": p.bin }))
                .unwrap_or(Value::Null);
            assert_eq!(got, case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    #[test]
    fn toolchains_shell_and_identity_write_the_rows_the_ts_wrote() {
        let f = w2("detect_small");
        for case in cases(&f, "detect_toolchains") {
            let h = host(&case["laptop"]);
            assert_eq!(rows(&block_on(detect_toolchains(&h))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
        for case in cases(&f, "detect_shell") {
            let h = host(&case["laptop"]);
            assert_eq!(rows(&block_on(detect_shell(&h))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
        for case in cases(&f, "login_shell") {
            let h = host(&case["laptop"]);
            assert_eq!(json!(block_on(login_shell(&h))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
        for case in cases(&f, "detect_identity") {
            let h = host(&case["laptop"]);
            assert_eq!(rows(&block_on(detect_identity(&h))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }
}
