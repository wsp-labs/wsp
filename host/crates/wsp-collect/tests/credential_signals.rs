// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/credential-signals.test.ts.
mod common;

use common::fake_host::block_on;
use wsp_collect::credential_signals::{
    bare_urls, file_signals, keys_signal, mode_signal, name_signal, pem_signal, top_level_keys, url_signal, BareUrls, FileShape, Signal,
};

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn bare(text: &str, urls: &[&str]) -> BareUrls {
    BareUrls { text: text.to_string(), urls: strs(urls) }
}

mod credential_shape {
    use super::*;

    #[test]
    fn name_patterns_token_and_secret_as_whole_words_env_files_npmrc_pypirc_git_credent() {
        let yes = [
            "credentials.db",
            ".credentials.json",
            "auth.json",
            "hosts.yml",
            "access-token",
            "server.pem",
            "atuin.key",
            "key",
            "id_rsa",
            ".netrc",
            "token.txt",
            "my_secret",
            "stored_tokens",
            "access_tokens.db",
            ".secrets",
            ".env",
            ".env.local",
            ".env.production",
            ".npmrc",
            ".pypirc",
            ".git-credentials",
            "7a66_tokens.json",
            "client_secret.json",
        ];
        let got: Vec<bool> = yes.iter().map(|n| name_signal(n)).collect();
        assert_eq!(got, vec![true; yes.len()]);
        let no = [
            "credentials.go",
            "auth.json.example",
            "id_rsa.pub",
            "token.ts",
            "config.yml",
            "settings.json",
            "README.md",
            "tokenizer.json",
            "tokenizer_config.json",
            "known_hosts",
            ".viminfo",
            ".CFUserTextEncoding",
            ".z",
            "zsh_history",
            ".env.example",
            ".envrc",
            "secretary.txt",
        ];
        let got: Vec<bool> = no.iter().map(|n| name_signal(n)).collect();
        assert_eq!(got, vec![false; no.len()]);
    }

    #[test]
    fn mode_owner_only_and_under_100_kb_never_empty() {
        assert!(mode_signal(FileShape { mode: 0o600, bytes: 100 }));
        assert!(mode_signal(FileShape { mode: 0o400, bytes: 100 }));
        assert!(!mode_signal(FileShape { mode: 0o644, bytes: 100 }));
        assert!(!mode_signal(FileShape { mode: 0o600, bytes: 0 }));
        assert!(!mode_signal(FileShape { mode: 0o600, bytes: 200 * 1024 }));
    }

    #[test]
    fn key_names_at_any_depth_of_json_yaml_toml_and_key_value_files() {
        assert_eq!(top_level_keys(r#"{"token":"x","user":{"password":"y"}}"#), strs(&["token", "user", "password"]));
        assert_eq!(
            top_level_keys("github.com:\n    oauth_token: x\nclient-secret: y\n"),
            strs(&["github.com", "oauth_token", "client-secret"])
        );
        assert_eq!(top_level_keys("[registry]\napi_key = \"x\"\n"), strs(&["api_key"]));
        assert_eq!(top_level_keys("//registry.npmjs.org/:_authToken=x\nregistry=https://r\n"), strs(&["_authToken", "registry"]));
        assert_eq!(top_level_keys("// settings\n{\n  \"theme\": \"dark\",\n  \"vim_mode\": true,\n}\n"), strs(&["theme", "vim_mode"]));
        assert!(keys_signal(r#"{"access_token":"x"}"#));
        assert!(keys_signal("client-secret: y\n"));
        assert!(keys_signal(r#"{"claudeAiOauth":{"accessToken":"x"}}"#));
        assert!(keys_signal(r#"{"auths":{}}"#));
        assert!(keys_signal("ANTHROPIC_API_KEY=sk-x\n"));
        assert!(keys_signal("//registry.npmjs.org/:_authToken=x\n"));
        assert!(!keys_signal(r#"{"theme":"dark","editor":{"fontSize":13}}"#));
        assert!(!keys_signal(r#"{"model_max_length":512}"#));
        assert!(!keys_signal(r#"{"os_crypt":{"encrypted_key":"x"},"sort_key":1,"max_tokens":3,"key":"pub"}"#));
        assert!(!keys_signal("sort_key=46\nhide_kernel_threads=1\n"));
        assert!(!keys_signal(r#"{"packages":{"node_modules/password-prompt":{"version":"1"}}}"#));
        assert!(!keys_signal(r#"["token"]"#));
        let crlf = "API_KEY=sk-live-000fake\r\nPASSWORD=hunter2\r\n";
        assert_eq!(top_level_keys(crlf), strs(&["API_KEY", "PASSWORD"]));
        assert!(keys_signal(crlf));
        assert!(keys_signal("github.com:\r\n    oauth_token: ghp_000fake\r\n"));
        assert!(keys_signal("[registry]\r\napi_key = \"xaat-000-fake\"\r\n"));
        assert!(!keys_signal("api_key = \"${AXIOM_TOKEN}\"\r\n"));
    }

    #[test]
    fn a_value_that_points_at_a_secret_is_not_one_a_workflows_template_reference_a_comp() {
        let workflow = "name: ci\njobs:\n  build:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          token: ${{ secrets.GITHUB_TOKEN }}\n";
        assert!(!keys_signal(workflow));
        for reference in [
            "password: ${DB_PASSWORD}\n",
            "api_key = \"${AXIOM_TOKEN}\"\n",
            "SECRET_KEY=$SECRET_KEY\n",
            "API_TOKEN=<your-token>\n",
            "API_TOKEN=\n",
            r#"{"token":""}"#,
            r#"{"token":"${TOKEN}"}"#,
            "access_key = \"${AWS_ACCESS_KEY_ID:-none}\"\n",
            "api_key = \"${AXIOM_TOKEN}\" # set in CI\n",
            "api_key: ${AXIOM_TOKEN} # prod\n",
            "token: ${{ secrets.X }}   # ci\n",
            "password: # not set\n",
        ] {
            assert!(!keys_signal(reference), "{reference:?}");
        }
        for held in [
            "API_KEY=sk-live-000fake\n",
            "api_key = \"sk-live-000fake\"\n",
            r#"{"token":"sk-ant-x"}"#,
            "password: hunter2\n",
            "API_KEY=abc#def\n",
            "password: hunter2 # note\n",
        ] {
            assert!(keys_signal(held), "{held:?}");
        }
        let compose = "services:\n  api:\n    environment:\n      DATABASE_URL: postgres://u:${PW}@db:5432/app\n";
        assert!(!url_signal(compose));
        assert_eq!(bare_urls(compose), bare(compose, &[]));
        for reference in [
            "DATABASE_URL=postgres://u:$PW@db:5432/app\n",
            "DATABASE_URL=postgres://u:<password>@db:5432/app\n",
            "REDIS_URL=redis://:${REDIS_PASSWORD}@cache:6379\n",
        ] {
            assert!(!url_signal(reference), "{reference:?}");
            assert_eq!(bare_urls(reference), bare(reference, &[]), "{reference:?}");
        }
        assert!(url_signal("DATABASE_URL=postgres://u:s3cr3t@db:5432/app\n"));
        let read = |name: &str, text: &str| {
            block_on(file_signals(name, FileShape { bytes: text.len() as u64, mode: 0o644 }, || async { Some(text.to_string()) }, false))
        };
        assert_eq!(read("ci.yml", workflow), None);
        assert_eq!(read("docker-compose.yml", compose), None);
        assert_eq!(read("vector.toml", "[sinks.axiom]\napi_key = \"${AXIOM_TOKEN}\"\n"), None);
        assert_eq!(read(".env", "API_KEY=sk-live-000fake\n"), Some(vec![Signal::Name, Signal::Keys]));
        assert_eq!(read(".env", "API_KEY=sk-live-000fake\r\n"), Some(vec![Signal::Name, Signal::Keys]));
    }

    #[test]
    fn pem_header() {
        assert!(pem_signal("-----BEGIN OPENSSH PRIVATE KEY-----\nx\n"));
        assert!(pem_signal("-----BEGIN RSA PRIVATE KEY-----\n"));
        assert!(!pem_signal("-----BEGIN CERTIFICATE-----\n"));
        assert!(!pem_signal("ssh-ed25519 AAAA\n"));
    }

    #[test]
    fn url_credentials_a_password_in_the_userinfo_anywhere_in_the_text_a_username_alone() {
        let config = "[remote \"origin\"]\n\turl = https://x-access-token:ghp_fake_token_000@github.com/example/proj.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[http]\n\tproxy = http://dev:fakepw@proxy.local:3128\n";
        assert!(url_signal(config));
        assert_eq!(
            bare_urls(config),
            bare(
                "[remote \"origin\"]\n\turl = https://github.com/example/proj.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[http]\n\tproxy = http://proxy.local:3128\n",
                &["https://github.com/example/proj.git", "http://proxy.local:3128"],
            )
        );
        for clean in [
            "url = https://github.com/example/proj.git\n",
            "url = https://dev@github.com/example/proj.git\n",
            "url = git@github.com:example/proj.git\n",
            "url = ssh://git@github.com/example/proj.git\n",
            "listen = http://localhost:8080/@me\n",
        ] {
            assert!(!url_signal(clean), "{clean:?}");
            assert_eq!(bare_urls(clean), bare(clean, &[]), "{clean:?}");
        }
        assert_eq!(
            bare_urls("url = \"https://dev:fakepw@github.com/example/proj.git\"\n"),
            bare("url = \"https://github.com/example/proj.git\"\n", &["https://github.com/example/proj.git"])
        );
        for empty in ["url = https://:ghp_fake_token_000@github.com/o/r\n", "REDIS_URL=redis://:fakepw@cache.local:6379\n"] {
            assert!(url_signal(empty), "{empty:?}");
        }
        assert_eq!(
            bare_urls("url = https://:ghp_fake_token_000@github.com/o/r\n"),
            bare("url = https://github.com/o/r\n", &["https://github.com/o/r"])
        );
        let read = |text: &str| {
            block_on(file_signals(
                "config",
                FileShape { bytes: text.len() as u64, mode: 0o644 },
                || async { Some(text.to_string()) },
                false,
            ))
        };
        assert_eq!(read(config), Some(vec![Signal::Url]));
        assert_eq!(read("[remote \"origin\"]\n\turl = https://github.com/example/proj.git\n[credential]\n\thelper = osxkeychain\n"), None);
        assert_eq!(read("https://dev:ghp_fake@github.com\n"), Some(vec![Signal::Url]));
    }
}

/// Every case of tests/fixtures/oracle/w2/credential_signals.json (oracle/w2-credential_signals.oracle.ts): spec
/// credential_signals's wave-2 rules.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, w2};
    use serde_json::{json, Value};
    use std::cell::Cell;

    fn text(case: &Value) -> &str {
        case["args"][0].as_str().unwrap()
    }

    #[test]
    fn every_name_mode_and_text_rule_answers_what_the_ts_answered() {
        let f = w2("credential_signals");
        for case in cases(&f, "name_signal") {
            assert_eq!(json!(name_signal(text(case))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "mode_signal") {
            let e = FileShape { bytes: case["args"][0]["bytes"].as_u64().unwrap(), mode: case["args"][0]["mode"].as_u64().unwrap() as u32 };
            assert_eq!(json!(mode_signal(e)), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "top_level_keys") {
            assert_eq!(json!(top_level_keys(text(case))), case["out"], "{}", case["name"]);
        }
        for key in ["keys_signal", "key_name_signal"] {
            for case in cases(&f, key) {
                assert_eq!(json!(keys_signal(text(case))), case["out"], "{key}: {}", case["name"]);
            }
        }
        for case in cases(&f, "pem_signal") {
            assert_eq!(json!(pem_signal(text(case))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "url_signal") {
            assert_eq!(json!(url_signal(text(case))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "bare_urls") {
            let b = bare_urls(text(case));
            assert_eq!(json!({ "text": b.text, "urls": b.urls }), case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn file_signals_reads_the_file_only_when_the_ts_did() {
        for case in cases(&w2("credential_signals"), "file_signals") {
            let a = &case["args"];
            let e = FileShape { bytes: a[1]["bytes"].as_u64().unwrap(), mode: a[1]["mode"].as_u64().unwrap() as u32 };
            let reads = Cell::new(0);
            let body = a[2].as_str().map(str::to_string);
            let read = || {
                reads.set(reads.get() + 1);
                let body = body.clone();
                async move { body }
            };
            let got = block_on(file_signals(a[0].as_str().unwrap(), e, read, a[3].as_bool().unwrap()));
            assert_eq!(json!({ "signals": got, "reads": reads.get() }), case["out"], "{}", case["name"]);
        }
    }
}
