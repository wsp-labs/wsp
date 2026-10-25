// SPDX-License-Identifier: AGPL-3.0-only
//! The place link, the machine link, limits, devices and URL handling after the fix round (#2445, fix round at 4abcbdf7b).
//! Every expected value is what node 22.23.4 or the zod 3.25.76 / TS bundle (`packages/protocol/src/wire/limits.ts`,
//! `place-link.ts`) answered for the same input.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use wsp_wire::types::check::{parse, Check};
use wsp_wire::wire::limits::{callback_port_of, host_of, is_http_url, redirects_to_machine};
use wsp_wire::wire::place_link::{parse_place_file, place_file_text, JoinRoad, PlaceDoorView};

fn name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("?")
}

/// zod refused `text` with this first issue; None where parse answers the same.
fn refused<T: DeserializeOwned + Serialize + Check>(text: &str, code: &str, path: Value, message: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(120)]);
    match parse::<T>(&input) {
        Ok(read) => Some(format!(
            "{label}: zod refused it ({code} at {path} {message:?}), read here as {}",
            serde_json::to_string(&read).unwrap_or_else(|e| format!("<unwritable: {e}>"))
        )),
        Err(e) if e.code == code && Value::Array(e.path.clone()) == path && e.message == message => None,
        Err(e) => Some(format!("{label}: zod answers {code} at {path} {message:?}, here {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod took `text` and its data is `wrote`, compared as values (key order aside).
fn takes<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: Value) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(120)]);
    match parse::<T>(&input) {
        Ok(read) if serde_json::to_value(&read).ok() == Some(wrote.clone()) => None,
        Ok(read) => Some(format!("{label}: zod's data is {wrote}, here {:?}", serde_json::to_value(&read))),
        Err(e) => Some(format!("{label}: zod took it, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike node's:\n{}", differ.len(), differ.join("\n"));
}

/// `host_of(url)` was `new URL(url).host` in node; `want` is node's answer (None where `new URL` threw).
fn host(url: &str, want: Option<&str>) -> Option<String> {
    let got = host_of(url);
    let short = |t: &str| {
        if t.chars().count() > 60 {
            format!("{}... ({} chars)", t.chars().take(40).collect::<String>(), t.chars().count())
        } else {
            t.to_string()
        }
    };
    (got.as_deref() != want).then(|| format!("host_of({:?}): node {:?}, here {:?}", short(url), want.map(short), got.as_deref().map(short)))
}

fn redirect_to(inner: &str) -> String {
    let mut encoded = String::new();
    for b in inner.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => encoded.push(b as char),
            _ => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    format!("https://a.com/o?redirect_uri={encoded}")
}

// ---- URLs ------------------------------------------------------------------------------------------------------

/// `limits.ts:117,146` run `new URL`; node's IDNA takes a label that starts with a digit (ASCII, Arabic-Indic or
/// extended) or a hyphen and holds Hebrew or Arabic letters, and a label made only of Arabic-Indic digits. The url crate
/// refuses all of them (bidi rule 1) and `node_host.rs:155` (`fallback_label`) re-admits only a label whose first
/// character is an ASCII letter (`head[0].is_ascii_alphabetic()`) or a single character. Of 400,000 generated
/// single-script hosts node took 261,875 and 17,204 of those are refused here. Real names: `3מכולת.co.il`, `۱۲۳ایران.com`,
/// `1.شبكة` (the Arabic "network" top-level domain under a numbered label). Of 89,339 hosts built from the 9,107 suffixes of the
/// Public Suffix List and nine prefixes each, 384 that node takes are refused here: every label that starts with a
/// digit under a right-to-left suffix (`123.ישראל`, `3x.امارات`, `1.ایران.ir`).
#[test]
fn a_label_that_starts_with_a_digit_and_holds_right_to_left_letters_is_a_host() {
    let cases: [(&str, &str); 14] = [
        ("http://123.ישראל/", "123.xn--4dbrk0ce"),
        ("http://3x.امارات/", "3x.xn--mgbaam7a8h"),
        ("http://1.ایران.ir/", "1.xn--mgba3a4f16a.ir"),
        ("http://xn--1-bicuf1d.com/", "xn--1-bicuf1d.com"),
        ("http://xn--9hbdgm.com/", "xn--9hbdgm.com"),
        ("http://xn--8hb.com/", "xn--8hb.com"),
        ("http://xn--mgba3a4f16a2ghi.com/", "xn--mgba3a4f16a2ghi.com"),
        ("http://1שלום.com/", "xn--1-bicuf1d.com"),
        ("http://3מכולת.co.il/", "xn--3-bicrfh9e.co.il"),
        ("http://٣٥٨١.com/", "xn--9hbdgm.com"),
        ("http://۱۲۳ایران.com/", "xn--mgba3a4f16a2ghi.com"),
        ("http://3مصر.com/", "xn--3-kncl7d.com"),
        ("http://1.شبكة/", "1.xn--ngbc5azd"),
        ("http://1-موقع.com/", "xn--1--wud9aos.com"),
    ];
    let mut found = Vec::new();
    for (url, want) in cases {
        found.push(host(url, Some(want)));
        found.push((!is_http_url(url)).then(|| format!("is_http_url({url:?}): node true, here false")));
    }
    none_differ(found);
}

/// `place-link.ts:318,323,329` read `.url()`, which is `new URL(text)` in zod 3.25.76. `place_file.rs:111`
/// (`url_string`) calls `url::Url::parse` directly and never goes through `node_host`, so the fix that reached
/// `is_http_url`, `host_of` and the redirect functions did not reach the three `.url()` fields: node refuses the 23,548
/// code points the crate maps away or keeps (`http://aㅤb.com/`, U+3164; `http://a⁡b.com/`, U+2061) and takes the ones
/// the crate refuses (`http://١.com/`, `http://1שלום.com/`).
#[test]
fn a_url_field_of_the_place_link_is_read_as_node_reads_it() {
    let door = |addresses: &str, relay: &str| format!(r#"{{"port":4420,"addresses":[{addresses}],"hostKey":"k"{relay}}}"#);
    let (code, msg) = ("invalid_string", "Invalid url");
    none_differ(vec![
        refused::<PlaceDoorView>(&door(r#""http://a\u3164b.com/""#, ""), code, serde_json::json!(["addresses", 0]), msg),
        refused::<PlaceDoorView>(&door(r#""http://x.com/","http://a\u2061b.com/""#, ""), code, serde_json::json!(["addresses", 1]), msg),
        refused::<PlaceDoorView>(
            &door(r#""http://x.com/""#, r#","relay":"https://a\u3164b.com/""#),
            code,
            serde_json::json!(["relay"]),
            msg,
        ),
        refused::<JoinRoad>(r#"{"url":"http://a\u3164b.com/","line":"l"}"#, code, serde_json::json!(["url"]), msg),
        takes::<PlaceDoorView>(
            &door(r#""http://١.com/""#, ""),
            serde_json::json!({"port":4420,"addresses":["http://١.com/"],"hostKey":"k"}),
        ),
        takes::<JoinRoad>(r#"{"url":"http://1שלום.com/","line":"l"}"#, serde_json::json!({"url":"http://1שלום.com/","line":"l"})),
    ]);
}

/// The same code points as ASCII punycode: `node_host.rs:41-45` routes a host to the tables only when its text holds a
/// non-ASCII character or a `%` (`parse`), so `xn--vhk.com` (U+3164, which node refuses as `ㅤ.com` and the fix refuses
/// as text) goes straight to `Url::parse`, and the url crate takes the 23,298 code points node refuses (node: false and
/// undefined for each; spec/daemon.md rule 13's "no host taken here and refused by node" holds for the Unicode spelling only).
#[test]
fn an_ascii_punycode_label_of_a_code_point_node_refuses_is_not_a_host() {
    let mut found = Vec::new();
    for label in ["xn--vhk", "xn--gxg", "xn--osd", "xn--zf0d", "xn--5l8d", "xn--kx7e", "xn--699g", "xn--8g0n", "xn--6zg", "xn--y78a"] {
        let url = format!("http://{label}.com/");
        found.push(host(&url, None));
        found.push(is_http_url(&url).then(|| format!("is_http_url({url:?}): node false, here true")));
    }
    none_differ(found);
}

/// Node's IDNA table gives code points assigned after its Unicode 15 (`🪩` U+1FAA9, `🥹`, `🛜`, `🩵`, U+20C0, U+2E53) no
/// bidi class, and refuses a label that holds Hebrew or Arabic letters on both sides of one; the url crate (Unicode 16)
/// takes it. node: `isHttpUrl` false, `hostOf` undefined. `node_host.rs` has no rule for it (spec/daemon.md rule 13
/// says no host was taken here and refused by node; 99 code points between two Hebrew letters are).
#[test]
fn a_hebrew_word_around_a_newer_symbol_is_not_a_host() {
    let mut found = Vec::new();
    for label in ["א🪩ב", "א🥹ב", "א🛜ב", "א🩵ב", "א⹓ב", "ا🪩ب", "א\u{20c0}ב"] {
        let url = format!("http://{label}.com/");
        found.push(host(&url, None));
        found.push(is_http_url(&url).then(|| format!("is_http_url({url:?}): node false, here true")));
    }
    none_differ(found);
}

/// Node takes a label of any length in `é` (2,000 characters: `xn--9c` and a run of `a`); the url crate's IDNA refuses a
/// non-ASCII label past about a thousand characters, so `host_of` is None and `is_http_url` false for a URL
/// `limits.ts:117` takes (it is under `HTTP_URL_MAX`).
#[test]
fn a_long_non_ascii_label_is_a_host() {
    let url = format!("http://{}.com/", "é".repeat(1005));
    let want = format!("xn--9c{}.com", "a".repeat(1005));
    none_differ(vec![host(&url, Some(&want)), (!is_http_url(&url)).then(|| "is_http_url: node true, here false".to_string())]);
}

/// RFC 3492's `delta` is a `u32` here (`node_host.rs:181-184`, `delta += (next - n) * (handled + 1)`, line 184), and the label that
/// reaches it is not capped: `redirects_to_machine` and `callback_port_of` read a sign-in URL a machine's tool printed,
/// with no `HTTP_URL_MAX` check before them (relay.ts:1088, agents-signin.ts:215), and `host_of` takes any string. A
/// label of 40,000 `a` and one Adlam letter (U+1E943, in `LAST`) overflows: "attempt to multiply with overflow" with
/// overflow checks on (dev, test, CI), a wrapped wrong punycode in release. node refuses the host (`new URL` throws), so
/// `hostOf` is undefined, `redirectsToMachine` false and `callbackPortOf` undefined.
#[test]
fn a_very_long_label_with_an_adlam_letter_does_not_overflow_and_is_refused() {
    let url = format!("http://{}\u{1e943}.com/", "a".repeat(40000));
    none_differ(vec![
        host(&url, None),
        redirects_to_machine(&url).then(|| "redirects_to_machine: node false, here true".to_string()),
        callback_port_of(&url).map(|p| format!("callback_port_of: node undefined, here {p}")),
    ]);
}

/// `limits.ts:165-166` parses the redirect with `new URL`. In a scheme that is not special WHATWG ends the authority
/// only at `/ ? #`, so a `\` after the port makes the port invalid and `new URL` throws; the url crate takes the
/// backslash as the end of the authority (`node_host.rs:48` `Url::parse`). node: `redirectsToMachine` false and
/// `callbackPortOf` undefined for each. 776 of the 787 redirect cases that differed in 200,000 generated ones.
#[test]
fn a_redirect_in_an_opaque_scheme_with_a_backslash_after_the_port_is_not_the_machine() {
    let mut found = Vec::new();
    for inner in ["vscode://127.0.0.1:5000\\cb", "myapp://localhost:1024\\cb", "vscode://[::1]:5000\\cb", "a+b://localhost:5000\\cb"] {
        let url = redirect_to(inner);
        found.push(redirects_to_machine(&url).then(|| format!("redirects_to_machine({inner:?}): node false, here true")));
        found.push(callback_port_of(&url).map(|p| format!("callback_port_of({inner:?}): node undefined, here {p}")));
    }
    none_differ(found);
}

/// `file` is a special scheme (WHATWG), so node maps and refuses its host as it does an http host. `node_host.rs:36`
/// lists `http https ws wss ftp` and leaves `file`, so a `file:` URL goes to the crate's IDNA alone. `new URL("file://ẞ.com/x").host`
/// is `ss.com` (here `xn--zca.com`), and a redirect to `file://127.0.0.1` followed by U+2061 is loopback only here.
#[test]
fn a_file_url_host_is_read_as_node_reads_it() {
    none_differ(vec![
        host("file://ẞ.com/x", Some("ss.com")),
        redirects_to_machine(&redirect_to("file://127.0.0.1\u{2061}/"))
            .then(|| "redirects_to_machine(file://127.0.0.1U+2061/): node false, here true".to_string()),
    ]);
}

// ---- the place file --------------------------------------------------------------------------------------------

const FILE: &str = r#""placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k""#;

fn written(file: &str, extra: &str) -> Option<String> {
    parse_place_file(&format!("{{{file}{extra}}}")).map(|f| place_file_text(&f))
}

fn text_of(extra: &str) -> String {
    let tail = if extra.is_empty() { String::new() } else { format!(",\n  {extra}") };
    format!(
        "{{\n  \"placeId\": \"p1\",\n  \"name\": \"n\",\n  \"hostName\": \"h\",\n  \"hostUrls\": [\n    \"http://a\"\n  ],\n  \"hostPublicKey\": \"k\",\n  \"keyPath\": \"/k\"{tail}\n}}\n"
    )
}

/// `place-link.ts:428` is `JSON.parse` and no zod, so a `__proto__` key is an own property and `JSON.stringify` writes it
/// (`place-link.ts:478`). `place_file.rs:33` reads the unknown keys through `kept::record`, which drops `__proto__` as zod
/// does for a record. The file is written back without it. node: `"__proto__": 1` after the known keys.
#[test]
fn a_place_file_keeps_a_proto_key_as_json_parse_does() {
    assert_eq!(written(FILE, r#","__proto__":1"#), Some(text_of("\"__proto__\": 1")));
}

/// `JSON.parse` is wider than serde_json: a number past a double (`1e400` is Infinity, written back `null`), a lone
/// surrogate escape (`"\ud800"`, written back the same) and arrays nested past serde_json's 128 levels all parse, so
/// `parsePlaceFile` returns the file. `parse_place_file` (`join_mint.rs:344`) reads through `serde_json::from_str` and
/// answers None, so a join the TS reads as joined reads here as no file.
#[test]
fn a_place_file_reads_what_json_parse_reads_past_serde_jsons_limits() {
    let deep = format!(r#","x":{}{}"#, "[".repeat(130), "]".repeat(130));
    let found = [
        (r#","x":1e400"#.to_string(), Some(text_of("\"x\": null"))),
        (r#","joinedAt":1e400"#.to_string(), Some(text_of("\"joinedAt\": null"))),
        (r#","x":"\ud800""#.to_string(), Some(text_of("\"x\": \"\\ud800\""))),
    ];
    let mut differ: Vec<String> = found
        .into_iter()
        .filter_map(|(extra, want)| {
            let got = written(FILE, &extra);
            (got != want).then(|| format!("{extra}: node wrote {want:?}, here {got:?}"))
        })
        .collect();
    if written(FILE, &deep).is_none() {
        differ.push("130 nested arrays: node reads the file, here None".to_string());
    }
    assert!(differ.is_empty(), "{} answer(s) unlike node's:\n{}", differ.len(), differ.join("\n"));
}
