// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/callback-relay.test.ts, port-forwards.test.ts and runtime/test/box-keep.test.ts,
//! with the cases no TS test pins written against values the TS module printed (node, protocol bundle at a3b870a6e):
//! helpers_limits.
mod common;
use common::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_wire::views::harness::HarnessOption;
use wsp_wire::wire::helpers::{base64_length, is_base64_of, is_cgroup_path, is_watch_name, js_key_order, listed, or_none, PortCloseDetail};
use wsp_wire::wire::limits::*;

const WRANGLER: &str = "https://dash.cloudflare.com/oauth2/auth?response_type=code&client_id=54d11594&redirect_uri=http%3A%2F%2Flocalhost%3A8976%2Foauth%2Fcallback&scope=account%3Aread&state=S&code_challenge=C&code_challenge_method=S256";
const CLAUDE_BROWSER: &str = "https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A45543%2Fcallback&scope=user%3Ainference&code_challenge=C&code_challenge_method=S256&state=S";
const CLAUDE_TERMINAL: &str = "https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a&response_type=code&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&scope=user%3Ainference&code_challenge=C&code_challenge_method=S256&state=S";
const AWS: &str = "https://d-1234.awsapps.com/start/authorize?response_type=code&client_id=abc&redirect_uri=http%3A%2F%2F127.0.0.1%3A53211%2Foauth%2Fcallback&state=S";
const IPV6: &str = "https://example.com/authorize?redirect_uri=http%3A%2F%2F%5B%3A%3A1%5D%3A8976%2Foauth%2Fcallback&state=S";
const GH_DEVICE: &str = "https://github.com/login/device";
const AWS_BARE: &str =
    "https://d-1234.awsapps.com/start/authorize?response_type=code&redirect_uri=http%3A%2F%2F127.0.0.1%2Foauth%2Fcallback&state=S";
const KEYCHAIN_STYLE: &str = "https://accounts.example.com/oauth2/auth?client_id=keychain&scope=openid";
const REMOTE_PORT: &str = "https://example.com/authorize?redirect_uri=https%3A%2F%2Fapp.example.com%3A8443%2Fcb";

// callback-relay.test.ts:22
#[test]
fn redirects_to_machine_reads_the_redirect_uri_host() {
    for (name, url, returns) in [
        ("wrangler's localhost callback", WRANGLER, true),
        ("aws sso, 127.0.0.1 with a port", AWS, true),
        ("aws's registered redirect without a port, whose listener the spotter finds", AWS_BARE, true),
        ("an IPv6 loopback host", IPV6, true),
        ("Claude Code's hosted paste-code callback", CLAUDE_TERMINAL, false),
        ("gh's device page: no redirect_uri at all", GH_DEVICE, false),
        ("a redirect_uri on another host, even with a port", REMOTE_PORT, false),
        ("not a URL", "paste code here", false),
    ] {
        assert_eq!(redirects_to_machine(url), returns, "{name}");
    }
}

// callback-relay.test.ts:37
#[test]
fn callback_port_of_reads_a_bindable_loopback_port() {
    for (name, url, port) in [
        ("wrangler, redirect_uri on localhost:8976", WRANGLER, Some(8976)),
        ("Claude Code's browser URL, random localhost port", CLAUDE_BROWSER, Some(45543)),
        ("aws sso, 127.0.0.1 with a port", AWS, Some(53211)),
        ("an IPv6 loopback host", IPV6, Some(8976)),
        ("Claude Code's printed URL: the hosted paste-code callback", CLAUDE_TERMINAL, None),
        ("aws's registered redirect without a port", AWS_BARE, None),
        ("a bare authorize URL with neither redirect_uri nor port", KEYCHAIN_STYLE, None),
        ("a redirect_uri on another host, even with a port", REMOTE_PORT, None),
        ("a loopback port below 1024", "https://x.test/a?redirect_uri=http%3A%2F%2Flocalhost%3A80%2Fcb", None),
        ("not a URL", "paste code here", None),
        ("a redirect_uri that is not a URL", "https://x.test/a?redirect_uri=nonsense", None),
    ] {
        assert_eq!(callback_port_of(url), port, "{name}");
    }
}

// Printed by the TS module: WHATWG URL rules the Rust port must keep. A default port is no port (https on 443, http on
// 80), a host is compared after the parser lower-cases and normalises it, 0.0.0.0 and 127.0.0.2 are not loopback by
// this rule, and the first redirect_uri of several is the one read.
#[test]
fn the_redirect_reading_follows_the_url_parser() {
    for (url, returns, port) in [
        ("https://x.test/a?redirect_uri=http%3A%2F%2FLOCALHOST%3A9000%2F", true, Some(9000)),
        ("https://x.test/a?redirect_uri=http%3A%2F%2F127.0.0.1%3A65535%2F", true, Some(65535)),
        ("https://x.test/a?redirect_uri=http%3A%2F%2F127.0.0.1%3A1024", true, Some(1024)),
        ("https://x.test/a?redirect_uri=http%3A%2F%2F0.0.0.0%3A9000%2F", false, None),
        ("https://x.test/a?redirect_uri=http%3A%2F%2Flocalhost%3A80%2F", true, None),
        ("https://x.test/a?redirect_uri=https%3A%2F%2Flocalhost%3A443%2F", true, None),
        ("https://x.test/a?redirect_uri=http%3A%2F%2F127.0.0.2%3A9000", false, None),
        ("https://x.test/a?redirect_uri=http%3A%2F%2F%5B0%3A0%3A0%3A0%3A0%3A0%3A0%3A1%5D%3A9000", true, Some(9000)),
        ("https://x.test/a?redirect_uri=http://localhost:9000/&redirect_uri=http://evil:1/", true, Some(9000)),
    ] {
        assert_eq!((redirects_to_machine(url), callback_port_of(url)), (returns, port), "{url}");
    }
}

// callback-relay.test.ts:84
#[test]
fn is_http_url_is_the_one_rule_http_or_https_in_any_case_nothing_else() {
    assert!(is_http_url("HTTPS://EXAMPLE.COM/A"));
    assert!(is_http_url("Http://x.test"));
    assert!(is_http_url("https://github.com/login/device"));
    for bad in ["file:///etc/passwd", "javascript:alert(1)", "smb://host/share", "\\\\host\\share", "ftp://x/y", "", "httpss://x"] {
        assert!(!is_http_url(bad), "{bad}");
    }
}

// callback-relay.test.ts:94
#[test]
fn is_http_url_also_caps_the_length_and_refuses_whitespace_and_control_characters() {
    let max = HTTP_URL_MAX as usize;
    assert!(!is_http_url(&format!("https://x.test/{}", "a".repeat(max))));
    assert!(is_http_url(&format!("https://x.test/{}", "a".repeat(max - 15))));
    // One past the cap, exactly: 8,193 characters.
    assert!(!is_http_url(&format!("https://x.test/{}", "a".repeat(max - 14))));
    for bad in [
        "https://x.test/a b",
        "https://x.test/a\nb",
        "https://x.test/\tq",
        "https://x.test/\x00",
        "https://x.test/\x7f",
        "https://x.test/a\r",
    ] {
        assert!(!is_http_url(bad), "{bad:?}");
    }
    // JavaScript's \s holds the no-break space and the line separator too.
    assert!(!is_http_url("http://a\u{a0}b"));
    assert!(!is_http_url("http://a\u{2028}b"));
    // Printed by the TS module: U+FEFF is JavaScript white space (Rust's \s does not hold it), U+0085 passes the
    // pattern and the URL parser refuses it in a host, U+180E is refused too.
    for odd in ["http://a\u{85}b", "http://a\u{feff}b", "http://a\u{180e}b"] {
        assert!(!is_http_url(odd), "{odd:?}");
    }
    assert!(is_http_url("http://é.test"));
    assert!(!is_http_url("http://"));
}

// callback-relay.test.ts:105
#[test]
fn is_http_url_also_requires_the_url_to_parse_so_a_hostname_can_always_be_read() {
    for bad in ["https://%", "https://[::1", "https://exa%mple.com/x"] {
        assert!(!is_http_url(bad), "{bad}");
    }
    assert!(is_http_url("https://[::1]:8976/cb"));
}

// callback-relay.test.ts:113
#[test]
fn host_of_reads_the_host_with_its_port_without_userinfo_and_never_throws() {
    assert_eq!(host_of("https://github.com/login/device").as_deref(), Some("github.com"));
    assert_eq!(host_of("https://github.com:8443/x").as_deref(), Some("github.com:8443"));
    assert_eq!(host_of("https://github.com@evil.example/login").as_deref(), Some("evil.example"));
    assert_eq!(host_of("https://[::1]:8976/cb").as_deref(), Some("[::1]:8976"));
    for bad in ["https://%", "https://[::1", "not a url", ""] {
        assert_eq!(host_of(bad), None, "{bad}");
    }
}

// Printed by the TS module: the default port drops, the host is lower-cased and punycoded, an IPv4 shorthand is
// expanded, a root dot stays, and a URL with no host answers the empty string rather than nothing.
#[test]
fn host_of_answers_the_parser_s_host() {
    for (url, host) in [
        ("HTTP://EXAMPLE.COM:80/x", "example.com"),
        ("https://example.com:443/", "example.com"),
        ("http://user:pw@h.test:8080/", "h.test:8080"),
        ("https://BÜCHER.example/", "xn--bcher-kva.example"),
        ("http://127.1/", "127.0.0.1"),
        ("https://h.test.:8080/", "h.test.:8080"),
        ("file:///etc", ""),
        ("mailto:x@y", ""),
    ] {
        assert_eq!(host_of(url).as_deref(), Some(host), "{url}");
    }
}

// port-forwards.test.ts:12
#[test]
fn a_forward_names_the_workspace_by_id_and_by_name_the_port_when_it_opened_and_which_kind() {
    let f = json!({ "workspaceId": "ws_1", "port": 8123, "startedAt": "2026-09-04T10:00:00.000Z", "name": "api", "kind": "url" });
    let read: PortForward = reads_exact(f.clone());
    assert_eq!((read.workspace_id.as_str(), read.port.get(), read.name.as_str(), read.kind), ("ws_1", 8123, "api", PortForwardKind::Url));
    let mut callback = f.clone();
    callback["kind"] = json!("callback");
    assert_eq!(reads::<PortForward>(callback).kind, PortForwardKind::Callback);
    let mut editor = f.clone();
    editor["kind"] = json!("editor");
    assert_eq!(reads::<PortForward>(editor).kind, PortForwardKind::Editor);
    let mut low = f.clone();
    low["port"] = json!(80);
    refused::<PortForward>(low);
    let mut other = f.clone();
    other["kind"] = json!("other");
    assert_eq!(refused::<PortForward>(other).path, path!["kind"]);
    let mut no_kind = f.clone();
    no_kind.as_object_mut().unwrap().shift_remove("kind");
    assert_eq!(refused::<PortForward>(no_kind).path, path!["kind"]);
    refused::<PortForward>(json!({ "workspaceId": "ws_1", "port": 8123, "name": "api", "kind": "url" }));
    refused::<PortForward>(json!({ "workspaceId": "ws_1", "port": 8123, "startedAt": "2026-09-04T10:00:00.000Z", "kind": "url" }));
}

// runtime/test/box-keep.test.ts:180 and limits.ts:168: a quarter of the memory, 400 MB each, never more than six.
#[test]
fn agents_kept_on_a_joined_computer_follow_its_memory() {
    for (mem, kept) in [
        (0.0, 0),
        (1024.0, 0),
        (1599.0, 0),
        (1600.0, 1),
        (1601.0, 1),
        (3999.0, 2),
        (4096.0, 2),
        (8192.0, 5),
        (16384.0, 6),
        (100000.0, 6),
        (-5.0, 0),
    ] {
        assert_eq!(agents_kept_on(mem), kept, "{mem}");
    }
    assert_eq!(AGENTS_KEPT, 6);
}

// limits.ts:212: half a second doubling to ten, plus floor(random * 250); random is the injected Math.random().
#[test]
fn link_backoff_doubles_from_half_a_second_to_ten_with_a_quarter_second_of_jitter() {
    for (attempt, ms) in [(-3, 500), (0, 500), (1, 500), (2, 1000), (3, 2000), (5, 8000), (6, 10000), (100, 10000)] {
        assert_eq!(link_backoff_ms(attempt, 0.0), ms, "attempt {attempt}");
    }
    for (attempt, ms) in [(1, 749), (2, 1249), (6, 10249), (100, 10249)] {
        assert_eq!(link_backoff_ms(attempt, 0.999999), ms, "attempt {attempt} at the top of the jitter");
    }
    assert_eq!(link_backoff_ms(i64::MAX, 0.5), 10125);
}

// limits.ts:240, printed by the TS module.
#[test]
fn is_plain_path_takes_absolute_plain_names_and_nothing_that_walks_up() {
    for (path, plain) in [
        ("/", true),
        ("/a", true),
        ("/a/", true),
        ("//a", false),
        ("/a//b", false),
        ("/a/./b", false),
        ("/a/../b", false),
        ("/.hidden", true),
        ("/a b", true),
        ("a", false),
        ("", false),
        ("/a;b", false),
        ("/a\nb", false),
        ("/é", false),
        ("/a/..", false),
        ("/a/...", true),
        ("/x+y@z:1,2-3_4", true),
    ] {
        assert_eq!(is_plain_path(path), plain, "{path:?}");
    }
}

// limits.ts:248, printed by the TS module.
#[test]
fn is_under_path_takes_a_relative_name_with_no_empty_or_climbing_part() {
    for (path, under) in [
        ("a", true),
        ("a/b", true),
        ("/a", false),
        ("a/", false),
        ("a//b", false),
        (".", false),
        ("..", false),
        ("a/..", false),
        (".a", true),
        ("", false),
        ("a/./b", false),
        ("a\\b", true),
    ] {
        assert_eq!(is_under_path(path), under, "{path:?}");
    }
}

// helpers.ts:99
#[test]
fn base64_length_counts_the_padding() {
    for (bytes, len) in [(0, 0), (1, 4), (2, 4), (3, 4), (4, 8), (32, 44), (44, 60), (64, 88)] {
        assert_eq!(base64_length(bytes), len, "{bytes}");
    }
}

// helpers.ts:101, the rule PlaceNonce reads (32 bytes), printed by the TS module.
#[test]
fn base64_of_n_bytes_counts_the_bytes_off_the_text_and_its_padding() {
    let a32 = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
    for (text, takes) in [
        (a32.to_string(), true),
        (a32[..a32.len() - 1].to_string(), false),
        (format!("{a32}A"), false),
        ("A".repeat(44), false),
        (format!("{}==", "A".repeat(42)), false),
        (format!("{}=", "A".repeat(43)), true),
        (" ".repeat(44), false),
        (a32.to_lowercase(), true),
        (format!("{}=", "-".repeat(43)), false),
        ("AAAA====".to_string(), false),
    ] {
        assert_eq!(is_base64_of(&text, 32), takes, "{text}");
    }
}

// helpers.ts:71, through ports.watch's cgroups in the TS module.
#[test]
fn a_cgroup_path_is_absolute_plain_names_under_512() {
    for (path, takes) in [
        ("/".to_string(), false),
        ("/a".to_string(), true),
        ("/a/b".to_string(), true),
        ("a".to_string(), false),
        ("/a/".to_string(), false),
        ("//a".to_string(), false),
        ("/a/./b".to_string(), false),
        ("/a/../b".to_string(), false),
        (format!("/{}", "x".repeat(511)), true),
        (format!("/{}", "x".repeat(512)), false),
        ("/.a".to_string(), true),
        ("/a b".to_string(), true),
    ] {
        assert_eq!(is_cgroup_path(&path), takes, "{path}");
    }
}

// helpers.ts:68
#[test]
fn a_watch_name_is_one_to_64_letters_digits_dashes_and_underscores() {
    for (name, takes) in [
        ("a".to_string(), true),
        ("A-_9".to_string(), true),
        (String::new(), false),
        ("x".repeat(64), true),
        ("x".repeat(65), false),
        ("a.b".to_string(), false),
        ("a b".to_string(), false),
        ("é".to_string(), false),
    ] {
        assert_eq!(is_watch_name(&name), takes, "{name}");
    }
}

// Rule 4.26: node 22 wrote JSON.stringify(JSON.parse('{"10":1,"b":2,"2":3,"01":4,"-1":5,"4294967295":6,
// "4294967294":7,"1.5":8,"0":9}')) as below. Index-like keys first, ascending; every other key keeps its place.
#[test]
fn js_key_order_moves_index_keys_first_as_node_writes_them() {
    let read: IndexMap<String, Value> =
        serde_json::from_str(r#"{"10":1,"b":2,"2":3,"01":4,"-1":5,"4294967295":6,"4294967294":7,"1.5":8,"0":9}"#).unwrap();
    let mut map = read;
    js_key_order(&mut map);
    assert_eq!(serde_json::to_string(&map).unwrap(), r#"{"0":9,"2":3,"10":1,"4294967294":7,"b":2,"01":4,"-1":5,"4294967295":6,"1.5":8}"#);
    // A map with no index-like key is left as it was.
    let mut plain: IndexMap<String, Value> = serde_json::from_str(r#"{"z":1,"a":2}"#).unwrap();
    js_key_order(&mut plain);
    assert_eq!(serde_json::to_string(&plain).unwrap(), r#"{"z":1,"a":2}"#);
    // The maps inside are not touched.
    let mut outer: IndexMap<String, Value> = serde_json::from_str(r#"{"b":{"2":1,"a":0},"1":true}"#).unwrap();
    js_key_order(&mut outer);
    assert_eq!(serde_json::to_string(&outer).unwrap(), r#"{"1":true,"b":{"2":1,"a":0}}"#);
}

fn option(value: &str, label: &str) -> HarnessOption {
    HarnessOption { value: value.into(), label: label.into(), description: None, is_default: None, short: None }
}

// helpers.ts:45
#[test]
fn listed_refuses_a_value_no_option_holds_naming_the_options_and_the_legacy_ones() {
    let options = [option("a", "Alpha"), option("b", "Beta")];
    let legacy = [option("old", "Old")];
    let hidden = [option("h", "Hidden")];
    assert_eq!(listed("claude", "model", &options, None, &legacy, &hidden), Ok(()));
    assert_eq!(listed("claude", "model", &options, Some("b"), &[], &[]), Ok(()));
    assert_eq!(listed("claude", "model", &options, Some("old"), &legacy, &[]), Ok(()));
    assert_eq!(listed("claude", "model", &options, Some("h"), &[], &hidden), Ok(()));
    let refused = listed("claude", "model", &options, Some("x"), &legacy, &hidden).unwrap_err();
    assert_eq!(refused.message, r#"model "x" is not one claude takes; one of: Alpha (a), Beta (b); legacy: Old (old)"#);
    assert_eq!((refused.kind, refused.offered), ("usage", 2));
    let bare = listed("claude", "model", &options, Some("x"), &[], &[]).unwrap_err();
    assert_eq!(bare.message, r#"model "x" is not one claude takes; one of: Alpha (a), Beta (b)"#);
    // An agent that offers none of a word refuses any value of it, the empty string included.
    let none = listed("cursor", "effort", &[], Some(""), &[], &[]).unwrap_err();
    assert_eq!((none.message.as_str(), none.offered), ("cursor takes no effort", 0));
}

#[derive(serde::Deserialize, Debug)]
struct Caught {
    #[serde(default, deserialize_with = "or_none")]
    detail: Option<PortCloseDetail>,
}

// zod `.catch(undefined)`: a field that does not read reads as absent, and the frame around it still stands.
#[test]
fn or_none_reads_a_value_that_does_not_parse_as_absent() {
    let read: Caught = serde_json::from_value(json!({ "detail": { "pid": "not a number" } })).unwrap();
    assert!(read.detail.is_none());
    let read: Caught = serde_json::from_value(json!({ "detail": null })).unwrap();
    assert!(read.detail.is_none());
    let read: Caught = serde_json::from_value(json!({ "detail": { "process": "node", "exited": true } })).unwrap();
    let detail = read.detail.unwrap();
    assert_eq!((detail.process.as_deref(), detail.exited), (Some("node"), Some(true)));
    let read: Caught = serde_json::from_value(json!({})).unwrap();
    assert!(read.detail.is_none());
}
