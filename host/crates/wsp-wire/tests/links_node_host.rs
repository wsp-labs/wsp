// SPDX-License-Identifier: AGPL-3.0-only
//! URL hosts and the place file as node 22.23.4 answered them: `new URL(text).host` for each URL below, and
//! `JSON.stringify(JSON.parse(text), null, 2)` for each place file. The bidi rows pin node's own rule for a label (it is
//! not RFC 5893's), the punycode rows its refusals of a label whose decoding it will not take, and the long rows its
//! running delta passing a signed 32 bit integer.

use wsp_wire::wire::limits::{callback_port_of, host_of, redirects_to_machine};
use wsp_wire::wire::place_link::{parse_place_file, place_file_text};

const BIDI: &[(&str, Option<&str>)] = &[
    ("http://a\u{5d0}.com/", Some("xn--a-0hc.com")),
    ("http://a\u{5d0}b.com/", None),
    ("http://a\u{5d0}\u{5d1}.com/", None),
    ("http://1\u{5d0}.com/", Some("xn--1-0hc.com")),
    ("http://\u{5d0}1.com/", Some("xn--1-zhc.com")),
    ("http://\u{5d0}a.com/", None),
    ("http://\u{5d0}-.com/", None),
    ("http://1\u{663}.com/", None),
    ("http://1\u{663}\u{5d0}.com/", None),
    ("http://\u{663}\u{5d0}.com/", Some("xn--4db70a.com")),
    ("http://\u{663}\u{6f1}.com/", None),
    ("http://-\u{5d0}.com/", Some("xn----0hc.com")),
    ("http://_\u{5d0}\u{5d1}.com/", Some("xn--_-0hcd.com")),
    ("http://a\u{870}a.com/", Some("xn--aa-h8e.com")),
    ("http://a\u{870}\u{5d0}.com/", None),
    ("http://a\u{870}b\u{5d0}.com/", None),
    ("http://\u{5d0}\u{870}.com/", None),
    ("http://\u{5d0}\u{870}\u{5d1}.com/", None),
    ("http://1\u{870}.com/", Some("xn--1-fld.com")),
    ("http://1\u{870}\u{5d0}.com/", None),
    ("http://\u{870}\u{5d0}.com/", None),
    ("http://\u{870}a.com/", Some("xn--a-eld.com")),
    ("http://\u{870}.com/", Some("xn--cxb.com")),
    ("http://\u{c3c}a.com/", Some("xn--a-x5e.com")),
    ("http://a\u{c3c}.com/", Some("xn--a-y5e.com")),
    ("http://\u{5d0}\u{c3c}\u{5d1}.com/", None),
    ("http://\u{5d0}\u{1faa9}\u{5d1}.com/", None),
    ("http://a\u{1faa9}\u{5d0}.com/", None),
    ("http://\u{5d0}\u{1faa9}.com/", None),
    ("http://\u{e9}\u{870}.com/", Some("xn--9ca869a.com")),
    ("http://\u{e9}\u{5d0}.com/", Some("xn--9ca42w.com")),
    ("http://\u{5d0}\u{e9}.com/", None),
    ("http://\u{e9}a\u{870}.com/", Some("xn--a-9fa698c.com")),
    ("http://1\u{5d0}\u{591}.com/", Some("xn--1-fec2q.com")),
    ("http://a\u{5d0}\u{591}.com/", Some("xn--a-fec2q.com")),
    ("http://\u{591}a.com/", None),
];
const PUNY: &[(&str, Option<&str>)] = &[
    ("http://xn--vhk.com/", None),
    ("http://xn--5l8d.com/", None),
    ("http://xn--1-bicuf1d.com/", Some("xn--1-bicuf1d.com")),
    ("http://xn--9hbdgm.com/", Some("xn--9hbdgm.com")),
    ("http://XN--1-BICUF1D.COM/", Some("xn--1-bicuf1d.com")),
    ("http://xn--a-.com/", Some("xn--a-.com")),
    ("http://xn--.com/", None),
    ("http://xn--zzzzzzzzzzzzzzzzzzzzzzzzzz.com/", None),
    ("http://xn--9ca.com/", Some("xn--9ca.com")),
    ("http://xn--a-bga.com/", Some("xn--a-bga.com")),
    ("http://xn--1-bicuf1d.com:8080/p", Some("xn--1-bicuf1d.com:8080")),
];
const FILE: &[(&str, Option<&str>)] = &[
    ("file://\u{1e9e}.com/x", Some("ss.com")),
    ("file:///x", Some("")),
    ("file://localhost/x", Some("")),
    ("file://LOCALHOST/x", Some("")),
    ("file://a\u{2061}b/x", None),
    ("file://\u{c4}\u{d6}.de/x", Some("xn--4ca0b.de")),
    ("file:///C:/x", Some("")),
    ("file://C:/x", Some("")),
    ("file:/x", Some("")),
    ("file://127.0.0.1/x", Some("127.0.0.1")),
    ("file://h:80/x", None),
    ("file://u@h/x", None),
    ("file://\u{661}\u{5d0}/x", Some("xn--4db30a")),
    ("file://1\u{5d0}.com/", Some("xn--1-0hc.com")),
];
const OPAQUE: &[(&str, Option<&str>)] = &[
    ("vscode://127.0.0.1:5000\\cb", None),
    ("vscode://127.0.0.1:5000/cb\\x", Some("127.0.0.1:5000")),
    ("vscode://u\\x@h:80/", Some("h:80")),
    ("vscode://localhost\\cb", None),
    ("vscode://[::1]:5000\\", None),
    ("vscode://h:80?\\", Some("h:80")),
    ("vscode://h:80#\\", Some("h:80")),
    ("vscode://h\\/x", None),
    ("a+b://\\@h/", Some("h")),
    ("vscode://u:p@h:80\\", None),
    ("vscode:\\\\h", Some("")),
    ("vscode://h:80/\\x", Some("h:80")),
];

/// (what it is, the text before the repeated unit, the unit, how many times, the text after, node's host as its length, its first 24
/// characters and its last 16; None where node refuses).
type Long = (&'static str, &'static str, &'static str, usize, &'static str, Option<(usize, &'static str, &'static str)>);

const LONG: &[Long] = &[
    ("digit then 2000 Arabic letters", "1", "\u{627}", 2000, ".com/", Some((2012, "xn--1-zmcaaaaaaaaaaaaaaa", "aaaaaaaaaaaa.com"))),
    ("2000 Hebrew letters then a digit", "", "\u{5d0}", 2000, "1.com/", Some((2012, "xn--1-zhcaaaaaaaaaaaaaaa", "aaaaaaaaaaaa.com"))),
    (
        "a, 1500 e acute, then a Hebrew letter",
        "a",
        "\u{e9}",
        1500,
        "\u{5d0}.com/",
        Some((1518, "xn--a-bgaaaaaaaaaaaaaaaa", "aaaaaa42169p.com")),
    ),
    ("17161 letters then an Adlam letter", "", "a", 17161, "\u{1e943}.com/", Some((17178, "xn--aaaaaaaaaaaaaaaaaaaa", "aaa-wy57046o.com"))),
    ("17162 letters then an Adlam letter", "", "a", 17162, "\u{1e943}.com/", None),
    ("40000 letters then an Adlam letter", "", "a", 40000, "\u{1e943}.com/", None),
];

const PLACE: &[(&str, &str)] = &[
    (
        r##"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","__proto__":1}"##,
        r##"{
  "placeId": "p1",
  "name": "n",
  "hostName": "h",
  "hostUrls": [
    "http://a"
  ],
  "hostPublicKey": "k",
  "keyPath": "/k",
  "__proto__": 1
}
"##,
    ),
    (
        r##"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","__proto__":{"a":1},"b":2}"##,
        r##"{
  "placeId": "p1",
  "name": "n",
  "hostName": "h",
  "hostUrls": [
    "http://a"
  ],
  "hostPublicKey": "k",
  "keyPath": "/k",
  "__proto__": {
    "a": 1
  },
  "b": 2
}
"##,
    ),
    (
        r##"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","x":1,"__proto__":null}"##,
        r##"{
  "placeId": "p1",
  "name": "n",
  "hostName": "h",
  "hostUrls": [
    "http://a"
  ],
  "hostPublicKey": "k",
  "keyPath": "/k",
  "x": 1,
  "__proto__": null
}
"##,
    ),
    (
        r##"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","10":"a","2":"b","z":1,"__proto__":[1]}"##,
        r##"{
  "2": "b",
  "10": "a",
  "placeId": "p1",
  "name": "n",
  "hostName": "h",
  "hostUrls": [
    "http://a"
  ],
  "hostPublicKey": "k",
  "keyPath": "/k",
  "z": 1,
  "__proto__": [
    1
  ]
}
"##,
    ),
    (
        r##"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","joinedAt":"t","__proto__":{"constructor":1}}"##,
        r##"{
  "placeId": "p1",
  "name": "n",
  "hostName": "h",
  "hostUrls": [
    "http://a"
  ],
  "hostPublicKey": "k",
  "keyPath": "/k",
  "joinedAt": "t",
  "__proto__": {
    "constructor": 1
  }
}
"##,
    ),
    (
        r##"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","__proto__":1,"__proto__":2}"##,
        r##"{
  "placeId": "p1",
  "name": "n",
  "hostName": "h",
  "hostUrls": [
    "http://a"
  ],
  "hostPublicKey": "k",
  "keyPath": "/k",
  "__proto__": 2
}
"##,
    ),
];

fn none_differ(found: Vec<String>) {
    assert!(found.is_empty(), "{} answer(s) unlike node's:\n{}", found.len(), found.join("\n"));
}

fn differing(table: &[(&str, Option<&str>)]) -> Vec<String> {
    table
        .iter()
        .filter_map(|(url, want)| {
            let got = host_of(url);
            (got.as_deref() != *want).then(|| format!("host_of({url:?}): node {want:?}, here {got:?}"))
        })
        .collect()
}

#[test]
fn a_label_is_held_to_nodes_bidi_rule_and_not_rfc_5893s() {
    none_differ(differing(BIDI));
}

#[test]
fn a_punycode_label_is_read_as_node_reads_it() {
    none_differ(differing(PUNY));
}

#[test]
fn a_file_url_host_is_read_as_node_reads_it() {
    none_differ(differing(FILE));
}

#[test]
fn a_url_in_a_scheme_that_is_not_special_ends_its_authority_at_a_slash_a_question_mark_or_a_hash() {
    none_differ(differing(OPAQUE));
}

#[test]
fn a_long_label_is_taken_whole_and_refused_where_the_punycode_delta_passes_an_i32() {
    let found = LONG
        .iter()
        .filter_map(|(what, head, unit, count, tail, want)| {
            let got = host_of(&format!("http://{head}{}{tail}", unit.repeat(*count)));
            let shape =
                got.as_ref().map(|h| (h.len(), h.chars().take(24).collect::<String>(), h.chars().rev().take(16).collect::<Vec<_>>()));
            let want_shape = want.map(|(n, first, last)| (n, first.to_string(), last.chars().rev().collect::<Vec<_>>()));
            (shape != want_shape).then(|| format!("{what}: node {want:?}, here {:?}", got.as_ref().map(|h| h.len())))
        })
        .collect();
    none_differ(found);
}

#[test]
fn a_refused_long_label_is_not_the_machine_in_a_redirect() {
    let url = format!("http://{}\u{1e943}.com/", "a".repeat(17162));
    assert!(!redirects_to_machine(&url));
    assert_eq!(callback_port_of(&url), None);
}

#[test]
fn a_place_file_is_written_back_as_json_stringify_writes_what_json_parse_read() {
    let found = PLACE
        .iter()
        .filter_map(|(text, want)| {
            let got = parse_place_file(text).map(|file| place_file_text(&file));
            (got.as_deref() != Some(want)).then(|| format!("{text}: node wrote {want:?}, here {got:?}"))
        })
        .collect();
    none_differ(found);
}
