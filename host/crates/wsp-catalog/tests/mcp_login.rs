// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp-login.test.ts.
use wsp_catalog::catalog::server_sign_in_road;
use wsp_catalog::mcp_login::{brokered_server_sign_in_line, CopyWhy, McpLogin, PtyFinish, ServerSignInRoad, BROKERED_SERVER_SIGN_IN_FIX};
use wsp_catalog::pending::{Kind, PageReach, SignInFinish, SignInRoad};
use wsp_catalog::signin::{sign_in_road_of, Headless, LoginSignIn, SignIn, SignInKind};
use wsp_catalog::signin_rows::sign_in_row;

fn pty(command: &str, finish: PtyFinish) -> ServerSignInRoad {
    ServerSignInRoad::Pty { command: command.to_string(), finish }
}

mod how_one_mcp_server_of_an_agent_is_signed_in {
    use super::*;

    #[test]
    fn is_each_harness_s_own_command_measured_off_its_help_or_the_line_its_own_session() {
        assert_eq!(McpLogin::Claude.command("notion"), Some("claude mcp login 'notion'".to_string()));
        assert_eq!(McpLogin::Claude.pasted("notion"), Some("claude mcp login 'notion' --no-browser".to_string()));
        assert_eq!(McpLogin::Codex.command("notion"), Some("codex mcp login 'notion'".to_string()));
        assert_eq!(McpLogin::Opencode.command("notion"), Some("opencode mcp auth 'notion'".to_string()));
        assert_eq!(McpLogin::Gemini.inside("notion"), Some("/mcp auth notion".to_string()));
    }

    #[test]
    fn runs_its_browser_sign_in_where_the_page_comes_back_to_it_the_pasted_address_wher() {
        let road = |agent: &str, reach: PageReach| server_sign_in_road(agent, "notion", reach, false);
        // Here the harness opens this computer's browser; through the relay its page opens here and the redirect is carried back.
        for reach in [PageReach::Here, PageReach::Relay] {
            assert_eq!(road("claude", reach), Some(Ok(pty("claude mcp login 'notion'", PtyFinish::Callback))));
            assert_eq!(road("codex", reach), Some(Ok(pty("codex mcp login 'notion'", PtyFinish::Callback))));
            assert_eq!(road("opencode", reach), Some(Ok(pty("opencode mcp auth 'notion'", PtyFinish::Callback))));
        }
        // Where no forward reaches, Claude Code alone finishes by the address the browser landed on.
        assert_eq!(road("claude", PageReach::None), Some(Ok(pty("claude mcp login 'notion' --no-browser", PtyFinish::Code))));
        assert_eq!(road("codex", PageReach::None), Some(Ok(ServerSignInRoad::Copy { line: "codex mcp login 'notion'".to_string(), why: CopyWhy::Callback })));
        assert_eq!(road("opencode", PageReach::None), Some(Ok(ServerSignInRoad::Copy { line: "opencode mcp auth 'notion'".to_string(), why: CopyWhy::Callback })));
        match road("gemini", PageReach::Here) {
            Some(Ok(ServerSignInRoad::Copy { line, .. })) => assert_eq!(line, "/mcp auth notion"),
            other => panic!("gemini here: {other:?}"),
        }
        assert_eq!(road("hermes", PageReach::Here), None);
    }

    #[test]
    fn is_refused_with_the_reason_on_a_machine_whose_credentials_the_host_keeps_under() {
        for agent in ["claude", "codex", "opencode", "gemini"] {
            for reach in [PageReach::Here, PageReach::Relay, PageReach::None] {
                let refused = match server_sign_in_road(agent, "notion", reach, true) {
                    Some(Err(e)) => e,
                    other => panic!("{agent} {reach:?} was not refused: {other:?}"),
                };
                assert_eq!(refused.message, format!("{}. {}", brokered_server_sign_in_line("notion"), BROKERED_SERVER_SIGN_IN_FIX));
                assert_eq!(refused.kind, Some(Kind::Usage));
            }
        }
        assert_eq!(brokered_server_sign_in_line("notion"), "A cloud machine holds no sign-in, so notion cannot be signed in on it");
    }
}

mod the_sign_in_road_a_report_names {
    use super::*;

    #[test]
    fn is_read_off_the_row_s_declared_headless_flow_never_off_the_words_of_its_command() {
        assert_eq!(sign_in_road_of(sign_in_row("codex")), SignInRoad::Device);
        assert_eq!(sign_in_road_of(sign_in_row("gemini")), SignInRoad::Code);
        assert_eq!(sign_in_road_of(sign_in_row("claude")), SignInRoad::Token);
        assert_eq!(sign_in_road_of(sign_in_row("opencode")), SignInRoad::Terminal);
        let row = LoginSignIn {
            kind: SignInKind::Oauth,
            login: "tool login".to_string(),
            sources: vec![],
            fallback: Some("tool login --no-browser".to_string()),
            headless: Some(Headless::Device),
            questions: None,
            finish: SignInFinish::Callback,
            code: None,
            key_env: None,
            status: None,
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![],
        };
        assert_eq!(sign_in_road_of(&SignIn::Login(row.clone())), SignInRoad::Device);
        let code = LoginSignIn { fallback: Some("tool login --device".to_string()), headless: Some(Headless::Code), ..row };
        assert_eq!(sign_in_road_of(&SignIn::Login(code)), SignInRoad::Code);
    }
}
