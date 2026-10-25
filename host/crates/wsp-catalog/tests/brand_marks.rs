// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/brand-marks.test.ts.
use std::collections::HashSet;

use regex::Regex;
use wsp_catalog::catalog::catalog_tools;
use wsp_catalog::marks::{brand_marks, server_mark, tool_mark};
use wsp_catalog::pending::McpRowTransport;

// The CLIs that draw the neutral terminal glyph on purpose: no mark is registered for them yet.
const TERMINAL_GLYPH: [&str; 58] = [
    "curl", "pnpm", "uv", "python", "git", "jq", "ripgrep", "build-essential", "fd", "sqlite3", "wget", "zip", "xz", "rsync",
    "agent-browser", "docker", "go", "rust", "java", "maven", "gradle", "bun", "yarn", "ruff", "black", "mypy", "pyright",
    "pytest", "prettier", "eslint", "typescript", "gcloud", "kubectl", "aws", "vercel", "netlify", "fly", "supabase",
    "railway", "doppler", "op", "ffmpeg", "yq", "git-lfs", "tmux", "ruby", "php", "postgresql-client", "redis-tools",
    "golangci-lint", "mise", "git-delta", "shellcheck", "swift", "elixir", "bazel", "llvm", "playwright",
];

fn http(host: &str) -> McpRowTransport {
    McpRowTransport::Http { host: host.to_string() }
}

fn stdio(line: &str) -> McpRowTransport {
    McpRowTransport::Stdio { line: line.to_string() }
}

fn server_id(transport: &McpRowTransport) -> Option<String> {
    server_mark(transport).map(|m| m.id.clone())
}

mod brand_marks {
    use super::*;

    #[test]
    fn every_catalog_cli_has_a_registered_mark_or_takes_the_terminal_glyph_on_purpose_n() {
        let marked: Vec<String> = catalog_tools().iter().filter(|t| tool_mark(&t.id).is_some()).map(|t| t.id.clone()).collect();
        assert_eq!(marked, ["node", "gh", "wrangler", "cloudflared"]);
        let unmarked: Vec<String> = catalog_tools().iter().filter(|t| tool_mark(&t.id).is_none()).map(|t| t.id.clone()).collect();
        assert_eq!(unmarked, TERMINAL_GLYPH);
    }

    #[test]
    fn a_mark_names_only_catalog_clis_each_under_one_mark() {
        let ids: HashSet<String> = catalog_tools().iter().map(|t| t.id.clone()).collect();
        let claimed: Vec<String> = brand_marks().iter().flat_map(|m| m.tools.clone()).collect();
        for id in &claimed {
            assert!(ids.contains(id), "{id}");
        }
        let unique: HashSet<&String> = claimed.iter().collect();
        assert_eq!(unique.len(), claimed.len());
        assert_eq!(tool_mark("gh").map(|m| m.id.as_str()), Some("github"));
        assert_eq!(tool_mark("not-a-tool"), None);
    }

    #[test]
    fn every_mark_is_simple_icons_path_at_a_pinned_release_under_cc0() {
        let source = Regex::new(r"^https://github\.com/simple-icons/simple-icons/blob/16\.30\.0/icons/[a-z]+\.svg$").unwrap();
        assert!(!brand_marks().is_empty());
        for m in brand_marks() {
            assert!(source.is_match(&m.mark.source), "{}", m.id);
            assert_eq!(m.mark.license, "CC0-1.0", "{}", m.id);
        }
    }

    #[test]
    fn a_server_reached_over_an_address_takes_the_mark_of_the_company_that_serves_it_by() {
        assert_eq!(server_id(&http("mcp.linear.app")).as_deref(), Some("linear"));
        assert_eq!(server_id(&http("MCP.Linear.App")).as_deref(), Some("linear"));
        assert_eq!(server_id(&http("api.githubcopilot.com")).as_deref(), Some("github"));
        assert_eq!(server_id(&http("bindings.mcp.cloudflare.com")).as_deref(), Some("cloudflare"));
        assert_eq!(server_id(&http("mcp.linear.app:8443")).as_deref(), Some("linear"));
        assert_eq!(server_id(&http("mcp.linear.app.")).as_deref(), Some("linear"));
        assert_eq!(server_id(&http("notlinear.app")), None);
        assert_eq!(server_id(&http("github.com.evil.io")), None);
        assert_eq!(server_id(&http("mcp.example.com")), None);
    }

    #[test]
    fn a_command_server_takes_the_mark_of_the_package_it_runs_or_of_an_address_on_its_l() {
        assert_eq!(server_id(&stdio("npx -y @modelcontextprotocol/server-github")).as_deref(), Some("github"));
        assert_eq!(server_id(&stdio("docker run -i --rm -e GITHUB_PERSONAL_ACCESS_TOKEN ghcr.io/github/github-mcp-server")).as_deref(), Some("github"));
        assert_eq!(server_id(&stdio("npx -y @cloudflare/mcp-server-cloudflare@0.4.1 run")).as_deref(), Some("cloudflare"));
        assert_eq!(server_id(&stdio("npx -y mcp-remote https://mcp.linear.app/sse")).as_deref(), Some("linear"));
        assert_eq!(server_id(&stdio("npx -y mcp-remote HTTPS://mcp.linear.app/sse")).as_deref(), Some("linear"));
        assert_eq!(server_id(&stdio("node ./my-github-helper.js")), None);
        assert_eq!(server_id(&stdio("")), None);
    }
}
