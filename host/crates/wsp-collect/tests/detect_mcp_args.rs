// SPDX-License-Identifier: AGPL-3.0-only
//! Port of the two packages/collect/test/mcp.test.ts cases that read only linuxFit and mcpRemoteHash.
use indexmap::IndexMap;
use md5::{Digest, Md5};
use wsp_catalog::mcp::{McpServer, McpServerScope, McpTransport};
use wsp_collect::detect::mcp_args::{linux_fit, mcp_remote_hash, LinuxFit};

const HOME: &str = "/Users/dev";
const NOTION: &str = "https://mcp.notion.com/mcp";

fn hash(text: &str) -> String {
    format!("{:x}", Md5::digest(text.as_bytes()))
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn stdio(command: &str, args: &[&str], env: &[(&str, &str)]) -> McpServer {
    McpServer {
        name: "s".to_string(),
        scope: McpServerScope::User,
        transport: McpTransport::Stdio {
            command: command.to_string(),
            args: strs(args),
            env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<IndexMap<_, _>>(),
            cwd: None,
            tool_timeout_sec: None,
        },
        env_refs: vec![],
        reads: None,
        disabled: None,
    }
}

mod mcp_servers {
    use super::*;

    #[test]
    fn linux_fit_home_paths_and_homebrews_prefix_have_a_linux_equivalent_library_and() {
        assert_eq!(linux_fit(&stdio("npx", &["-y", "pkg"], &[]), HOME), LinuxFit::Ok { needs: "runs via npx".to_string() });
        assert_eq!(
            linux_fit(&stdio(&format!("{HOME}/.local/bin/uv"), &["--directory", &format!("{HOME}/code/x"), "run", "main.py"], &[]), HOME),
            LinuxFit::Ok { needs: "needs uv, installed on the machine when missing".to_string() }
        );
        assert_eq!(
            linux_fit(&stdio("/opt/homebrew/bin/node", &["server.js"], &[]), HOME),
            LinuxFit::Ok { needs: "needs node on the machine".to_string() }
        );
        assert_eq!(
            linux_fit(&stdio("npx", &["pkg"], &[("CONF", &format!("{HOME}/Library/Preferences/x.plist"))]), HOME),
            LinuxFit::No { reason: "path ~/Library/Preferences/x.plist is macOS-only, will not run".to_string() }
        );
        assert_eq!(
            linux_fit(&stdio("/Applications/X.app/Contents/MacOS/x", &[], &[]), HOME),
            LinuxFit::No { reason: "command /Applications/X.app/Contents/MacOS/x is macOS-only, will not run".to_string() }
        );
        let http = McpServer {
            name: "h".to_string(),
            scope: McpServerScope::User,
            transport: McpTransport::Http { url: "https://h".to_string(), headers: IndexMap::new() },
            env_refs: vec![],
            reads: None,
            disabled: None,
        };
        assert_eq!(linux_fit(&http, HOME), LinuxFit::Ok { needs: "nothing to install".to_string() });
    }

    #[test]
    fn mcp_remote_hash_matches_the_bridges_store_md5_of_the_url_resource_sorted() {
        assert_eq!(mcp_remote_hash(&strs(&["-y", "mcp-remote", NOTION])), Some(hash(NOTION)));
        assert_eq!(
            mcp_remote_hash(&strs(&["mcp-remote@0.8.3", NOTION, "--header", "X-B: 2", "--header", "A: 1"])),
            Some(hash(&format!("{NOTION}|{}", r#"{"A":"1","X-B":"2"}"#)))
        );
        assert_eq!(
            mcp_remote_hash(&strs(&[
                "mcp-remote",
                NOTION,
                "--client-metadata-url",
                "https://m.example/c.json",
                "--resource",
                "https://r.example",
                "--authorize-param",
                "b=2",
                "--authorize-param",
                "a=1",
                "--header",
                "K: v"
            ])),
            Some(hash(&format!("{NOTION}|https://r.example|{}|{}|https://m.example/c.json", r#"{"a":"1","b":"2"}"#, r#"{"K":"v"}"#)))
        );
        assert_eq!(mcp_remote_hash(&strs(&["-y", "@scope/other", NOTION])), None);
    }
}

/// Rules of spec/mcp_args.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use indexmap::IndexMap;
    use wsp_catalog::mcp::{McpServer, McpServerScope, McpTransport};
    use wsp_collect::detect::mcp_args::{
        hides_value, home_paths, mcp_remote_of, plain_arg, read_args, stdio_line, ArgKind, ArgRead, McpRemote,
    };

    const U: &str = "https://mcp.example.com/mcp";

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn shown(s: &str) -> ArgRead {
        ArgRead { show: Some(s.into()), ..Default::default() }
    }

    fn secret(show: &str, secret: &str, key: &str, kind: ArgKind) -> ArgRead {
        ArgRead { show: Some(show.into()), secret: Some(secret.into()), key: Some(key.into()), kind: Some(kind) }
    }

    #[test]
    fn mcp_remote_of_reads_v1_off_the_pinned_version() {
        let cases = [
            ("mcp-remote", true),
            ("mcp-remote@latest", true),
            ("mcp-remote@0.3.2", true),
            ("mcp-remote@0.3.1", false),
            ("mcp-remote@0.4.0", true),
            ("mcp-remote@1.0.0", true),
            ("mcp-remote@0.2.9", false),
            ("mcp-remote@^0.3.2", false),
            ("mcp-remote@next", false),
        ];
        for (spec, v1) in cases {
            assert_eq!(mcp_remote_of(&v(&[spec, U])).map(|r| r.v1), Some(v1), "{spec}");
        }
    }

    #[test]
    fn mcp_remote_of_lets_a_later_header_replace_an_earlier_one() {
        let got = mcp_remote_of(&v(&[
            "mcp-remote",
            U,
            "--header",
            "A: 1",
            "-H",
            "B:2",
            "--header",
            "A: 3",
            "--header",
            ":x",
            "--client-credentials",
        ]));
        let headers: IndexMap<String, String> = [("A", "3"), ("B", "2")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        assert_eq!(
            got,
            Some(McpRemote { url: U.into(), headers, hash: "7b8e8d04d113d4805ffc6cdd5450d173".into(), client_credentials: true, v1: true })
        );
        assert_eq!(mcp_remote_of(&v(&["mcp-remote"])), None);
        // A trailing --header reads nothing, so the hash is the url's alone.
        assert_eq!(
            mcp_remote_of(&v(&["mcp-remote", U, "--header"])).map(|r| (r.headers.len(), r.hash)),
            Some((0, "94baabe3c3fadb285b26c035e4e86046".into()))
        );
    }

    #[test]
    fn hides_value_reads_a_flag_as_a_secret_name_with_dashes_as_underscores() {
        let flags = ["--api-key", "--token", "-H", "--header", "--static-oauth-client-info", "--verbose", "--auth-file", "--port"];
        let got: Vec<bool> = flags.iter().map(|f| hides_value(f)).collect();
        assert_eq!(got, vec![true, true, true, true, true, false, true, false]);
    }

    #[test]
    fn plain_arg_takes_paths_and_plain_addresses_and_refuses_a_login_query_or_key_segment() {
        let args = [
            ("/x", true),
            ("~/x", true),
            ("https://x.example/a", true),
            // WHATWG URL reads a bare `?` as an empty search.
            ("https://x.example/?", true),
            ("https://u:p@x.example/", false),
            ("https://x.example/?q=1", false),
            ("https://x.example/abcdefghijklmnopqrstuvwx1", false),
            ("https://x.example/abcdefghijklmnopqrstuvwxyz", true),
            ("postgres://db/x", true),
            ("x", false),
            ("not a url://", false),
        ];
        for (arg, plain) in args {
            assert_eq!(plain_arg(arg), plain, "{arg}");
        }
    }

    #[test]
    fn read_args_reads_flags_assignments_and_positionals_as_the_detail_line_shows_them() {
        let args = v(&[
            "-y",
            "pkg",
            "--api-key",
            "sekret",
            "--token=abc",
            "API_KEY=xyz",
            "--env=DB_PASSWORD=\u{e9}",
            "--port",
            "8080",
            "-v",
            "--",
            "-y",
            "positional",
            "/abs",
            "https://H.example:443/p?q=1",
        ]);
        assert_eq!(
            read_args("npx", &args, "/h"),
            vec![
                ArgRead::default(),
                shown("pkg"),
                shown("--api-key"),
                secret("\u{2026}", "flag --api-key (6 B)", "--api-key", ArgKind::Flag),
                secret("--token=\u{2026}", "flag --token (3 B)", "--token", ArgKind::Flag),
                secret("API_KEY=\u{2026}", "arg API_KEY (3 B)", "API_KEY", ArgKind::Assign),
                secret("--env=DB_PASSWORD=\u{2026}", "arg DB_PASSWORD (2 B)", "DB_PASSWORD", ArgKind::Assign),
                shown("--port"),
                shown("8080"),
                shown("-v"),
                shown("--"),
                // Past `--` a dashed word is a value like any other.
                secret("\u{2026}", "arg 12 (2 B)", "argument 12", ArgKind::Positional),
                secret("\u{2026}", "arg 13 (10 B)", "argument 13", ArgKind::Positional),
                shown("/abs"),
                secret("\u{2026}", "arg 15 (27 B)", "argument 15", ArgKind::Positional),
            ]
        );
        // A program that is no runner names no package, so its first word is a value too.
        assert_eq!(
            read_args("/opt/homebrew/bin/node", &v(&["server.js", "value", "URL=https://u:p@x/", "-1=x"]), "/h"),
            vec![
                secret("\u{2026}", "arg 1 (9 B)", "argument 1", ArgKind::Positional),
                secret("\u{2026}", "arg 2 (5 B)", "argument 2", ArgKind::Positional),
                secret("URL=\u{2026}", "arg URL (14 B)", "URL", ArgKind::Assign),
                shown("-1=x"),
            ]
        );
    }

    #[test]
    fn stdio_line_cuts_past_five_words() {
        assert_eq!(stdio_line("/h/bin/x", &v(&["a", "/b", "/c", "/d", "/e", "/f", "/g"]), "/h"), "~/bin/x \u{2026} /b /c /d /e \u{2026}");
        assert_eq!(stdio_line("/h/bin/x", &v(&["/a", "/b", "/c", "/d", "/e"]), "/h"), "~/bin/x /a /b /c /d /e");
    }

    #[test]
    fn home_paths_lists_home_paths_once_in_order_leaving_local_bin_out() {
        let env: IndexMap<String, String> =
            [("A", "~/env"), ("B", "/h/b")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let server = McpServer {
            name: "s".into(),
            scope: McpServerScope::User,
            transport: McpTransport::Stdio {
                command: "/h/x".into(),
                args: v(&["--dir=/h/a", "~/b", "/h/.local/bin/c", "/elsewhere", "/h/a", "/h"]),
                env,
                cwd: Some("/h/cwd".into()),
                tool_timeout_sec: None,
            },
            env_refs: vec![],
            reads: None,
            disabled: None,
        };
        assert_eq!(home_paths(&server, "/h"), vec!["~/a", "~/b", "~/cwd", "~/env"]);
        let http = McpServer { transport: McpTransport::Http { url: U.into(), headers: IndexMap::new() }, ..server };
        assert_eq!(home_paths(&http, "/h"), Vec::<String>::new());
    }
}
