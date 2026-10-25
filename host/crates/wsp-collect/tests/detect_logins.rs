// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/logins.test.ts at a3b870a6e.
mod common;

use common::fake_host::*;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wsp_catalog::catalog::{catalog, login_rows};
use wsp_catalog::signin::{login_id_of, SignInKind};
use wsp_collect::detect::logins::{claude_keychain_service, detect_logins, login_default};
use wsp_collect::host::Platform;
use wsp_collect::manifest::{ManifestEntry, RowDefault};

/// A credentials file holding Claude Code's own login, its token a stand-in.
const CLAUDE_LOGIN: &str = r#"{"claudeAiOauth":{"accessToken":"placeholder"},"mcpOAuth":{}}"#;

const CLAUDE_ROW_DETAIL: &str =
    "Claude Code signs in with the token claude setup-token prints on this computer; nothing of its login here travels";

fn rows_json(rows: &[ManifestEntry]) -> Value {
    serde_json::to_value(rows).unwrap()
}

fn ids(rows: &[ManifestEntry]) -> Vec<String> {
    rows.iter().map(|r| r.id.clone()).collect()
}

fn claude_row() -> Value {
    json!({
        "rung": "logins", "id": "logins/claude", "label": "Claude Code login", "group": "Agent logins",
        "paths": [], "bytes": 0, "default": "skip", "detail": CLAUDE_ROW_DETAIL
    })
}

fn with_dirs(laptop: FakeLaptop, claude_config_dir: Option<&str>, codex_home: Option<&str>) -> FakeHost {
    let mut host = fake_host(laptop);
    host.host.claude_config_dir = claude_config_dir.map(str::to_string);
    host.host.codex_home = codex_home.map(str::to_string);
    host
}

mod logins {
    use super::*;

    #[test]
    fn presence_by_path_a_browser_or_device_sign_in_starts_as_a_sign_in_on_the_machine() {
        // (name, platform, files, id, paths, bytes, default)
        type Row = (&'static str, Platform, Vec<(&'static str, u64)>, &'static str, Vec<&'static str>, u64, &'static str);
        let table: Vec<Row> = vec![
            (
                "gh with its token in hosts.yml",
                Platform::Darwin,
                vec![("~/.config/gh/hosts.yml", 200)],
                "logins/gh",
                vec!["~/.config/gh/hosts.yml"],
                200,
                "skip",
            ),
            (
                "gcloud",
                Platform::Darwin,
                vec![
                    ("~/.config/gcloud/credentials.db", 4000),
                    ("~/.config/gcloud/configurations/config_default", 50),
                    ("~/.config/gcloud/logs/x.log", 9999),
                ],
                "logins/gcloud",
                vec!["~/.config/gcloud/credentials.db", "~/.config/gcloud/configurations"],
                4050,
                "skip",
            ),
            (
                "wrangler on macOS",
                Platform::Darwin,
                vec![("~/Library/Preferences/.wrangler/config/default.toml", 300)],
                "logins/wrangler",
                vec!["~/Library/Preferences/.wrangler/config/default.toml"],
                300,
                "skip",
            ),
            (
                "wrangler on Linux",
                Platform::Linux,
                vec![("~/.config/.wrangler/config/default.toml", 300)],
                "logins/wrangler",
                vec!["~/.config/.wrangler/config/default.toml"],
                300,
                "skip",
            ),
            (
                "cloudflared",
                Platform::Darwin,
                vec![("~/.cloudflared/cert.pem", 800)],
                "logins/cloudflared",
                vec!["~/.cloudflared/cert.pem"],
                800,
                "skip",
            ),
            (
                "vercel on macOS",
                Platform::Darwin,
                vec![("~/Library/Application Support/com.vercel.cli/auth.json", 100)],
                "logins/vercel",
                vec!["~/Library/Application Support/com.vercel.cli/auth.json"],
                100,
                "skip",
            ),
            (
                "vercel on Linux",
                Platform::Linux,
                vec![("~/.config/com.vercel.cli/auth.json", 100)],
                "logins/vercel",
                vec!["~/.config/com.vercel.cli/auth.json"],
                100,
                "skip",
            ),
            (
                "aws",
                Platform::Darwin,
                vec![("~/.aws/credentials", 120), ("~/.aws/config", 300), ("~/.aws/sso/cache/x.json", 900)],
                "logins/aws",
                vec!["~/.aws/credentials", "~/.aws/config"],
                420,
                "skip",
            ),
            ("kubectl", Platform::Darwin, vec![("~/.kube/config", 6000)], "logins/kube", vec!["~/.kube/config"], 6000, "bring"),
            (
                "Gemini CLI",
                Platform::Darwin,
                vec![("~/.gemini/oauth_creds.json", 500)],
                "logins/gemini",
                vec!["~/.gemini/oauth_creds.json"],
                500,
                "skip",
            ),
            (
                "OpenCode",
                Platform::Darwin,
                vec![("~/.local/share/opencode/auth.json", 200)],
                "logins/opencode",
                vec!["~/.local/share/opencode/auth.json"],
                200,
                "bring",
            ),
            (
                "Pi",
                Platform::Darwin,
                vec![("~/.pi/agent/auth.json", 900), ("~/.pi/agent/settings.json", 80)],
                "logins/pi",
                vec!["~/.pi/agent/auth.json"],
                900,
                "skip",
            ),
        ];
        assert_eq!(table.len(), 12);
        for (name, platform, files, id, paths, bytes, dflt) in table {
            let mut laptop = FakeLaptop::new().platform(platform);
            for (path, size) in &files {
                laptop = laptop.file(path, *size);
            }
            let host = fake_host(laptop);
            let rows = block_on(detect_logins(&host));
            let mut got = rows_json(&rows);
            let arr = got.as_array_mut().unwrap_or_else(|| panic!("{name}: rows are an array"));
            assert_eq!(arr.len(), 1, "{name}");
            let row = arr[0].as_object_mut().unwrap();
            // expect.any(String), expect.any(String), expect.any(Number)
            assert!(row.remove("label").unwrap().is_string(), "{name}: label");
            assert!(row.remove("group").unwrap().is_string(), "{name}: group");
            assert!(row.get("bytes").unwrap().is_number(), "{name}: bytes");
            // Beyond the TS: the bytes are the found paths' sizes.
            assert_eq!(row.remove("bytes").unwrap(), json!(bytes), "{name}: bytes value");
            assert_eq!(got, json!([{ "rung": "logins", "id": id, "paths": paths, "default": dflt }]), "{name}");
        }
    }

    #[test]
    fn codex_is_found_by_its_file_and_offers_nothing_of_it() {
        let host = fake_host(FakeLaptop::new().file("~/.codex/auth.json", 900));
        assert_eq!(
            rows_json(&block_on(detect_logins(&host))),
            json!([{
                "rung": "logins", "id": "logins/codex", "label": "Codex login", "group": "Agent logins", "paths": [], "bytes": 0,
                "default": "skip", "detail": "it signs in once on the computer that runs your workspaces; nothing of it travels"
            }])
        );
    }

    #[test]
    fn hermes_agent_is_two_rows_its_device_login_and_the_keys_file_beside_it() {
        let host =
            fake_host(FakeLaptop::new().file("~/.hermes/.env", 25_000).file("~/.hermes/auth.json", 400).file("~/.hermes/config.yaml", 600));
        assert_eq!(
            rows_json(&block_on(detect_logins(&host))),
            json!([
                { "rung": "logins", "id": "logins/hermes", "label": "Hermes Agent login", "group": "Agent logins", "paths": ["~/.hermes/auth.json"], "bytes": 400, "default": "skip" },
                {
                    "rung": "logins", "id": "logins/hermes-keys", "label": "Hermes Agent API keys", "group": "Agent logins", "paths": ["~/.hermes/.env"],
                    "bytes": 25_000, "default": "bring", "detail": "the keys in ~/.hermes/.env travel only by copy; no sign-in produces them"
                }
            ])
        );
        assert_eq!(login_default("hermes"), RowDefault::Skip);
        assert_eq!(login_default("hermes-keys"), RowDefault::Bring);
        // Each row stands on its own file: keys with no login, a login with no keys.
        let keys_only = fake_host(FakeLaptop::new().file("~/.hermes/.env", 100));
        assert_eq!(ids(&block_on(detect_logins(&keys_only))), vec!["logins/hermes-keys"]);
        let login_only = fake_host(FakeLaptop::new().file("~/.hermes/auth.json", 100));
        assert_eq!(ids(&block_on(detect_logins(&login_only))), vec!["logins/hermes"]);
    }

    #[test]
    fn every_login_rows_default_follows_its_catalog_entrys_sign_in_kind() {
        let travels = |k: SignInKind| k != SignInKind::Oauth && k != SignInKind::Device && k != SignInKind::Token;
        let want = |k: SignInKind| if travels(k) { RowDefault::Bring } else { RowDefault::Skip };
        for e in catalog() {
            assert_eq!(login_default(&login_id_of(e.id())), want(e.sign_in().kind()), "{}", e.id());
        }
        for r in login_rows() {
            assert_eq!(login_default(&r.id), want(r.sign_in.kind()), "{}", r.id);
        }
        let kubectl = catalog().iter().find(|e| e.id() == "kubectl");
        assert_eq!(kubectl.map(|e| e.sign_in().kind()), Some(SignInKind::None));
        assert_eq!(login_default("kube"), RowDefault::Bring);
        assert_eq!(login_default("opencode"), RowDefault::Bring);
        assert_eq!(login_default("some-new-tool"), RowDefault::Bring);
    }

    #[test]
    fn claude_codes_login_is_found_here_and_nothing_of_it_is_offered_to_copy() {
        let mac = fake_host(FakeLaptop::new().exec("security find-generic-password -s Claude Code-credentials", "keychain: ...\n"));
        assert_eq!(rows_json(&block_on(detect_logins(&mac))), json!([claude_row()]));
        let linux = fake_host(FakeLaptop::new().platform(Platform::Linux).file("~/.claude/.credentials.json", CLAUDE_LOGIN));
        assert_eq!(rows_json(&block_on(detect_logins(&linux))), json!([claude_row()]));
    }

    #[test]
    fn finds_claude_codes_and_codexs_logins_where_claude_config_dir_and_codex_home_put_them() {
        let dir = "/home/me/.claude-cfg";
        // The Keychain item's name carries the first eight hex of the folder's sha256, as Claude Code names it.
        let digest = format!("{:x}", Sha256::digest(dir.as_bytes()));
        assert_eq!(claude_keychain_service(Some(dir)), format!("Claude Code-credentials-{}", &digest[..8]));
        assert_eq!(claude_keychain_service(None), "Claude Code-credentials");
        let service = format!("Claude Code-credentials-{}", &digest[..8]);
        let mac = |key: &str| {
            let host = with_dirs(FakeLaptop::new().exec(key, "keychain: ...\n"), Some(dir), None);
            ids(&block_on(detect_logins(&host)))
        };
        assert_eq!(mac(&format!("security find-generic-password -s {service}")), vec!["logins/claude"]);
        assert_eq!(mac("security find-generic-password -s Claude Code-credentials"), Vec::<String>::new());
        let linux = |files: &[(&str, &str)]| {
            let mut laptop = FakeLaptop::new().platform(Platform::Linux);
            for (path, text) in files {
                laptop = laptop.file(path, *text);
            }
            let host = with_dirs(laptop, Some(dir), Some("/home/me/.codex-own"));
            let mut got = ids(&block_on(detect_logins(&host)));
            got.sort();
            got
        };
        assert_eq!(
            linux(&[("/home/me/.claude-cfg/.credentials.json", CLAUDE_LOGIN), ("/home/me/.codex-own/auth.json", "x".repeat(300).as_str())]),
            vec!["logins/claude", "logins/codex"]
        );
        assert_eq!(
            linux(&[("~/.claude/.credentials.json", CLAUDE_LOGIN), ("~/.codex/auth.json", "x".repeat(300).as_str())]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn on_linux_claude_code_is_signed_in_only_where_its_credentials_file_holds_its_own_login() {
        let claude_row_ids = |text: &str| {
            let host = fake_host(FakeLaptop::new().platform(Platform::Linux).file("~/.claude/.credentials.json", text));
            ids(&block_on(detect_logins(&host)))
        };
        assert_eq!(claude_row_ids(CLAUDE_LOGIN), vec!["logins/claude"]);
        assert_eq!(claude_row_ids(r#"{"mcpOAuth":{"acme|0a1b":{"accessToken":"placeholder"}}}"#), Vec::<String>::new());
        assert_eq!(claude_row_ids(r#"{"claudeAiOauth":{"accessToken":""}}"#), Vec::<String>::new());
        assert_eq!(claude_row_ids("not json"), Vec::<String>::new());
    }

    #[test]
    fn claude_code_with_every_key_source_here_still_travels_nothing() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.zshrc", "export A=1\nexport ANTHROPIC_API_KEY=sk-ant-x\n")
                .file(
                    "~/.claude/settings.json",
                    r#"{"apiKeyHelper": "security find-generic-password -s anthropic-api-key -w", "model": "opus"}"#,
                )
                .exec("security find-generic-password -s Claude Code-credentials", "keychain: ...\n"),
        );
        assert_eq!(rows_json(&block_on(detect_logins(&host))), json!([claude_row()]));
    }

    #[test]
    fn an_api_key_helper_or_an_exported_key_here_is_a_claude_row_with_nothing_to_copy() {
        let helper = fake_host(
            FakeLaptop::new()
                .file("~/.claude/settings.json", r#"{"apiKeyHelper": "security find-generic-password -s anthropic-api-key -w"}"#),
        );
        assert_eq!(rows_json(&block_on(detect_logins(&helper))), json!([claude_row()]));
        let exported = fake_host(
            FakeLaptop::new().platform(Platform::Linux).file("~/.bashrc", "ANTHROPIC_API_KEY=sk-ant-x; export ANTHROPIC_API_KEY\n"),
        );
        assert_eq!(rows_json(&block_on(detect_logins(&exported))), json!([claude_row()]));
        // A settings.json without a helper, or one that is not JSON, is no source.
        let no_helper = fake_host(FakeLaptop::new().file("~/.claude/settings.json", r#"{"model": "opus"}"#));
        assert_eq!(rows_json(&block_on(detect_logins(&no_helper))), json!([]));
        let not_json = fake_host(FakeLaptop::new().file("~/.claude/settings.json", "{ not json"));
        assert_eq!(rows_json(&block_on(detect_logins(&not_json))), json!([]));
    }

    #[test]
    fn gh_with_its_token_in_the_macos_keychain_carries_that_item_as_a_second_path() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.config/gh/hosts.yml", 200)
                .exec("security find-generic-password -s gh:github.com", "keychain: ...\n"),
        );
        assert_eq!(
            rows_json(&block_on(detect_logins(&host))),
            json!([{
                "rung": "logins", "id": "logins/gh", "label": "GitHub CLI login", "group": "CLI logins",
                "paths": ["~/.config/gh/hosts.yml", "Keychain: gh:github.com"], "bytes": 200, "default": "skip"
            }])
        );
    }

    #[test]
    fn is_labelled_by_name_any_sentence_about_it_is_a_detail() {
        let table: Vec<(&str, FakeLaptop, &str)> = vec![
            (
                "gh with its token in the Keychain",
                FakeLaptop::new()
                    .file("~/.config/gh/hosts.yml", 200)
                    .exec("security find-generic-password -s gh:github.com", "keychain: ...\n"),
                "GitHub CLI login",
            ),
            ("aws", FakeLaptop::new().file("~/.aws/credentials", 120).file("~/.aws/config", 300), "AWS keys and profiles"),
        ];
        assert_eq!(table.len(), 2);
        for (name, laptop, label) in table {
            let host = fake_host(laptop);
            let rows = block_on(detect_logins(&host));
            let row = rows.first().unwrap_or_else(|| panic!("{name}: a row"));
            assert_eq!(row.label, label, "{name}");
            assert_eq!(row.reason, None, "{name}");
            assert!(!row.label.contains('(') && !row.label.contains(')'), "{name}");
            assert!(row.label.chars().count() <= 40, "{name}");
        }
    }

    #[test]
    fn onepassword_cli_is_listed_locked_off() {
        let host = fake_host(FakeLaptop::new().which(&["op"]));
        assert_eq!(
            rows_json(&block_on(detect_logins(&host))),
            json!([{
                "rung": "logins", "id": "logins/op", "label": "1Password CLI", "group": "CLI logins", "paths": [], "bytes": 0, "default": "skip",
                "reason": "needs the 1Password desktop app; the machine uses a service account token"
            }])
        );
    }

    #[test]
    fn never_reads_a_login_file_and_never_asks_the_keychain_for_a_secret() {
        // Only the rc files and Claude's settings are read, for names.
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.config/gh/hosts.yml", "oauth_token: x")
                .file("~/.codex/auth.json", "{}")
                .file("~/.aws/credentials", "k")
                .file("~/.hermes/.env", "ANTHROPIC_API_KEY=sk-ant-x")
                .file("~/.pi/agent/auth.json", "{}")
                .file("~/.zshrc", "export A=1\n")
                .exec("security find-generic-password -s Claude Code-credentials", "x"),
        );
        block_on(detect_logins(&host));
        let calls = host.calls();
        let prefix = "read /Users/dev/";
        let reads: Vec<&str> =
            calls.iter().filter(|c| c.starts_with("read")).map(|c| c.strip_prefix(prefix).unwrap_or(c.as_str())).collect();
        assert!(reads.contains(&".zshrc"), "{reads:?}");
        let rc = Regex::new(r"^\.[a-z_]+$").unwrap();
        assert!(reads.iter().all(|r| *r == ".claude/settings.json" || r.starts_with(".config/fish/") || rc.is_match(r)), "{reads:?}");
        assert_eq!(calls.iter().filter(|c| c.contains(" -w")).count(), 0);
    }

    #[test]
    fn no_logins_on_an_empty_laptop() {
        assert_eq!(rows_json(&block_on(detect_logins(&empty_host()))), json!([]));
    }
}

/// Rules of spec/logins.md the TS cases leave unpinned.
mod pins {
    use super::*;
    use wsp_collect::detect::logins::{api_key_helper_of, billed_here, holds_claude_login};

    #[test]
    fn rows_come_in_table_order_gh_first_claude_last() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.hermes/.env", 10)
                .file("~/.hermes/auth.json", 10)
                .file("~/.aws/config", 10)
                .file("~/.config/gcloud/active_config", 10)
                .file("~/.config/gh/hosts.yml", 10)
                .which(&["op"])
                .exec("security find-generic-password -s Claude Code-credentials", "x"),
        );
        let ids: Vec<String> = block_on(detect_logins(&host)).into_iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            vec!["logins/gh", "logins/gcloud", "logins/aws", "logins/hermes", "logins/hermes-keys", "logins/op", "logins/claude"]
        );
    }

    #[test]
    fn gh_on_linux_asks_no_keychain() {
        let host = fake_host(
            FakeLaptop::new()
                .platform(Platform::Linux)
                .file("~/.config/gh/hosts.yml", 200)
                .exec("security find-generic-password -s gh:github.com", "keychain: ...\n"),
        );
        let rows = block_on(detect_logins(&host));
        assert_eq!(
            rows_json(&rows),
            json!([{"rung": "logins", "id": "logins/gh", "label": "GitHub CLI login", "group": "CLI logins", "paths": ["~/.config/gh/hosts.yml"], "bytes": 200, "default": "skip"}])
        );
        assert!(host.calls().iter().all(|c| !c.starts_with("run security")), "{:?}", host.calls());
    }

    #[test]
    fn claude_keychain_service_names_the_config_folder_by_its_nfc_sha256() {
        assert_eq!(claude_keychain_service(None), "Claude Code-credentials");
        let nfc = "/Users/dev/caf\u{e9}";
        let hex: String = Sha256::digest(nfc.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let want = format!("Claude Code-credentials-{}", &hex[..8]);
        assert_eq!(claude_keychain_service(Some(nfc)), want);
        // The decomposed spelling of the same folder names the same item.
        assert_eq!(claude_keychain_service(Some("/Users/dev/cafe\u{301}")), want);
    }

    #[test]
    fn api_key_helper_of_reads_a_non_blank_string_helper_only() {
        assert_eq!(api_key_helper_of(None), None);
        for text in ["not json", "null", "[]", r#"{"apiKeyHelper":"  "}"#, r#"{"apiKeyHelper":5}"#, "{}"] {
            assert_eq!(api_key_helper_of(Some(text)), None, "{text}");
        }
        // The helper comes back as written, its blanks kept.
        assert_eq!(api_key_helper_of(Some(r#"{"apiKeyHelper":" ~/bin/key "}"#)), Some(" ~/bin/key ".to_string()));
    }

    #[test]
    fn holds_claude_login_wants_a_non_empty_access_token() {
        assert!(!holds_claude_login(None));
        for text in [
            "null",
            "not json",
            r#"{"claudeAiOauth":{"accessToken":""}}"#,
            r#"{"mcpOAuth":{"x":{"accessToken":"t"}}}"#,
            r#"{"claudeAiOauth":{"accessToken":5}}"#,
            r#"{"claudeAiOauth":null}"#,
        ] {
            assert!(!holds_claude_login(Some(text)), "{text}");
        }
        assert!(holds_claude_login(Some(CLAUDE_LOGIN)));
    }

    #[test]
    fn billed_here_names_claude_only_when_it_bills_here() {
        let signed = fake_host(FakeLaptop::new().platform(Platform::Linux).file("~/.claude/.credentials.json", CLAUDE_LOGIN));
        assert_eq!(serde_json::to_value(block_on(billed_here(&signed))).unwrap(), json!({"claude": "login"}));
        let keyed = fake_host(FakeLaptop::new().platform(Platform::Linux).file("~/.zshrc", "export ANTHROPIC_API_KEY=sk-x\n"));
        assert_eq!(serde_json::to_value(block_on(billed_here(&keyed))).unwrap(), json!({"claude": "key"}));
        assert_eq!(serde_json::to_value(block_on(billed_here(&empty_host()))).unwrap(), json!({}));
    }
}
