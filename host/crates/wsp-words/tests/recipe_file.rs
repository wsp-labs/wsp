// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/gh-status.test.ts whole (2 cases); every other case is pinned off the TS under node.

use wsp_words::recipe_file::{gh_status_of, GhStatus};

mod gh_s_status_as_wsp_reads_it {
    use super::*;

    #[test]
    fn reads_the_first_account_s_login_and_its_token_s_scopes_and_nothing_of_the_token_i() {
        let said = [
            "github.com",
            "  \u{2713} Logged in to github.com account Zingzy (keyring)",
            "  - Active account: true",
            "  - Token: gho_************************************",
            "  - Token scopes: 'gist', 'read:org', 'repo', 'workflow'",
            "",
            "  \u{2713} Logged in to github.com account other (keyring)",
            "  - Token scopes: 'repo'",
        ]
        .join("\n");
        assert_eq!(
            gh_status_of(&said),
            GhStatus { account: Some("Zingzy".into()), scopes: Some(vec!["gist".into(), "read:org".into(), "repo".into(), "workflow".into()]) }
        );
    }

    #[test]
    fn reads_an_older_gh_s_line_and_says_nothing_it_was_not_told() {
        assert_eq!(gh_status_of("github.com\n  \u{2713} Logged in to github.com as dev (oauth_token)\n"), GhStatus { account: Some("dev".into()), scopes: None });
        assert_eq!(gh_status_of("You are not logged into any GitHub hosts."), GhStatus::default());
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::recipe_file::*;

    use serde_json::json;
    use wsp_wire::types::recipe_file::RecipeFile;

    fn rf(v: serde_json::Value) -> RecipeFile {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_recipe_is_named_counted_and_its_own_server_left_out() {
        assert_eq!(wsp_words::recipe_file::wsp_own_server("wsp"), true); // wspOwnServer("wsp")
        assert_eq!(wsp_words::recipe_file::wsp_own_server("github"), false); // wspOwnServer("github")
        assert_eq!(wsp_words::recipe_file::wsp_own_server("Wsp"), false); // wspOwnServer("Wsp")
        assert_eq!(wsp_words::recipe_file::recipe_slug("My Recipe!"), "my-recipe"); // recipeSlug("My Recipe!")
        assert_eq!(wsp_words::recipe_file::recipe_slug("  --x--  "), "x"); // recipeSlug("  --x--  ")
        assert_eq!(wsp_words::recipe_file::recipe_slug(""), ""); // recipeSlug("")
        assert_eq!(wsp_words::recipe_file::recipe_slug("\u{dc}n\u{ef}code"), "n-code"); // recipeSlug("Ünïcode")
        assert_eq!(wsp_words::recipe_file::recipe_slug("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // recipeSlug("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        assert_eq!(wsp_words::recipe_file::recipe_summary(&rf(json!({"name":"base","agents":{},"mcp":{},"clis":{},"skills":{},"plugins":{},"folders":{},"configs":{}}))), "nothing picked"); // recipeSummary({"name":"base","agents":{},"mcp":{},"clis":{},"skills":{},"plugins":{},"folders":{},"configs":{}})
    }

    #[test]
    fn the_lines_a_recipe_refuses_or_keeps_keys_with() {
        assert_eq!(wsp_words::recipe_file::keys_kept_line(&[] as &[&str], None), "not copied: it needs a key; to send them, press Copy the keys on this row in the computer's Setup in Settings > Computers"); // keysKeptLine([])
        assert_eq!(wsp_words::recipe_file::keys_kept_line(&["OPENAI_API_KEY"] as &[&str], Some("work")), "not copied: it needs OPENAI_API_KEY; to send it, pick Copy the keys under MCP servers on the recipe work in Settings > Recipes"); // keysKeptLine(["OPENAI_API_KEY"], "work")
        assert_eq!(wsp_words::recipe_file::keys_kept_line(&["A","B"] as &[&str], None), "not copied: it needs A and B; to send them, press Copy the keys on this row in the computer's Setup in Settings > Computers"); // keysKeptLine(["A","B"])
        assert_eq!(wsp_words::recipe_file::no_such_recipe_refusal("x", &[] as &[&str]), "no recipe named x; none is saved yet"); // noSuchRecipeRefusal("x", [])
        assert_eq!(wsp_words::recipe_file::no_such_recipe_refusal("x", &["base","work"] as &[&str]), "no recipe named x; you have base, work"); // noSuchRecipeRefusal("x", ["base","work"])
        assert_eq!(wsp_words::recipe_file::recipe_from_here_refusal("x"), "x is not a computer you added; a recipe is saved from one that was set up with picks"); // recipeFromHereRefusal("x")
        assert_eq!(wsp_words::recipe_file::no_picks_refusal("base"), "base was set up before recipes, so it carries no picks to save; a recipe is written from the app's Add a computer, or from a computer set up with one"); // noPicksRefusal("base")
    }
}
