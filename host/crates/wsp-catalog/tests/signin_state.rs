// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/signin-state.test.ts.
use std::collections::HashSet;

use wsp_catalog::catalog::{catalog, catalog_agents, catalog_entry, login_sign_in};
use wsp_catalog::roads::GUEST_HOME;
use wsp_catalog::signin::{has_login, key_env_of, lives_on_computer, login_home_in, login_state_paths, mints_token, no_sign_in, shared_login_of, token_in, SharedLogin, SignInKind};
use wsp_catalog::signin_rows::{shares_in, sign_in_row, Share, GH_LOGIN_PATHS, LOGIN_PATHS, NEVER_IN_IMAGE, NEVER_ON_BROKERED, SHARED_LOGINS, SIGN_IN_ROWS, VAULT_VARIABLES};

fn set(items: impl IntoIterator<Item = String>) -> HashSet<String> {
    items.into_iter().collect()
}

mod sign_in_state_on_the_machine {
    use super::*;

    #[test]
    fn every_row_with_a_login_or_a_source_names_at_least_one_path_home_relative_and_wit() {
        for (id, row) in SIGN_IN_ROWS.iter() {
            // A login that lives on the computer that runs the workspaces keeps no state on any machine.
            if (has_login(row).is_none() && row.sources().is_empty()) || lives_on_computer(row) {
                continue;
            }
            let paths = row.state_on_machine();
            assert!(!paths.is_empty(), "{id} names no state on the machine");
            for p in paths {
                assert!(!p.starts_with('/'), "{id}'s {p} is not home-relative");
                assert!(!p.starts_with('~'), "{id}'s {p} is not home-relative");
                assert!(!p.split('/').any(|part| part == ".."), "{id}'s {p} climbs out of the home");
            }
        }
    }

    #[test]
    fn loginstatepaths_answers_guest_paths_under_the_guest_home_and_nothing_for_an_entr() {
        assert_eq!(login_state_paths(catalog_entry("gh").unwrap().sign_in()), vec![format!("{GUEST_HOME}/.config/gh/hosts.yml")]);
        assert_eq!(login_state_paths(catalog_entry("gemini").unwrap().sign_in()), vec![format!("{GUEST_HOME}/.gemini/oauth_creds.json")]);
        assert_eq!(login_state_paths(&no_sign_in()), Vec::<String>::new());
        for entry in catalog() {
            for p in login_state_paths(entry.sign_in()) {
                assert!(p.starts_with(&format!("{GUEST_HOME}/")), "{p}");
            }
        }
    }

    #[test]
    fn claude_s_login_and_codex_s_have_no_state_on_the_machine_one_is_a_token_held_on_t() {
        assert_eq!(login_state_paths(catalog_entry("claude").unwrap().sign_in()), Vec::<String>::new());
        assert_eq!(login_state_paths(catalog_entry("codex").unwrap().sign_in()), Vec::<String>::new());
    }
}

mod the_token_sign_in_and_the_vault {
    use super::*;

    #[test]
    fn claude_s_row_mints_a_token_on_this_computer_and_nothing_of_it_travels() {
        let s = sign_in_row("claude");
        assert_eq!(s.kind(), SignInKind::Token);
        let token = mints_token(s).unwrap();
        assert!(has_login(s).is_none());
        assert_eq!(token.mint, "claude setup-token");
        assert_eq!(token.token_env, "CLAUDE_CODE_OAUTH_TOKEN");
        assert!(token.token.is_match("sk-ant-oat01-TESTONLYaaaaaaaaaaaaaaaaaaaa"));
        assert!(!token.token.is_match("not-a-token"));
        assert!(s.sources().is_empty());
        assert!(s.state_on_machine().is_empty());
    }

    #[test]
    fn codex_s_login_lives_on_the_computer_that_runs_the_workspaces_and_is_never_copied() {
        let s = sign_in_row("codex");
        assert!(lives_on_computer(s));
        assert!(s.sources().is_empty());
        assert!(s.state_on_machine().is_empty());
        assert!(!lives_on_computer(sign_in_row("gh")));
    }

    #[test]
    fn the_key_variable_a_row_reads_is_written_once_and_a_row_that_reads_none_says_so() {
        assert_eq!(key_env_of(sign_in_row("claude")), Some("ANTHROPIC_API_KEY"));
        assert_eq!(key_env_of(sign_in_row("codex")), Some("OPENAI_API_KEY"));
        assert_eq!(key_env_of(sign_in_row("gemini")), Some("GEMINI_API_KEY"));
        assert_eq!(key_env_of(sign_in_row("gh")), None);
        assert_eq!(key_env_of(&no_sign_in()), None);
    }

    #[test]
    fn the_token_in_a_paste_is_the_whole_of_it_and_nothing_around_it_trimmed() {
        let row = mints_token(sign_in_row("claude")).unwrap();
        let token = "sk-ant-oat01-TESTONLYaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(token_in(row, &format!("  {token}\n")), Some(token.to_string()));
        // A line copied with the tool's own words around it is not a token, so nothing of it is saved.
        assert_eq!(token_in(row, &format!("Paste code here if prompted > {token}")), None);
        assert_eq!(token_in(row, &format!("{token} # my token")), None);
        assert_eq!(token_in(row, "my password"), None);
        assert_eq!(token_in(row, ""), None);
    }

    #[test]
    fn a_manifest_s_login_row_reads_its_catalog_sign_in_by_one_rule_with_or_without_the() {
        assert_eq!(login_sign_in("logins/claude"), Some(sign_in_row("claude")));
        assert_eq!(login_sign_in("claude"), Some(sign_in_row("claude")));
        // The collector files kubectl's login under its kubeconfig, which is the name its row carries.
        assert_eq!(login_sign_in("logins/kube"), Some(sign_in_row("kubectl")));
        assert_eq!(login_sign_in("logins/nothing-here"), None);
        assert_eq!(login_sign_in("agents/claude"), None);
    }

    #[test]
    fn the_vault_holds_the_agents_token_and_key_variables_and_nothing_else() {
        let mut vault: Vec<String> = VAULT_VARIABLES.iter().cloned().collect();
        vault.sort();
        assert_eq!(vault, vec!["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "CURSOR_API_KEY", "GEMINI_API_KEY", "OPENAI_API_KEY"]);
        let mut declared = HashSet::new();
        for a in catalog_agents() {
            if let Some(t) = mints_token(&a.sign_in) {
                declared.insert(t.token_env.clone());
            }
            if let Some(k) = key_env_of(&a.sign_in) {
                declared.insert(k.to_string());
            }
        }
        for name in VAULT_VARIABLES.iter() {
            assert!(declared.contains(name), "{name} is in the vault but no agent row declares it");
        }
    }

    #[test]
    fn the_paths_a_builder_may_never_hold_are_absolute_guest_paths_the_claude_credentia() {
        assert_eq!(*NEVER_IN_IMAGE, vec![format!("{GUEST_HOME}/.claude-cfg/.credentials.json"), format!("{GUEST_HOME}/.claude-cfg/anthropic-api-key"), format!("{GUEST_HOME}/.codex/auth.json")]);
        for p in NEVER_IN_IMAGE.iter() {
            assert!(p.starts_with(&format!("{GUEST_HOME}/")), "{p}");
        }
        // The shared login's own path is not spelled twice: what a workspace reads it at is what may not be sealed.
        for shared in SHARED_LOGINS.iter() {
            assert!(NEVER_IN_IMAGE.contains(&shared.target), "{}", shared.target);
        }
    }

    #[test]
    fn the_login_paths_a_provider_s_nap_and_snapshot_read_are_every_row_s_state_every_p() {
        let rows: Vec<String> = SIGN_IN_ROWS.values().flat_map(login_state_paths).collect();
        let expected = set(NEVER_IN_IMAGE.iter().cloned().chain(rows).chain(["/etc/profile.d/wsp-secrets.sh".to_string(), "/etc/fish/conf.d/wsp-secrets.fish".to_string()]));
        assert_eq!(set(LOGIN_PATHS.iter().cloned()), expected);
        assert_eq!(LOGIN_PATHS.len(), set(LOGIN_PATHS.iter().cloned()).len());
        for p in [format!("{GUEST_HOME}/.config/gh/hosts.yml"), format!("{GUEST_HOME}/.config/gcloud"), format!("{GUEST_HOME}/.aws"), format!("{GUEST_HOME}/.hermes/.env")] {
            assert!(LOGIN_PATHS.contains(&p), "{p}");
        }
    }

    #[test]
    fn what_a_provider_s_machine_is_never_sealed_holding_is_every_login_path_claude_s_o() {
        let expected = set(LOGIN_PATHS.iter().cloned().chain(["/root/.mcp-auth".to_string(), "/root/.git-credentials".to_string()]));
        assert_eq!(set(NEVER_ON_BROKERED.iter().cloned()), expected);
        assert_eq!(NEVER_ON_BROKERED.len(), set(NEVER_ON_BROKERED.iter().cloned()).len());
        for p in ["/root/.claude-cfg/.credentials.json", "/root/.claude-cfg/anthropic-api-key", "/root/.codex/auth.json", "/etc/profile.d/wsp-secrets.sh", "/etc/fish/conf.d/wsp-secrets.fish"] {
            assert!(NEVER_ON_BROKERED.contains(&p.to_string()), "{p}");
        }
        assert_eq!(*GH_LOGIN_PATHS, vec!["/root/.config/gh/hosts.yml".to_string()]);
    }
}

mod a_login_signed_in_once_on_the_computer_that_runs_the_workspaces {
    use super::*;

    #[test]
    fn codex_declares_the_file_it_shares_where_the_tool_reads_it_inside_and_the_variabl() {
        let shared = shared_login_of(sign_in_row("codex"));
        let want = SharedLogin { dir: "codex".to_string(), file: "auth.json".to_string(), target: format!("{GUEST_HOME}/.codex/auth.json"), home_env: "CODEX_HOME".to_string() };
        assert_eq!(shared, Some(&want));
        // The predicate and the declaration are one thing: a row that shares nothing lives in the image as before.
        assert!(lives_on_computer(sign_in_row("codex")));
        assert_eq!(shared_login_of(sign_in_row("gh")), None);
        assert_eq!(shared_login_of(sign_in_row("claude")), None);
        assert_eq!(*SHARED_LOGINS, vec![want]);
    }

    #[test]
    fn what_a_create_shares_into_a_workspace_is_that_computer_s_own_file_at_the_path_th() {
        assert_eq!(shares_in("/wsp/logins"), vec![Share { source: "/wsp/logins/codex/auth.json".to_string(), target: format!("{GUEST_HOME}/.codex/auth.json") }]);
        // The sign-in on that computer runs with the tool's store pointed here, which is where the shared file lands.
        assert_eq!(login_home_in("/wsp/logins", shared_login_of(sign_in_row("codex")).unwrap()), "/wsp/logins/codex");
        for share in shares_in("/wsp/logins") {
            assert!(share.source.starts_with("/wsp/logins/"), "{}", share.source);
        }
    }
}
